use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Extension, Json, Router};
use axum_auth::AuthBasic;
use chrono::{DateTime, Duration, Utc};
use eyre::{Context, Result};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    let env = envy::from_env::<Env>().wrap_err("Reading environment variables failed")?;

    println!("Running migrations...");
    let dbc_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env.database_url)
        .await
        .wrap_err("Connecting to database failed")?;
    sqlx::migrate!()
        .run(&dbc_pool)
        .await
        .wrap_err("Running migrations failed")?;

    let app = Router::new().route("/v1/send", post(send_data)).layer(
        ServiceBuilder::new()
            .layer(Extension(dbc_pool))
            .layer(Extension(Arc::new(Auth {
                username: env.basic_auth_username,
                password: Some(env.basic_auth_password),
            }))),
    );

    let addr: SocketAddr = env
        .bind
        .unwrap_or_else(|| "127.0.0.1:8080".parse().unwrap());
    let listener = TcpListener::bind(&addr).await?;

    println!("Starting server on {}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

struct Auth {
    username: String,
    password: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Env {
    bind: Option<SocketAddr>,
    database_url: String,
    basic_auth_username: String,
    basic_auth_password: String,
}

type Database = Pool<Postgres>;

#[derive(Deserialize)]
struct SendDataBody {
    sensor: String,
    timestamp: DateTime<Utc>,
    temperature: f64,
    humidity: f64,
}

async fn send_data(
    Extension(dbc): Extension<Database>,
    Extension(auth): Extension<Arc<Auth>>,
    AuthBasic((username, password)): AuthBasic,
    body: Json<SendDataBody>,
) -> impl IntoResponse {
    if username != auth.username || password != auth.password {
        return (StatusCode::UNAUTHORIZED, "Unauthorized");
    }

    let now = Utc::now();
    if body.timestamp > now {
        return (StatusCode::BAD_REQUEST, "Timestamp in the future");
    }
    if now - body.timestamp > Duration::minutes(5) {
        return (StatusCode::BAD_REQUEST, "Timestamp too old");
    }

    let result = sqlx::query(
        r#"
INSERT INTO sensor_data_v1 (sensor_id, timestamp, temperature, humidity)
SELECT sensor.id, $1, $2, $3
FROM sensor WHERE sensor.short_name = $4
"#,
    )
    .bind(body.timestamp)
    .bind(body.temperature)
    .bind(body.humidity)
    .bind(&body.sensor)
    .execute(&dbc)
    .await
    .unwrap();

    if result.rows_affected() == 1 {
        (StatusCode::OK, "OK")
    } else {
        (StatusCode::NOT_FOUND, "Sensor not found")
    }
}
