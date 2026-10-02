use crate::AppState;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::routing::get;
use business::sea_orm::ConnectionTrait;
use std::env;
use std::time::Duration;

/// EPIC-XF-10-S05 (HRMS-042): the endpoint a deployment is verified against.
///
/// Deliberately **not** in `routes/*.rs` and **not** in `ApiDoc`, following
/// `welcome_route`'s precedent: `documented_paths_match_the_registered_routes`
/// compares the OpenAPI document against the literals in `routes/*.rs`, so an
/// operational route registered there without a `#[utoipa::path]` would fail
/// that test, and documenting it would put a liveness probe in the tenant-facing
/// API contract. It is infrastructure, not product surface.
///
/// It answers *"can this instance serve a request that needs the database"*,
/// not merely *"is the process up"* — a process that is listening but cannot
/// reach MariaDB is exactly the post-deploy failure `HRMS-042` exists to catch,
/// and it is the state a container orchestrator would otherwise call healthy.
/// How long the probe waits for the database before calling it unreachable.
///
/// This bound is not decoration. Without it the probe inherits SeaORM's
/// connection-acquire timeout — measured at **~10.1s** against a stopped
/// MariaDB — while the two things that actually consume this endpoint allow
/// far less: the image's `HEALTHCHECK` gives it 5s, and Traefik's load-balancer
/// health check defaults to 5s as well. The result was the worst of both
/// worlds: the endpoint computed the correct 503 but nobody ever read it,
/// because both checkers had already timed out.
///
/// A timeout and a 503 are not the same signal. `docs/OPERATIONS.md` asks the
/// on-call to distinguish "the process is gone" from "the process is up but the
/// database is unreachable", and that distinction only survives if the probe
/// answers inside the window it is given.
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub fn health_route() -> Router<AppState> {
    Router::new().route(
        "/health",
        get(|State(state): State<AppState>| async move {
            let probe = state.conn.execute_unprepared("SELECT 1");

            let reachable = match tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await {
                Ok(Ok(_)) => true,
                Ok(Err(error)) => {
                    log::error!("Health check failed: database unreachable ({error})");
                    false
                }
                Err(_elapsed) => {
                    log::error!(
                        "Health check failed: database did not respond within {}s",
                        HEALTH_PROBE_TIMEOUT.as_secs()
                    );
                    false
                }
            };

            health_response(reachable)
        }),
    )
}

/// The mapping `HRMS-042` actually turns on, kept pure so it is tested without
/// a database: an instance that cannot reach MariaDB must answer **503**, not
/// 200. A health endpoint that returns 200 whenever the process is listening
/// tells a deploy script nothing it did not already know from the port being
/// open.
fn health_response(database_reachable: bool) -> (StatusCode, &'static str) {
    if database_reachable {
        (StatusCode::OK, r#"{"status":"ok","database":"up"}"#)
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"status":"degraded","database":"down"}"#,
        )
    }
}

/// `APP_ENV` unset or `development` is a developer machine; anything else
/// (`production`, `staging`, ...) is treated as production-like. One definition
/// drives both rules below, so they can never disagree about where they are.
fn is_development(app_env: Option<&str>) -> bool {
    match app_env.map(str::trim) {
        None | Some("") => true,
        Some(env) => env.eq_ignore_ascii_case("development"),
    }
}

pub fn app_env() -> Option<String> {
    env::var("APP_ENV").ok()
}

/// Origens permitidas, de `CORS_ALLOWED_ORIGINS` (lista separada por vírgula).
fn parse_allowed_origins(raw: Option<String>) -> Option<Vec<HeaderValue>> {
    let origins: Vec<HeaderValue> = raw?
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| origin.parse().ok())
        .collect();
    (!origins.is_empty()).then_some(origins)
}

