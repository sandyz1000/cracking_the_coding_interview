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
use std::{collections::HashMap, fmt::Debug, net::SocketAddr};
use std::{collections::HashSet, sync::Arc};
use std::{
    pin::Pin,
    sync::atomic::{AtomicU32, Ordering},
};
use tower_http::cors::CorsLayer;

use sqlx::{self, Pool, QueryBuilder, Row, Sqlite, SqlitePool, prelude::FromRow};
use tokio::{signal, sync::RwLock};

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
                source: AirportCode {
                    name: "Indira Gandhi Intl".into(),
                    code: "DEL".into(),
                },
                destination: AirportCode {
                    name: "Chhatrapati Shivaji Intl".into(),
                    code: "BOM".into(),
                },
                fare: 8500,
                departure_time: DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap(),
                arrival_time: DateTime::<Utc>::from_timestamp(1700003600, 0).unwrap(),
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
                source: AirportCode {
                    name: "Chhatrapati Shivaji Intl".into(),
                    code: "BOM".into(),
                },
                destination: AirportCode {
                    name: "Kempegowda Intl".into(),
                    code: "BLR".into(),
                },
                fare: 3200,
                departure_time: DateTime::<Utc>::from_timestamp(1700010000, 0).unwrap(),
                arrival_time: DateTime::<Utc>::from_timestamp(1700012800, 0).unwrap(),
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
    indexer: Arc<RwLock<FlightIndex>>,
    booking_processor: BookingProcessor,
    flight_repo: FlightRepository,
}

// This will be added to the database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AirportCode {
    name: String,
    code: String,
}

