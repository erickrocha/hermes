mod authentication;
mod commons;
mod endpoints;
mod infrastructure;
mod routes;

use crate::endpoints::welcome_endpoint::welcome;
use crate::routes::authentication_routes::auth_routes;
use crate::routes::business_plan_routes::business_plan_routes;
use crate::routes::resource_routes::resources_routes;
use crate::routes::tenant_routes::tenant_routes;
use crate::routes::user_routes::user_routes;
use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::get;
use business::sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use migration::{Migrator, MigratorTrait};
use std::env;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
	modifiers(&SecurityAddon),
	paths(
		endpoints::auth_endpoint::sign_in,
		endpoints::auth_endpoint::refresh_token,
		endpoints::auth_endpoint::accept_invite,
		endpoints::tenant_endpoint::add,
		endpoints::tenant_endpoint::get_by_id,
		endpoints::tenant_endpoint::get_by_uuid,
		endpoints::tenant_endpoint::list_all,
		endpoints::tenant_endpoint::update,
		endpoints::tenant_endpoint::add_plan,
		endpoints::tenant_endpoint::get_active_plan,
		endpoints::business_plan_endpoint::add,
		endpoints::business_plan_endpoint::list_all,
		endpoints::business_plan_endpoint::get_by_id,
		endpoints::business_plan_endpoint::get_by_uuid,
		endpoints::business_plan_endpoint::update,
		endpoints::business_plan_endpoint::delete,
		endpoints::user_endpoint::get_by_id,
		endpoints::user_endpoint::add,
		endpoints::user_endpoint::list_all,
		endpoints::user_endpoint::update,
		endpoints::user_endpoint::change_password,
        endpoints::province_endpoint::list_all,
        endpoints::province_endpoint::list_page,
        endpoints::province_endpoint::get_by_id,
        endpoints::province_endpoint::save,
        endpoints::province_endpoint::import,
        endpoints::city_endpoint::list_all,
        endpoints::city_endpoint::get_by_province,
        endpoints::city_endpoint::get_by_id,
        endpoints::city_endpoint::save,
        endpoints::city_endpoint::import
	),
	components(
		schemas(
			endpoints::json::user_json::UserJson,
			endpoints::json::login_request::LoginRequest,
			endpoints::auth_endpoint::AcceptInviteJson,
			endpoints::json::change_password_request::ChangePasswordRequest,
			endpoints::json::refresh_token_request::RefreshTokenRequest,
			endpoints::json::access_token_json::AccessTokenJson,
			endpoints::json::tenant_json::TenantJson,
			endpoints::json::tenant_json::SetTenantPlanJson,
			endpoints::json::business_plan_json::CreateBusinessPlanJson,
			endpoints::json::business_plan_json::UpdateBusinessPlanJson,
			endpoints::json::business_plan_json::BusinessPlanJson,
            endpoints::json::province_json::ProvinceJson,
            endpoints::json::city_json::CityJson,
		),
	),
	tags(
        (name = "Hermes", description = "REST API for Hermes")
	)
)]
struct ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub conn: Arc<DatabaseConnection>,
}

// ==================== Route Builders ====================
/// Build public welcome route
fn welcome_route() -> Router<AppState> {
    Router::new().route("/", get(welcome))
}

/// Origens permitidas em produção, de `CORS_ALLOWED_ORIGINS` (lista separada por vírgula).
/// `None` = sem lista configurada, o chamador libera qualquer origem (só desenvolvimento).
fn parse_allowed_origins(raw: Option<String>) -> Option<Vec<HeaderValue>> {
    let origins: Vec<HeaderValue> = raw?
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| origin.parse().ok())
        .collect();
    (!origins.is_empty()).then_some(origins)
}

fn cors_allowed_origins() -> Option<Vec<HeaderValue>> {
    parse_allowed_origins(env::var("CORS_ALLOWED_ORIGINS").ok())
}

/// O explorador interativo é conveniência de desenvolvimento: fica fora de produção
/// salvo opt-in explícito. O documento OpenAPI em si não depende disto.
fn swagger_ui_is_enabled(app_env: Option<String>, override_flag: Option<String>) -> bool {
    if let Some(flag) = override_flag {
        return flag.trim().eq_ignore_ascii_case("true");
    }
    !matches!(
        app_env.as_deref().map(str::trim),
        Some(env) if env.eq_ignore_ascii_case("production")
    )
}

fn swagger_ui_enabled() -> bool {
    swagger_ui_is_enabled(env::var("APP_ENV").ok(), env::var("SWAGGER_UI_ENABLED").ok())
}