/// D-11 / OBS-1 (owner, 2026-09-18): production defines its origins; a
/// developer machine accepts any. Outside development a missing list is a boot
/// error, not a silent fall back to `*`.
pub fn resolve_cors_origins(
    app_env: Option<&str>,
    raw: Option<String>,
) -> Result<Option<Vec<HeaderValue>>, String> {
    match parse_allowed_origins(raw) {
        Some(origins) => Ok(Some(origins)),
        None if is_development(app_env) => Ok(None),
        None => Err("CORS_ALLOWED_ORIGINS must be set outside development".to_string()),
    }
}

/// PD-032 as amended by the owner (2026-09-18, DEF-XF-05): the OpenAPI surface
/// -- Swagger UI *and* the JSON document -- exists only in development.
pub fn api_docs_enabled() -> bool {
    is_development(app_env().as_deref())
}

/// A missing `.env` is normal (production takes its environment from the
/// platform). A *malformed* one is not: dotenvy stops at the first bad line and
/// silently skips everything after it. `.ok()` used to hide that, and an
/// unquoted `SMTP_FROM=Hermes <...>` meant every variable below it -- including
/// `APP_ENV` and `CORS_ALLOWED_ORIGINS` -- was never loaded, so a production
/// posture written in `.env` quietly fell back to Swagger on and CORS `*`.
pub fn load_dotenv() -> anyhow::Result<()> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => anyhow::bail!(
            "Refusing to start: .env is malformed ({error}). Quote values that contain spaces or <>."
        ),
    }
}

/// DEF-XF-03 / DEF-XF-04: the token secrets are checked once, before anything
/// is served. They used to be read lazily with `expect()` on each token
/// operation, so an empty `ACCESS_TOKEN_SECRET` booted fine and a SysAdmin
/// token forged with an empty HMAC key was accepted, while a missing one booted
/// and then crashed the first login.
///
/// 32 bytes is the floor for an HMAC-SHA256 key worth the name. The two must
/// also differ: with one shared secret a refresh token verifies as an access
/// token and the other way round.
const MIN_TOKEN_SECRET_BYTES: usize = 32;

fn validate_token_secrets(access: Option<String>, refresh: Option<String>) -> Result<(), String> {
    let check = |name: &str, value: &Option<String>| -> Result<(), String> {
        match value.as_deref().map(str::trim) {
            None | Some("") => Err(format!("{name} must be set")),
            Some(secret) if secret.len() < MIN_TOKEN_SECRET_BYTES => Err(format!(
                "{name} must be at least {MIN_TOKEN_SECRET_BYTES} bytes (generate one with `openssl rand -hex 32`)"
            )),
            Some(_) => Ok(()),
        }
    };
    check("ACCESS_TOKEN_SECRET", &access)?;
    check("REFRESH_TOKEN_SECRET", &refresh)?;
    if access.as_deref().map(str::trim) == refresh.as_deref().map(str::trim) {
        return Err("ACCESS_TOKEN_SECRET and REFRESH_TOKEN_SECRET must differ".to_string());
    }
    Ok(())
}

/// DEF-IA-09 (HRMS-109): the seeded administrator is the most privileged
/// account on the platform, and it was the one password on it exempt from the
/// minimum length -- `SYSADMIN_PASSWORD=Short1` booted and signed in. Checked
/// here, with the token secrets, so a bad value is a refusal to start rather
/// than a weak account discovered later.
fn validate_sysadmin_credentials(
    email: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    match email.as_deref().map(str::trim) {
        None | Some("") => return Err("SYSADMIN_EMAIL must be set".to_string()),
        Some(_) => {}
    }
    match password.as_deref() {
        None => Err("SYSADMIN_PASSWORD must be set".to_string()),
        Some(secret) if secret.trim().is_empty() => {
            Err("SYSADMIN_PASSWORD must be set".to_string())
        }
        Some(secret) => business::domain::password_policy::validate_length(secret)
            .map_err(|problem| format!("SYSADMIN_PASSWORD {}", problem.message.to_lowercase())),
    }
}

