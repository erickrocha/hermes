use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::commons::password;
use crate::domain::access_token::{AccessToken, Claims};
use crate::domain::business_error::BusinessError;
use crate::domain::user::{User, UserEntityMapper};
use crate::gateway::user_gateway::UserGateway;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::DbConn;
use std::env;

/// PD-030: 3 h / 7 d continuam os padrões; o que muda é que deixam de ser
/// literais. Um valor inválido ou ausente cai no padrão em vez de derrubar o
/// boot — um token com vida errada é pior que um token com a vida padrão.
const DEFAULT_ACCESS_TOKEN_HOURS: i64 = 3;
const DEFAULT_REFRESH_TOKEN_DAYS: i64 = 7;

fn positive_duration_from(raw: Option<String>, default: i64) -> i64 {
    raw.and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|parsed| *parsed > 0)
        .unwrap_or(default)
}

fn access_token_hours() -> i64 {
    positive_duration_from(
        env::var("ACCESS_TOKEN_HOURS").ok(),
        DEFAULT_ACCESS_TOKEN_HOURS,
    )
}

fn refresh_token_days() -> i64 {
    positive_duration_from(
        env::var("REFRESH_TOKEN_DAYS").ok(),
        DEFAULT_REFRESH_TOKEN_DAYS,
    )
}

pub struct AuthenticationUseCase {}

