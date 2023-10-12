#![allow(unused)]
use std::{collections::HashMap, marker::PhantomData};
use strum_macros::Display;
use std::sync::Arc; 
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Display)]
pub enum VehicleType {
    Car,
    Truck,
    Electric,
    Van,
    Motorbike,
}

#[derive(Debug, Clone, Copy, Display)]
pub enum ParkingSpotType {
    Handicapped,
    Compact,
    Large,
    Motorbike,
    Electric,
}

#[derive(Debug, Display, Clone)]
enum AccountStatus {
    Active,
    Blocked,
    Banned,
    Compromised,
    Archived,
    Unknown,
}

#[derive(Debug, Display, Clone)]
enum ParkingTicketStatus {
    Active,
    Paid,
    Lost,
}

#[derive(Debug, Display, Clone)]
enum PaymentStatus {
    Unpaid,
    Pending,
    Completed,
    Filled,
    Declined,
    Cancelled,
    Abandoned,
    Settling,
    Settled,
    Refunded,
}

#[derive(Debug, Clone)]
struct Address {
    street_address: String,
    city: String,
    state: String,
    zip_code: String,
    country: String,
}

#[derive(Debug, Clone)]
struct Person {
    name: String,
    address: Address,
    email: String,
    phone: String,
}

/// Account, Admin, and ParkingAttendant:** These classes represent various people that interact with our system:
#[derive(Debug, Clone)]
struct Account {
    user_name: String,
    password: String,
    person: Person,
    status: AccountStatus,
}

impl Account {
    fn new(user_name: String, password: String, person: Person, status: AccountStatus) -> Self {
        Account {
            user_name,
            password,
            person,
            status,
        }
    }

