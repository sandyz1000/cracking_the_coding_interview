#![allow(unused)]
use std::fmt;

use strum_macros::Display;

#[derive(Debug, Display)]
enum ReservationStatus {
    Requested,
    Pending,
    Confirmed,
    CheckedIn,
    Canceled,
    Abandoned,
}

#[derive(Debug, Display, Clone)]
enum SeatType {
    Regular,
    Kid,
    Accessible,
    Other,
}

#[derive(Debug, Display, Clone)]
enum OrderStatus {
    Received,
    Preparing,
    Completed,
    Canceled,
    None,
}

#[derive(Debug, Display, Clone)]
enum TableStatus {
    Free,
    Reserved,
    Occupied,
    Other,
}


#[derive(Debug, Display)]
enum AccountStatus {
    Active,
    Closed,
    Canceled,
    Blacklisted,
    Blocked,
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

#[derive(Debug)]
struct Address {
    street_address: String,
    city: String,
    state: String,
    zip_code: String,
    country: String,
}

impl Address {
    fn new(street: String, city: String, state: String, zip_code: String, country: String) -> Self {
        Address {
            street_address: street,
            city,
            state,
            zip_code,
            country,
        }
    }
}

use std::fmt::Display;

#[derive(Debug)]
struct Account {
    id: String,
    password: String,
    address: Address,
    status: AccountStatus,
}

impl Account {
    fn new(id: String, password: String, address: Address, status: AccountStatus) -> Self {
        Account {
            id,
            password,
            address,
            status,
        }
    }

    fn reset_password(&self) {
        // Implementation goes here
    }
}

// Persons
trait Employee {
    fn employee_id(&self) -> &str;
    fn date_joined(&self) -> &str;
    fn account(&self) -> &Account;
}

/// Receptionist create a table reservation for a certain date/time for an available table
#[derive(Debug)]
struct Receptionist {
    id: String,
    account: Account,
    name: String,
    email: String,
    phone: String,
    employee_id: String,
    date_joined: chrono::DateTime<chrono::Utc>,
}

impl Employee for Receptionist {
    fn employee_id(&self) -> &str {
        &self.id
    }

    fn date_joined(&self) -> &str {
        // Implementation goes here
        unimplemented!()
    }

    fn account(&self) -> &Account {
        &self.account
    }
}

impl Receptionist {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Receptionist {
            id,
            account,
            name,
            email,
            phone,
            employee_id: 0.to_string(),  // Generate a employee id from the system
            date_joined: chrono::Utc::now(),
        }
    }

    /// To create a table reservation for a certain date/time for an available table
    fn create_reservation(&mut self) -> bool {
        // Implementation goes here
        false
    }

    fn cancel_reservation(&mut self) -> bool {
        // Implementation goes here
        false
    }

    fn search_customer(&self, name: &str) {
        // Implementation goes here
    }
}

#[derive(Debug)]
struct Manager {
    id: String,
    account: Account,
    name: String,
    email: String,
    phone: String,
    employee_id: String,
    date_joined: chrono::DateTime<chrono::Utc>,
}

impl Employee for Manager {
    fn employee_id(&self) -> &str {
        &self.id
    }

    fn date_joined(&self) -> &str {
        // Implementation goes here
        unimplemented!()
    }

    fn account(&self) -> &Account {
        &self.account
    }
}

impl Manager {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Manager {
            id,
            account,
            name,
            email,
            phone,
            employee_id: 0.to_string(),  // Generate a employee id from the system
            date_joined: chrono::Utc::now(),
        }
    }

    fn add_employee(&self) -> bool {
        // Implementation goes here
        false
    }
}

/// Chef works in kitchen & prepare Orders 
#[derive(Debug)]
struct Chef {
    id: String,
    account: Account,
    name: String,
    email: String,
    phone: String,
    employee_id: String,
    date_joined: chrono::DateTime<chrono::Utc>,
}

impl Employee for Chef {
    fn employee_id(&self) -> &str {
        &self.id
    }

    fn date_joined(&self) -> &str {
        // Implementation goes here
        unimplemented!()
    }

    fn account(&self) -> &Account {
        &self.account
    }
}

impl Chef {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Chef {
            id,
            account,
            name,
            email,
            phone,
            employee_id: 0.to_string(),  // Generate a employee id from the system
            date_joined: chrono::Utc::now(),
        }
    }

    fn take_order(&self) {
        // Implementation goes here
    }
}

#[derive(Debug)]
struct Waiter {
    id: String,
    account: Account,
    name: String,
    email: String,
    phone: String,
    employee_id: String,
    date_joined: chrono::DateTime<chrono::Utc>,
}

impl Waiter {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Waiter {
            id,
            account,
            name,
            email,
            phone,
            employee_id: 0.to_string(),  // Generate a employee id from the system
            date_joined: chrono::Utc::now(),
        }
    }

    fn create_order(&mut self) -> bool {
        // Implementation goes here
        false
    }
}

impl Employee for Waiter {
    fn employee_id(&self) -> &str {
        &self.id
    }

