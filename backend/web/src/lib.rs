

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
		
	),
	components(
		schemas(
			
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
	Router::new()
		.route("/", get(welcome))
}

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    env::set_var("RUST_LOG", "debug");
	tracing_subscriber::fmt::init();
	dotenvy::dotenv().ok();
	let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
	let host = env::var("HOST").expect("HOST is not set in .env file");
	let port = env::var("PORT").expect("PORT is not set in .env file");
	let server_url = format!("{host}:{port}");

    let connection = Database::connect(&db_url)
		.await
		.expect("Failed to connect to database");

    // Multiple instances can boot concurrently (App Runner scale-out, rolling deploys).
	// Serialize migrations with a DB-level advisory lock so they don't race on the same DDL.
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
		.execute_unprepared("SELECT RELEASE_LOCK('socialfit_migrations')")
		.await
		.ok();

	migration_result?;

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
		.allow_origin(Any)
		.allow_headers([
			header::CONTENT_TYPE,
			header::AUTHORIZATION,
			header::ACCEPT,
			header::ORIGIN,
			header::ACCESS_CONTROL_ALLOW_ORIGIN,
			header::ACCESS_CONTROL_ALLOW_METHODS,
			header::ACCESS_CONTROL_ALLOW_HEADERS,
		]);

        let app = Router::new()
		.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))

		// Public routes (no authentication)
		.merge(welcome_route())
        .layer(cors)
		.with_state(state);

    let listener = tokio::net::TcpListener::bind(&server_url).await?;
	log::info!("Server started on address {}", server_url);
	axum::serve(listener, app).await?;

	Ok(())

}

pub fn main() {
	let result = start();

	if let Some(err) = result.err() {
		println!("Error: {err}");
	}
}