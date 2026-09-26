use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::password_policy;
use crate::domain::user::User;
use crate::gateway::user_gateway::UserGateway;
use base64::Engine;
use chrono::{DateTime, Utc};
use entity::user_entity;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{ActiveModelTrait, DbConn, Set};
use serde::{Deserialize, Serialize};
use std::env;

/// Validade do convite. Curta o bastante para limitar a janela de um link vazado,
/// longa o bastante para o paciente abrir o e-mail no fim de semana.
const INVITE_VALIDITY_DAYS: i64 = 7;

#[derive(Debug, Serialize, Deserialize)]
struct InviteClaims {
    /// E-mail do convidado.
    sub: String,
    exp: i64,
    /// Separa este token dos de acesso e de refresh: um token de login não vale
    /// como convite e vice-versa, mesmo que o segredo base seja o mesmo.
    typ: String,
}

pub struct AccountInvite {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct AccountInviteUseCase;

impl AccountInviteUseCase {
    /// Emite um convite de uso único para o paciente definir a própria senha.
    ///
    /// Substitui a senha padrão compartilhada: a conta nasce com um segredo
    /// aleatório que ninguém conhece, e o único caminho para dentro é este
    /// convite. O uso único não precisa de tabela nem de job de limpeza — a
    /// chave de assinatura inclui o hash atual da senha, então **assim que o
    /// paciente define a senha o token deixa de validar sozinho**. O mesmo vale
    /// se a clínica emitir um convite novo: o anterior morre junto.
    ///
    /// Takes the domain `User` returned by `UserUseCase::create` (its
    /// `password` is already the stored hash at that point, which is exactly
    /// what the signing key needs) rather than the raw entity model -- there
    /// is no reason for a use case to reach past its own crate's domain type
    /// for a field both already carry under the same name.
    pub fn issue(user: &User) -> Result<AccountInvite, BusinessError> {
        let expires_at = Utc::now() + chrono::Duration::days(INVITE_VALIDITY_DAYS);
        let claims = InviteClaims {
            sub: user.email.clone(),
            exp: expires_at.timestamp(),
            typ: "invite".to_string(),
        };

        let token = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(Self::signing_key(&user.password)?.as_bytes()),
        )
        .map_err(|e| BusinessError::new(format!("Failed to issue invite: {}", e)))?;

        log::info!(
            "[AccountInviteUseCase::issue] Invite issued for {}",
            user.email
        );
        Ok(AccountInvite { token, expires_at })
    }

    /// Consome o convite: valida e grava a senha escolhida pelo paciente.
    pub async fn accept(db: &DbConn, token: &str, new_password: &str) -> Result<(), BusinessError> {
        password_policy::validate_length(new_password)?;

        // O e-mail sai do payload sem validar assinatura, só para localizar a conta:
        // a assinatura só pode ser conferida depois, porque a chave depende do hash
        // da senha guardado no banco. Nada deste passo é confiado — a verificação
        // real acontece logo abaixo.
        let email = Self::peek_subject(token)?;

        let user = UserGateway::find_by_email(db, email)
            .await
            .map_err(|e| BusinessError::new(format!("Database error validating invite: {}", e)))?
            .ok_or_else(|| BusinessError::new("Invalid invite".to_string()))?;

        // DEF-IA-04 (HRMS-102, PD-001, HRMS-124): a disabled account stays
        // disabled. Accepting the invitation used to write `enabled = true`
        // unconditionally, so an owner who removed a person inside the 7-day
        // window -- a hire withdrawn, an account created disabled on purpose --
        // had that decision quietly reversed by the invitee's own click.
        if !user.enabled {
            log::warn!(
                "[AccountInviteUseCase::accept] Invite refused for disabled account {}",
                user.email
            );
            return Err(BusinessError::new(
                "Invite is invalid or has already been used".to_string(),
            ));
        }

        // Agora sim: assinatura conferida contra a chave derivada do hash atual.
        // Se a senha já foi definida, a chave mudou e este convite não abre mais.
        let mut strict = Validation::new(Algorithm::HS512);
        strict.validate_exp = true;
        let verified = decode::<InviteClaims>(
            token,
            &DecodingKey::from_secret(Self::signing_key(&user.password)?.as_bytes()),
            &strict,
        )
        .map_err(|_| {
            log::warn!(
                "[AccountInviteUseCase::accept] Rejected invite for {}",
                user.email
            );
            BusinessError::new("Invite is invalid or has already been used".to_string())
        })?;

        if verified.claims.typ != "invite" {
            return Err(BusinessError::new("Invalid invite".to_string()));
        }

        let hashed = crate::commons::password::hash(new_password)
            .map_err(|e| BusinessError::new(format!("Failed to hash password: {}", e)))?;

        let mut active: user_entity::ActiveModel = user.clone().into();
        active.password = Set(hashed);
        // D-05: `/accept-invite` é rota pública — não há sessão, logo não há
        // escopo, logo a gravação seria recusada. O convidado age sobre a
        // própria conta, então a escrita roda com a identidade dele: quem tem
        // tenant grava dentro do próprio tenant, e o audit trail continua
        // apontando para a pessoa, não para "system".
        let actor = entity::audit::AuditUser {
            id: user.id,
            email: user.email.clone(),
            tenant_id: user.tenant_id,
            enforce_tenant: user.tenant_id.is_some(),
        };
        entity::audit::run_with_user(Some(actor), active.update(db))
            .await
            .map_err(|e| BusinessError::new(format!("Failed to set password: {}", e)))?;

        log::info!(
            "[AccountInviteUseCase::accept] Invite accepted for {}",
            user.email
        );
        Ok(())
    }