impl AuthenticationUseCase {
    /// Carrega o usuário pelo e-mail e recusa conta desabilitada.
    ///
    /// Todo caminho de autenticação passa por aqui — login, validação de access
    /// token e de refresh token —, então é este o ponto que faz `user.enabled`
    /// valer. Sem ele o flag existe no schema e não protege nada: desabilitar
    /// uma conta (ou atender um pedido de exclusão) não derrubaria a sessão em
    /// curso nem impediria um novo login.
    async fn load_enabled_user(
        db: &DbConn,
        email: &str,
        context: &str,
    ) -> Result<User, BusinessError> {
        let found = UserGateway::find_by_email(db, email.to_string())
            .await
            .map_err(|err| {
                log::error!(
                    "[AuthenticationUseCase::{}] Database error for user {}: {}",
                    context,
                    email,
                    err
                );
                BusinessError::new("Invalid credentials".to_string())
            })?;

        let Some(model) = found else {
            log::error!(
                "[AuthenticationUseCase::{}] User not found: {}",
                context,
                email
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        };

        if !model.enabled {
            log::warn!(
                "[AuthenticationUseCase::{}] Disabled account rejected: {}",
                context,
                email
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        Ok(UserEntityMapper::from_model(model))
    }

    /// DEF-IA-01/DEF-IA-02 (HRMS-104, HRMS-102, HRMS-003): resolves the account
    /// a *token* was issued to, by its immutable id.
    ///
    /// The address is not an identity. Before this, every token path looked the
    /// account up by `sub` (the address), so once an address was freed by a
    /// rename and handed to someone else, the old holder's tokens resolved to
    /// the new person -- in another tenant, for the whole remaining lifetime of
    /// the token. The address is still compared here, but as a *check*: a token
    /// minted before a rename no longer describes the account and is refused.
    async fn load_enabled_account(
        db: &DbConn,
        user_id: i64,
        email: &str,
        context: &str,
    ) -> Result<User, BusinessError> {
        if user_id <= 0 {
            log::warn!(
                "[AuthenticationUseCase::{}] Token carries no usable user id",
                context
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        let found = UserGateway::find_by_user_id(db, user_id)
            .await
            .map_err(|err| {
                log::error!(
                    "[AuthenticationUseCase::{}] Database error for user id {}: {}",
                    context,
                    user_id,
                    err
                );
                BusinessError::new("Invalid credentials".to_string())
            })?;

        let Some(model) = found else {
            log::error!(
                "[AuthenticationUseCase::{}] User not found by id: {}",
                context,
                user_id
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        };

        if !model.enabled {
            log::warn!(
                "[AuthenticationUseCase::{}] Disabled account rejected: id {}",
                context,
                user_id
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        if model.id != user_id {
            log::error!(
                "[AuthenticationUseCase::{}] Account lookup returned id {} for id {}",
                context,
                model.id,
                user_id
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        if model.email != email {
            log::warn!(
                "[AuthenticationUseCase::{}] Token subject no longer matches account id {}",
                context,
                user_id
            );
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        Ok(UserEntityMapper::from_model(model))
    }

    pub async fn execute(
        db: &DbConn,
        email: String,
        password: String,
    ) -> Result<AccessToken, BusinessError> {
        log::info!(
            "[AuthenticationUseCase::execute] Executing login for user: {}",
            email
        );
        if email.is_empty() || password.is_empty() {
            log::error!("[AuthenticationUseCase::execute] Email and password are required");
            return Err(BusinessError::new(
                "Email and password are required".to_string(),
            ));
        }
        let user = Self::load_enabled_user(db, &email, "execute").await;

        // DEF-IA-05 (HRMS-103): the three refusals -- unknown address, disabled
        // account, wrong password -- must be indistinguishable, and a body that
        // matches is only half of that. Returning before any hash verification
        // answered in ~1 ms where a wrong password cost ~360 ms, which told an
        // anonymous caller exactly which addresses have an enabled account. The
        // no-account paths now pay the same Argon2 cost as the real one.
        let user = match user {
            Ok(user) => user,
            Err(error) => {
                password::verify_dummy(&password);
                return Err(error);
            }
        };

        if password::verify(&password, user.password.as_str()) {
            log::info!(
                "[AuthenticationUseCase::execute] Password verified for user: {}",
                email
            );
            // PD-029: o login é o único ponto que tem a senha em claro, logo o
            // único que pode reescrever um hash legado em Argon2id. Falhar aqui
            // nunca recusa o login — o usuário acertou a senha; só significa que
            // a conta tenta de novo no próximo login.
            let user = Self::upgrade_legacy_hash(db, user, &password).await;
            let access_token = Self::generate_access_token(user);
            Ok(access_token)
        } else {
            log::error!(
                "[AuthenticationUseCase::execute] Invalid password for user: {}",
                email
            );
            Err(BusinessError::new("Invalid credentials".to_string()))
        }
    }

    /// Reescreve um hash bcrypt em Argon2id depois de um login bem-sucedido.
    /// Devolve o usuário com o hash novo quando conseguiu, o original quando não.
    async fn upgrade_legacy_hash(db: &DbConn, user: User, plaintext: &str) -> User {
        if !password::needs_rehash(&user.password) {
            return user;
        }
        let Ok(rehashed) = password::hash(plaintext) else {
            log::warn!(
                "[AuthenticationUseCase::upgrade_legacy_hash] Could not rehash {}",
                user.email
            );
            return user;
        };
        let upgraded = User {
            password: rehashed,
            ..user
        };
        // D-05: o login acontece antes de existir sessão, logo fora de escopo.
        // A reescrita do hash é trabalho da plataforma sobre a própria conta.
        match entity::audit::run_as_platform(UserGateway::new(db.clone()).persist(upgraded.clone()))
            .await
        {
            Ok(_) => {
                log::info!(
                    "[AuthenticationUseCase::upgrade_legacy_hash] Upgraded {} to Argon2id",
                    upgraded.email
                );
                upgraded
            }
            Err(error) => {
                log::warn!(
                    "[AuthenticationUseCase::upgrade_legacy_hash] Upgrade failed for {}: {}",
                    upgraded.email,
                    error
                );
                upgraded
            }
        }
    }

    pub fn generate_access_token(user: User) -> AccessToken {
        log::info!(
            "[AuthenticationUseCase::generate_access_token] Generating access token for: {}",
            user.email
        );
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::hours(access_token_hours()))
            .expect("valid timestamp")
            .timestamp();
        let claims = Claims::builder()
            .with_sub(user.email.clone())
            .exp(expiration)
            .uuid(user.uuid.clone().unwrap_or_default())
            .name(user.name.clone().unwrap_or_default())
            .user_id(user.id.unwrap_or(0))
            .role(user.role.clone())
            .tenant_id(user.tenant_id)
            .build()
            .expect("missing required claims field");

        let header = Header::new(Algorithm::HS512);
        let private_key = env::var("ACCESS_TOKEN_SECRET").expect("ACCESS_TOKEN_SECRET must be set");
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(private_key.as_bytes()),
        )
        .unwrap();
        let refresh_token = Self::generate_refresh_token(user.clone());
        AccessToken {
            access_token: token,
            token_type: "Bearer".to_string(),
            expire_in: expiration,
            refresh_token: Some(refresh_token),
            email: claims.sub.clone(),
            uuid: claims.uuid.clone(),
            name: claims.name.clone(),
            user_id: claims.user_id,
            role: claims.role,
            tenant_id: claims.tenant_id,
            tenant_uuid: None,
        }
    }

    fn generate_refresh_token(user: User) -> String {
        log::info!(
            "[AuthenticationUseCase::generate_refresh_token] Generating refresh token for: {}",
            user.email
        );
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::days(refresh_token_days()))
            .expect("valid timestamp")
            .timestamp();
        let claims = Claims::builder()
            .with_sub(user.email.clone())
            .exp(expiration)
            .uuid(user.uuid.clone().unwrap_or_default())
            .name(user.name.clone().unwrap_or_default())
            .user_id(user.id.unwrap_or(0))
            .role(user.role.clone())
            .tenant_id(user.tenant_id)
            .build()
            .expect("missing required claims field");

        let header = Header::new(Algorithm::HS512);
        let private_key =
            env::var("REFRESH_TOKEN_SECRET").expect("REFRESH_TOKEN_SECRET must be set");
        encode(
            &header,
            &claims,
            &EncodingKey::from_secret(private_key.as_bytes()),
        )
        .unwrap()
    }

    /// DEF-XF-09 (owner, 2026-09-18): identity and tenant scope come from the
    /// token. At login the user is found by email (unique across the platform),
    /// the password verified, and the token minted with that user's role and
    /// tenant -- empty only for a SysAdmin. Every protected request then decodes
    /// the token and uses *those* claims; the database is never the source of
    /// role or tenant here.
    ///
    /// The one database read left is HRM-042: the account must still exist and
    /// be enabled, so disabling someone ends their session at once instead of
    /// when the token expires. A role or tenant change takes effect at the next
    /// login or refresh, which re-reads the account and mints fresh claims.
    pub async fn validate(db: &DbConn, token: String) -> Result<User, BusinessError> {
        log::info!("[AuthenticationUseCase::validate] Validating access token");
        let public_key = env::var("ACCESS_TOKEN_SECRET").expect("ACCESS_TOKEN_SECRET must be set");
        let claims = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(public_key.as_bytes()),
            &Validation::new(Algorithm::HS512),
        )
        .map_err(|err| {
            log::error!(
                "[AuthenticationUseCase::validate] Token decode error: {:?}",
                err
            );
            BusinessError::new("Token is invalid".to_string())
        })?
        .claims;
        log::info!(
            "[AuthenticationUseCase::validate] Token valid for subject: {}",
            claims.sub
        );

        let account =
            Self::load_enabled_account(db, claims.user_id, &claims.sub, "validate").await?;
        Ok(User {
            id: Some(claims.user_id),
            role: claims.role,
            tenant_id: claims.tenant_id,
            ..account
        })
    }

    pub async fn validate_refresh_token(db: &DbConn, token: String) -> Result<User, BusinessError> {
        log::info!("[AuthenticationUseCase::validate_refresh_token] Validating refresh token");
        let public_key =
            env::var("REFRESH_TOKEN_SECRET").expect("REFRESH_TOKEN_SECRET must be set");
        let result = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(public_key.as_bytes()),
            &Validation::new(Algorithm::HS512),
        );

        if let Err(err) = &result {
            log::error!(
                "[AuthenticationUseCase::validate_refresh_token] Refresh token decode error: {:?}",
                err
            );
            return Err(BusinessError::new("Token is invalid".to_string()));
        }

        let authentication = result.unwrap();
        log::info!(
            "[AuthenticationUseCase::validate_refresh_token] Refresh token valid for subject: {}",
            authentication.claims.sub
        );
        let claims = authentication.claims;

        // DEF-IA-01: the refresh path mints a *new* session, so getting the
        // account wrong here is the worst of the two. Resolved by id, and the
        // freshly read role and tenant (not the token's) go into the new claims.
        Self::load_enabled_account(db, claims.user_id, &claims.sub, "validate_refresh_token").await
    }

    pub async fn refresh_token(
        db: &DbConn,
        refresh_token: String,
    ) -> Result<AccessToken, BusinessError> {
        log::info!("[AuthenticationUseCase::refresh_token] Refreshing token");
        let user = AuthenticationUseCase::validate_refresh_token(db, refresh_token).await?;
        Ok(AuthenticationUseCase::generate_access_token(user))
    }
}

#[cfg(test)]
mod token_lifetime_tests {
    use super::{DEFAULT_ACCESS_TOKEN_HOURS, positive_duration_from};

    #[test]
    fn a_configured_positive_value_wins() {
        assert_eq!(
            positive_duration_from(Some(" 12 ".to_string()), DEFAULT_ACCESS_TOKEN_HOURS),
            12
        );
    }

    #[test]
    fn absent_unparseable_or_non_positive_values_fall_back_to_the_default() {
        for raw in [
            None,
            Some("".to_string()),
            Some("abc".to_string()),
            Some("0".to_string()),
            Some("-4".to_string()),
        ] {
            assert_eq!(
                positive_duration_from(raw, DEFAULT_ACCESS_TOKEN_HOURS),
                DEFAULT_ACCESS_TOKEN_HOURS
            );
        }
    }
}