/// A missing `.env` is normal (production takes its environment from the
/// platform). A *malformed* one is not: dotenvy stops at the first bad line and
/// silently skips everything after it. `.ok()` used to hide that, and an
/// unquoted `SMTP_FROM=Hermes <...>` meant every variable below it -- including
/// `APP_ENV` and `CORS_ALLOWED_ORIGINS` -- was never loaded, so a production
/// posture written in `.env` quietly fell back to Swagger on and CORS `*`.
fn load_dotenv() -> anyhow::Result<()> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => anyhow::bail!("Refusing to start: .env is malformed ({error}). Quote values that contain spaces or <>."),
    }
}

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    // env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();
    load_dotenv()?;
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let host = env::var("HOST").expect("HOST is not set in .env file");
    let port = env::var("PORT").expect("PORT is not set in .env file");
    let server_url = format!("{host}:{port}");

    let connection = Database::connect(&db_url)
        .await
        .expect("Failed to connect to database");

    // PD-033: migrations are a deliberate deploy step, not a boot side effect.
    // Booting no longer applies them -- it refuses to start when the schema is
    // behind, which keeps HRM-072's "never serve on a bad schema" guarantee
    // without letting an application restart rewrite the schema by surprise.
    let pending = Migrator::get_pending_migrations(&connection).await?;
    if !pending.is_empty() {
        let names: Vec<String> = pending.iter().map(|m| m.name().to_string()).collect();
        anyhow::bail!(
            "Database schema is {} migration(s) behind: {}. Run `hermes_server migrate` \
             as a deploy step before starting the server (PD-033).",
            names.len(),
            names.join(", ")
        );
    }

    // D-05: o seed roda fora de qualquer requisição. Com deny-by-default isso
    // é `Denied`, então o alcance de plataforma passa a ser pedido aqui,
    // explicitamente, em vez de herdado por omissão.
    entity::audit::run_as_platform(
        business::use_cases::user_use_case::UserUseCase::seed_sysadmin(&connection),
    )
    .await;

    let state = AppState {
        conn: Arc::new(connection),
    };

    log::info!("Starting server...");

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::ACCESS_CONTROL_ALLOW_METHODS,
            header::ACCESS_CONTROL_ALLOW_HEADERS,
        ]);

    let cors = match cors_allowed_origins() {
        Some(origins) => cors.allow_origin(origins),
        None => cors.allow_origin(Any),
    };

    let mut app = Router::new();
    if swagger_ui_enabled() {
        app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }
    let app = app
        // Public routes (no authentication)
        .merge(welcome_route())
        .merge(auth_routes(state.clone()))
        .merge(resources_routes(state.clone()))
        .nest("/tenant", tenant_routes(state.clone()))
        .nest("/business-plan", business_plan_routes(state.clone()))
        .nest("/user", user_routes(state.clone()))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&server_url).await?;
    log::info!("Server started on address {}", server_url);
    axum::serve(listener, app).await?;

    Ok(())
}

/// PD-033: o passo de deploy. Mantém o lock consultivo porque dois operadores
/// (ou dois jobs de deploy) podem disparar isto ao mesmo tempo — o que saiu do
/// boot foi a aplicação automática, não a necessidade de serializar o DDL.
#[tokio::main]
async fn migrate() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    load_dotenv()?;
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let connection = Database::connect(&db_url)
        .await
        .expect("Failed to connect to database");

    let backend = connection.get_database_backend();
    let lock_row = connection
        .query_one_raw(Statement::from_string(
            backend,
            "SELECT GET_LOCK('hermes_migrations', 30) AS acquired".to_owned(),
        ))
        .await?
        .expect("GET_LOCK query returned no rows");
    let acquired: i64 = lock_row.try_get("", "acquired").unwrap_or(0);
    if acquired != 1 {
        anyhow::bail!("Could not acquire migration lock within timeout");
    }

    let migration_result = Migrator::up(&connection, None).await;

    connection
        .execute_unprepared("SELECT RELEASE_LOCK('hermes_migrations')")
        .await
        .ok();

    migration_result?;
    log::info!("Migrations applied.");
    Ok(())
}