const CABIN_COLUMNS: [char; 6] = ['A', 'B', 'C', 'D', 'E', 'F'];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum SeatType {
    Economy,
    Business,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum SeatStatus {
    Reserved,
    Booked,
    Available,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatRecord {
    pub number: String,
    pub seat_type: SeatType,
    pub status: SeatStatus,
    /// Passenger id holding this seat (Reserved or Booked).
    pub holder: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum FlightStatus {
    Scheduled,
    Cancelled,
    Departed,
    Delayed,
    Landed,
}

pub enum AircraftStatus {
    Serviced,
    Functioning,
}

#[derive(Debug, Clone, Serialize, FromRow)]
struct Aircraft {
    id: u32,
    tail_no: String,
    total_seats: i64,
    // This should either Boeign, Airbus etc
    model_no: String,
}

impl GetById for Aircraft {
    type Output = Self;

    fn get_by_id(pool: &SqlitePool, entry_id: u32) -> PinFuture<anyhow::Result<Self::Output>> {
        let pool = pool.clone();
        Box::pin(async move { Aircraft::get_by_id(&pool, entry_id).await })
    }
}
impl Aircraft {
    fn get_next_id() -> u64 {
        0
    }
    // This returns the default layout with 1 two row as Business class
    fn default_seats(&self) -> Vec<(String, SeatType)> {
        let mut out = Vec::with_capacity(self.total_seats as usize);
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
    async fn get_by_id(pool: &SqlitePool, id: u32) -> anyhow::Result<Aircraft> {
        sqlx::query_as::<_, Aircraft>("SELECT * FROM Aircraft WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Aircraft {} not found: {}", id, e))
    }
}

#[derive(Debug, Clone)]
struct BookingProcessor {
    pool: SqlitePool,
    payment_processor: Arc<dyn PaymentProcessor>,
    indexer: Arc<RwLock<FlightIndex>>,
}

impl BookingProcessor {
    fn new(
        pool: SqlitePool,
        payment_processor: Arc<dyn PaymentProcessor>,
        indexer: Arc<RwLock<FlightIndex>>,
    ) -> Self {
        Self {
            pool,
            payment_processor,
            indexer,
        }
    }

    // TODO: replace with a DB sequence / AUTOINCREMENT when persisting bookings.
    fn next_id() -> u32 {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    async fn get_by_id(pool: &SqlitePool, booking_id: u32) -> anyhow::Result<Booking> {
        sqlx::query_as::<_, Booking>("SELECT * FROM Booking WHERE id = ?")
            .bind(booking_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Booking {} not found", booking_id))
    }

    async fn book(
        &mut self,
        user: &User,
        flight_number: &str,
        seat_number: &[String],
        _method: PaymentMethod,
    ) -> anyhow::Result<Booking> {
        let mut flight = Flight::get_by_number(&self.pool, flight_number).await?;

        // Atomically claims the seats in the DB (rejecting if any are no
        // longer Available) and mirrors the result into `flight` in memory.
        let reserved = flight
            .reserve(&self.pool, seat_number, user.id as u64)
            .await?;

        // Charge the passenger; payment processor returns the txn id.
        let booking_id = Self::next_id();
        let txn_id = match self
            .payment_processor
            .charge(booking_id, reserved.total_amount as u32)
            .await
        {
            Ok(txn_id) => txn_id,
            Err(e) => {
                // Charge failed: release the seats we just reserved instead
                // of leaving them stuck as Reserved with no booking.
                flight.release(&self.pool, seat_number).await?;
                self.indexer.write().await.upsert(flight);
                return Err(e);
            }
        };

        // Mark the seats confirmed, then persist the booking.
        flight.confirm(&self.pool, seat_number).await?;

        // Sync the in-memory index so concurrent reads see the updated seats.
        self.indexer.write().await.upsert(flight);

        let seats_json = sqlx::types::Json(reserved.seats.clone());

        let booking = sqlx::query_as::<_, Booking>(
            "INSERT INTO Booking (id, transaction_id, flight_no, passenger_id, seat, amount, refund, status, baggage, booking_date)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?, NULL, ?)
             RETURNING *"
        )
        .bind(booking_id)
        .bind(txn_id)
        .bind(flight_number)
        .bind(user.id)
        .bind(&seats_json)
        .bind(reserved.total_amount as i64)
        .bind(BookingStatus::Reserved)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Unable to create booking: {}", e))?;

        Ok(booking)
    }

    async fn cancel(&mut self, booking_id: u32, user: &User) -> anyhow::Result<CancelBooking> {
        // Booking must exist and belong to the calling user.
        let booking: Booking = Self::get_by_id(&self.pool, booking_id).await?;
        if booking.passenger_id != user.id {
            return Err(anyhow::anyhow!("Cancelling user is not the booking user"));
        }
        // Refund the payment, then mark the booking cancelled.
        self.payment_processor
            .refund(booking.txn_id, booking.amount)
            .await?;
        sqlx::query("UPDATE Booking SET status = ? WHERE id = ?")
            .bind(BookingStatus::Cancelled)
            .bind(booking_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Unable to cancel booking: {}", e))?;
        Ok(CancelBooking {
            booking_id,
            txn_id: booking.txn_id,
            refunded_amount: booking.amount as u64,
        })
    }

    async fn change_flight(
        &mut self,
        booking_id: u32,
        flight_no: &String,
        user: &User,
    ) -> anyhow::Result<ChangeResult> {
        let booking = Self::get_by_id(&self.pool, booking_id).await?;
        if booking.passenger_id != user.id {
            return Err(anyhow::anyhow!("Booking does not belong to user"));
        }
        let seat_numbers: Vec<String> = booking.seat.0.keys().cloned().collect();

        // Free the seats on the old flight.
        let mut old_flight = Flight::get_by_number(&self.pool, &booking.flight_no).await?;
        old_flight.release(&self.pool, &seat_numbers).await?;
        self.indexer.write().await.upsert(old_flight);

        // Reserve and confirm the same seats on the new flight.
        let mut new_flight = Flight::get_by_number(&self.pool, flight_no).await?;
        let reserved = new_flight
            .reserve(&self.pool, &seat_numbers, user.id as u64)
            .await?;
        new_flight.confirm(&self.pool, &seat_numbers).await?;
        self.indexer.write().await.upsert(new_flight);

        sqlx::query("UPDATE Booking SET flight_no = ?, amount = ? WHERE id = ?")
            .bind(flight_no)
            .bind(reserved.total_amount as i64)
            .bind(booking_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Unable to update booking: {}", e))?;

        Ok(ChangeResult { booking_id })
    }

    async fn set_baggage(
        &mut self,
        booking_id: u32,
        baggage: Option<Baggage>,
        user: &User,
    ) -> anyhow::Result<()> {
        // Get the user booking and add the baggage
        let booking = Self::get_by_id(&self.pool, booking_id).await?;
        if booking.passenger_id != user.id {
            return Err(anyhow::anyhow!("Booking does not belong to user"));
        }

        sqlx::query("UPDATE Booking SET baggage = ? WHERE id = ?")
            .bind(baggage.map(sqlx::types::Json))
            .bind(booking_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Unable to set baggage: {}", e))?;
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "UPPERCASE")]
enum BookingStatus {
    Reserved,
    Confirmed,
    Cancelled,
    Completed,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Booking {
    id: u32,
    #[sqlx(rename = "transaction_id")]
    txn_id: u32,
    flight_no: String,
    passenger_id: u32,
    seat: sqlx::types::Json<HashMap<String, SeatType>>,
    amount: i64,
    refund: i64,
    status: BookingStatus,
    baggage: Option<sqlx::types::Json<Baggage>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Baggage {
    pub pieces: u32,
    pub total_weight_kg: f64,
    pub checked: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum PaymentMethod {
    CreditCard,
    ApplePay,
    GPay,
    DebitCard,
    Upi,
}

type PinFuture<F> = Pin<Box<dyn Future<Output = F> + Send>>;

// #[async_trait]
trait PaymentProcessor: std::fmt::Debug + Send + Sync {
    // This return a transaction id
    fn charge(&self, booking_id: u32, amount: u32) -> PinFuture<anyhow::Result<u32>>;
    fn refund(&self, txn_id: u32, amount: i64) -> PinFuture<anyhow::Result<()>>;
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

// #[async_trait]
impl PaymentProcessor for MockPaymentProcessor {
    fn charge(&self, booking_id: u32, amount: u32) -> PinFuture<anyhow::Result<u32>> {
        Box::pin(async move {
            println!("Mock charge: {} for booking {}", amount, booking_id);
            Ok(booking_id)
        })
    }

    fn refund(&self, txn_id: u32, amount: i64) -> PinFuture<anyhow::Result<()>> {
        Box::pin(async move {
            println!("Mock refund: {} for txn {}", amount, txn_id);
            Ok(())
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum UserKind {
    Crew,
    Admin,
    Staff,
    Passenger,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct User {
    id: u32,
    kind: UserKind,
    name: String,
    email: String,
    contact: String,
}
trait GetById {
    type Output;

    fn get_by_id(pool: &SqlitePool, entry_id: u32) -> PinFuture<anyhow::Result<Self::Output>>;
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
    source: AirportCode,
    destination: AirportCode,
    fare: u64,
    departure_time: DateTime<Utc>,
    arrival_time: DateTime<Utc>,
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

    fn get_by_id(pool: &SqlitePool, id: u32) -> PinFuture<anyhow::Result<Self::Output>> {
        let pool = pool.clone();
        Box::pin(async move {
            let row = sqlx::query("SELECT * FROM Flight WHERE id = ?")
                .bind(id)
                .fetch_optional(&pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Flight {} not found", id))?;
            Flight::from_row(&pool, row).await
        })
    }
}

struct FlightReserved {
    seats: HashMap<String, SeatType>,
    total_amount: u64,
}

impl Flight {
    // Source/destination/crew are stored as JSON TEXT columns and seats live
    // in a separate `flight_seats` table, so a plain `#[derive(FromRow)]`
    // can't decode a Flight in one shot; map the row by hand.
    async fn from_row(pool: &SqlitePool, row: sqlx::sqlite::SqliteRow) -> anyhow::Result<Flight> {
        let source: String = row.try_get("source")?;
        let destination: String = row.try_get("destination")?;
        let crew: String = row.try_get("crew")?;
        let id = row.try_get::<i64, _>("id")? as u32;
        let seats = Self::load_seats(pool, id).await?;
        Ok(Flight {
            id,
            aircraft_id: row.try_get::<i64, _>("aircraft_id")? as u32,
            flight_no: row.try_get("flight_no")?,
            source: serde_json::from_str(&source)?,
            destination: serde_json::from_str(&destination)?,
            fare: row.try_get::<i64, _>("fare")? as u64,
            departure_time: row.try_get("departure_time")?,
            arrival_time: row.try_get("arrival_time")?,
            status: row.try_get("status")?,
            seats,
            crew: serde_json::from_str(&crew)?,
        })
    }

    async fn load_seats(
        pool: &SqlitePool,
        flight_id: u32,
    ) -> anyhow::Result<HashMap<String, SeatRecord>> {
        let rows = sqlx::query(
            "SELECT seat_no, seat_type, status, holder FROM flight_seats WHERE flight_id = ?",
        )
        .bind(flight_id)
        .fetch_all(pool)
        .await?;
        let mut seats = HashMap::with_capacity(rows.len());
        for row in rows {
            let number: String = row.try_get("seat_no")?;
            let holder: Option<i64> = row.try_get("holder")?;
            seats.insert(
                number.clone(),
                SeatRecord {
                    number,
                    seat_type: row.try_get("seat_type")?,
                    status: row.try_get("status")?,
                    holder: holder.map(|h| h as u64),
                },
            );
        }
        Ok(seats)
    }

    // Batch-write seat status/holder for a flight. Callers that need "only
    // if still Available" semantics use `reserve`'s conditional UPDATE
    // instead — this is for transitions the caller already owns exclusively
    // (Reserved -> Booked, or releasing back to Available).
    async fn persist_seats(
        pool: &SqlitePool,
        flight_id: u32,
        seat_numbers: &[String],
        status: SeatStatus,
        holder: Option<u64>,
    ) -> anyhow::Result<()> {
        let mut builder = QueryBuilder::new("UPDATE flight_seats SET status = ");
        builder.push_bind(status);
        builder
            .push(", holder = ")
            .push_bind(holder.map(|h| h as i64));
        builder.push(" WHERE flight_id = ").push_bind(flight_id);
        builder.push(" AND seat_no IN (");
        let mut separated = builder.separated(", ");
        for seat in seat_numbers {
            separated.push_bind(seat);
        }
        separated.push_unseparated(")");

        builder.build().execute(pool).await?;
        Ok(())
    }

    async fn get_by_number(pool: &SqlitePool, flight_no: &str) -> anyhow::Result<Flight> {
        let row = sqlx::query("SELECT * FROM Flight WHERE flight_no = ?")
            .bind(flight_no)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Flight {} not found", flight_no))?;
        Flight::from_row(pool, row).await
    }

    fn available_seat(&self) -> anyhow::Result<usize> {
        let remaining = self
            .seats
            .values()
            .filter(|s| s.status == SeatStatus::Available)
            .count();
        Ok(remaining)
    }

    // Atomically claims `seat_no` in the DB — a single conditional UPDATE
    // gated on `status = Available`, run in a transaction and checked against
    // the number of rows it actually touched. SQLite serializes writer
    // transactions, so this is the arbiter of "who got the seat": a
    // read-then-write can let two concurrent bookings both see Available and
    // both proceed, but this can't — the second UPDATE simply matches zero
    // of the already-Reserved rows and the rowcount check rejects it.
    async fn reserve(
        &mut self,
        pool: &SqlitePool,
        seat_no: &[String],
        user_id: u64,
    ) -> anyhow::Result<FlightReserved> {
        let mut tx = pool.begin().await?;

        let mut builder = QueryBuilder::new("UPDATE flight_seats SET status = ");
        builder.push_bind(SeatStatus::Reserved);
        builder.push(", holder = ").push_bind(user_id as i64);
        builder.push(" WHERE flight_id = ").push_bind(self.id);
        builder
            .push(" AND status = ")
            .push_bind(SeatStatus::Available);
        builder.push(" AND seat_no IN (");
        {
            let mut separated = builder.separated(", ");
            for seat in seat_no {
                separated.push_bind(seat);
            }
            separated.push_unseparated(")");
        }
        let result = builder.build().execute(&mut *tx).await?;

        if result.rows_affected() as usize != seat_no.len() {
            tx.rollback().await?;
            return Err(anyhow::anyhow!(
                "One or more requested seats are no longer available"
            ));
        }
        tx.commit().await?;

        // DB claim succeeded; mirror it into the in-memory copy and price it.
        let mut seats = HashMap::new();
        let mut total_amount = 0;
        for s in seat_no {
            let record = self
                .seats
                .get_mut(s)
                .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
            seats.insert(s.to_string(), record.seat_type);
            total_amount += self.fare; // TODO: price may vary by seat_type
            record.holder = Some(user_id);
            record.status = SeatStatus::Reserved;
        }
        Ok(FlightReserved {
            seats,
            total_amount,
        })
    }

    async fn confirm(&mut self, pool: &SqlitePool, seat_no: &[String]) -> anyhow::Result<()> {
        let mut holder = None;
        for s in seat_no {
            let record = self
                .seats
                .get_mut(s)
                .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
            if record.status != SeatStatus::Reserved {
                return Err(anyhow::anyhow!("Seat {} not reserved", s));
            }
            record.status = SeatStatus::Booked;
            holder = record.holder;
        }
        Self::persist_seats(pool, self.id, seat_no, SeatStatus::Booked, holder).await
    }

    async fn release(&mut self, pool: &SqlitePool, seat_no: &[String]) -> anyhow::Result<()> {
        for s in seat_no {
            let record = self
                .seats
                .get_mut(s)
                .ok_or_else(|| anyhow::anyhow!("Seat code is invalid"))?;
            record.status = SeatStatus::Available;
            record.holder = None;
        }
        Self::persist_seats(pool, self.id, seat_no, SeatStatus::Available, None).await
    }

    fn snapshot(&mut self) -> FlightSnapshot {
        FlightSnapshot {
            flight_no: self.flight_no.clone(),
            fare: self.fare,
            departure_time: self.departure_time,
            arrival_time: self.arrival_time,
            source: self.source.clone(),
            destination: self.destination.clone(),
            available_seat: self.available_seat().unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone)]
struct FlightRepository {
    db: SqlitePool,
}

impl FlightRepository {
    fn new(db: SqlitePool) -> FlightRepository {
        Self { db }
    }

    // Creates a new flight entry (and its default seat map) in the database.
    async fn add_flight(
        &self,
        aircraft_id: u32,
        flight_no: String,
        spec: FlightSpec,
    ) -> anyhow::Result<Flight> {
        let aircraft = Aircraft::get_by_id(&self.db, aircraft_id).await?;
        let seats: HashMap<String, SeatRecord> = aircraft
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

        let source_json = serde_json::to_string(&spec.source)?;
        let destination_json = serde_json::to_string(&spec.destination)?;

        // Flight row + its full seat map are one unit: a crash between the
        // two inserts must not leave a Flight with no seats, so both run in
        // the same transaction.
        let mut tx = self.db.begin().await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO Flight (aircraft_id, flight_no, source, destination, fare, departure_time, arrival_time, status, crew)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, '[]') RETURNING id",
        )
        .bind(aircraft_id)
        .bind(&flight_no)
        .bind(&source_json)
        .bind(&destination_json)
        .bind(spec.fare as i64)
        .bind(spec.departure_time)
        .bind(spec.arrival_time)
        .bind(FlightStatus::Scheduled)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| anyhow::anyhow!("Unable to create flight: {}", e))?;
        let id = id as u32;

        if !seats.is_empty() {
            let mut builder = QueryBuilder::new(
                "INSERT INTO flight_seats (flight_id, seat_no, seat_type, status, holder) ",
            );
            builder.push_values(seats.values(), |mut row, record| {
                row.push_bind(id)
                    .push_bind(&record.number)
                    .push_bind(record.seat_type)
                    .push_bind(record.status)
                    .push_bind(None::<i64>);
            });
            builder
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|e| anyhow::anyhow!("Unable to create seats: {}", e))?;
        }

        tx.commit().await?;

        Ok(Flight {
            id,
            aircraft_id,
            source: spec.source,
            destination: spec.destination,
            arrival_time: spec.arrival_time,
            departure_time: spec.departure_time,
            fare: spec.fare,
            flight_no,
            status: FlightStatus::Scheduled,
            seats,
            crew: vec![],
        })
    }

    async fn get_by_number(&self, flight_no: &str) -> anyhow::Result<Flight> {
        Flight::get_by_number(&self.db, flight_no).await
    }

    async fn add_crew(&mut self, flight_id: u32, member: User) -> anyhow::Result<()> {
        let mut flight = Flight::get_by_id(&self.db, flight_id).await?;
        flight.crew.push(member);
        // TODO: Update the entry in the flight table
        Ok(())
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
        let route = (flight.source.code.clone(), flight.destination.code.clone());
        let date = flight.departure_time.date_naive();

        // Only the first upsert of a given id should add it to the secondary
        // indexes — re-upserting after a booking (same route/date, changed
        // seats) must not push a duplicate id into by_route/by_date, or
        // `search` would return that flight multiple times.
        let is_new = self.flights.insert(id, flight).is_none();
        if is_new {
            self.by_route.entry(route).or_default().push(id);
            self.by_date.entry(date).or_default().push(id);
        }
    }

    /// Intersect any number of id lists, smallest first. Adding a secondary
    /// (optional) filter to `search` means adding one more list to the `Vec`
    /// passed in here — this doesn't change.
    fn intersect_ids(mut lists: Vec<&Vec<u32>>) -> Vec<u32> {
        lists.sort_by_key(|l| l.len());
        let Some((smallest, rest)) = lists.split_first() else {
            return vec![];
        };
        let mut acc: HashSet<u32> = smallest.iter().copied().collect();
        for list in rest {
            let probe: HashSet<u32> = list.iter().copied().collect();
            acc.retain(|id| probe.contains(id));
        }
        acc.into_iter().collect()
    }
}

/// Search criteria for `FlightIndex::search`. `source`/`destination`/`date`
/// are the primary filters: always required, always indexed (`by_route`,
/// `by_date`), always included in the intersection.
///
/// A secondary (optional) filter is added here as an `Option<T>` field, with
/// a matching `HashMap<T, Vec<u32>>` index on `FlightIndex` kept in sync in
/// `upsert`, and one `if let Some(..)` block in `search` that pushes its
/// id-list into the intersection when present. No secondary filters exist
/// yet, so `search` today only ever builds the two mandatory lists.
struct FlightQuery<'a> {
    source: &'a str,
    destination: &'a str,
    date: NaiveDate,
}

trait Indexer {
    /// Hot-path search: intersect the id lists for every filter in `query`
    /// that applies (primary filters always, secondary filters when `Some`),
    /// then deref each surviving id via `flights`.
    fn search(&self, query: FlightQuery) -> Vec<&Flight>;
    /// Rebuild the whole index from the DB (startup / resync).
    async fn build(&mut self, pool: &SqlitePool) -> anyhow::Result<()>;
}

impl Indexer for FlightIndex {
    fn search(&self, query: FlightQuery) -> Vec<&Flight> {
        let route_ids = self
            .by_route
            .get(&(query.source.to_string(), query.destination.to_string()));
        let date_ids = self.by_date.get(&query.date);

        let lists = match (route_ids, date_ids) {
            (Some(r), Some(d)) => vec![r, d],
            _ => return vec![],
        };

        // Secondary filters plug in here: for each `Some` field on `query`,
        // look up its index and `lists.push(ids)` (missing -> empty result,
        // same as a primary filter). None exist yet.

        Self::intersect_ids(lists)
            .iter()
            .filter_map(|id| self.flights.get(id))
            .collect()
    }

    async fn build(&mut self, pool: &SqlitePool) -> anyhow::Result<()> {
        let rows = sqlx::query("SELECT * FROM Flight").fetch_all(pool).await?;
        for row in rows {
            self.upsert(Flight::from_row(pool, row).await?);
        }
        Ok(())
    }
}

// Api types
#[derive(Debug, Deserialize)]
struct AddFlight {
    aircraft_id: u32,
    source: AirportCode,
    destination: AirportCode,
    flight_no: String,
    fare: u64,
    // Iso string date format
    departure_time: String,
    // Iso string date format
    arrival_time: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFlight {
    status: Option<FlightStatus>,
}

#[derive(Debug, Deserialize)]
struct AddAircraft {
    tail_no: String,
    total_seats: i64,
    model_no: String,
}

#[derive(Debug, Deserialize)]
struct UpdateAircraft {
    tail_no: Option<String>,
    total_seats: Option<i64>,
    model_no: Option<String>,
}

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
    flight_no: String,
    seat_numbers: Vec<String>,
    passenger: User,
    payment_method: PaymentMethod,
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
    let aircraft = sqlx::query_as::<_, Aircraft>(
        "INSERT INTO Aircraft (tail_no, total_seats, model_no) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(payload.tail_no)
    .bind(payload.total_seats)
    .bind(payload.model_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError(anyhow::anyhow!("Unable to create aircraft: {}", e)))?;
    Ok(Json(aircraft))
}

async fn aircraft_patch(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateAircraft>,
) -> Result<(), AppError> {
    if let Some(tail_no) = payload.tail_no {
        sqlx::query("UPDATE Aircraft SET tail_no = ? WHERE id = ?")
            .bind(tail_no)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError(anyhow::anyhow!("Unable to update aircraft: {}", e)))?;
    }
    if let Some(total_seats) = payload.total_seats {
        sqlx::query("UPDATE Aircraft SET total_seats = ? WHERE id = ?")
            .bind(total_seats)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError(anyhow::anyhow!("Unable to update aircraft: {}", e)))?;
    }
    if let Some(model_no) = payload.model_no {
        sqlx::query("UPDATE Aircraft SET model_no = ? WHERE id = ?")
            .bind(model_no)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError(anyhow::anyhow!("Unable to update aircraft: {}", e)))?;
    }
    Ok(())
}

// ----- Flight handler
async fn add_flight(
    State(state): State<AppState>,
    Json(payload): Json<AddFlight>,
) -> Result<Json<Flight>, AppError> {
    let departure_time = DateTime::parse_from_rfc3339(&payload.departure_time)
        .map_err(|e| AppError(anyhow::anyhow!("invalid departure_time: {}", e)))?
        .with_timezone(&Utc);
    let arrival_time = DateTime::parse_from_rfc3339(&payload.arrival_time)
        .map_err(|e| AppError(anyhow::anyhow!("invalid arrival_time: {}", e)))?
        .with_timezone(&Utc);

    let spec = FlightSpec::new(
        &payload.flight_no,
        payload.source,
        payload.destination,
        payload.fare,
        departure_time,
        arrival_time,
    );

    let flight = state
        .flight_repo
        .add_flight(payload.aircraft_id, payload.flight_no, spec)
        .await
        .map_err(AppError)?;

    // Keep the hot-path search index in sync with the DB write.
    state.indexer.write().await.upsert(flight.clone());

    Ok(Json(flight))
}

async fn flights_index(State(state): State<AppState>) -> Result<Json<Vec<Flight>>, AppError> {
    // DB is source of truth; indexer mirrors it for hot-path reads.
    let idx = state.indexer.read().await;
    Ok(Json(idx.flights.values().cloned().collect()))
}

async fn flight_patch(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateFlight>,
) -> Result<(), AppError> {
    if let Some(status) = payload.status {
        sqlx::query("UPDATE Flight SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError(anyhow::anyhow!("Unable to update flight: {}", e)))?;
    }
    Ok(())
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
        .read()
        .await
        .search(FlightQuery {
            source: &source,
            destination: &destination,
            date,
        })
        .into_iter()
        .cloned()
        .collect();
    Ok(Json(found))
}

async fn book_flight(
    State(mut state): State<AppState>,
    Json(payload): Json<BookFlight>,
) -> Result<Json<Booking>, AppError> {
    let booking = state
        .booking_processor
        .book(
            &payload.passenger,
            &payload.flight_no,
            &payload.seat_numbers,
            payload.payment_method,
        )
        .await
        .map_err(AppError)?;
    Ok(Json(booking))
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
    // let pool = Arc::new(&db);
    sqlx::migrate!().run(&db).await?;

    // Seed the in-memory index directly (avoids circular db call).
    let mut indexer = FlightIndex::new();
    for flight in records::flights() {
        indexer.upsert(flight);
    }
    let indexer = Arc::new(RwLock::new(indexer));

    let payment_processor = std::sync::Arc::new(MockPaymentProcessor {});

    let booking_processor =
        BookingProcessor::new(db.clone(), payment_processor.clone(), indexer.clone());
    let flight_repo = FlightRepository::new(db.clone());
    let state = AppState {
        db,
        indexer,
        booking_processor,
        flight_repo,
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
    use super::{
        BookingProcessor, FlightIndex, FlightQuery, Indexer, MockPaymentProcessor, NaiveDate,
        PaymentMethod, User, UserKind,
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn query<'a>(source: &'a str, destination: &'a str, date: NaiveDate) -> FlightQuery<'a> {
        FlightQuery {
            source,
            destination,
            date,
        }
    }

    #[test]
    fn seed_two_flights() {
        assert_eq!(records::aircrafts().len(), 2);
        assert_eq!(records::flights().len(), 2);
    }

    #[test]
    fn seed_flights_have_route() {
        let f = records::flights();
        assert_eq!(f[0].source.code, "DEL");
        assert_eq!(f[0].destination.code, "BOM");
        assert_eq!(f[1].source.code, "BOM");
        assert_eq!(f[1].destination.code, "BLR");
    }

    #[test]
    fn search_matches_route_and_date_from_index() {
        let mut idx = super::FlightIndex::new();
        for f in records::flights() {
            idx.upsert(f);
        }

        let date = records::flights()[0].departure_time.date_naive();

        let hits = idx.search(query("DEL", "BOM", date));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].flight_no, "AI-101");

        // Wrong route -> no hits
        assert!(idx.search(query("DEL", "BLR", date)).is_empty());
        // Wrong date -> no hits
        assert!(
            idx.search(query("DEL", "BOM", NaiveDate::from_ymd_opt(1999, 1, 1).unwrap()))
                .is_empty()
        );
    }

    // Re-upserting an already-indexed flight (e.g. the write-through after a
    // booking) must not duplicate its id in the secondary indexes.
    #[test]
    fn upsert_twice_does_not_duplicate_search_hits() {
        let mut idx = super::FlightIndex::new();
        let flight = records::flights().into_iter().next().unwrap();
        let date = flight.departure_time.date_naive();

        idx.upsert(flight.clone());
        idx.upsert(flight);

        assert_eq!(idx.search(query("DEL", "BOM", date)).len(), 1);
    }

    // Exercises the exact race `Flight::reserve` closes: two bookings for
    // the same seat, issued concurrently against a real SQLite DB. Without
    // the conditional `UPDATE ... WHERE status = 'AVAILABLE'` + rowcount
    // check, both would read the seat as Available and both would succeed.
    #[tokio::test]
    async fn concurrent_booking_only_one_wins_the_seat() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "flight_booking_test_{}_{}.db",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO Aircraft (id, tail_no, total_seats, model_no) VALUES (1, 'VT-TEST', 6, 'Test')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO Flight (id, aircraft_id, flight_no, source, destination, fare, departure_time, arrival_time, status, crew)
             VALUES (1, 1, 'AI-1', ?, ?, 1000, ?, ?, 'SCHEDULED', '[]')",
        )
        .bind(r#"{"name":"Airport A","code":"AAA"}"#)
        .bind(r#"{"name":"Airport B","code":"BBB"}"#)
        .bind(super::Utc::now())
        .bind(super::Utc::now())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO flight_seats (flight_id, seat_no, seat_type, status, holder) VALUES (1, '1A', 'ECONOMY', 'AVAILABLE', NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let indexer = Arc::new(RwLock::new(FlightIndex::new()));
        let payment_processor = Arc::new(MockPaymentProcessor {});
        let mut bp1 =
            BookingProcessor::new(pool.clone(), payment_processor.clone(), indexer.clone());
        let mut bp2 = BookingProcessor::new(pool.clone(), payment_processor.clone(), indexer);

        let user1 = User {
            id: 1,
            kind: UserKind::Passenger,
            name: "Alice".into(),
            email: "alice@example.com".into(),
            contact: "111".into(),
        };
        let user2 = User {
            id: 2,
            kind: UserKind::Passenger,
            name: "Bob".into(),
            email: "bob@example.com".into(),
            contact: "222".into(),
        };
        let seats = vec!["1A".to_string()];

        let (r1, r2) = tokio::join!(
            bp1.book(&user1, "AI-1", &seats, PaymentMethod::CreditCard),
            bp2.book(&user2, "AI-1", &seats, PaymentMethod::CreditCard),
        );

        let ok_count = [r1.is_ok(), r2.is_ok()].into_iter().filter(|ok| *ok).count();
        assert_eq!(ok_count, 1, "exactly one concurrent booking should win the seat");

        let final_status: String = sqlx::query_scalar("SELECT status FROM flight_seats WHERE flight_id = 1 AND seat_no = '1A'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(final_status, "BOOKED");

        let _ = std::fs::remove_file(&path);
    }
}
