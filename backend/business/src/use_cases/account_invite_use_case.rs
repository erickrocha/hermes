use crate::domain::business_error::BusinessError;
use crate::gateway::user_gateway::UserGateway;
use chrono::{DateTime, Utc};
use entity::user_entity;
use base64::Engine;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{ActiveModelTrait, DbConn, Set};
use serde::{Deserialize, Serialize};
use std::env;

/// Validade do convite. Curta o bastante para limitar a janela de um link vazado,
/// longa o bastante para o paciente abrir o e-mail no fim de semana.
const INVITE_VALIDITY_DAYS: i64 = 7;

/// Tamanho mínimo da senha escolhida pelo paciente.
const MIN_PASSWORD_LEN: usize = 8;

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
    pub fn issue(user: &user_entity::Model) -> Result<AccountInvite, BusinessError> {
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

        log::info!("[AccountInviteUseCase::issue] Invite issued for {}", user.email);
        Ok(AccountInvite { token, expires_at })
    }

    /// Consome o convite: valida e grava a senha escolhida pelo paciente.
    pub async fn accept(
        db: &DbConn,
        token: &str,
        new_password: &str,
    ) -> Result<(), BusinessError> {
        if new_password.chars().count() < MIN_PASSWORD_LEN {
            return Err(BusinessError::new(format!(
                "Password must be at least {} characters long",
                MIN_PASSWORD_LEN
            )));
        }

        // O e-mail sai do payload sem validar assinatura, só para localizar a conta:
        // a assinatura só pode ser conferida depois, porque a chave depende do hash
        // da senha guardado no banco. Nada deste passo é confiado — a verificação
        // real acontece logo abaixo.
        let email = Self::peek_subject(token)?;

        let user = UserGateway::find_by_email(db, email)
            .await
            .map_err(|e| BusinessError::new(format!("Database error validating invite: {}", e)))?
            .ok_or_else(|| BusinessError::new("Invalid invite".to_string()))?;

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
            log::warn!("[AccountInviteUseCase::accept] Rejected invite for {}", user.email);
            BusinessError::new("Invite is invalid or has already been used".to_string())
        })?;

        if verified.claims.typ != "invite" {
            return Err(BusinessError::new("Invalid invite".to_string()));
        }

        let hashed = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .map_err(|e| BusinessError::new(format!("Failed to hash password: {}", e)))?;

        let mut active: user_entity::ActiveModel = user.clone().into();
        active.password = Set(hashed);
        active.first_login = Set(false);
        active.enabled = Set(true);
        active
            .update(db)
            .await
            .map_err(|e| BusinessError::new(format!("Failed to set password: {}", e)))?;

        log::info!("[AccountInviteUseCase::accept] Invite accepted for {}", user.email);
        Ok(())
    }

    /// Gera uma senha aleatória para contas recém-provisionadas. Ela nunca é
    /// exibida a ninguém: existe só para que a conta não tenha segredo previsível
    /// antes de o paciente aceitar o convite.
    pub fn unguessable_secret() -> String {
        format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4())
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