pub fn health_app() -> anyhow::Result<()> {
    load_dotenv()?;
    validate_token_secrets(
        env::var("ACCESS_TOKEN_SECRET").ok(),
        env::var("REFRESH_TOKEN_SECRET").ok(),
    )
    .map_err(|problem| anyhow::anyhow!("Refusing to start: {problem}"))?;
    validate_sysadmin_credentials(
        env::var("SYSADMIN_EMAIL").ok(),
        env::var("SYSADMIN_PASSWORD").ok(),
    )
    .map_err(|problem| anyhow::anyhow!("Refusing to start: {problem}"))?;
    Ok(())
}

/// EPIC-XF-10-S05 (HRMS-042).
#[cfg(test)]
mod health_endpoint_tests {
    use super::{HEALTH_PROBE_TIMEOUT, health_response};
    use crate::ApiDoc;
    use axum::http::StatusCode;

    /// Guards the bug this endpoint actually shipped with, which an in-process
    /// test could never have caught: the probe returned the correct 503, but
    /// only after ~10.1s, because it inherited SeaORM's connection-acquire
    /// timeout. Both consumers give it 5s — the image `HEALTHCHECK` and
    /// Traefik's load-balancer health check — so the 503 was computed and then
    /// thrown away, and the observable behaviour was a timeout.
    ///
    /// 4s leaves headroom under the tighter of the two windows. If someone
    /// raises `HEALTH_PROBE_TIMEOUT` past it, they must also raise both
    /// checkers, and this test is where they find that out.
    #[test]
    fn the_probe_answers_inside_the_window_its_checkers_allow() {
        assert!(
            HEALTH_PROBE_TIMEOUT.as_secs() < 4,
            "HEALTH_PROBE_TIMEOUT is {}s; the image HEALTHCHECK and Traefik both \
             allow 5s, so a probe at or above that window reports a timeout \
             instead of the 503 it computed",
            HEALTH_PROBE_TIMEOUT.as_secs()
        );
    }

    #[test]
    fn a_reachable_database_is_healthy() {
        let (status, body) = health_response(true);
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(r#""status":"ok""#), "body was {body}");
    }

    /// The case the endpoint exists for. A deploy that leaves the API listening
    /// but unable to reach MariaDB — wrong `DATABASE_URL`, database container
    /// not up yet, credentials rotated without the app being told — is the
    /// failure `HRMS-042` must report as failed rather than as silently running.
    #[test]
    fn an_unreachable_database_is_not_healthy() {
        let (status, body) = health_response(false);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains(r#""database":"down""#), "body was {body}");
    }

    /// `/health` is intentionally absent from both `routes/*.rs` and `ApiDoc`.
    /// `openapi_contract_tests::documented_paths_match_the_registered_routes`
    /// compares those two sets exactly, so registering the operational route in
    /// `routes/` without documenting it would turn that test red. This pins the
    /// arrangement so a later move does not break the contract test in a way
    /// that looks unrelated to whoever moved it.
    #[test]
    fn health_is_operational_surface_not_api_contract() {
        let routes_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes");
        for entry in std::fs::read_dir(&routes_dir).expect("routes directory is readable") {
            let file = entry.expect("readable dir entry").path();
            if file.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&file).expect("readable route file");
            assert!(
                !source.contains("\"/health\""),
                "{} registers /health; it must stay in infrastructure/health.rs and out of the OpenAPI document",
                file.display()
            );
        }

        use utoipa::OpenApi as _;
        assert!(
            !ApiDoc::openapi().paths.paths.contains_key("/health"),
            "/health must not appear in the tenant-facing API contract"
        );
    }
}

#[cfg(test)]
mod environment_gates_tests {
    use super::{is_development, parse_allowed_origins, resolve_cors_origins};

