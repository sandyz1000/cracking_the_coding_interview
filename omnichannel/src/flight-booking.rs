// # Flight Booking and Management System
// - It should allow user to search for flight by source, destination and travel datetime
// - It should allow user to researve a seat
// -

use axum::{
    Json, Router,
    extract::{FromRef, FromRequestParts, Path, Query, State},
    http::{HeaderValue, Method, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{collections::HashMap, fmt::Debug, net::SocketAddr};
use tower_http::cors::CorsLayer;

use sqlx::{self, Pool, Sqlite, SqlitePool};
use tokio::signal;

mod records {
    use crate::{Aircraft, AirportCode, Flight, FlightSpec, FlightStatus};
    use chrono::{DateTime, Utc};

    // Return test/seed data to exercise handlers before DB wiring is done.
    pub fn aircrafts() -> Vec<Aircraft> {
        vec![
            Aircraft {
                id: 1,
                tail_no: "VT-ABC".into(),
                total_seats: 180,
                model_no: "Boeing 737-800".into(),
            },
            Aircraft {
                id: 2,
                tail_no: "VT-XYZ".into(),
                total_seats: 240,
                model_no: "Airbus A320neo".into(),
            },
        ]
    }

    pub fn flights() -> Vec<Flight> {
        fn seat(number: &str, seat_type: crate::SeatType) -> (String, crate::SeatRecord) {
            (
                number.to_string(),
                crate::SeatRecord {
                    number: number.to_string(),
                    seat_type,
                    status: crate::SeatStatus::Available,
                    holder: None,
                },
            )
        }

        vec![
            Flight {
                id: 1,
                aircraft_id: 1,
                flight_no: "AI-101".into(),
                spec: FlightSpec::new(
                    &"AI-101".into(),
                    AirportCode {
                        name: "Indira Gandhi Intl".into(),
                        code: "DEL".into(),
                    },
                    AirportCode {
                        name: "Chhatrapati Shivaji Intl".into(),
                        code: "BOM".into(),
                    },
                    8500,
                    DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap(),
                    DateTime::<Utc>::from_timestamp(1700003600, 0).unwrap(),
                ),
                status: FlightStatus::Scheduled,
                seats: [
                    seat("1A", crate::SeatType::Business),
                    seat("1B", crate::SeatType::Business),
                    seat("10A", crate::SeatType::Economy),
                    seat("10B", crate::SeatType::Economy),
                ]
                .into(),
                crew: vec![],
            },
            Flight {
                id: 2,
                aircraft_id: 2,
                flight_no: "6E-202".into(),
                spec: FlightSpec::new(
                    &"6E-202".into(),
                    AirportCode {
                        name: "Chhatrapati Shivaji Intl".into(),
                        code: "BOM".into(),
                    },
                    AirportCode {
                        name: "Kempegowda Intl".into(),
                        code: "BLR".into(),
                    },
                    3200,
                    DateTime::<Utc>::from_timestamp(1700010000, 0).unwrap(),
                    DateTime::<Utc>::from_timestamp(1700012800, 0).unwrap(),
                ),
                status: FlightStatus::Scheduled,
                seats: [
                    seat("1A", crate::SeatType::Business),
                    seat("1B", crate::SeatType::Business),
                    seat("10A", crate::SeatType::Economy),
                    seat("10B", crate::SeatType::Economy),
                ]
                .into(),
                crew: vec![],
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct AppState {
    db: Pool<Sqlite>,
    indexer: FlightIndex,
    booking_processor: BookingProcessor,
    payment_processor: std::sync::Arc<dyn PaymentProcessor>,
}

// This will be added to the database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AirportCode {
    name: String,
    code: String,
}

const CABIN_COLUMNS: [char; 6] = ['A', 'B', 'C', 'D', 'E', 'F'];

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum SeatType {
    Economy,
    Business,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum SeatStatus {
    Reserved,
    Booked,
    Available,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeatRecord {
    pub number: String,
    pub seat_type: SeatType,
    pub status: SeatStatus,
    /// Passenger id holding this seat (Reserved or Booked).
    pub holder: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
enum FlightStatus {
    Scheduled,
    Cancelled,
    Departed,
    Delayed,
    Landed,
}

enum AircraftStatus {
    Serviced,
    Functioning,
}

#[derive(Debug, Clone, Serialize)]
struct Aircraft {
    id: u32,
    tail_no: String,
    total_seats: usize,
    // This should either Boeign, Airbus etc
    model_no: String,
}

impl Aircraft {
    // This returns the default layout with 1 two row as Business class
    fn default_seats(&self) -> Vec<(String, SeatType)> {
        let mut out = Vec::with_capacity(self.total_seats);
        for row in 1..=self.total_seats / 6 {
            let seat = if row < 3 {
                SeatType::Business
            } else {
                SeatType::Economy
            };
            for col in CABIN_COLUMNS {
                out.push((format!("{}{}", row, col), seat));
            }
        }
        out
    }

    // Invoke a sql ops to find the aircraft details by Id
    fn get_by_id(id: u32) -> anyhow::Result<Aircraft> {
        todo!()
    }
}

#[derive(Debug, Clone)]
struct BookingProcessor {
    payment_processor: Arc<dyn PaymentProcessor>,
}

impl BookingProcessor {
    fn new(payment_processor: Arc<dyn PaymentProcessor>) -> Self {
        Self { payment_processor }
    }

    // TODO: replace with a DB sequence / AUTOINCREMENT when persisting bookings.
    fn next_id() -> u32 {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    fn get_by_id(booking_id: u32) -> anyhow::Result<Booking> {
        todo!()
    }

    fn book(
        &mut self,
        user: &User,
        flight_number: &str,
        seat_number: &String,
        _method: PaymentMethod,
    ) -> anyhow::Result<Booking> {
        let mut flight = Flight::get_by_number(flight_number)?;

        // Reserve the seat: returns (seat_type, fare) or errors if taken/invalid.
        let (seat_type, amount) = flight.reserve(seat_number, user.id as u64)?;

        // Charge the passenger; payment processor returns the txn id.
        let booking_id = Self::next_id();
        let txn_id = self.payment_processor.charge(booking_id, amount as u32)?;

        // Mark the seat confirmed, then persist the booking.
        flight.confirm(seat_number)?;

        Ok(Booking {
            id: booking_id,
            txn_id,
            flight_no: flight_number.to_string(),
            passenger_id: user.id,
            seat_no: seat_number.clone(),
            seat_type,
            amount,
            refund: 0,
            status: BookingStatus::Reserved,
            baggage: None,
            booking_date: Utc::now(),
        })
    }

    fn cancel(&mut self, booking_id: u32, user: &User) -> anyhow::Result<CancelBooking> {
        // Booking must exist and belong to the calling user.
        let booking = Self::get_by_id(booking_id)?;
        if booking.passenger_id != user.id {
            return Err(anyhow::anyhow!("Cancelling user is not the booking user"));
        }
        // Refund the payment, then mark the booking cancelled.
        self.payment_processor
            .refund(booking.txn_id, booking.amount as u32)?;
        // TODO: update the booking table status to Cancelled.
        Ok(CancelBooking {
            booking_id,
            txn_id: booking.txn_id,
            refunded_amount: booking.amount,
        })
    }

    fn change_flight(&mut self, flight_no: &String, user: &User) -> anyhow::Result<ChangeResult> {
        todo!()
    }

    fn set_baggage(
        &mut self,
        booking_id: u32,
        baggage: Option<Baggage>,
        user: &User,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
#[derive(Debug)]
enum BookingStatus {
    Reserved,
    Confirmed,
    Cancelled,
    Completed,
}

#[derive(Debug)]
struct Booking {
    id: u32,
    txn_id: u32,
    flight_no: String,
    passenger_id: u32,
    seat_no: String,
    seat_type: SeatType,
    amount: u64,
    refund: u64,
    status: BookingStatus,
    baggage: Option<Baggage>,
    booking_date: DateTime<Utc>,
}

#[derive(Debug)]
struct CancelBooking {
    booking_id: u32,
    txn_id: u32,
    refunded_amount: u64,
}

struct ChangeResult {
    booking_id: u32,
}

#[derive(Debug)]
struct Baggage {
    pub pieces: u32,
    pub total_weight_kg: f64,
    pub checked: bool,
}

enum PaymentMethod {
    CreditCard,
    ApplePay,
    GPay,
    DebitCard,
    Upi,
}

trait PaymentProcessor: std::fmt::Debug + Send + Sync {
    // This return a transaction id
    fn charge(&self, booking_id: u32, amount: u32) -> anyhow::Result<u32>;
    fn refund(&self, txn_id: u32, amount: u32) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
struct MockPaymentProcessor {}

#[derive(Debug, Clone)]
struct Payment {
    txn_id: u32,
    booking_id: u32,
    // Store as integer with precesion as pence
    amount: u32,
    refund: u32,
}

impl PaymentProcessor for MockPaymentProcessor {
    fn charge(&self, booking_id: u32, amount: u32) -> anyhow::Result<u32> {
        todo!()
    }

    fn refund(&self, txn_id: u32, amount: u32) -> anyhow::Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug, Serialize)]
enum UserKind {
    Crew,
    Admin,
    Staff,
    Passenger,
}

#[derive(Clone, Debug, Serialize)]
struct User {
    id: u32,
    kind: UserKind,
    name: String,
    email: String,
    contact: String,
}
trait GetById {
    type Output;

    fn get_by_id(entry_id: u32) -> Self::Output;
}

#[derive(Debug, Clone, Serialize)]
struct FlightSpec {
    flight_no: String,
    source: AirportCode,
    destination: AirportCode,
    fare: u64,
    departure_time: DateTime<Utc>,
    arrival_time: DateTime<Utc>,
}

impl FlightSpec {
    fn new(
        flight_no: &String,
        source: AirportCode,
        destination: AirportCode,
        fare: u64,
        departure_time: DateTime<Utc>,
        arrival_time: DateTime<Utc>,
    ) -> Self {
        Self {
            flight_no: flight_no.clone(),
            source,
            destination,
            fare,
            departure_time,
            arrival_time,
        }
    }

    fn get_by_number(flight_no: &String) -> FlightSpec {
        todo!()
    }
}

#[derive(Debug, Clone, Serialize)]
struct Flight {
    id: u32,
    aircraft_id: u32,
    flight_no: String,
    spec: FlightSpec,
    status: FlightStatus,
    // Here key is the seat no
    seats: HashMap<String, SeatRecord>,
    // This should be User of type crew
    crew: Vec<User>,
}

// This is the flight snapshot
#[derive(Debug)]
struct FlightSnapshot {
    flight_no: String,
    fare: u64,
    departure_time: DateTime<Utc>,
    arrival_time: DateTime<Utc>,
    source: AirportCode,
    destination: AirportCode,
    available_seat: usize,
}

impl GetById for Flight {
    type Output = Self;

    fn get_by_id(id: u32) -> Self::Output {
        todo!()
    }
}

impl Flight {
    // Auto generate id for flight
    fn gen_id() -> u32 {
        0
    }

    // This create a new flight entry in the database
    fn new(aircraft_id: u32, flight_no: String, spec: FlightSpec) -> Self {
        let aircraft = Aircraft::get_by_id(aircraft_id).unwrap();
        let seats = aircraft
            .default_seats()
            .into_iter()
            .map(|(number, seat_type)| {
                (
                    number.clone(),
                    SeatRecord {
                        number,
                        seat_type,
                        status: SeatStatus::Available,
                        holder: None,
                    },
                )
            })
            .collect();
        Self {
            id: Self::gen_id(),
            aircraft_id,
            spec,
            flight_no,
            status: FlightStatus::Scheduled,
            seats,
            crew: vec![],
        }
    }

    fn get_by_number(flight_no: &str) -> anyhow::Result<Flight> {
        todo!()
    }

    fn add_crew(&mut self, member: User) -> anyhow::Result<()> {
        // TODO: Update the entry in the flight table
        self.crew.push(member);
        Ok(())
    }

    fn available_seat(&self) -> anyhow::Result<usize> {
        let remaining = self
            .seats
            .values()
            .filter(|s| s.status == SeatStatus::Available)
            .count();
        Ok(remaining)
    }

    // This reserve the seat and return the seatType and price
    fn reserve(&mut self, seat_no: &String, user_id: u64) -> anyhow::Result<(SeatType, u64)> {
        let record = self
            .seats
            .get_mut(seat_no)
            .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
        if record.status != SeatStatus::Available {
            return Err(anyhow::anyhow!("Seat {} not available", seat_no));
        }
        let seat_type = record.seat_type;
        let fare = self.spec.fare; // TODO: price may vary by seat_type
        record.holder = Some(user_id);
        record.status = SeatStatus::Reserved;
        Ok((seat_type, fare))
    }

    fn confirm(&mut self, seat_no: &String) -> anyhow::Result<()> {
        let record = self
            .seats
            .get_mut(seat_no)
            .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
        if record.status != SeatStatus::Reserved {
            return Err(anyhow::anyhow!("Seat {} not reserved", seat_no));
        }
        record.status = SeatStatus::Booked;
        Ok(())
    }

    fn release(&mut self, seat_no: &String) -> anyhow::Result<()> {
        let record = self
            .seats
            .get_mut(seat_no)
            .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
        record.status = SeatStatus::Available;
        record.holder = None;
        Ok(())
    }

    fn snapshot(&mut self) -> FlightSnapshot {
        todo!()
    }
}

#[derive(Debug, Clone)]
struct FlightIndex {
    /// Primary store: full flights keyed by id. Mirror of the DB for hot-path reads.
    flights: HashMap<u32, Flight>,
    /// Secondary search index: (source_code, destination_code) -> flight ids
    by_route: HashMap<(String, String), Vec<u32>>,
    /// Secondary search index: departure date -> flight ids
    by_date: HashMap<NaiveDate, Vec<u32>>,
}

impl FlightIndex {
    fn new() -> Self {
        Self {
            flights: HashMap::new(),
            by_route: HashMap::new(),
            by_date: HashMap::new(),
        }
    }

    /// Insert (or replace) a flight and keep every secondary index in sync.
    /// Called on startup (build) and after DB writes (write-through).
    fn upsert(&mut self, flight: Flight) {
        let id = flight.id;
        let route = (
            flight.spec.source.code.clone(),
            flight.spec.destination.code.clone(),
        );
        let date = flight.spec.departure_time.date_naive();
        self.by_route.entry(route).or_default().push(id);
        self.by_date.entry(date).or_default().push(id);
        self.flights.insert(id, flight);
    }

    fn get(&self, id: u32) -> Option<&Flight> {
        self.flights.get(&id)
    }
}

trait Indexer {
    /// Hot-path search: return flights matching route on a date, served from memory.
    /// Intersect `by_route` and `by_date`, then deref each id via `flights`.
    fn search(&self, source: &str, destination: &str, date: NaiveDate) -> Vec<&Flight>;
    /// Rebuild the whole index from the DB (startup / resync).
    fn build(&mut self);
}

impl Indexer for FlightIndex {
    fn search(&self, source: &str, destination: &str, date: NaiveDate) -> Vec<&Flight> {
        // Intersect the two id lists. Iterate the smaller one, probing the larger
        // via a HashSet of ids — avoids cloning the full flights.
        let route_ids = self.by_route.get(&(source.to_string(), destination.to_string()));
        let date_ids = self.by_date.get(&date);

        let (small, large) = match (route_ids, date_ids) {
            (Some(r), Some(d)) if r.len() <= d.len() => (r, d),
            (Some(r), Some(d)) => (d, r),
            _ => return vec![],
        };

        let large_set: HashSet<&u32> = large.iter().collect();
        small
            .iter()
            .filter(|id| large_set.contains(id))
            .filter_map(|id| self.flights.get(id))
            .collect()
    }

    fn build(&mut self) {
        // TODO: SELECT all flights (+ seats) from DB, then self.upsert() each.
        todo!()
    }
}

// Api types
#[derive(Debug, Deserialize)]
struct AddFlight {
    aircraft_id: u32,
    source: AirportCode,
    destination: AirportCode,
    flight_no: String,
    // Iso string date format
    departure_time: String,
    // Iso string date format
    arrival_time: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFlight {}

#[derive(Debug, Deserialize)]
struct AddAircraft {}

#[derive(Debug, Deserialize)]
struct UpdateAircraft {}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Pagination {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    source: String,
    destination: String,
    // ISO formatted dates
    departure_dates: String,
}

#[derive(Debug, Deserialize)]
struct BookFlight {
    // TODO
}

// ----- Aircraft Handler
// #[axum::debug_handler]
async fn aircrafts_index(State(state): State<AppState>) -> Result<Json<Vec<Aircraft>>, AppError> {
    Ok(Json(records::aircrafts()))
}

async fn aircraft_add(
    State(state): State<AppState>,
    Json(payload): Json<AddAircraft>,
) -> Result<Json<Aircraft>, AppError> {
    Err(AppError(anyhow::anyhow!("Not implemented!")))
}

async fn aircraft_patch(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateAircraft>,
) -> Result<(), AppError> {
    Err(AppError(anyhow::anyhow!("Not implemented!")))
}

// ----- Flight handler
async fn add_flight(
    State(state): State<AppState>,
    Json(payload): Json<AddFlight>,
) -> Result<Json<Flight>, AppError> {
    Err(AppError(anyhow::anyhow!("Not implemented!")))
}

async fn flights_index(State(state): State<AppState>) -> Result<Json<Vec<Flight>>, AppError> {
    // DB is source of truth; indexer mirrors it for hot-path reads.
    Ok(Json(state.indexer.flights.values().cloned().collect()))
}

async fn flight_patch(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateFlight>,
) -> Result<(), AppError> {
    Err(AppError(anyhow::anyhow!("Not implemented!")))
}

// ---- Search Api

async fn find_flights(
    State(state): State<AppState>,
    Query(SearchQuery {
        source,
        destination,
        departure_dates,
    }): Query<SearchQuery>,
) -> Result<Json<Vec<Flight>>, AppError> {
    // Expect YYYY-MM-DD (or full ISO). Closure at the serialization boundary only.
    let date = NaiveDate::parse_from_str(&departure_dates, "%Y-%m-%d")
        .or_else(|_| {
            // parse_from_rfc3339 is on DateTime<FixedOffset>; Z timezone is
            // lexed as +00:00, so a plain date fails here -> fall back gracefully.
            chrono::DateTime::parse_from_rfc3339(&departure_dates).map(|dt| dt.date_naive())
        })
        .map_err(|e| AppError(anyhow::anyhow!("invalid date: {}", e)))?;

    let found = state
        .indexer
        .search(&source, &destination, date)
        .into_iter()
        .cloned()
        .collect();
    Ok(Json(found))
}

async fn book_flight(
    State(state): State<AppState>,
    Json(payload): Json<BookFlight>,
) -> Result<Json<Flight>, AppError> {
    Err(AppError(anyhow::anyhow!("Not implemented!")))
}

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
#[allow(dead_code)]
struct DatabaseConnection(sqlx::pool::PoolConnection<sqlx::Sqlite>);

impl<S> FromRequestParts<S> for DatabaseConnection
where
    SqlitePool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);

        let conn = pool.acquire().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}

/// Utilty function for mapping any error into a `500 Internal Server Error` response.
fn internal_error<E: std::error::Error>(err: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

async fn app() -> anyhow::Result<Router> {
    //
    // Load .env if it's available, ignore if not.
    let _ = dotenvy::dotenv();

    let db = SqlitePool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
        .await?;
    sqlx::migrate!().run(&db).await?;

    // Seed the in-memory index directly (avoids circular db call).
    let mut indexer = FlightIndex::new();
    for flight in records::flights() {
        indexer.upsert(flight);
    }

    let payment_processor = std::sync::Arc::new(MockPaymentProcessor {});
    let booking_processor = BookingProcessor::new(payment_processor.clone());
    let state = AppState {
        db,
        indexer,
        booking_processor,
        payment_processor,
    };

    let router = Router::new()
        .merge(
            Router::new()
                .route("/aircrafts", get(aircrafts_index).post(aircraft_add))
                // This is need to update the aircraft essentially required for maintainence
                .route("/aircraft/{id}", patch(aircraft_patch)),
        )
        .merge(
            Router::new()
                .route("/flights", post(add_flight).get(flights_index))
                .route("/flight/{id}", patch(flight_patch)),
        )
        .merge(Router::new().route("/search", get(find_flights)))
        .merge(Router::new().route("/reserve", post(book_flight)))
        .layer(
            CorsLayer::new()
                .allow_origin("http://127.0.0.1:3000".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::PATCH]),
        )
        .with_state(state);

    Ok(router)
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
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
async fn main() -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on port: {:?}", 3000);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let router = app().await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::records;
    use super::{FlightIndex, Indexer, NaiveDate};

    #[test]
    fn seed_two_flights() {
        assert_eq!(records::aircrafts().len(), 2);
        assert_eq!(records::flights().len(), 2);
    }

    #[test]
    fn seed_flights_have_route() {
        let f = records::flights();
        assert_eq!(f[0].spec.source.code, "DEL");
        assert_eq!(f[0].spec.destination.code, "BOM");
        assert_eq!(f[1].spec.source.code, "BOM");
        assert_eq!(f[1].spec.destination.code, "BLR");
    }

    #[test]
    fn search_matches_route_and_date_from_index() {
        let mut idx = super::FlightIndex::new();
        for f in records::flights() {
            idx.upsert(f);
        }

        let date = records::flights()[0].spec.departure_time.date_naive();

        let hits = idx.search("DEL", "BOM", date);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].flight_no, "AI-101");

        // Wrong route -> no hits
        assert!(idx.search("DEL", "BLR", date).is_empty());
        // Wrong date -> no hits
        assert!(idx.search("DEL", "BOM", NaiveDate::from_ymd_opt(1999, 1, 1).unwrap()).is_empty());
    }
}
