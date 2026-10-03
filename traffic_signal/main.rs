//! Traffic signal control system. Full requirements: see `readme.md`. Same
//! architectural DNA as library_management/music_streaming (`OnceLock`
//! singleton composition root, `RwLock`-guarded shared state, thiserror,
//! strum `Display`) scaled to this domain: one road is ever green at a
//! time, and an emergency preemption must never leave two roads green.

/*
# Designing a Traffic Signal Control System

## Requirements

1. The traffic signal system should control the flow of traffic at an intersection with multiple roads.
2. The system should support different types of signals, such as red, yellow, and green.
3. The duration of each signal should be configurable and adjustable based on traffic conditions.
4. The system should handle the transition between signals smoothly, ensuring safe and efficient traffic flow.
5. The system should be able to detect and handle emergency situations, such as an ambulance or fire truck
approaching the intersection.
6. The system should be scalable and extensible to support additional features and functionality.

## Classes, Interfaces and Enumerations

1. The **Signal** enum represents the different states of a traffic light: red, yellow, and green.
2. The **Road** class represents a road in the traffic signal system, with properties such as ID, name, and an
associated traffic light.
3. The **TrafficLight** class represents a traffic light, with properties such as ID, current signal, and
durations for each signal state. It provides methods to change the signal and notify observers (e.g., roads)
about signal changes.
4. The **TrafficController** class serves as the central controller for the traffic signal system. It follows
the Singleton pattern to ensure a single instance of the controller. It manages the roads and their associated
traffic lights, starts the traffic control process, and handles emergency situations.
5. The **TrafficSignalSystemDemo** class is the main entry point of the application. It demonstrates the usage
of the traffic signal system by creating roads, traffic lights, assigning traffic lights to roads, and starting
the traffic control process.
*/

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum_macros::Display)]
pub enum Signal {
    Red,
    Yellow,
    Green,
}

impl Signal {
    /// The one safe cycle: Green -> Yellow -> Red -> Green. Requirement #4.
    fn next(self) -> Self {
        match self {
            Signal::Green => Signal::Yellow,
            Signal::Yellow => Signal::Red,
            Signal::Red => Signal::Green,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TrafficError {
    #[error("road {id} not found")]
    RoadNotFound { id: uuid::Uuid },
}

pub type TrafficResult<T> = std::result::Result<T, TrafficError>;

/// Requirement #3: per-signal durations, adjustable per light.
#[derive(Debug, Clone)]
pub struct TrafficLight {
    pub id: uuid::Uuid,
    signal: Signal,
    durations: HashMap<Signal, u32>,
}

impl TrafficLight {
    pub fn new(red_secs: u32, yellow_secs: u32, green_secs: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            signal: Signal::Red,
            durations: HashMap::from([
                (Signal::Red, red_secs),
                (Signal::Yellow, yellow_secs),
                (Signal::Green, green_secs),
            ]),
        }
    }

    pub fn signal(&self) -> Signal {
        self.signal
    }

    pub fn duration(&self, signal: Signal) -> u32 {
        *self.durations.get(&signal).unwrap_or(&0)
    }

    pub fn set_duration(&mut self, signal: Signal, secs: u32) {
        self.durations.insert(signal, secs);
    }

    fn set_signal(&mut self, signal: Signal) {
        self.signal = signal;
    }
}

#[derive(Debug, Clone)]
pub struct Road {
    pub id: uuid::Uuid,
    pub name: String,
    light: TrafficLight,
}

impl Road {
    pub fn new(name: impl Into<String>, light: TrafficLight) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            light,
        }
    }

    pub fn signal(&self) -> Signal {
        self.light.signal()
    }

    /// The "notify observers about signal changes" behavior from the design
    /// doc — this road is the only observer of its own light, so the
    /// notification is just the printed line.
    fn set_signal(&mut self, signal: Signal) {
        self.light.set_signal(signal);
        println!(
            "  {} ({}) -> {} for {}s",
            self.name,
            self.id,
            signal,
            self.light.duration(signal)
        );
    }
}

/// Singleton control point for one intersection (requirement #4).
pub struct TrafficController {
    roads: RwLock<Vec<Road>>,
}

