use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::flights::{FlightRegistry, FlightSnapshot};
use crate::locks::{rd, wr};
use crate::time::Date;

type SearchKey = (String, String, Date);

/// Read-optimised index over (source, destination, date). Lookups never touch
/// the registry; the index is rebuilt on schedule changes.
pub struct FlightSearch {
    flights: Arc<FlightRegistry>,
    index: RwLock<HashMap<SearchKey, Vec<String>>>,
}

impl FlightSearch {
    pub fn new(flights: Arc<FlightRegistry>) -> Self {
        let search = Self {
            flights,
            index: RwLock::new(HashMap::new()),
        };
        search.rebuild_index();
        search
    }

    pub fn rebuild_index(&self) {
        // Collect under the registry read guard, then drop it before writing
        // the index: lock order is always registry before index.
        let mut by_key: HashMap<SearchKey, Vec<String>> = HashMap::new();
        {
            let flights = rd(&self.flights);
            for flight in flights.values() {
                by_key
                    .entry((
                        flight.spec.source.clone(),
                        flight.spec.destination.clone(),
                        flight.spec.date,
                    ))
                    .or_default()
                    .push(flight.flight_number.clone());
            }
        }
        for numbers in by_key.values_mut() {
            numbers.sort();
        }
        *wr(&self.index) = by_key;
    }

    pub fn search(&self, source: &str, destination: &str, date: Date) -> Vec<FlightSnapshot> {
        let numbers = {
            let index = rd(&self.index);
            index
                .get(&(source.to_string(), destination.to_string(), date))
                .cloned()
                .unwrap_or_default()
        };
        let flights = rd(&self.flights);
        let mut results: Vec<FlightSnapshot> = numbers
            .iter()
            .filter_map(|number| flights.get(number).map(|flight| flight.snapshot()))
            .collect();
        drop(flights);
        results.sort_by_key(|snapshot| snapshot.departure);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util;

    #[test]
    fn test_search_hit() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);

        let hits = ams
            .flight_search
            .search("DEL", "BOM", Date::new(2026, 1, 1));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].flight_number, flight.flight_number);
        assert_eq!(hits[0].available_seats, 36);
    }
}
