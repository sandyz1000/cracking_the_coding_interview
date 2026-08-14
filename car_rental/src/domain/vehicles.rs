use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::locks::{rd, wr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VehicleStatus {
    Available,
    Reserved,
    Loaned,
    Lost,
    BeingServiced,
    Other,
}

/// Constructor input for `Vehicle::new`.
#[derive(Clone, Debug)]
pub struct VehicleSpec {
    pub barcode: String,
    pub license_number: String,
    pub stock_number: String,
    pub capacity: u32,
    pub make: String,
    pub model: String,
    pub year: u32,
    pub mileage: u32,
    pub price_per_day: u32,
}

/// `price_per_day` is in cents. `status` is interior-mutable because one
/// Vehicle is shared across search, reservation, and ops flows.
pub struct Vehicle {
    pub barcode: String,
    pub license_number: String,
    pub stock_number: String,
    pub capacity: u32,
    pub make: String,
    pub model: String,
    pub year: u32,
    pub mileage: u32,
    pub price_per_day: u32,
    status: RwLock<VehicleStatus>,
}

impl Vehicle {
    pub fn new(spec: VehicleSpec) -> Self {
        Self {
            barcode: spec.barcode,
            license_number: spec.license_number,
            stock_number: spec.stock_number,
            capacity: spec.capacity,
            make: spec.make,
            model: spec.model,
            year: spec.year,
            mileage: spec.mileage,
            price_per_day: spec.price_per_day,
            status: RwLock::new(VehicleStatus::Available),
        }
    }

    pub fn status(&self) -> VehicleStatus {
        *rd(&self.status)
    }

    pub fn set_status(&self, status: VehicleStatus) {
        *wr(&self.status) = status;
    }

    pub fn snapshot(&self) -> VehicleSnapshot {
        VehicleSnapshot {
            barcode: self.barcode.clone(),
            license_number: self.license_number.clone(),
            stock_number: self.stock_number.clone(),
            capacity: self.capacity,
            make: self.make.clone(),
            model: self.model.clone(),
            year: self.year,
            mileage: self.mileage,
            price_per_day: self.price_per_day,
            status: self.status(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VehicleSnapshot {
    pub barcode: String,
    pub license_number: String,
    pub stock_number: String,
    pub capacity: u32,
    pub make: String,
    pub model: String,
    pub year: u32,
    pub mileage: u32,
    pub price_per_day: u32,
    pub status: VehicleStatus,
}

pub type VehicleRegistry = RwLock<HashMap<String, Arc<Vehicle>>>;
