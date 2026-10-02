//! Delivers the account-invitation email (EPIC-IA-07, D-07). Before this
//! module, `AccountInviteUseCase::issue` minted a token with nowhere to go --
//! the decision register was explicit that "wire `issue` into user creation"
//! and "add an email path" are the same piece of work, not two.
//!
//! SMTP configuration is read lazily, per send, rather than once at boot
//! (contrast `ACCESS_TOKEN_SECRET`/`SYSADMIN_EMAIL` in `application`'s `start` (main.rs), which
//! fail the whole process at startup if absent). Deliberately not fail-fast:
//! those two are secrets whose *absence* is itself a security posture worth
//! refusing to boot over. A missing mail server only blocks one workflow
//! (a new user can't yet set their password) -- the caller of `send_invite`
//! decides whether that should fail the request or just be logged loudly,
//! and today it is logged (see `user_endpoint::add`), not fatal.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::env;

pub struct EmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl EmailSender {
    /// `SMTP_HOST`/`SMTP_USER`/`SMTP_PASSWORD`/`SMTP_FROM` are required;
    /// `SMTP_PORT` defaults to `587` (STARTTLS).
    pub fn from_env() -> Result<Self, String> {
        let host = env::var("SMTP_HOST").map_err(|_| "SMTP_HOST must be set".to_string())?;
        let user = env::var("SMTP_USER").map_err(|_| "SMTP_USER must be set".to_string())?;
        let password =
            env::var("SMTP_PASSWORD").map_err(|_| "SMTP_PASSWORD must be set".to_string())?;
        let from = env::var("SMTP_FROM").map_err(|_| "SMTP_FROM must be set".to_string())?;
        let port: u16 = env::var("SMTP_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(587);

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .map_err(|e| format!("Invalid SMTP_HOST: {e}"))?
            .port(port)
            .credentials(Credentials::new(user, password))
            .build();

        Ok(Self { transport, from })
    }

    /// `accept_url` is the backoffice's `/accept-invite?token=...` link --
    /// built by the caller, which is the one place that knows the
    /// backoffice's own base URL.
    pub async fn send_invite(&self, to_email: &str, accept_url: &str) -> Result<(), String> {
        let body = format!(
            "You've been invited to Hermes.\n\nSet your password to get started:\n{accept_url}\n\n\
             This link expires in 7 days and can only be used once."
        );
        let email = Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|e| format!("Invalid SMTP_FROM: {e}"))?,
            )
            .to(to_email
                .parse()
                .map_err(|e| format!("Invalid recipient address: {e}"))?)
            .header(ContentType::TEXT_PLAIN)
            .subject("You're invited to Hermes")
            .body(body)
            .map_err(|e| format!("Failed to build invite email: {e}"))?;

        self.transport
            .send(email)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to send invite email: {e}"))
    }
}