    /// Gera uma senha aleatória para contas recém-provisionadas. Ela nunca é
    /// exibida a ninguém: existe só para que a conta não tenha segredo previsível
    /// antes de o paciente aceitar o convite.
    pub fn unguessable_secret() -> String {
        format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4())
    }

    /// DEF-IA-07 (HRMS-125, D-07): issues a fresh invitation for an existing
    /// account, superseding any outstanding one.
    ///
    /// Until this existed, `POST /user` was the only operation that ever issued
    /// an invitation, and it cannot run twice for the same address (the address
    /// is unique). An invitation lost to a mail failure -- sending is
    /// best-effort, and `POST /user` answers 201 either way -- or simply left to
    /// expire after 7 days had no replacement, and the only way in was for an
    /// administrator to type a password on the person's behalf, which is exactly
    /// what PD-002 set out to remove.
    ///
    /// Supersession is not bookkeeping here: the signing key is derived from the
    /// account's current password hash, so replacing that hash with a new
    /// unguessable secret invalidates every earlier invitation by construction.
    /// The account is left with a secret nobody knows, which is the same state
    /// `POST /user` leaves it in.
    pub async fn reissue(db: &DbConn, user: User) -> Result<(User, AccountInvite), BusinessError> {
        if !user.enabled {
            // DEF-IA-04's rule, stated on the issuing side too: a disabled
            // account is not invited back in.
            return Err(BusinessError::new(
                "Cannot invite a disabled account".to_string(),
            ));
        }

        let hashed = crate::commons::password::hash(&Self::unguessable_secret())
            .map_err(|e| BusinessError::new(format!("Failed to hash password: {}", e)))?;

        let refreshed = User {
            password: hashed,
            updated_at: None,
            ..user
        };

        let saved = UserGateway::new(db.clone())
            .persist(refreshed.clone())
            .await
            .map_err(|e| BusinessError::new(format!("Failed to re-issue invite: {}", e)))?;

        let _ = saved;
        let invite = Self::issue(&refreshed)?;
        log::info!(
            "[AccountInviteUseCase::reissue] Invite re-issued for {}",
            refreshed.email
        );
        Ok((refreshed, invite))
    }

    /// Lê o `sub` do payload do JWT sem verificar assinatura, apenas para saber
    /// qual conta carregar. O valor não é confiável e só serve para a busca.
    fn peek_subject(token: &str) -> Result<String, BusinessError> {
        let payload = token
            .split('.')
            .nth(1)
            .ok_or_else(|| BusinessError::new("Invalid invite".to_string()))?;
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| BusinessError::new("Invalid invite".to_string()))?;
        let claims: InviteClaims = serde_json::from_slice(&decoded)
            .map_err(|_| BusinessError::new("Invalid invite".to_string()))?;
        Ok(claims.sub)
    }

    fn signing_key(password_hash: &str) -> Result<String, BusinessError> {
        let base = env::var("ACCESS_TOKEN_SECRET")
            .map_err(|_| BusinessError::new("ACCESS_TOKEN_SECRET must be set".to_string()))?;
        Ok(format!("{}:invite:{}", base, password_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountInviteUseCase, INVITE_VALIDITY_DAYS};
    use crate::domain::enums::Role;
    use crate::domain::user::User;
    use chrono::Utc;

    // EPIC-IA-07/D-07: `issue` had no test at all before this -- unreachable
    // code doesn't get exercised by accident. `ACCESS_TOKEN_SECRET` is set
    // here rather than relied on from the environment: nothing else in this
    // crate's test suite reads it, so this is the only place it needs a
    // value.
    fn with_access_token_secret<T>(run: impl FnOnce() -> T) -> T {
        // SAFETY: test-only, single-threaded within this function's call,
        // and no other test in this crate reads this variable.
        unsafe { std::env::set_var("ACCESS_TOKEN_SECRET", "test-secret") };
        run()
    }

    fn invited_user() -> User {
        User {
            id: Some(1),
            uuid: Some("11111111-1111-1111-1111-111111111111".to_string()),
            email: "new.hire@transmega.com".to_string(),
            name: Some("New Hire".to_string()),
            password: "$2b$12$abcdefghijklmnopqrstuv".to_string(),
            enabled: true,
            tenant_id: Some(1),
            role: Role::TenantUser,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn issues_a_token_valid_for_exactly_the_documented_window() {
        with_access_token_secret(|| {
            let before = Utc::now();
            let invite = AccountInviteUseCase::issue(&invited_user()).unwrap();
            assert!(!invite.token.is_empty());
            let expected = before + chrono::Duration::days(INVITE_VALIDITY_DAYS);
            let drift = (invite.expires_at - expected).num_seconds().abs();
            assert!(
                drift < 5,
                "expected ~{INVITE_VALIDITY_DAYS}d validity, drift was {drift}s"
            );
        });
    }

    #[test]
    fn the_token_carries_the_invited_email_and_the_invite_type_claim() {
        // HRMS-053: an invite token must be unusable as an access token and
        // vice versa -- `accept()` enforces this by checking `typ`, so the
        // claim actually needs to be there and readable, not just present
        // in the type definition.
        use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Claims {
            sub: String,
            typ: String,
        }

        with_access_token_secret(|| {
            let user = invited_user();
            let invite = AccountInviteUseCase::issue(&user).unwrap();
            let key = format!(
                "{}:invite:{}",
                std::env::var("ACCESS_TOKEN_SECRET").unwrap(),
                user.password
            );
            let decoded = decode::<Claims>(
                &invite.token,
                &DecodingKey::from_secret(key.as_bytes()),
                &Validation::new(Algorithm::HS512),
            )
            .unwrap();
            assert_eq!(decoded.claims.sub, user.email);
            assert_eq!(decoded.claims.typ, "invite");
        });
    }

    #[test]
    fn a_password_hash_change_produces_a_differently_signed_token() {
        // This is PD-003's actual mechanism: the signing key is derived from
        // the password hash, so an invite issued before a password change
        // cannot be mistaken for one issued after -- accept() is what
        // rejects the *old* token against the *new* hash, but the
        // precondition for that (different signature) starts here.
        with_access_token_secret(|| {
            let before_reset = invited_user();
            let after_reset = User {
                password: "$2b$12$totally-different-hash".to_string(),
                ..invited_user()
            };
            let first = AccountInviteUseCase::issue(&before_reset).unwrap();
            let second = AccountInviteUseCase::issue(&after_reset).unwrap();
            assert_ne!(first.token, second.token);
        });
    }
}