    #[test]
    fn only_unset_or_development_counts_as_development() {
        assert!(is_development(None));
        assert!(is_development(Some("")));
        assert!(is_development(Some(" Development ")));
        assert!(!is_development(Some("production")));
        assert!(!is_development(Some("staging")));
    }

    #[test]
    fn origin_list_is_split_and_trimmed() {
        let origins = parse_allowed_origins(Some(
            " https://app.hermes.io , https://admin.hermes.io ".to_string(),
        ))
        .expect("two valid origins");
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0], "https://app.hermes.io");
        assert_eq!(origins[1], "https://admin.hermes.io");
        assert!(parse_allowed_origins(Some("  ,  ".to_string())).is_none());
    }

    #[test]
    fn development_without_a_list_accepts_any_origin() {
        assert_eq!(resolve_cors_origins(Some("development"), None), Ok(None));
        assert_eq!(resolve_cors_origins(None, None), Ok(None));
    }

    #[test]
    fn production_without_a_list_refuses_to_start() {
        assert!(resolve_cors_origins(Some("production"), None).is_err());
        assert!(resolve_cors_origins(Some("production"), Some(" , ".to_string())).is_err());
        assert!(resolve_cors_origins(Some("staging"), None).is_err());
    }

    #[test]
    fn a_configured_list_is_used_everywhere() {
        let list = Some("https://app.hermes.io".to_string());
        assert_eq!(
            resolve_cors_origins(Some("production"), list.clone())
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            resolve_cors_origins(Some("development"), list)
                .unwrap()
                .unwrap()
                .len(),
            1
        );
    }
}

#[cfg(test)]
mod token_secret_tests {
    use super::validate_token_secrets;

    fn secret(c: char) -> Option<String> {
        Some(c.to_string().repeat(32))
    }

    #[test]
    fn two_distinct_long_secrets_are_accepted() {
        assert!(validate_token_secrets(secret('a'), secret('b')).is_ok());
    }

    #[test]
    fn missing_or_blank_secrets_are_refused() {
        assert!(validate_token_secrets(None, secret('b')).is_err());
        assert!(validate_token_secrets(Some(String::new()), secret('b')).is_err());
        assert!(validate_token_secrets(Some("   ".to_string()), secret('b')).is_err());
        assert!(validate_token_secrets(secret('a'), None).is_err());
    }

    #[test]
    fn short_secrets_are_refused_including_the_example_placeholders() {
        let example_access = Some("change-me-access-secret".to_string());
        let example_refresh = Some("change-me-refresh-secret".to_string());
        assert!(validate_token_secrets(example_access, example_refresh).is_err());
    }

    #[test]
    fn a_shared_secret_is_refused() {
        assert!(validate_token_secrets(secret('a'), secret('a')).is_err());
    }
}

#[cfg(test)]
mod sysadmin_credential_tests {
    use super::validate_sysadmin_credentials;

    fn email() -> Option<String> {
        Some("admin@hermes.io".to_string())
    }

    #[test]
    fn a_long_enough_password_is_accepted() {
        assert!(validate_sysadmin_credentials(email(), Some("Str0ngEnough".to_string())).is_ok());
    }

    #[test]
    fn the_reported_six_character_password_is_refused() {
        // DEF-IA-09 reproduced: `SYSADMIN_PASSWORD=Short1` used to boot.
        assert!(validate_sysadmin_credentials(email(), Some("Short1".to_string())).is_err());
    }

    #[test]
    fn missing_or_blank_values_are_refused() {
        assert!(validate_sysadmin_credentials(None, Some("Str0ngEnough".to_string())).is_err());
        assert!(
            validate_sysadmin_credentials(Some("  ".to_string()), Some("Str0ngEnough".to_string()))
                .is_err()
        );
        assert!(validate_sysadmin_credentials(email(), None).is_err());
        assert!(validate_sysadmin_credentials(email(), Some("   ".to_string())).is_err());
    }
}