    fn date_joined(&self) -> &str {
        todo!()
    }

    fn account(&self) -> &Account {
        todo!()
    }
}

#[derive(Debug)]
struct Customer {
    id: String,
    name: String,
    email: String,
    phone: String,
    last_visited: chrono::DateTime<chrono::Utc>,
}

impl Customer {
    fn new(id: String, name: String, email: String, phone: String) -> Self {
        Customer {
            id,
            name,
            email,
            phone,
            last_visited: chrono::Utc::now(),
        }
    }
}


/// The basic building block of the system. Every table will have a unique identifier, maximum sitting 
/// capacity, etc. Each table will have multiple seats.
#[derive(Debug, Clone)]
struct Table {
    table_id: String,
    max_capacity: u32,
    location_identifier: String,
    status: TableStatus,
    seats: Vec<TableSeat>,
}

impl Table {
    fn new(table_id: String, max_capacity: u32, location_identifier: String, status: TableStatus) -> Self {
        Table {
            table_id,
            max_capacity,
            location_identifier,
            status,
            seats: Vec::new(),
        }
    }

    fn is_table_free(&self) -> bool {
        // Implementation goes here
        false
    }

    /// To create a table reservation for a certain date/time for an available table.
    fn add_reservation(&mut self) -> bool {
        // Implementation goes here
        false
    }

    ///  To search for available tables for reservation.
    fn search(&self, capacity: u32, start_time: chrono::DateTime<chrono::Utc>) -> Vec<&Table> {
        // Implementation goes here
        Vec::new()
    }
}

#[derive(Debug, Clone)]
struct TableSeat {
    table_seat_number: u32,
    seat_type: SeatType,
}

impl TableSeat {
    fn new() -> Self {
        TableSeat {
            table_seat_number: 0,
            seat_type: SeatType::Regular,
        }
    }
    
    fn update_seat_type(&mut self, seat_type: SeatType) {
        // Implementation goes here
    }
}

/// Will take care of sending notifications to customers.
trait Notification: std::fmt::Debug {
    /// The system should be able to send notifications whenever the reservation time is approaching. 
    fn send_notification(&self) -> bool;
}

#[derive(Debug)]
struct EmailNotification {
    email: String,
}

impl Notification for EmailNotification {
    fn send_notification(&self) -> bool {
        // Implementation goes here
        false
    }
}

#[derive(Debug)]
struct SMSNotification {
    phone: String,
}

impl Notification for SMSNotification {
    fn send_notification(&self) -> bool {
        // Implementation goes here
        false
    }
}

/// Create a reservation for Table
#[derive(Debug)]
struct Reservation {
    reservation_id: String,
    time_of_reservation: chrono::DateTime<chrono::Utc>,
    people_count: u32,
    status: ReservationStatus,
    notes: String,
    checkin_time: chrono::DateTime<chrono::Utc>,
    // It should have one customer
    customer: Customer,
    // A customer can book multiple tables
    tables: Vec<Table>,
    notifications: Vec<Box<dyn Notification>>,
}

impl Reservation {
    fn new(
        reservation_id: String,
        people_count: u32,
        notes: String,
        customer: Customer,
    ) -> Self {
        Reservation {
            reservation_id,
            time_of_reservation: chrono::Utc::now(),
            people_count,
            status: ReservationStatus::Requested,
            notes,
            checkin_time: chrono::Utc::now(),
            customer,
            tables: Vec::new(),
            notifications: Vec::new(),
        }
    }

    fn add_table(&mut self, table: &Table) -> bool {
        // Implementation goes here
        self.tables.push((*table).clone());
        true
    }

    fn update_people_count(&mut self, count: u32) -> bool {
        // Implementation goes here
        self.people_count = count;
        false
    }

}

struct Kitchen {
    name: String,
    chefs: Vec<Chef>,
}

impl Kitchen {
    fn new(name: String) -> Self {
        Kitchen {
            name,
            chefs: Vec::new(),
        }
    }

    fn assign_chef(&mut self, chef: Chef) {
        // Implementation goes here
    }
}

/// Any restaurants can have multiple branches. Each branch will have its own set of employees and menus.
struct Branch {
    name: String,
    location: String,
    kitchen: Kitchen,
}

impl Branch {
    fn new(name: String, location: String, kitchen: Kitchen) -> Self {
        Branch {
            name,
            location,
            kitchen,
        }
    }

    fn add_table_chart(&self, table_chart: &TableChart) -> bool {
        // Implementation goes here
        false
    }
}


/// This class represents a restaurant. Each restaurant has registered employees. The employees are part 
/// of the restaurant because if the restaurant becomes inactive, all its employees will automatically be deactivated.
struct Restaurant {
    name: String,
    branches: Vec<Branch>,
}

impl Restaurant {
    fn new(name: String) -> Self {
        Restaurant {
            name,
            branches: Vec::new(),
        }
    }

    fn add_branch(&mut self, branch: Branch) {
        // Implementation goes here
    }
}

struct TableChart {
    table_chart_id: String,
    table_chart_image: Vec<u8>,
}

