use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, middleware::Logger};
use actix_cors::Cors;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use diesel::{RunQueryDsl, SqliteConnection};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};
use shared::init_logging;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod common;
mod config;
mod notes;
mod schema;

use crate::common::TokenService;
use crate::config::Config;
use crate::notes::api::{NotesApiDoc, NotesApiState};
use crate::notes::repository::NotesRepository;
use crate::notes::service::NotesService;
use shared::drive_client::DriveClient;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

fn create_db_pool(database_url: &str) -> Result<DbPool, String> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .map_err(|e| format!("Failed to create DB pool: {}", e))
}

fn run_migrations(pool: &DbPool) -> Result<(), String> {
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get DB connection: {}", e))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| format!("Failed to run migrations: {}", e))?;
    Ok(())
}

#[get("/health")]
async fn health(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!("Health check DB connection error: {:?}", e);
            return HttpResponse::ServiceUnavailable().json(json!({
                "error": {
                    "code": "DB_UNAVAILABLE",
                    "message": "Database connection unavailable"
                }
            }));
        }
    };

    match diesel::sql_query("SELECT 1").execute(&mut conn) {
        Ok(_) => HttpResponse::Ok().json(json!({"status": "ok"})),
        Err(e) => {
            error!("Health check DB query error: {:?}", e);
            HttpResponse::ServiceUnavailable().json(json!({
                "error": {
                    "code": "DB_UNHEALTHY",
                    "message": "Database health check failed"
                }
            }))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    });

    // Must be held for the lifetime of main — dropping it shuts down the background log flush thread.
    let _log_guard = init_logging(&config.log_level, config.log_path);

    info!("Starting Neutrino Notes service");
    info!("Connecting to database: {}", config.database_url);

    let pool = create_db_pool(&config.database_url).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });

    run_migrations(&pool).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });

    let token_service = Arc::new(TokenService::new(config.jwt_secret.clone()));
    let drive_client = Arc::new(DriveClient::new(config.drive_base_url.clone()));
    let notes_repo = Arc::new(NotesRepository::new(pool.clone()));
    let notes_service = Arc::new(NotesService::new(
        notes_repo,
        drive_client,
        config.drive_base_url.clone(),
    ));
    let notes_state = web::Data::new(NotesApiState { notes_service });

    let token_service_data = web::Data::new(token_service.clone());
    let pool_data = web::Data::new(pool.clone());
    let bind_addr = format!("0.0.0.0:{}", config.port);

    info!("Listening on {}", bind_addr);

    HttpServer::new(move || {
        let openapi = NotesApiDoc::openapi();

        App::new()
            .app_data(pool_data.clone())
            .app_data(notes_state.clone())
            .app_data(token_service_data.clone())
            .wrap(Logger::default())
            .wrap(Cors::permissive())
            .service(health)
            .service(
                web::scope("/api/v1")
                    .configure(notes::api::configure),
            )
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", openapi),
            )
    })
    .bind(&bind_addr)?
    .run()
    .await
}
