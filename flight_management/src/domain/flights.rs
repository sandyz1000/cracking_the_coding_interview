use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::{AmsError, AmsResult};
use crate::locks::{rd, wr};
use crate::time::{Date, Time};

const CABIN_COLUMNS: [char; 6] = ['A', 'B', 'C', 'D', 'E', 'F'];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatType {
    Economy,
    Business,
    First,
}

impl SeatType {
    pub fn fare_multiplier(self) -> f64 {
        match self {
            SeatType::Economy => 1.0,
            SeatType::Business => 2.2,
            SeatType::First => 3.5,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatStatus {
    /// Seat is held while payment is being processed.
    Available,
    Reserved,
    Booked,
}

#[derive(Clone, Debug)]
pub struct Aircraft {
    pub tail_number: String,
    pub model: String,
    pub total_seats: usize,
}

impl Aircraft {
    /// Default 6-abreast cabin (A-F): row 1 = First, rows 2-3 = Business,
    /// the rest Economy. In production this comes from a configurable layout.
    pub fn default_layout(&self) -> Vec<(String, SeatType)> {
        let mut out = Vec::with_capacity(self.total_seats);
        for row in 1..=self.total_seats / 6 {
            let seat_type = if row == 1 {
                SeatType::First
            } else if row <= 3 {
                SeatType::Business
            } else {
                SeatType::Economy
            };
            for column in CABIN_COLUMNS {
                out.push((format!("{row}{column}"), seat_type));
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct SeatRecord {
    pub number: String,
    pub seat_type: SeatType,
    pub status: SeatStatus,
    /// Passenger id holding this seat (Reserved or Booked).
    pub holder: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CrewRole {
    Captain,
    FirstOfficer,
    CabinCrew,
}

#[derive(Clone, Debug)]
pub struct CrewMember {
    pub id: u64,
    pub name: String,
    pub role: CrewRole,
}

#[derive(Clone, Debug)]
pub struct FlightSpec {
    pub source: String,
    pub destination: String,
    pub date: Date,
    pub departure: Time,
    pub arrival: Time,
}

pub type FlightRegistry = RwLock<HashMap<String, Arc<Flight>>>;

/// The flight inventory. Interior mutability is required because a Flight is
/// shared (`Arc`) across search, booking, and cancellation.
pub struct Flight {
    pub flight_number: String,
    pub spec: FlightSpec,
    pub base_fare: f64,
    aircraft: RwLock<Arc<Aircraft>>,
    seats: RwLock<HashMap<String, SeatRecord>>,
    crew: RwLock<Vec<CrewMember>>,
}

impl Flight {
    pub fn new(
        flight_number: String,
        spec: FlightSpec,
        aircraft: Arc<Aircraft>,
        base_fare: f64,
    ) -> Self {
        let seats = aircraft
            .default_layout()
            .into_iter()
            .map(|(number, seat_type)| {
                let record = SeatRecord {
                    number: number.clone(),
                    seat_type,
                    status: SeatStatus::Available,
                    holder: None,
                };
                (number, record)
            })
            .collect();
        Self {
            flight_number,
            spec,
            base_fare,
            aircraft: RwLock::new(aircraft),
            seats: RwLock::new(seats),
            crew: RwLock::new(Vec::new()),
        }
    }

    /// Must NOT take the seats lock: callers (reserve) may already hold it.
    pub fn price_for(&self, seat_type: SeatType) -> f64 {
        self.base_fare * seat_type.fare_multiplier()
    }

    pub fn available_seats(&self) -> usize {
        rd(&self.seats)
            .values()
            .filter(|seat| seat.status == SeatStatus::Available)
            .count()
    }

    pub fn seat_map(&self) -> Vec<SeatRecord> {
        let mut seats: Vec<SeatRecord> = rd(&self.seats).values().cloned().collect();
        seats.sort_by(|a, b| a.number.cmp(&b.number));
        seats
    }

    pub fn crew_manifest(&self) -> Vec<CrewMember> {
        rd(&self.crew).clone()
    }

    pub fn assign_crew(&self, crew: Vec<CrewMember>) {
        wr(&self.crew).extend(crew);
    }

    // copy all the passenger from old aircraft to new aircraft
    pub fn swap_aircraft(&self, aircraft: Arc<Aircraft>) -> AmsResult<()> {
        let new_layout = aircraft.default_layout();
        let mut seats = wr(&self.seats);
        // Validation
        for (number, record) in seats.iter() {
            // If the flight layout doesn't match i.e. row, col should match the new aircraft
            if record.status != SeatStatus::Available
                && !new_layout.iter().any(|(n, _)| n == number)
            {
                return Err(AmsError::SeatNotAvailable(format!(
                    "seat {number} is {:?} and missing from the new layout",
                    record.status
                )));
            }
        }
        let mut next = HashMap::new();
        for (number, seat_type) in new_layout {
            let record = seats.get(&number).cloned().unwrap_or_else(|| SeatRecord {
                number: number.clone(),
                seat_type,
                status: SeatStatus::Available,
                holder: None,
            });
            next.insert(number, record);
        }
        *seats = next;
        drop(seats);
        *wr(&self.aircraft) = aircraft;
        Ok(())
    }

    /// Reserve a seat (holds it while payment is processed). Returns the
    /// seat type and price. Caller owns the seat lifecycle: confirm or
    /// release when the payment outcome is known.
    pub(crate) fn reserve(&self, seat_number: &str, holder: u64) -> AmsResult<(SeatType, f64)> {
        let mut seats = wr(&self.seats);
        let record = seats
            .get_mut(seat_number)
            .ok_or_else(|| AmsError::SeatNotFound(seat_number.to_string()))?;
        if record.status != SeatStatus::Available {
            return Err(AmsError::SeatNotAvailable(format!(
                "seat {seat_number} is {:?}",
                record.status
            )));
        }
        record.status = SeatStatus::Reserved;
        record.holder = Some(holder);
        Ok((record.seat_type, self.price_for(record.seat_type)))
    }

    /// Mark a reserved seat as paid for.
    pub(crate) fn confirm(&self, seat_number: &str) {
        if let Some(record) = wr(&self.seats).get_mut(seat_number) {
            record.status = SeatStatus::Booked;
        }
    }

    /// Return a reserved or booked seat to the pool.
    pub(crate) fn release(&self, seat_number: &str) {
        if let Some(record) = wr(&self.seats).get_mut(seat_number) {
            record.status = SeatStatus::Available;
            record.holder = None;
        }
    }

    pub fn snapshot(&self) -> FlightSnapshot {
        FlightSnapshot {
            flight_number: self.flight_number.clone(),
            source: self.spec.source.clone(),
            destination: self.spec.destination.clone(),
            date: self.spec.date,
            departure: self.spec.departure,
            arrival: self.spec.arrival,
            aircraft_model: rd(&self.aircraft).model.clone(),
            available_seats: self.available_seats(),
            min_fare: self.base_fare,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlightSnapshot {
    pub flight_number: String,
    pub source: String,
    pub destination: String,
    pub date: Date,
    pub departure: Time,
    pub arrival: Time,
    pub aircraft_model: String,
    pub available_seats: usize,
    pub min_fare: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let aircraft = Aircraft {
            tail_number: "T1".into(),
            model: "TestJet".into(),
            total_seats: 36,
        };
        let layout = aircraft.default_layout();

        assert_eq!(layout.len(), 36);
        assert_eq!(layout[0], ("1A".to_string(), SeatType::First));
        assert_eq!(layout[5], ("1F".to_string(), SeatType::First));
        assert_eq!(layout[6], ("2A".to_string(), SeatType::Business));
        assert_eq!(layout[17], ("3F".to_string(), SeatType::Business));
        assert_eq!(layout[18], ("4A".to_string(), SeatType::Economy));
        assert_eq!(layout[35], ("6F".to_string(), SeatType::Economy));
    }

    #[test]
    fn test_reserve_conflict() {
        let spec = FlightSpec {
            source: "DEL".into(),
            destination: "BOM".into(),
            date: Date::new(2026, 1, 1),
            departure: Time::new(8, 0),
            arrival: Time::new(10, 0),
        };
        let flight = Flight::new(
            "AI0001".into(),
            spec,
            Arc::new(Aircraft {
                tail_number: "T1".into(),
                model: "TestJet".into(),
                total_seats: 36,
            }),
            100.0,
        );

        flight.reserve("1A", 1).unwrap();
        assert!(matches!(
            flight.reserve("1A", 2),
            Err(AmsError::SeatNotAvailable(_))
        ));
        flight.release("1A");
        flight.reserve("1A", 3).unwrap();
    }
}
