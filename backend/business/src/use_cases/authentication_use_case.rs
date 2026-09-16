use crate::commons::entity_mapper::EntityMapper;
use crate::domain::access_token::{AccessToken, Claims};
use crate::domain::business_error::BusinessError;
use crate::domain::user::{User, UserEntityMapper};
use crate::gateway::user_gateway::UserGateway;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::DbConn;
use std::env;

pub struct AuthenticationUseCase {}

impl AuthenticationUseCase {
    /// Carrega o usuário pelo e-mail e recusa conta desabilitada.
    ///
    /// Todo caminho de autenticação passa por aqui — login, validação de access
    /// token e de refresh token —, então é este o ponto que faz `user.enabled`
    /// valer. Sem ele o flag existe no schema e não protege nada: desabilitar
    /// uma conta (ou atender um pedido de exclusão) não derrubaria a sessão em
    /// curso nem impediria um novo login.
    async fn load_enabled_user(db: &DbConn,email: &str,context: &str) -> Result<User, BusinessError> {
        let found = UserGateway::find_by_email(db, email.to_string())
            .await
            .map_err(|err| {
                log::error!("[AuthenticationUseCase::{}] Database error for user {}: {}", context, email, err);
                BusinessError::new("Invalid credentials".to_string())
            })?;

        let Some(model) = found else {
            log::error!("[AuthenticationUseCase::{}] User not found: {}", context, email);
            return Err(BusinessError::new("Invalid credentials".to_string()));
        };

        if !model.enabled {
            log::warn!("[AuthenticationUseCase::{}] Disabled account rejected: {}", context, email);
            return Err(BusinessError::new("Invalid credentials".to_string()));
        }

        Ok(UserEntityMapper::from_model(model))
    }

    pub async fn execute(db: &DbConn, email: String, password: String) -> Result<AccessToken, BusinessError> {
        log::info!("[AuthenticationUseCase::execute] Executing login for user: {}", email);
        if email.is_empty() || password.is_empty() {
            log::error!("[AuthenticationUseCase::execute] Email and password are required");
            return Err(BusinessError::new("Email and password are required".to_string()));
        }
        let user = Self::load_enabled_user(db, &email, "execute").await?;

        if bcrypt::verify(password, user.password.as_str()).unwrap_or(false) {
            log::info!("[AuthenticationUseCase::execute] Password verified for user: {}", email);
            let access_token = Self::generate_access_token(user);
            Ok(access_token)
        } else {
            log::error!("[AuthenticationUseCase::execute] Invalid password for user: {}", email);
            Err(BusinessError::new("Invalid credentials".to_string()))
        }
    }

    pub fn generate_access_token(user: User) -> AccessToken {
        log::info!("[AuthenticationUseCase::generate_access_token] Generating access token for: {}", user.email);
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::hours(3))
            .expect("valid timestamp")
            .timestamp();
        let claims = Claims::builder()
            .with_sub(user.email.clone())
            .exp(expiration)
            .uuid(user.email.clone())
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
            first_login: user.first_login,
        }
    }

    fn generate_refresh_token(user: User) -> String {
        log::info!("[AuthenticationUseCase::generate_refresh_token] Generating refresh token for: {}", user.email);
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::days(7))
            .expect("valid timestamp")
            .timestamp();
        let claims = Claims::builder()
            .with_sub(user.email.clone())
            .exp(expiration)
            .uuid(user.email.clone())
            .name(user.name.clone().unwrap_or_default())
            .user_id(user.id.unwrap_or(0))
            .role(user.role.clone())
            .tenant_id(user.tenant_id)
            .build()
            .expect("missing required claims field");

        let header = Header::new(Algorithm::HS512);
        let private_key = env::var("REFRESH_TOKEN_SECRET").expect("REFRESH_TOKEN_SECRET must be set");
        encode(&header, &claims, &EncodingKey::from_secret(private_key.as_bytes())).unwrap()
    }

    pub async fn validate(db: &DbConn, token: String) -> Result<User, BusinessError> {
        log::info!("[AuthenticationUseCase::validate] Validating access token");
        let public_key = env::var("ACCESS_TOKEN_SECRET").expect("ACCESS_TOKEN_SECRET must be set");
        let result = decode::<Claims>(&token, &DecodingKey::from_secret(public_key.as_bytes()), &Validation::new(Algorithm::HS512));

        if let Err(err) = &result {
            log::error!("[AuthenticationUseCase::validate] Token decode error: {:?}", err);
            return Err(BusinessError::new("Token is invalid".to_string()));
        }

        let authentication = result.unwrap();
        log::info!("[AuthenticationUseCase::validate] Token valid for subject: {}", authentication.claims.sub);
        let email = authentication.claims.sub;

        Self::load_enabled_user(db, &email, "validate").await
    }

    pub async fn validate_refresh_token(db: &DbConn, token: String) -> Result<User, BusinessError> {
        log::info!("[AuthenticationUseCase::validate_refresh_token] Validating refresh token");
        let public_key = env::var("REFRESH_TOKEN_SECRET").expect("REFRESH_TOKEN_SECRET must be set");
        let result = decode::<Claims>(&token, &DecodingKey::from_secret(public_key.as_bytes()), &Validation::new(Algorithm::HS512));

        if let Err(err) = &result {
            log::error!("[AuthenticationUseCase::validate_refresh_token] Refresh token decode error: {:?}", err);
            return Err(BusinessError::new("Token is invalid".to_string()));
        }

        let authentication = result.unwrap();
        log::info!("[AuthenticationUseCase::validate_refresh_token] Refresh token valid for subject: {}", authentication.claims.sub);
        let email = authentication.claims.sub;

        Self::load_enabled_user(db, &email, "validate_refresh_token").await
    }

    pub async fn refresh_token(db: &DbConn, refresh_token: String) -> Result<AccessToken, BusinessError> {
        log::info!("[AuthenticationUseCase::refresh_token] Refreshing token");
        let user = AuthenticationUseCase::validate_refresh_token(db, refresh_token).await?;
        Ok(AuthenticationUseCase::generate_access_token(user))
    }
}
