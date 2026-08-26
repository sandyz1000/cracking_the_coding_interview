// Concert Ticket booking
//
// ## Entity
// Concert, Seat, Booking, BookingProcessor, PaymentProcessor, User, UserType(Admin, Staff, User), Waitlist
// Artist, Venue

// ## Descriptions
// User should be able to search concert by artist and venue
// User should be able to book multiple the concert ticket
// The system should allow to put user to waitlist if no ticket is found

use axum::{
    Json, Router, extract::{Query, State}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, SqlitePool};
use std::{collections::HashMap, net::SocketAddr, ops::Deref, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Debug)]
struct Location {
    name: String,
    address: String,
    lat: f32,
    long: f32,
    zip_code: String,
}
#[derive(Debug)]
enum SeatType {
    Premium,
    Regular,
}

#[derive(Debug)]
enum SeatStatus {
    Available,
    Reserved,
    Booked,
}

#[derive(Debug)]
struct Seat {
    kind: SeatType,
    price: u64,
    number: String,
    status: SeatStatus,
}

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
enum UserType {
    Admin,
    Staff,
    Customer,
}

#[derive(Debug, sqlx::FromRow)]
struct User {
    name: String,
    email: Email,
    user_type: UserType,
    #[sqlx(rename = "contact_number")]
    contact_no: ContactNumber,
}

impl User {
    fn is_admin(&self) -> bool {
        matches!(self.user_type, UserType::Admin)
    }

    fn new_customer(name: String, email: Email, contact_no: ContactNumber) -> Self {
        Self {
            name,
            email,
            user_type: UserType::Customer,
            contact_no,
        }
    }

    async fn create(self, db: &SqlitePool) -> Result<(), AppError> {
        let mut builder = QueryBuilder::default();
        builder.push("INSERT INTO Users (email, name, contact_number, user_type) ");

        builder.push_values(vec![self], |mut row, record| {
            row.push_bind(record.email)
                .push_bind(record.name)
                .push_bind(record.contact_no)
                .push_bind(record.user_type);
        });

        builder
            .build()
            .execute(&*db)
            .await
            .map_err(|e| AppError::UserCreate(e.to_string()))?;

        Ok(())
    }
}

struct Waitlist {
    concert: String,
    user_id: u64,
    order: u64
}

#[derive(Debug)]
struct Concert {
    venue: Location,
    artist: String,
    event_date: DateTime<Utc>,
    seats: Vec<Seat>
}

#[derive(Debug)]
struct BookingProcessor {
    db: SqlitePool
}

impl BookingProcessor {
    fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    async fn book(&mut self, seat: &[Seat], user_id: u64) -> Result<(), AppError> {
        // Check if the seats
        todo!()
    }

    async fn cancel(&mut self, booking_id: u64, user_id: u64) -> Result<(), AppError> {
        todo!()
    }

    // This put the user to waitlist so that they are given priority when there are cancellation 
    async fn join_waitlist(&mut self, seat: &[Seat], user_id: u64) -> Result<(), AppError> {
        todo!()
    }
}

#[derive(Debug, sqlx::FromRow)]
struct Notification {
    email: String,
    booking_id: u64,
    message: String,
    dispatch_at: DateTime<Utc>
}

// This send notification to user
#[derive(Debug)]
struct NotificationService {
    db: SqlitePool
}

impl NotificationService {
    fn send(&mut self, booking_id: u64, user_id: u64) -> Result<(), AppError> {
        // Get the user for this user_id
        // Get the booking for this user
        // Save the notification to db and invoke the external service 
        Ok(())
    }
}

trait Indexer {
    fn search(&self, search: SearchQuery) -> Result<Concert, AppError>;

    fn build(&self, db: &SqlitePool) -> Result<(), AppError>;
}

type SearchKey = ();

#[derive(Debug)]
struct SearchEngine {
    indexed: HashMap<SearchKey, Concert>
}

impl Indexer for SearchEngine {
    fn search(&self, search: SearchQuery) -> Result<Concert, AppError> {
        todo!()
    }

    fn build(&self, db: &SqlitePool) -> Result<(), AppError> {
        // Find all the concert from the database and build the index for the search
        todo!()
    }
}

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("Unable to create user {0}")]
    UserCreate(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::UserCreate(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
struct Email(String);

impl Email {
    fn validation(&self) -> bool {
        true
    }
}

impl Deref for Email {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
struct ContactNumber(String);

impl ContactNumber {
    fn validation() -> bool {
        true
    }
}

impl Deref for ContactNumber {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
struct UserCreate {
    name: String,
    email: Email, // This will need validation
    contact_number: ContactNumber,
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UserCreate>,
) -> Result<Json<()>, AppError> {
    let user = User::new_customer(payload.name, payload.email, payload.contact_number);
    user.create(&state.db).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    datetime: DateTime<Utc>,
    venue: String,
    artist: Option<String>,
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(SearchQuery): Query<SearchQuery>,
) -> impl IntoResponse {
    Json(())
}

#[derive(Debug, Deserialize)]
struct ConcertAdd {
    venue: String,
    description: String,
    artist: Vec<String>,
    datetime: DateTime<Utc>,
}

async fn concert_add(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConcertAdd>,
) -> impl IntoResponse {
    Json(())
}

async fn concert_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(())
}

async fn book() -> impl IntoResponse {
    Json(())
}

#[derive(Debug, Clone)]
struct AppState {
    db: SqlitePool,
}

async fn app() -> Router {
    let _ = dotenvy::dotenv();

    let db = SqlitePool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
        .await
        .expect("Unable to connect to sqlite db");
    // let pool = Arc::new(&db);
    sqlx::migrate!()
        .run(&db)
        .await
        .expect("Unable to run migration");

    let state = Arc::new(AppState { db });
    let router = Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status": "OK"})) }),
        )
        .merge(Router::new().route("/users", post(create_user)))
        .merge(
            Router::new()
                .route("/concert/search", get(search))
                .route("/concert/book", post(book)),
        )
        .merge(Router::new().route("/concert", post(concert_add).get(concert_index)))
        .layer(
            CorsLayer::new()
                .allow_origin(
                    "http://127.0.0.1:3000"
                        .parse::<axum::http::HeaderValue>()
                        .unwrap(),
                )
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PATCH,
                ]),
        )
        .with_state(state);

    router
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on port: {:?}", 3000);
    let router = app().await;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Email, ContactNumber, User};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[tokio::test]
    async fn create_persists_a_row() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("concert_ticket_test_{}.db", nanos));
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let user = User::new_customer(
            "Alice".to_string(),
            Email("alice@example.com".to_string()),
            ContactNumber("111".to_string()),
        );
        user.create(&pool).await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM Users WHERE email = ?")
            .bind("alice@example.com")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let _ = std::fs::remove_file(&path);
    }
}
