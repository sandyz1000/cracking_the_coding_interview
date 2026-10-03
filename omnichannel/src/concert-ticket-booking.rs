// Concert Ticket booking
//
// ## Entity
// Concert, Seat, Booking, BookingProcessor, PaymentProcessor, User, UserType(Admin, Staff, User), Waitlist
// Artist, Venue

// ## Descriptions
// User should be able to search concert by artist and venue
// User should be able to book multiple the concert ticket
// The system should allow to put user to waitlist if no ticket is found
use std::sync::RwLock;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    ops::Deref,
    pin::Pin,
    sync::Arc,
};
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Location {
    name: String,
    address: String,
    lat: f32,
    long: f32,
    zip_code: String,
}

// `sqlx(rename_all)` governs SQL binding (how `transition_seat` writes this
// into the seats JSON via json_set); `serde(rename_all)` governs
// serde_json's own (de)serialization of that same JSON blob. Both need to
// agree on casing, or a status written by one path fails to parse via the
// other — they're independent derives with independent defaults.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "UPPERCASE")]
#[sqlx(rename_all = "UPPERCASE")]
enum SeatType {
    Premium,
    Regular,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "UPPERCASE")]
#[sqlx(rename_all = "UPPERCASE")]
enum SeatStatus {
    Available,
    Reserved,
    Booked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Seat {
    kind: SeatType,
    price: i64,
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
    order: u64,
}

#[derive(Debug, Clone)]
struct Concert {
    id: u64,
    venue: sqlx::types::Json<Location>,
    artist: String,
    event_date: DateTime<Utc>,
    seats: Vec<Seat>,
}

impl sqlx::FromRow<'_, SqliteRow> for Concert {
    // `venue` and `seats` are both stored as JSON TEXT on this same row, so
    // both decode here same as each other — no second query needed.
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let json_err = |e: serde_json::Error| sqlx::Error::Decode(Box::new(e));
        let venue: String = row.try_get("venue")?;
        let seats: String = row.try_get("seats")?;
        Ok(Concert {
            id: row.try_get::<i64, _>("id")? as u64,
            venue: sqlx::types::Json(serde_json::from_str(&venue).map_err(json_err)?),
            artist: row.try_get("artist")?,
            event_date: row.try_get("event_date")?,
            seats: serde_json::from_str(&seats).map_err(json_err)?,
        })
    }
}

