mod authentication;
mod commons;
mod endpoints;
mod infrastructure;
mod routes;

use crate::endpoints::welcome_endpoint::welcome;
use crate::infrastructure::health::{
    api_docs_enabled, app_env, health_app, health_route, load_dotenv, resolve_cors_origins,
};
use crate::routes::authentication_routes::auth_routes;
use crate::routes::business_plan_routes::business_plan_routes;
use crate::routes::customer_routes::customer_routes;
use crate::routes::holiday_routes::holiday_routes;
use crate::routes::checklist_template_routes::checklist_template_routes;
use crate::routes::checklist_run_routes::checklist_run_routes;
use crate::routes::work_order_routes::work_order_routes;
use crate::routes::maintenance_plan_routes::maintenance_plan_routes;
use crate::routes::service_type_routes::service_type_routes;
use crate::routes::priced_service_routes::priced_service_routes;
use crate::routes::transport_demand_routes::transport_demand_routes;
use crate::routes::extra_trip_routes::extra_trip_routes;
use crate::routes::resource_routes::resources_routes;
use crate::routes::tenant_routes::tenant_routes;
use crate::routes::user_routes::user_routes;
use crate::routes::vehicle_routes::vehicle_routes;
use axum::Router;
use axum::http::{Method, header};
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
        endpoints::tenant_endpoint::get_by_uuid,
        endpoints::tenant_endpoint::list_all,
        endpoints::tenant_endpoint::update_by_uuid,
        endpoints::tenant_endpoint::add_plan_by_uuid,
        endpoints::tenant_endpoint::get_active_plan_by_uuid,
        endpoints::business_plan_endpoint::add,
        endpoints::business_plan_endpoint::list_all,
        endpoints::business_plan_endpoint::get_by_uuid,
        endpoints::business_plan_endpoint::update,
        endpoints::business_plan_endpoint::delete,
        endpoints::user_endpoint::get_by_uuid,
        endpoints::user_endpoint::add,
        endpoints::user_endpoint::list_all,
        endpoints::user_endpoint::update,
        endpoints::user_endpoint::reissue_invite,
        endpoints::user_endpoint::change_password,
        endpoints::province_endpoint::list_all,
        endpoints::province_endpoint::list_countries,
        endpoints::province_endpoint::list_page,
        endpoints::province_endpoint::get_by_id,
        endpoints::province_endpoint::save,
        endpoints::province_endpoint::import,
        endpoints::city_endpoint::list_all,
        endpoints::city_endpoint::get_by_province,
        endpoints::city_endpoint::get_by_id,
        endpoints::city_endpoint::save,
        endpoints::city_endpoint::import,
        endpoints::customer_endpoint::add,
        endpoints::customer_endpoint::list_all,
        endpoints::customer_endpoint::get_by_uuid,
        endpoints::customer_endpoint::update,
        endpoints::customer_day_off_endpoint::add,
        endpoints::customer_day_off_endpoint::history,
        endpoints::customer_day_off_endpoint::remove,
        endpoints::holiday_endpoint::add,
        endpoints::holiday_endpoint::list_all,
        endpoints::holiday_endpoint::remove,
        endpoints::transport_demand_endpoint::add,
        endpoints::transport_demand_endpoint::list_all,
        endpoints::transport_demand_endpoint::get_by_uuid,
        endpoints::transport_demand_endpoint::update,
        endpoints::transport_demand_allocation_endpoint::add,
        endpoints::transport_demand_allocation_endpoint::history,
        endpoints::transport_demand_allocation_endpoint::update,
        endpoints::daily_schedule_endpoint::add,
        endpoints::daily_schedule_endpoint::history,
        endpoints::daily_schedule_endpoint::remove,
        endpoints::schedule_exception_endpoint::add,
        endpoints::schedule_exception_endpoint::history,
        endpoints::schedule_exception_endpoint::remove,
        endpoints::extra_trip_endpoint::add,
        endpoints::extra_trip_endpoint::list_all,
        endpoints::extra_trip_endpoint::get_by_uuid,
        endpoints::extra_trip_endpoint::update,
        endpoints::extra_trip_endpoint::import,
        endpoints::vehicle_endpoint::add,
        endpoints::vehicle_endpoint::list_all,
        endpoints::vehicle_endpoint::get_by_uuid,
        endpoints::vehicle_endpoint::update,
        endpoints::vehicle_endpoint::tracking,
        endpoints::km_evolution_endpoint::add,
        endpoints::km_evolution_endpoint::history,
        endpoints::checklist_template_endpoint::add,
        endpoints::checklist_template_endpoint::get_by_uuid,
        endpoints::checklist_template_endpoint::list_all,
        endpoints::checklist_run_endpoint::add,
        endpoints::checklist_run_endpoint::get_by_uuid,
        endpoints::checklist_run_endpoint::possession,
        endpoints::work_order_endpoint::add,
        endpoints::work_order_endpoint::get_by_uuid,
        endpoints::work_order_endpoint::list_all,
        endpoints::work_order_endpoint::add_item,
        endpoints::work_order_endpoint::conclude,
        endpoints::work_order_endpoint::cancel,
        endpoints::maintenance_plan_endpoint::add,
        endpoints::maintenance_plan_endpoint::get_by_uuid,
        endpoints::maintenance_plan_endpoint::list_all,
        endpoints::service_type_endpoint::add,
        endpoints::service_type_endpoint::get_by_uuid,
        endpoints::service_type_endpoint::list_all,
        endpoints::priced_service_endpoint::add,
        endpoints::priced_service_endpoint::get_by_uuid,
        endpoints::priced_service_endpoint::list_all,
        endpoints::vehicle_assignment_endpoint::assign,
        endpoints::vehicle_assignment_endpoint::current,
        endpoints::vehicle_assignment_endpoint::end,
        endpoints::vehicle_assignment_endpoint::history
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
            endpoints::json::customer_json::CustomerJson,
            endpoints::json::customer_day_off_json::CustomerDayOffJson,
            endpoints::json::holiday_json::HolidayJson,
            endpoints::json::transport_demand_json::TransportDemandJson,
            endpoints::json::transport_demand_allocation_json::TransportDemandAllocationJson,
            endpoints::json::daily_schedule_json::DailyScheduleJson,
            endpoints::json::schedule_exception_json::ScheduleExceptionJson,
            endpoints::json::extra_trip_json::ExtraTripJson,
            endpoints::json::km_evolution_json::KmEvolutionJson,
            endpoints::json::checklist_template_json::ChecklistTemplateJson,
            endpoints::json::checklist_template_item_json::ChecklistTemplateItemJson,
            endpoints::json::checklist_run_json::ChecklistRunJson,
            endpoints::json::checklist_answer_json::ChecklistAnswerJson,
            endpoints::json::work_order_json::WorkOrderJson,
            endpoints::json::work_order_item_json::WorkOrderItemJson,
            endpoints::json::maintenance_plan_json::MaintenancePlanJson,
            endpoints::json::service_type_json::ServiceTypeJson,
            endpoints::json::priced_service_json::PricedServiceJson,
            endpoints::json::vehicle_json::VehicleJson,
            endpoints::json::vehicle_assignment_json::VehicleAssignmentJson,
            endpoints::json::vehicle_assignment_json::AssignDriverRequest,
            business::domain::vehicle_tracking::VehicleTrackingStatus,
            business::domain::vehicle_tracking::GeoPoint,
            business::domain::vehicle_tracking::IgnitionState,
            business::domain::vehicle_tracking::TrackingSource,
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
    // `/` redirects to Swagger UI, which only exists in development; elsewhere
    // a redirect to a 404 would be the first thing anyone sees.
    if api_docs_enabled() {
        Router::new().route("/", get(welcome))
    } else {
        Router::new().route("/", get(|| async { "Hermes API" }))
    }
}

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    health_app().expect("ENV don't have the required config");
    tracing_subscriber::fmt::init();

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
            "Database schema is {} migration(s) behind: {}. Run `hermes migrate` \
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

    let cors =
        match resolve_cors_origins(app_env().as_deref(), env::var("CORS_ALLOWED_ORIGINS").ok())
            .map_err(|problem| anyhow::anyhow!("Refusing to start: {problem}"))?
        {
            Some(origins) => cors.allow_origin(origins),
            None => cors.allow_origin(Any),
        };

    let mut app = Router::new();
    if api_docs_enabled() {
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }
    let app = app
        // Public routes (no authentication)
        .merge(welcome_route())
        .merge(health_route())
        .merge(auth_routes(state.clone()))
        .merge(resources_routes(state.clone()))
        .nest("/tenant", tenant_routes(state.clone()))
        .nest("/business-plan", business_plan_routes(state.clone()))
        .nest("/customer", customer_routes(state.clone()))
        .nest("/holiday", holiday_routes(state.clone()))
        .nest("/checklist-template", checklist_template_routes(state.clone()))
        .nest("/checklist-run", checklist_run_routes(state.clone()))
        .nest("/work-order", work_order_routes(state.clone()))
        .nest("/maintenance-plan", maintenance_plan_routes(state.clone()))
        .nest("/service-type", service_type_routes(state.clone()))
        .nest("/priced-service", priced_service_routes(state.clone()))
        .nest("/transport-demand", transport_demand_routes(state.clone()))
        .nest("/extra-trip", extra_trip_routes(state.clone()))
        .nest("/user", user_routes(state.clone()))
        .nest("/vehicle", vehicle_routes(state.clone()))
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
    // Only the database is needed here: the server's secrets are checked when it boots.
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
            let prefix = nest_prefix(&file);
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
        // rustfmt puts a long `.route(` call's path on the next line.
        for (index, _) in source.match_indices(".route(") {
            let rest = source[index + ".route(".len()..].trim_start();
            if let Some(literal) = rest.strip_prefix('"')
                && let Some(end) = literal.find('"')
            {
                found.push(literal[..end].to_string());
            }
        }
        found
    }

    /// Os routers de recurso são montados sob um prefixo em `main.rs`; o nome do
    /// arquivo diz qual, para o caminho documentado bater com o servido.
    fn nest_prefix(file: &std::path::Path) -> &'static str {
        match file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
        {
            "tenant_routes" => "/tenant",
            "business_plan_routes" => "/business-plan",
            "user_routes" => "/user",
            "vehicle_routes" => "/vehicle",
            "customer_routes" => "/customer",
            "holiday_routes" => "/holiday",
            "checklist_template_routes" => "/checklist-template",
            "checklist_run_routes" => "/checklist-run",
            "work_order_routes" => "/work-order",
            "maintenance_plan_routes" => "/maintenance-plan",
            "service_type_routes" => "/service-type",
            "priced_service_routes" => "/priced-service",
            "transport_demand_routes" => "/transport-demand",
            "extra_trip_routes" => "/extra-trip",
            _ => "",
        }
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

    /// EPIC-IA-09-S04 (HRMS-132, D-22): "the role matrix is a published
    /// contract" means every operation's own documentation says who may
    /// reach it, not that a reader can work it out from
    /// `authorization.rs`. Each `#[utoipa::path]` carries the statement on
    /// at least one response as `**Roles:** ...`; this only proves the
    /// marker is present on every operation, not that it names the right
    /// roles -- that half is unenforceable without duplicating
    /// `authorization.rs` here.
    #[test]
    fn every_operation_states_who_may_reach_it() {
        use utoipa::openapi::RefOr;
        use utoipa::openapi::path::{Operation, PathItem};

        fn operations(item: &PathItem) -> Vec<(&'static str, &Operation)> {
            [
                ("GET", &item.get),
                ("PUT", &item.put),
                ("POST", &item.post),
                ("DELETE", &item.delete),
            ]
            .into_iter()
            .filter_map(|(method, op)| op.as_ref().map(|op| (method, op)))
            .collect()
        }

        let openapi = ApiDoc::openapi();
        let mut missing = Vec::new();
        for (path, item) in &openapi.paths.paths {
            for (method, operation) in operations(item) {
                let states_roles = operation.responses.responses.values().any(|r| match r {
                    RefOr::T(response) => response.description.contains("**Roles:**"),
                    RefOr::Ref(_) => false,
                });
                if !states_roles {
                    missing.push(format!("{method} {path}"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "these operations document no response with a **Roles:** statement (HRMS-132): {missing:?}"
        );
    }
}