    fn reset_password(&self) {
        // Implementation of resetting the password
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
struct Admin<S, T> {
    account: Account,
    _marker1: PhantomData<S>,
    _marker2: PhantomData<T>,
}

impl<S, T> Admin<S, T>
where
    S: ParkingSpot<T>,
    T: Vehicle,
{
    fn new(user_name: String, password: String, person: Person, status: AccountStatus) -> Self {
        Admin {
            account: Account::new(user_name, password, person, status),
            _marker1: PhantomData,
            _marker2: PhantomData,
        }
    }

    /// Implementation of adding a parking floor
    fn add_parking_floor(&self, floor: ParkingFloor<S, T>) -> bool {
        false
    }

    /// Implementation of adding a parking spot
    fn add_parking_spot(&self, floor_name: String, spot: S) -> bool {
        false
    }

    /// Implementation of adding a parking display board
    fn add_parking_display_board(
        &self,
        floor_name: String,
        display_board: ParkingDisplayBoard<S, T>,
    ) -> bool {
        false
    }

    /// Implementation of adding a customer info panel
    fn add_customer_info_panel(&self, floor_name: String, info_panel: CustomerInfoPanel) -> bool {
        false
    }

    /// Implementation of adding an entrance panel
    fn add_entrance_panel(&self, entrance_panel: EntrancePanel) -> bool {
        false
    }

    /// Implementation of adding an exit panel
    fn add_exit_panel(&self, exit_panel: ExitPanel) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct ParkingAttendant {
    account: Account,
}

impl ParkingAttendant {
    fn new(user_name: String, password: String, person: Person, status: AccountStatus) -> Self {
        ParkingAttendant {
            account: Account::new(user_name, password, person, status),
        }
    }

    fn process_ticket(&self, ticket_number: u32) -> bool {
        // Implementation of processing a ticket
        false
    }
}

/// Each parking floor will have a display board to show available parking spots for each spot type. 
/// This class will be responsible for displaying the latest availability of free parking spots to 
/// the customers.
#[derive(Debug, Clone)]
struct ParkingDisplayBoard<S, T> {
    id: u32,
    handicapped_free_spot: Option<S>,
    compact_free_spot: Option<S>,
    large_free_spot: Option<S>,
    motorbike_free_spot: Option<S>,
    electric_free_spot: Option<S>,
    // PhantomData is needed to perform type checking at compile time
    // These extra parameters hold no storage values, and have no runtime behavior.
    _marker: PhantomData<T>,
}

impl<S, T> ParkingDisplayBoard<S, T>
where
    S: ParkingSpot<T>,
    T: Vehicle,
{
    fn new(id: u32) -> Self {
        ParkingDisplayBoard {
            id,
            handicapped_free_spot: None,
            compact_free_spot: None,
            large_free_spot: None,
            motorbike_free_spot: None,
            electric_free_spot: None,
            _marker: PhantomData,
        }
    }
    pub fn get_compact_free_spot(&self) -> &S {
        unimplemented!()
    }

    pub fn set_compact_free_spot(&mut self, spot: &S) {
        unimplemented!()
    }

    pub fn get_handicapped_free_spot(&self) -> &S {
        unimplemented!()
    }

    pub fn set_handicapped_free_spot(&mut self, spot: &S) {
        unimplemented!()
    }

    fn show_empty_spot_number(&self) {
        let mut message = String::new();

        if let Some(handicapped_spot) = &self.handicapped_free_spot {
            if handicapped_spot.is_free() {
                message += &format!("Free Handicapped: {}\n", handicapped_spot.get_number());
            } else {
                message += "Handicapped is full\n";
            }
        }

        if let Some(compact_spot) = &self.compact_free_spot {
            if compact_spot.is_free() {
                message += &format!("Free Compact: {}\n", compact_spot.get_number());
            } else {
                message += "Compact is full\n";
            }
        }

        if let Some(large_spot) = &self.large_free_spot {
            if large_spot.is_free() {
                message += &format!("Free Large: {}\n", large_spot.get_number());
            } else {
                message += "Large is full\n";
            }
        }

        if let Some(motorbike_spot) = &self.motorbike_free_spot {
            if motorbike_spot.is_free() {
                message += &format!("Free Motorbike: {}\n", motorbike_spot.get_number());
            } else {
                message += "Motorbike is full\n";
            }
        }

        if let Some(electric_spot) = &self.electric_free_spot {
            if electric_spot.is_free() {
                message += &format!("Free Electric: {}\n", electric_spot.get_number());
            } else {
                message += "Electric is full\n";
            }
        }

        println!("{}", message);
    }
}

/// This class will encapsulate all the operations that an attendant can perform, like 
/// scanning tickets and processing payments.
#[derive(Debug, Clone)]
struct ParkingAttendantPanel {
    id: u32,
}

impl ParkingAttendantPanel {
    fn new(id: u32) -> Self {
        ParkingAttendantPanel { id }
    }

    fn process_payment(&self) -> bool {
        false
    }

    fn scan_ticket(&self) -> bool {
        false
    }
}

/// This class will encapsulate the info portal that customers use to pay for the parking 
/// ticket. Once paid, the info portal will update the ticket to keep track of the payment.
#[derive(Debug, Clone)]
struct CustomerInfoPanel {
    id: u32,
}

impl CustomerInfoPanel {
    fn new(id: u32) -> Self {
        CustomerInfoPanel { id }
    }

    fn process_payment(&self) -> bool {
        false
    }

    fn scan_ticket(&self) -> bool {
        false
    }
}

/// EntrancePanel will print tickets, and ExitPanel will facilitate payment of the ticket fee. 
#[derive(Debug, Clone)]
struct EntrancePanel {
    id: u32,
}

impl EntrancePanel {
    fn print_ticket(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct ExitPanel {
    id: u32,
}

impl ExitPanel {
    fn scan_ticket(&self) -> bool {
        false
    }

    fn process_payment(&self) -> bool {
        false
    }
}


/// Customers will use the electric panels to pay and charge their electric vehicles.
#[derive(Debug, Clone)]
struct ElectricPanel {
    pay_for_time: u32,
    charging_start_time: chrono::DateTime<chrono::Utc>,
}

impl ElectricPanel {
    fn new(&self, pay_for_time: u32, charging_start_time: chrono::DateTime<chrono::Utc>) -> Self {
        ElectricPanel { pay_for_time, charging_start_time }    
    }

    fn cancel_charging(&self) -> bool {
        false
    }
}

/// The parking lot will have many parking floors.
#[derive(Debug, Clone)]
struct ParkingFloor<S, T> {
    name: String,
    handicapped_spots: HashMap<u32, S>,
    compact_spots: HashMap<u32, S>,
    large_spots: HashMap<u32, S>,
    motorbike_spots: HashMap<u32, S>,
    electric_spots: HashMap<u32, S>,
    info_portals: HashMap<u32, CustomerInfoPanel>,
    free_handicapped_spot_count: u32,
    free_compact_spot_count: u32,
    free_large_spot_count: u32,
    free_motorbike_spot_count: u32,
    free_electric_spot_count: u32,
    display_board: ParkingDisplayBoard<S, T>,
    // parking_attendant_panel: ParkingAttendantPanel,
    _marker: PhantomData<T>,
}

impl<S, T> ParkingFloor<S, T>
where
    S: ParkingSpot<T>,
    T: Vehicle 
{
    fn new(name: String) -> Self {
        ParkingFloor {
            name,
            handicapped_spots: HashMap::new(),
            compact_spots: HashMap::new(),
            large_spots: HashMap::new(),
            motorbike_spots: HashMap::new(),
            electric_spots: HashMap::new(),
            info_portals: HashMap::new(),
            free_handicapped_spot_count: 0,
            free_compact_spot_count: 0,
            free_large_spot_count: 0,
            free_motorbike_spot_count: 0,
            free_electric_spot_count: 0,
            // Generate an id for parking display board
            display_board: ParkingDisplayBoard::new(0),
            // parking_attendant_panel: ParkingAttendantPanel::new(0),
            _marker: PhantomData,
        }
    }

    pub fn add_parking_spot(&mut self, spot: S) {
        match spot.get_type() {
            ParkingSpotType::Handicapped => {
                self.handicapped_spots.insert(spot.get_number(), spot);
            }
            ParkingSpotType::Compact => {
                self.compact_spots.insert(spot.get_number(), spot);
            }
            ParkingSpotType::Large => {
                self.large_spots.insert(spot.get_number(), spot);
            }
            ParkingSpotType::Motorbike => {
                self.motorbike_spots.insert(spot.get_number(), spot);
            }
            ParkingSpotType::Electric => {
                self.electric_spots.insert(spot.get_number(), spot);
            }
        }
    }

    pub fn assign_vehicle_to_spot(&mut self, vehicle: T, spot: &mut S) {
        spot.assign_vehicle(vehicle);
        match spot.get_type() {
            ParkingSpotType::Handicapped => {
                self.update_display_board_for_handicapped(spot);
            }
            ParkingSpotType::Compact => {
                self.update_display_board_for_compact(spot);
            }
            ParkingSpotType::Large => {
                self.update_display_board_for_large(spot);
            }
            ParkingSpotType::Motorbike => {
                self.update_display_board_for_motorbike(spot);
            }
            ParkingSpotType::Electric => {
                self.update_display_board_for_electric(spot);
            }
        }
    }

    fn update_display_board_for_large(&mut self, spot: &S) {
        unimplemented!()
    }

    fn update_display_board_for_motorbike(&mut self, spot: &S) {
        unimplemented!()
    }

    fn update_display_board_for_electric(&mut self, spot: &S) {
        unimplemented!()
    }

    fn update_display_board_for_handicapped(&mut self, spot: &S) {
        if self.display_board.get_handicapped_free_spot().get_number() == spot.get_number() {
            // find another free handicapped parking and assign to display_board
            for (_, value) in &self.handicapped_spots {
                if value.is_free() {
                    self.display_board.set_handicapped_free_spot(value);
                    break;
                }
            }

            self.display_board.show_empty_spot_number();
        }
    }

    fn update_display_board_for_compact(&mut self, spot: &S) {
        if self.display_board.get_compact_free_spot().get_number() == spot.get_number() {
            // find another free compact parking and assign to display_board
            for (_, value) in &self.compact_spots {
                if value.is_free() {
                    self.display_board.set_compact_free_spot(value);
                    break;
                }
            }

            self.display_board.show_empty_spot_number();
        }
    }

    fn free_spot(&mut self, spot: &mut S) {
        spot.remove_vehicle();
        match spot.get_type() {
            ParkingSpotType::Handicapped => {
                self.free_handicapped_spot_count += 1;
            }
            ParkingSpotType::Compact => {
                self.free_compact_spot_count += 1;
            }
            ParkingSpotType::Large => {
                self.free_large_spot_count += 1;
            }
            ParkingSpotType::Motorbike => {
                self.free_motorbike_spot_count += 1;
            }
            ParkingSpotType::Electric => {
                self.free_electric_spot_count += 1;
            }
        }
    }
}

/// Vehicles will be parked in the parking spots. Our system will support different types of vehicles 
/// 1) Car, 2) Truck, 3) Electric, 4) Van and 5) Motorcycle. 
pub trait Vehicle {
    fn get_license_number(&self) -> &str;

    fn get_type(&self) -> VehicleType;

    fn get_ticket(&self) -> Option<&ParkingTicket>;

    fn assign_ticket(&mut self, ticket: ParkingTicket);
}

#[derive(Debug)]
pub struct Car {
    license_number: String,
    ticket: Option<ParkingTicket>,
}

impl Car {
    pub fn new(license_number: String, ticket: Option<ParkingTicket>) -> Self {
        Car {
            license_number,
            ticket,
        }
    }
}

impl Vehicle for Car {
    fn get_license_number(&self) -> &str {
        &self.license_number
    }

    fn get_type(&self) -> VehicleType {
        VehicleType::Car
    }

    fn get_ticket(&self) -> Option<&ParkingTicket> {
        self.ticket.as_ref()
    }

    fn assign_ticket(&mut self, ticket: ParkingTicket) {
        self.ticket = Some(ticket);
    }
}

#[derive(Debug)]
pub struct Van {
    license_number: String,
    ticket: Option<ParkingTicket>,
}

impl Van {
    pub fn new(license_number: String, ticket: Option<ParkingTicket>) -> Self {
        Van {
            license_number,
            ticket,
        }
    }
}

impl Vehicle for Van {
    fn get_license_number(&self) -> &str {
        &self.license_number
    }

    fn get_type(&self) -> VehicleType {
        VehicleType::Van
    }

    fn get_ticket(&self) -> Option<&ParkingTicket> {
        self.ticket.as_ref()
    }

    fn assign_ticket(&mut self, ticket: ParkingTicket) {
        self.ticket = Some(ticket);
    }
}

#[derive(Debug)]
pub struct Truck {
    license_number: String,
    ticket: Option<ParkingTicket>,
}

impl Truck {
    pub fn new(license_number: String, ticket: Option<ParkingTicket>) -> Self {
        Truck {
            license_number,
            ticket,
        }
    }
}

impl Vehicle for Truck {
    fn get_license_number(&self) -> &str {
        &self.license_number
    }

    fn get_type(&self) -> VehicleType {
        VehicleType::Truck
    }

    fn get_ticket(&self) -> Option<&ParkingTicket> {
        self.ticket.as_ref()
    }

    fn assign_ticket(&mut self, ticket: ParkingTicket) {
        self.ticket = Some(ticket);
    }
}

#[derive(Debug, Clone)]
pub struct ParkingTicket {
    // Ticket details
    ticket_number: String,
    issued_at: chrono::DateTime<chrono::Utc>,
    payed_at: chrono::DateTime<chrono::Utc>,
    payed_amount: f64,
    status: ParkingTicketStatus,
}

impl ParkingTicket {
    fn new(payed_amout: f64) -> Self {
        Self {
            ticket_number: String::new(),
            issued_at: chrono::Utc::now(),
            payed_at: chrono::Utc::now(),
            payed_amount: payed_amout,
            status: ParkingTicketStatus::Active
        }
    }

    fn save_in_db(&self) {
        unimplemented!()
    }

    fn get_ticket_number(&self) -> String {
        unimplemented!()
    }
}

/// Each parking floor will have many parking spots. Our system will support different parking spots 
/// 1) Handicapped, 2) Compact, 3) Large, 4) Motorcycle, and 5) Electric.
pub trait ParkingSpot<T> {
    /// Implementation of checking if the spot is free
    fn is_free(&self) -> bool;

    fn assign_vehicle(&mut self, vehicle: T);

    fn remove_vehicle(&mut self);

    fn get_type(&self) -> ParkingSpotType;

    fn get_number(&self) -> u32;
}

#[derive(Debug, Clone)]
pub struct HandicappedSpot<T: Vehicle> {
    number: u32,
    free: bool,
    vehicle: Option<T>,
}

impl<T: Vehicle> HandicappedSpot<T> {
    pub fn new(number: u32) -> Self {
        HandicappedSpot {
            number,
            free: true,
            vehicle: None,
        }
    }
}

impl<T: Vehicle> ParkingSpot<T> for HandicappedSpot<T> {
    fn get_number(&self) -> u32 {
        self.number
    }

    fn is_free(&self) -> bool {
        self.free
    }

    fn assign_vehicle(&mut self, vehicle: T) {
        self.vehicle = Some(vehicle);
        self.free = false;
    }

    fn remove_vehicle(&mut self) {
        self.vehicle = None;
        self.free = true;
    }

    fn get_type(&self) -> ParkingSpotType {
        ParkingSpotType::Handicapped
    }
}

#[derive(Debug, Clone)]
pub struct CompactSpot<T> {
    number: u32,
    free: bool,
    vehicle: Option<T>,
}

impl<T: Vehicle> CompactSpot<T> {
    pub fn new(number: u32) -> Self {
        CompactSpot {
            number,
            free: true,
            vehicle: None,
        }
    }
}

impl<T: Vehicle> ParkingSpot<T> for CompactSpot<T> {
    fn get_number(&self) -> u32 {
        self.number
    }

    fn is_free(&self) -> bool {
        self.free
    }

    fn assign_vehicle(&mut self, vehicle: T) {
        self.vehicle = Some(vehicle);
        self.free = false;
    }

    fn remove_vehicle(&mut self) {
        self.vehicle = None;
        self.free = true;
    }

    fn get_type(&self) -> ParkingSpotType {
        ParkingSpotType::Compact
    }
}

#[derive(Debug, Clone)]
pub struct LargeSpot<T> {
    number: u32,
    free: bool,
    vehicle: Option<T>,
}

impl<T: Vehicle> LargeSpot<T> {
    pub fn new(number: u32) -> Self {
        LargeSpot {
            number,
            free: true,
            vehicle: None,
        }
    }
}

impl<T: Vehicle> ParkingSpot<T> for LargeSpot<T> {
    fn get_number(&self) -> u32 {
        self.number
    }

    fn is_free(&self) -> bool {
        self.free
    }

    fn assign_vehicle(&mut self, vehicle: T) {
        self.vehicle = Some(vehicle);
        self.free = false;
    }

    fn remove_vehicle(&mut self) {
        self.vehicle = None;
        self.free = true;
    }

    fn get_type(&self) -> ParkingSpotType {
        ParkingSpotType::Large
    }
}

#[derive(Debug, Clone)]
pub struct MotorbikeSpot<T> {
    number: u32,
    free: bool,
    vehicle: Option<T>,
}

impl<T: Vehicle> MotorbikeSpot<T> {
    pub fn new(number: u32) -> Self {
        MotorbikeSpot {
            number,
            free: true,
            vehicle: None,
        }
    }
}

impl<T: Vehicle> ParkingSpot<T> for MotorbikeSpot<T> {
    fn get_number(&self) -> u32 {
        self.number
    }

    fn is_free(&self) -> bool {
        self.free
    }

    fn assign_vehicle(&mut self, vehicle: T) {
        self.vehicle = Some(vehicle);
        self.free = false;
    }

    fn remove_vehicle(&mut self) {
        self.vehicle = None;
        self.free = true;
    }

    fn get_type(&self) -> ParkingSpotType {
        ParkingSpotType::Motorbike
    }
}

#[derive(Debug, Clone)]
pub struct ElectricSpot<T: Vehicle> {
    number: u32,
    free: bool,
    vehicle: Option<T>,
}

impl<T: Vehicle> ElectricSpot<T> {
    pub fn new(number: u32) -> Self {
        ElectricSpot {
            number,
            free: true,
            vehicle: None,
        }
    }
}

impl<T: Vehicle> ParkingSpot<T> for ElectricSpot<T> {
    fn get_number(&self) -> u32 {
        self.number
    }

    fn is_free(&self) -> bool {
        self.free
    }

    fn assign_vehicle(&mut self, vehicle: T) {
        self.vehicle = Some(vehicle);
        self.free = false;
    }

    fn remove_vehicle(&mut self) {
        self.vehicle = None;
        self.free = true;
    }

    fn get_type(&self) -> ParkingSpotType {
        ParkingSpotType::Electric
    }
}


/// This class will keep track of the hourly parking rates. It will specify a dollar amount for each hour. 
/// For example, for a two hour parking ticket, this class will define the cost for the first and the second hour.
#[derive(Debug, Clone, Default)]
struct ParkingRate {
    hour_number: u32,
    rate: u32,
}

impl ParkingRate {
    fn new(hour_number: u32, rate: u32) -> Self {
        ParkingRate {hour_number, rate}
    }
}

/// The central part of the organization for which this software has been designed. It has attributes 
/// like 'Name' to distinguish it from any other parking lots and ‘Address’ to define its location
#[derive(Debug, Clone)]
struct ParkingLot<S, T> {
    name: String,
    address: Address,
    parking_rate: ParkingRate,
    compact_spot_count: i32,
    large_spot_count: i32,
    motorbike_spot_count: i32,
    electric_spot_count: i32,
    max_compact_count: i32,
    max_large_count: i32,
    max_motorbike_count: i32,
    max_electric_count: i32,
    entrance_panels: HashMap<String, EntrancePanel>,
    exit_panels: HashMap<String, ExitPanel>,
    parking_floors: HashMap<String, ParkingFloor<S, T>>,
    active_tickets: HashMap<String, ParkingTicket>,
    lock: Arc<Mutex<()>>,
}

impl<S, T> ParkingLot<S, T> 
where 
    S: ParkingSpot<T>, T: Vehicle 
{
    fn new(name: String, address: Address) -> Self {
        ParkingLot {
            name,
            address,
            parking_rate: ParkingRate::default(),
            compact_spot_count: 0,
            large_spot_count: 0,
            motorbike_spot_count: 0,
            electric_spot_count: 0,
            max_compact_count: 0,
            max_large_count: 0,
            max_motorbike_count: 0,
            max_electric_count: 0,
            entrance_panels: HashMap::new(),
            exit_panels: HashMap::new(),
            parking_floors: HashMap::new(),
            active_tickets: HashMap::new(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn add_parking_floor(&mut self, parking_floor: ParkingFloor<S, T>) -> bool {
        false
    }

    pub fn add_entrance_panel(&mut self, entrance_panel: EntrancePanel) -> bool {
        false
    }

    fn get_new_parking_ticket<V: Vehicle>(&mut self, mut vehicle: V) -> Result<ParkingTicket, String> {
        if self.is_full(vehicle.get_type()) {
            return Err(String::from("Parking full!"));
        }
        // let _lock = self.lock.lock().unwrap();
        let ticket = ParkingTicket::new(0.0);
        // vehicle.assign_ticket(ticket.clone());
        // ticket.save_in_db(); // Assuming this function exists
        // self.increment_spot_count(vehicle.get_type());
        // self.active_tickets
        //     .insert(ticket.get_ticket_number(), ticket);
        Ok(ticket)
    }

    fn is_full(&self, vehicle_type: VehicleType) -> bool {
        match vehicle_type {
            VehicleType::Truck | VehicleType::Van => self.large_spot_count >= self.max_large_count,
            VehicleType::Motorbike => self.motorbike_spot_count >= self.max_motorbike_count,
            VehicleType::Car => {
                self.compact_spot_count + self.large_spot_count
                    >= self.max_compact_count + self.max_large_count
            }
            VehicleType::Electric => {
                self.compact_spot_count + self.large_spot_count + self.electric_spot_count
                    >= self.max_compact_count + self.max_large_count + self.max_electric_count
            }
        }
    }

    fn increment_spot_count(&mut self, vehicle_type: VehicleType) {
        match vehicle_type {
            VehicleType::Truck | VehicleType::Van => self.large_spot_count += 1,
            VehicleType::Motorbike => self.motorbike_spot_count += 1,
            VehicleType::Car => {
                if self.compact_spot_count < self.max_compact_count {
                    self.compact_spot_count += 1;
                } else {
                    self.large_spot_count += 1;
                }
            }
            VehicleType::Electric => {
                if self.electric_spot_count < self.max_electric_count {
                    self.electric_spot_count += 1;
                } else if self.compact_spot_count < self.max_compact_count {
                    self.compact_spot_count += 1;
                } else {
                    self.large_spot_count += 1;
                }
            }
        }
    }

}

/// This class will be responsible for making payments. The system will support credit card and cash transactions.
trait Payment {
    fn create_transaction(&self) -> bool;
}

struct CreditCardTransaction {
    id: u32,
    amount: f32,
    status: PaymentStatus,
    creation_date: chrono::NaiveDateTime,
    card_number: String,
    expiration_date: String,
    cvv: String,    
}

struct CashTransaction {
    id: u32,
    amount: f32,
    status: PaymentStatus,
    creation_date: chrono::NaiveDateTime,
}

impl Payment for CreditCardTransaction {
    fn create_transaction(&self) -> bool {
        todo!()
    }
}

impl Payment for CashTransaction {
    fn create_transaction(&self) -> bool {
        todo!()
    }
}

fn main() {
    let person = Person {
        name: String::from("John Doe"),
        address: Address {
            street_address: String::from("123 Main St"),
            city: String::from("City"),
            state: String::from("State"),
            zip_code: String::from("12345"),
            country: String::from("Country"),
        },
        email: String::from("john.doe@example.com"),
        phone: String::from("123-456-7890"),
    };

    // let admin = Admin::new(String::from("admin"), String::from("password"), person.clone(), AccountStatus::Active);
    // let parking_attendant = ParkingAttendant::new(String::from("attendant"), String::from("password"), person.clone(), AccountStatus::Active);
}