impl Concert {
    // `seats` decodes straight off the row via `FromRow` now (it's a JSON
    // column, same as `venue`), so this only needs the one column — no need
    // to round-trip through a full `Concert`.
    async fn available_seats(pool: &SqlitePool, concert_id: u64) -> Result<Vec<Seat>, AppError> {
        let seats: String = sqlx::query_scalar("SELECT seats FROM concerts WHERE id = ?")
            .bind(concert_id as i64)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::DbQueryError(e.to_string()))?
            .ok_or_else(|| AppError::DbQueryError(format!("concert {} not found", concert_id)))?;
        serde_json::from_str(&seats).map_err(|e| AppError::DbQueryError(e.to_string()))
    }

    async fn get_by_id(pool: &SqlitePool, id: u64) -> Result<Concert, AppError> {
        sqlx::query_as("SELECT * FROM concerts WHERE id = ?")
            .bind(id as i64)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::DbQueryError(e.to_string()))?
            .ok_or_else(|| AppError::DbQueryError(format!("concert {} not found", id)))
    }

    // Dummy data for demos/tests: one concert, three seats, all Available.
    // Not seeded via a migration on purpose — seed/fixture data doesn't
    // belong baked into a schema migration that would also run in
    // production; this exists to be called explicitly (currently: only
    // from the e2e test).
    async fn seed_dummy(pool: &SqlitePool) -> Result<u64, AppError> {
        let json_err = |e: serde_json::Error| AppError::DbQueryError(e.to_string());
        let venue = serde_json::to_string(&Location {
            name: "Madison Square Garden".to_string(),
            address: "4 Pennsylvania Plaza, New York, NY".to_string(),
            lat: 40.7505,
            long: -73.9934,
            zip_code: "10001".to_string(),
        })
        .map_err(json_err)?;
        let seats = serde_json::to_string(&vec![
            Seat {
                kind: SeatType::Premium,
                price: 250,
                number: "A1".to_string(),
                status: SeatStatus::Available,
            },
            Seat {
                kind: SeatType::Premium,
                price: 250,
                number: "A2".to_string(),
                status: SeatStatus::Available,
            },
            Seat {
                kind: SeatType::Regular,
                price: 90,
                number: "B1".to_string(),
                status: SeatStatus::Available,
            },
        ])
        .map_err(json_err)?;

        sqlx::query_scalar::<_, i64>(
            "INSERT INTO concerts (venue, artist, event_date, seats) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(venue)
        .bind("Coldplay")
        .bind(Utc::now())
        .bind(seats)
        .fetch_one(pool)
        .await
        .map(|id| id as u64)
        .map_err(|e| AppError::DbQueryError(e.to_string()))
    }

    // Atomically flips one seat from `from` to `to`, entirely inside SQLite
    // (json_each/json_extract/json_set) — no read-modify-write through the
    // app. Returns whether it actually happened: `false` means the seat
    // wasn't in `from` state (or the concert doesn't exist), not an error —
    // callers decide what that means for them.
    async fn transition_seat(
        conn: &mut sqlx::SqliteConnection,
        concert_id: u64,
        seat_no: &str,
        from: SeatStatus,
        to: SeatStatus,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE concerts
             SET seats = (
                 SELECT json_group_array(
                     CASE
                         WHEN json_extract(value, '$.number') = ?
                              AND json_extract(value, '$.status') = ?
                         THEN json_set(value, '$.status', ?)
                         ELSE value
                     END
                 )
                 FROM json_each(seats)
             )
             WHERE id = ?
               AND EXISTS (
                   SELECT 1 FROM json_each(seats)
                   WHERE json_extract(value, '$.number') = ?
                     AND json_extract(value, '$.status') = ?
               )",
        )
        .bind(seat_no)
        .bind(from)
        .bind(to)
        .bind(concert_id as i64)
        .bind(seat_no)
        .bind(from)
        .execute(conn)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
enum BookingStatus {
    Booked,
    Cancalled,
    Reserved,
    Completed,
}

#[derive(Debug, sqlx::FromRow)]
struct Booking {
    id: u64,
    concert_id: u64,
    user_id: u64,
    #[sqlx(rename = "transaction_id")]
    txn_id: u32,
    amount: i64,
    refund: i64,
    status: BookingStatus,
    booking_at: DateTime<Utc>,
}

type PinFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
// Same idea as `PinFut`, but for a method whose future needs to borrow
// `self` (or another argument) across its `.await`s — `PinFut` is
// implicitly `'static`, which only works because `PaymentProcessor`'s
// methods don't capture `self`. `Indexer::build` mutates `self.indexed`
// inside the future, so it needs an explicit, non-'static lifetime instead.
type PinFutRef<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

trait PaymentProcessor: Send + Sync + std::fmt::Debug {
    fn charge(&self, amount: u64) -> PinFut<Result<i64, AppError>>;
    fn refund(&self, amount: u64, transaction_id: i64) -> PinFut<Result<i64, AppError>>;
}

#[derive(Debug)]
struct MockPayment {}

impl PaymentProcessor for MockPayment {
    fn charge(&self, amount: u64) -> PinFut<Result<i64, AppError>> {
        Box::pin(async move {
            static NEXT_TXN_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
            println!("Mock charge: {}", amount);
            Ok(NEXT_TXN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        })
    }

    fn refund(&self, amount: u64, transaction_id: i64) -> PinFut<Result<i64, AppError>> {
        Box::pin(async move {
            println!("Mock refund: {} for txn {}", amount, transaction_id);
            Ok(transaction_id)
        })
    }
}

#[derive(Debug)]
struct BookingProcessor {
    db: SqlitePool,
    payment: Arc<dyn PaymentProcessor>,
}

// First reserve the seats
// Proceed with the payment
// Confirm the seats after payment
impl BookingProcessor {
    fn new(db: SqlitePool, payment: Arc<dyn PaymentProcessor>) -> Self {
        Self { db, payment }
    }

    async fn available_seats(&self, concert_id: u64) -> Result<Vec<Seat>, AppError> {
        let seats = Concert::available_seats(&self.db, concert_id)
            .await?
            .into_iter()
            .filter(|s| s.status == SeatStatus::Available)
            .collect();
        Ok(seats)
    }

    async fn reserve(&mut self, concert_id: u64, seat_number: &[String]) -> Result<(), AppError> {
        // All seats in this call succeed or none do — one transaction,
        // rolled back (never committed) on the first seat that isn't
        // actually Available.
        let mut tx = self.db.begin().await?;

        for number in seat_number {
            let claimed = Concert::transition_seat(
                &mut tx,
                concert_id,
                number,
                SeatStatus::Available,
                SeatStatus::Reserved,
            )
            .await?;
            if !claimed {
                return Err(AppError::SeatUnavailable);
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn book(
        &mut self,
        concert_id: u64,
        seat_number: &[String],
        user_id: u64,
    ) -> Result<u64, AppError> {
        // Price the requested seats before touching their status — price is
        // static metadata, not concurrency-sensitive state, so a plain read
        // is fine here (unlike `status`, which only ever changes through
        // `transition_seat`'s atomic check-and-set).
        let concert = Concert::get_by_id(&self.db, concert_id).await?;
        let requested: HashSet<&String> = seat_number.iter().collect();
        let amount: i64 = concert
            .seats
            .iter()
            .filter(|s| requested.contains(&s.number))
            .map(|s| s.price)
            .sum();

        let mut tx = self.db.begin().await?;
        for seat in seat_number {
            let claimed = Concert::transition_seat(
                &mut tx,
                concert_id,
                seat,
                SeatStatus::Reserved,
                SeatStatus::Booked,
            )
            .await?;
            if !claimed {
                return Err(AppError::SeatUnavailable);
            }
        }
        tx.commit().await?;

        // Initiate payment
        let transaction_id = self.payment.charge(amount as u64).await?;
        let booking_id: Result<i64, sqlx::Error> = sqlx::query_scalar(
            "INSERT INTO ConcertBooking (concert_id, user_id, transaction_id, amount, refund, status, booking_at)
             VALUES (?, ?, ?, ?, 0, ?, ?) RETURNING id",
        )
        .bind(concert_id as i64)
        .bind(user_id as i64)
        .bind(transaction_id)
        .bind(amount)
        .bind(BookingStatus::Booked)
        .bind(Utc::now())
        .fetch_one(&self.db)
        .await;

        let booking_id = match booking_id {
            Ok(id) => id,
            Err(e) => {
                // Payment went through and the seats are marked Booked, but
                // the booking record itself failed to persist: refund, and
                // put the seats back to Reserved (not Available) — they
                // were legitimately held by the `reserve()` this `book()`
                // is confirming, not freed up.
                self.payment.refund(amount as u64, transaction_id).await?;
                let mut tx = self.db.begin().await?;
                for seat in seat_number {
                    Concert::transition_seat(
                        &mut tx,
                        concert_id,
                        seat,
                        SeatStatus::Booked,
                        SeatStatus::Reserved,
                    )
                    .await?;
                }
                tx.commit().await?;
                return Err(AppError::BookingError(e.to_string()));
            }
        };

        Ok(booking_id as u64)
    }

    async fn cancel(&mut self, booking_id: u64, user_id: u64) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
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
    dispatch_at: DateTime<Utc>,
}

// This send notification to user
#[derive(Debug)]
struct NotificationService {
    db: SqlitePool,
}

impl NotificationService {
    fn send(&mut self, booking_id: u64, user_id: u64) -> Result<(), AppError> {
        // Get the user for this user_id
        // Get the booking for this user
        // Save the notification to db and invoke the external service
        Ok(())
    }
}

// Not `async fn` — that would make the trait not dyn-compatible (an opaque
// per-call return type can't go in a vtable). Manually boxing the future
// instead keeps `Box<dyn Indexer>` / `Arc<dyn Indexer>` usable, same as
// `PaymentProcessor` already relies on for swappable payment backends —
// this trait is meant to support swappable indexer implementations the
// same way.
trait Indexer: Send + Sync + std::fmt::Debug {
    /// All concerts matching `search`. Empty is reported as `EventNotFound`
    /// rather than an empty `Vec` — the caller asked for a specific event
    /// (or set of them under a shared venue/date/artist) and got none.
    fn search(&self, search: SearchQuery) -> PinFutRef<'_, Result<Vec<Concert>, AppError>>;

    /// Add or refresh one concert's entry, without rebuilding anything else.
    fn upsert(&mut self, concert: Concert);

    /// Full rebuild from the DB — startup / cold resync. Everything else
    /// (a write to `concerts`) should call `upsert` instead of this.
    fn build<'a>(&'a mut self, db: &'a SqlitePool) -> PinFutRef<'a, Result<(), AppError>>;
}

#[derive(Debug, Default)]
struct SearchEngine {
    /// Primary store: full concerts keyed by id.
    concerts: HashMap<u64, Concert>,
    /// Secondary index: every valid partial search key -> matching ids.
    /// `venue` is mandatory; `date` (day granularity, not the exact
    /// `event_date` timestamp) and `artist` are independently optional, so
    /// one concert lands under all 4 combinations of "with/without date"
    /// x "with/without artist".
    by_key: HashMap<SearchQuery, Vec<u64>>,
}

impl Indexer for SearchEngine {
    fn search(&self, search: SearchQuery) -> PinFutRef<'_, Result<Vec<Concert>, AppError>> {
        Box::pin(async move {
            let concerts: Vec<Concert> = self
                .by_key
                .get(&search)
                .into_iter()
                .flatten()
                .filter_map(|id| self.concerts.get(id).cloned())
                .collect();
            if concerts.is_empty() {
                return Err(AppError::EventNotFound(search.to_string()));
            }
            Ok(concerts)
        })
    }

    fn upsert(&mut self, concert: Concert) {
        let id = concert.id;
        let venue = concert.venue.name.clone();
        let date = concert.event_date.date_naive();
        let artist = concert.artist.clone();

        // Only index a concert's keys the first time it's seen -- a
        // re-upsert of an already-known id must not push a duplicate id
        // into by_key, same reasoning as FlightIndex::upsert's dedup guard
        // in flight-booking.rs.
        let is_new = self.concerts.insert(id, concert).is_none();
        if is_new {
            // There will 2^n possible combination of key
            for key in [
                SearchQuery {
                    venue: venue.clone(),
                    date: None,
                    artist: None,
                },
                SearchQuery {
                    venue: venue.clone(),
                    date: Some(date),
                    artist: None,
                },
                SearchQuery {
                    venue: venue.clone(),
                    date: None,
                    artist: Some(artist.clone()),
                },
                SearchQuery {
                    venue,
                    date: Some(date),
                    artist: Some(artist),
                },
            ] {
                self.by_key.entry(key).or_default().push(id);
            }
        }
    }

    fn build<'a>(&'a mut self, db: &'a SqlitePool) -> PinFutRef<'a, Result<(), AppError>> {
        Box::pin(async move {
            let concerts: Vec<Concert> = sqlx::query_as("SELECT * FROM concerts")
                .fetch_all(db)
                .await?;
            for concert in concerts {
                self.upsert(concert);
            }
            Ok(())
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("Unable to create user {0}")]
    UserCreate(String),
    #[error("Seats selected is unavailable")]
    SeatUnavailable,
    #[error("Unable to fetch record {0}")]
    DbQueryError(String),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error("Unable to reserve seats {0}")]
    BookingError(String),
    #[error("Event not found for key {0}")]
    EventNotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::UserCreate(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            AppError::DbQueryError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            AppError::SeatUnavailable => (StatusCode::NOT_FOUND, "Not found").into_response(),
            AppError::SqlError(_e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Sqlx error").into_response()
            }
            AppError::BookingError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            AppError::EventNotFound(e) => (StatusCode::NOT_FOUND, e).into_response(),
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

// `venue` is mandatory; `date` and `artist` are independently optional —
// see `SearchEngine::upsert`, which indexes each concert under all 4
// combinations this allows.
#[derive(Debug, Deserialize, Hash, PartialEq, Eq)]
struct SearchQuery {
    venue: String,
    date: Option<NaiveDate>,
    artist: Option<String>,
}

impl std::fmt::Display for SearchQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{} {:?} {:?}", self.venue, self.date, self.artist)
    }
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

#[derive(Debug, Deserialize)]
struct ConcertReserve {
    concert_id: u64,
    seat_numbers: Vec<String>,
}

async fn reserve_seats(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConcertReserve>,
) -> Result<Json<()>, AppError> {
    let mut processor = BookingProcessor::new(state.db.clone(), state.payment.clone());
    processor
        .reserve(payload.concert_id, &payload.seat_numbers)
        .await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct ConcertBook {
    concert_id: u64,
    seat_numbers: Vec<String>,
    user_id: u64,
}

async fn book(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConcertBook>,
) -> Result<Json<u64>, AppError> {
    let mut processor = BookingProcessor::new(state.db.clone(), state.payment.clone());
    let booking_id = processor
        .book(payload.concert_id, &payload.seat_numbers, payload.user_id)
        .await?;
    Ok(Json(booking_id))
}

// Not `Clone` — `RwLock` never is (there's nothing sensible to clone), and
// it doesn't need to be: axum's state is always this struct wrapped in
// `Arc<AppState>`, and `Arc<T>: Clone` regardless of `T`.
#[derive(Debug)]
struct AppState {
    db: SqlitePool,
    payment: Arc<dyn PaymentProcessor>,
    indexed: RwLock<Arc<dyn Indexer>>
}

async fn app() -> Router {
    let _ = dotenvy::dotenv();

    // Own db file per bin — see the same comment in flight-booking.rs.
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://concert-ticket-booking.db?mode=rwc".to_string());
    let db = SqlitePool::connect(&database_url)
        .await
        .expect("Unable to connect to sqlite db");
    sqlx::migrate!("./migrations/concert-ticket-booking")
        .run(&db)
        .await
        .expect("Unable to run migration");

    // Cold-start build: index whatever's already in `concerts` before
    // serving any traffic. From here on, writes to `concerts` should call
    // `Indexer::upsert` instead of rebuilding the whole thing again.
    let mut search_engine = SearchEngine::default();
    search_engine
        .build(&db)
        .await
        .expect("Unable to build search index");

    build_router(db, Arc::new(search_engine))
}

// Split out from `app()` so tests can build the real router (real routing,
// real extractors, real handlers) against a throwaway test DB, instead of
// either duplicating the router or only ever testing `BookingProcessor`
// directly and never the HTTP layer above it. The indexer is injected
// (rather than built inside here) so a test can hand in an empty one
// without needing a populated `concerts` table.
fn build_router(db: SqlitePool, indexer: Arc<dyn Indexer>) -> Router {
    let payment = Arc::new(MockPayment {});
    let indexed: RwLock<Arc<dyn Indexer>> = RwLock::new(indexer);
    let state = Arc::new(AppState { db, indexed, payment });
    Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status": "OK"})) }),
        )
        .merge(Router::new().route("/users", post(create_user)))
        .merge(
            Router::new()
                .route("/concert/search", get(search))
                .route("/concert/reserve", post(reserve_seats))
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
        .with_state(state)
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
    use super::{
        BookingProcessor, Concert, ContactNumber, Email, Indexer, Location, MockPayment,
        SearchEngine, SearchQuery, SeatStatus, User,
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::{Row, SqlitePool};
    use std::sync::Arc;

    async fn test_pool(name: &str) -> (SqlitePool, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("concert_ticket_test_{}_{}.db", name, nanos));
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./migrations/concert-ticket-booking")
            .run(&pool)
            .await
            .unwrap();
        (pool, path)
    }

    #[tokio::test]
    async fn create_persists_a_row() {
        let (pool, path) = test_pool("create_user").await;

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

    #[tokio::test]
    async fn reserve_then_book_charges_the_seat_total() {
        let (pool, path) = test_pool("reserve_book").await;

        let seats = serde_json::json!([
            {"kind": "PREMIUM", "price": 100, "number": "A1", "status": "AVAILABLE"},
            {"kind": "REGULAR", "price": 50, "number": "A2", "status": "AVAILABLE"},
        ])
        .to_string();
        let venue = serde_json::json!({
            "name": "Test Venue", "address": "1 Main St", "lat": 0.0, "long": 0.0, "zip_code": "00000"
        })
        .to_string();
        sqlx::query(
            "INSERT INTO concerts (id, venue, artist, event_date, seats) VALUES (1, ?, 'Test Artist', ?, ?)",
        )
        .bind(&venue)
        .bind(super::Utc::now())
        .bind(&seats)
        .execute(&pool)
        .await
        .unwrap();

        let mut processor = BookingProcessor::new(pool.clone(), Arc::new(MockPayment {}));

        let seat_numbers = vec!["A1".to_string()];
        processor.reserve(1, &seat_numbers).await.unwrap();

        // Same seat, already Reserved by someone else's flow: must fail.
        assert!(processor.reserve(1, &["A1".to_string()]).await.is_err());

        let booking_id = processor.book(1, &seat_numbers, 42).await.unwrap();
        assert!(booking_id > 0);

        let (amount, status): (i64, String) =
            sqlx::query("SELECT amount, status FROM ConcertBooking WHERE id = ?")
                .bind(booking_id as i64)
                .fetch_one(&pool)
                .await
                .map(|row| (row.get(0), row.get(1)))
                .unwrap();
        assert_eq!(amount, 100);
        assert_eq!(status, "BOOKED");

        let seats = Concert::available_seats(&pool, 1).await.unwrap();
        let a1 = seats.iter().find(|s| s.number == "A1").unwrap();
        assert_eq!(a1.status, SeatStatus::Booked);
        let a2 = seats.iter().find(|s| s.number == "A2").unwrap();
        assert_eq!(a2.status, SeatStatus::Available);

        let _ = std::fs::remove_file(&path);
    }

    // The above test proves the booking *logic* is correct by calling
    // `BookingProcessor` directly. This one instead proves the HTTP layer on
    // top of it is wired correctly — routing, JSON (de)serialization, state
    // extraction — by driving the real `Router` from `build_router` (same
    // one `app()` serves) over `tower::ServiceExt::oneshot`, with no real
    // socket involved. It intentionally doesn't re-assert the pricing/seat
    // logic already covered above.
    #[tokio::test]
    async fn reserve_and_book_over_http() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let (pool, path) = test_pool("e2e_book").await;
        let concert_id = Concert::seed_dummy(&pool).await.unwrap();
        let router = super::build_router(
            pool.clone(),
            Arc::new(super::SearchEngine::default()),
        );

        let post_json = |uri: &'static str, body: serde_json::Value| {
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };

        let response = router
            .clone()
            .oneshot(post_json(
                "/concert/reserve",
                serde_json::json!({"concert_id": concert_id, "seat_numbers": ["A1"]}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(post_json(
                "/concert/book",
                serde_json::json!({
                    "concert_id": concert_id,
                    "seat_numbers": ["A1"],
                    "user_id": 42,
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let booking_id: u64 = serde_json::from_slice(&body).unwrap();

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM ConcertBooking WHERE id = ?")
                .bind(booking_id as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "BOOKED");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn upsert_indexes_every_facet_and_dedups_on_reupsert() {
        let mut engine = SearchEngine::default();
        let venue = Location {
            name: "Test Venue".to_string(),
            address: "1 Main St".to_string(),
            lat: 0.0,
            long: 0.0,
            zip_code: "00000".to_string(),
        };
        let date = super::Utc::now();

        let concert_a = Concert {
            id: 1,
            venue: sqlx::types::Json(venue.clone()),
            artist: "Artist A".to_string(),
            event_date: date,
            seats: vec![],
        };
        let concert_b = Concert {
            id: 2,
            venue: sqlx::types::Json(venue),
            artist: "Artist B".to_string(),
            event_date: date,
            seats: vec![],
        };

        engine.upsert(concert_a);
        engine.upsert(concert_b.clone());
        // Same concert again: must not duplicate its id in venue-only.
        engine.upsert(concert_b);

        let venue_only = SearchQuery {
            venue: "Test Venue".to_string(),
            date: None,
            artist: None,
        };
        let mut ids = engine.by_key.get(&venue_only).unwrap().clone();
        ids.sort();
        assert_eq!(ids, vec![1, 2], "venue-only key should match both, once each");

        let by_artist = SearchQuery {
            venue: "Test Venue".to_string(),
            date: None,
            artist: Some("Artist A".to_string()),
        };
        assert_eq!(
            engine.by_key.get(&by_artist).unwrap(),
            &vec![1],
            "venue+artist key should only match the concert with that artist"
        );
    }
}