pub fn main() {
    // PD-033: `migrate` applies the schema; no argument starts the server.
    if env::args().nth(1).as_deref() == Some("migrate") {
        if let Err(err) = migrate() {
            log::error!("Migration failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    // EPIC-XF-04-S03 (HRMS-023): a failed migration or an unavailable lock
    // must abort start-up visibly. Before this, `start()`'s error was
    // printed and `main` returned normally -- exit code 0, indistinguishable
    // from a clean shutdown to any process supervisor deciding whether to
    // restart the instance or roll back the deploy. No HTTP server ever
    // bound (the failure happens before `axum::serve`), so no half-migrated
    // instance served traffic either way; what was missing was the signal.
    if let Err(err) = start() {
        log::error!("Startup failed: {err}");
        std::process::exit(1);
    }
}

/// EPIC-XF-05-S03 (HRMS-029, D-10): every path this test lists is copied by
/// hand from the actual route registrations in `routes/*.rs` -- axum 0.8
/// exposes no cheap way to list a live `Router`'s registered paths, so this
/// is not introspecting the router itself, and keeping the list in sync
/// when a route file changes is still a human responsibility. What the
/// test buys is that the two sides that are checkable -- the OpenAPI
/// document `utoipa` generates from `#[utoipa::path]` annotations, and this
/// hand-maintained mirror of the router -- are compared for exact
/// agreement on every build, instead of silently drifting the way `/city`
/// vs `/cities` did for two of three city operations.
#[cfg(test)]
mod openapi_contract_tests {
    use super::ApiDoc;
    use std::collections::BTreeSet;
    use utoipa::OpenApi as _;

    /// Lê as rotas dos próprios `routes/*.rs` em vez de repetir a lista à mão.
    ///
    /// A versão anterior era um array digitado, e por isso o teste podia passar
    /// **com** drift: cinco rotas de PD-027 entraram sem documentação e a suíte
    /// seguiu verde, porque ninguém tinha atualizado nenhum dos dois lados.
    /// Uma constante mantida à mão não prova nada sobre o router — que é
    /// exatamente a crítica que o defeito D-10 fez a AD-006.
    fn actually_registered_paths() -> BTreeSet<String> {
        let routes_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes");
        let mut paths = BTreeSet::new();

        for entry in std::fs::read_dir(&routes_dir).expect("routes directory is readable") {
            let file = entry.expect("readable dir entry").path();
            if file.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&file).expect("readable route file");
            let (prefix, source) = nest_prefix(&source, &file);
            for literal in route_literals(&source) {
                // `.route("/")` dentro de um `nest` é a raiz do prefixo:
                // `/tenant`, não `/tenant/`.
                let full = format!("{prefix}{literal}");
                let full = if !prefix.is_empty() && full.ends_with('/') {
                    full.trim_end_matches('/').to_string()
                } else {
                    full
                };
                paths.insert(full);
            }
        }
        paths
    }

    /// `.route("/x", ...)` e `.nest("/tenant", ...)` — só os literais.
    fn route_literals(source: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (index, _) in source.match_indices(".route(\"") {
            let start = index + ".route(\"".len();
            if let Some(end) = source[start..].find('"') {
                found.push(source[start..start + end].to_string());
            }
        }
        found
    }

    /// Os routers de recurso são montados sob um prefixo em `lib.rs`; o nome do
    /// arquivo diz qual, para o caminho documentado bater com o servido.
    fn nest_prefix<'a>(source: &'a str, file: &std::path::Path) -> (&'static str, &'a str) {
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let prefix = match stem {
            "tenant_routes" => "/tenant",
            "business_plan_routes" => "/business-plan",
            "user_routes" => "/user",
            _ => "",
        };
        (prefix, source)
    }

    #[test]
    fn documented_paths_match_the_registered_routes() {
        let openapi = ApiDoc::openapi();
        let documented: BTreeSet<String> = openapi.paths.paths.keys().cloned().collect();
        let registered = actually_registered_paths();
        assert_eq!(
            documented, registered,
            "OpenAPI documentation and routes/*.rs have drifted apart (D-10) -- update whichever side is now wrong"
        );
    }
}

#[cfg(test)]
mod environment_gates_tests {
    use super::{parse_allowed_origins, swagger_ui_is_enabled};

    #[test]
    fn no_origin_list_means_unrestricted() {
        assert!(parse_allowed_origins(None).is_none());
        assert!(parse_allowed_origins(Some("  ,  ".to_string())).is_none());
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
    }

    #[test]
    fn swagger_ui_is_off_in_production_and_on_elsewhere() {
        assert!(!swagger_ui_is_enabled(Some("production".to_string()), None));
        assert!(!swagger_ui_is_enabled(Some(" PRODUCTION ".to_string()), None));
        assert!(swagger_ui_is_enabled(Some("staging".to_string()), None));
        assert!(swagger_ui_is_enabled(None, None));
    }

    #[test]
    fn explicit_flag_overrides_the_environment() {
        assert!(swagger_ui_is_enabled(
            Some("production".to_string()),
            Some("true".to_string())
        ));
        assert!(!swagger_ui_is_enabled(
            Some("staging".to_string()),
            Some("false".to_string())
        ));
    }
}