impl TableChart {
    fn new(id: String) -> Self {
        TableChart {
            table_chart_id: id,
            table_chart_image: Vec::new(),
        }
    }

    fn print(&self) -> bool {
        // Implementation goes here
        false
    }
}

struct MenuItem {
    menu_item_id: String,
    title: String,
    description: String,
    price: f32,
}

impl MenuItem {
    fn new(id: String, title: String, description: String, price: f32) -> Self {
        MenuItem {
            menu_item_id: id,
            title,
            description,
            price,
        }
    }

    fn update_price(&mut self, price: f32) -> bool {
        // Implementation goes here
        false
    }
}

struct MenuSection {
    menu_section_id: String,
    title: String,
    description: String,
    menu_items: Vec<MenuItem>,
}

impl MenuSection {
    fn new(id: String, title: String, description: String) -> Self {
        MenuSection {
            menu_section_id: id,
            title,
            description,
            menu_items: Vec::new(),
        }
    }

    fn add_menu_item(&mut self, menu_item: MenuItem) {
        // Implementation goes here
    }
}

struct Menu {
    menu_id: String,
    title: String,
    description: String,
    menu_sections: Vec<MenuSection>,
}

impl Menu {
    fn new(id: String, title: String, description: String) -> Self {
        Menu {
            menu_id: id,
            title,
            description,
            menu_sections: Vec::new(),
        }
    }

    fn add_menu_section(&mut self, menu_section: MenuSection) {
        // Implementation goes here
        unimplemented!()
    }

    fn print(&self) {
        // Implementation goes here
        unimplemented!()
    }
}

/// Each Meal will consist of one or more meal items corresponding to a menu item.
struct MealItem {
    meal_item_id: String,
    quantity: u32,
    menu_item: MenuItem,
}

impl MealItem {
    fn new(meal_item_id: String, quantity: u32, menu_item: MenuItem) -> Self {
        MealItem {
            meal_item_id,
            quantity,
            menu_item,
        }
    }

    fn update_quantity(&mut self, quantity: u32) -> bool {
        // Implementation goes here
        false
    }
}

struct Seat;

/// Each order will consist of separate meals for each table seat.
struct Meal {
    meal_id: String,
    seat: Seat,
    meal_items: Vec<MealItem>,
}

impl Meal {
    fn new(meal_id: String, seat: Seat) -> Self {
        Meal {
            meal_id,
            seat,
            meal_items: Vec::new(),
        }
    }

    fn add_meal_item(&mut self, meal_item: MealItem) -> bool {
        // Implementation goes here
        false
    }
}

/// Create order for table 
struct Order {
    order_id: String,
    status: OrderStatus,
    creation_time: chrono::DateTime<chrono::Utc>,
    meals: Vec<Meal>,
    table: Table,
    waiter: Waiter,
    chef: Chef,
    check: Check,
}

impl Order {
    fn new(
        order_id: String,
        status: OrderStatus,
        table: Table,
        waiter: Waiter,
        chef: Chef,
    ) -> Self {
        Order {
            order_id,
            status,
            creation_time: chrono::Utc::now(),
            meals: Vec::new(),
            table,
            waiter,
            chef,
            check: Check {},
        }
    }

    fn add_meal(&mut self, meal: Meal) -> bool {
        // Implementation goes here
        false
    }

    fn remove_meal(&mut self, meal: Meal) -> bool {
        // Implementation goes here
        false
    }

    fn get_status(&self) -> OrderStatus {
        self.status.clone()
    }

    fn set_status(&mut self, status: OrderStatus) -> OrderStatus {
        // Implementation goes here
        OrderStatus::Completed
    }
}

// Payment 

#[derive(Debug, Clone)]
struct Payment {
    payment_id: String,
    amount: f32,
    status: PaymentStatus,
    creation_time: chrono::DateTime<chrono::Utc>,
}

impl Payment {
    fn new(amount: f32) -> Self {
        Payment {
            payment_id: String::new(),
            amount,
            status: PaymentStatus::Filled,
            creation_time: chrono::Utc::now(),
        }
    }

    fn initiate_transaction(&self) -> bool {
        // Implementation goes here
        false
    }
}

#[derive(Debug, Clone)]
struct Check {
    payment: Payment,
    check_number: String,
    bank_name: String,
}

#[derive(Debug, Clone)]
struct CashTransaction {
    payment: Payment,
    cash_tendered: f32,  // Sum of money given by customer
}

#[derive(Debug, Clone)]
struct CreditCardTransaction {
    payment: Payment,
    credit_card_number: String,
    credit_card_type: String,
    credit_card_expiration_date: String,
    credit_card_security_code: String,
    credit_card_holder_name: String,
}

fn main() {
    // Activity:
    // Place order: Any waiter can perform this activity. Here are the steps to place an order:

    // Make a reservation: Any receptionist can perform this activity. Here are the steps to make a reservation:

    // Cancel a reservation: Any receptionist can perform this activity. Here are the steps to cancel a reservation:
    println!("Hello, world!");
}