impl TrafficController {
    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<TrafficController>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(TrafficController::new()))
    }

    pub fn new() -> Self {
        Self {
            roads: RwLock::new(Vec::new()),
        }
    }

    pub fn add_road(&self, road: Road) -> uuid::Uuid {
        let id = road.id;
        write_guard(&self.roads).push(road);
        id
    }

    pub fn road_signal(&self, id: uuid::Uuid) -> TrafficResult<Signal> {
        read_guard(&self.roads)
            .iter()
            .find(|road| road.id == id)
            .map(|road| road.signal())
            .ok_or(TrafficError::RoadNotFound { id })
    }

    pub fn signals(&self) -> Vec<(uuid::Uuid, Signal)> {
        read_guard(&self.roads)
            .iter()
            .map(|road| (road.id, road.signal()))
            .collect()
    }

    /// Requirement #1 + #4: round-robin through every road, one green at a
    /// time. Each road's turn sets it Green (every other road forced Red in
    /// the same write-locked pass, so no two roads are ever green together),
    /// then transitions Yellow -> Red before the next road's turn.
    pub fn run_cycles(&self, cycles: u32) {
        let count = read_guard(&self.roads).len();
        for _ in 0..cycles {
            for turn in 0..count {
                let mut roads = write_guard(&self.roads);
                for (i, road) in roads.iter_mut().enumerate() {
                    road.set_signal(if i == turn { Signal::Green } else { Signal::Red });
                }
                drop(roads);
                for _ in 0..2 {
                    let mut roads = write_guard(&self.roads);
                    let next = roads[turn].signal().next();
                    roads[turn].set_signal(next);
                }
            }
        }
    }

    /// Requirement #5: an emergency vehicle on `road_id` gets an immediate
    /// green; every other road is forced red in the same write-locked pass,
    /// so a reader (or a second, racing emergency call) can never observe
    /// two roads green at once.
    pub fn handle_emergency(&self, road_id: uuid::Uuid) -> TrafficResult<()> {
        let mut roads = write_guard(&self.roads);
        if !roads.iter().any(|road| road.id == road_id) {
            return Err(TrafficError::RoadNotFound { id: road_id });
        }
        println!("EMERGENCY: preempting for road {road_id}");
        for road in roads.iter_mut() {
            road.set_signal(if road.id == road_id {
                Signal::Green
            } else {
                Signal::Red
            });
        }
        Ok(())
    }
}

impl Default for TrafficController {
    fn default() -> Self {
        Self::new()
    }
}

fn run_demo() {
    let controller = TrafficController::instance();

    let main_st = controller.add_road(Road::new("Main St", TrafficLight::new(20, 3, 15)));
    let oak_ave = controller.add_road(Road::new("Oak Ave", TrafficLight::new(20, 3, 10)));
    let elm_rd = controller.add_road(Road::new("Elm Rd", TrafficLight::new(20, 3, 12)));

    println!("== Normal cycling (2 rotations) ==");
    controller.run_cycles(2);
    assert!(
        controller
            .signals()
            .iter()
            .all(|(_, signal)| *signal == Signal::Red),
        "a full rotation returns every road to red"
    );

    println!("\n== Emergency vehicle approaching Oak Ave ==");
    controller.handle_emergency(oak_ave).expect("road exists");
    assert_eq!(controller.road_signal(oak_ave).unwrap(), Signal::Green);
    assert_eq!(controller.road_signal(main_st).unwrap(), Signal::Red);
    assert_eq!(controller.road_signal(elm_rd).unwrap(), Signal::Red);

    println!("\n== Resume normal cycling ==");
    controller.run_cycles(1);

    match controller.handle_emergency(uuid::Uuid::new_v4()) {
        Err(TrafficError::RoadNotFound { .. }) => {
            println!("\nUnknown road correctly rejected for emergency preemption")
        }
        other => panic!("expected RoadNotFound, got {other:?}"),
    }
}

fn main() {
    run_demo();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_cycle_wraps() {
        assert_eq!(Signal::Green.next(), Signal::Yellow);
        assert_eq!(Signal::Yellow.next(), Signal::Red);
        assert_eq!(Signal::Red.next(), Signal::Green);
    }

    #[test]
    fn test_run_cycles_ends_all_red() {
        let controller = TrafficController::new();
        controller.add_road(Road::new("A", TrafficLight::new(20, 3, 15)));
        controller.add_road(Road::new("B", TrafficLight::new(20, 3, 15)));
        controller.run_cycles(1);
        assert!(
            controller
                .signals()
                .iter()
                .all(|(_, signal)| *signal == Signal::Red)
        );
    }

    #[test]
    fn test_handle_emergency_unknown_road() {
        let controller = TrafficController::new();
        let err = controller.handle_emergency(uuid::Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, TrafficError::RoadNotFound { .. }));
    }

    /// Requirement #5, under concurrency: many threads race to preempt for
    /// different roads. Each `handle_emergency` sets its target green and
    /// every other road red inside one write-lock scope, so no matter how
    /// the threads interleave, the final state must have exactly one green
    /// road — never zero, never two.
    #[test]
    fn test_concurrent_emergencies_never_leave_two_roads_green() {
        let controller = Arc::new(TrafficController::new());
        let ids: Vec<uuid::Uuid> = (0..8)
            .map(|i| controller.add_road(Road::new(format!("Road {i}"), TrafficLight::new(20, 3, 15))))
            .collect();

        let handles: Vec<_> = ids
            .iter()
            .copied()
            .map(|id| {
                let controller = Arc::clone(&controller);
                std::thread::spawn(move || controller.handle_emergency(id))
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().expect("road exists");
        }

        let green_count = controller
            .signals()
            .iter()
            .filter(|(_, signal)| *signal == Signal::Green)
            .count();
        assert_eq!(green_count, 1, "exactly one road must be green after the race");
    }
}
