//! # Designing Restaurant Management System
//!
//! ## Requirements
//! 1. The restaurant management system should allow customers to place orders, view the menu, and make reservations.
//! 2. The system should manage the restaurant's inventory, including ingredients and menu items.
//! 3. The system should handle order processing, including order preparation, billing, and payment.
//! 4. The system should support multiple payment methods, such as cash, credit card, and mobile payments.
//! 5. The system should manage staff information, including roles, schedules, and performance tracking.
//! 6. The system should generate reports and analytics for management, such as sales reports and inventory analysis.
//! 7. The system should handle concurrent access and ensure data consistency.
//!
//!
//! ## Classes, Interfaces and Enumerations
//! 1. The **MenuItem**  class represents a menu item in the restaurant, with properties such as ID, name, 
//! description, price, and availability.
//! 2. The **Order**  class represents an order placed by a customer, with properties such as ID, list of 
//! menu items, total amount, order status, and timestamp.
//! 3. The **OrderStatus**  enum represents the different statuses an order can have, such as pending, 
//! preparing, ready, completed, or cancelled.
//! 4. The **Reservation**  class represents a reservation made by a customer, with properties such as ID, 
//! customer name, contact number, party size, and reservation time.
//! 5. The **Payment**  class represents a payment made for an order, with properties such as ID, amount, 
//! payment method, and payment status.
//! 6. The **PaymentMethod**  enum represents the different payment methods supported by the restaurant, 
//! such as cash, credit card, or mobile payment.
//! 7. The **PaymentStatus**  enum represents the status of a payment, which can be pending, completed, or 
//! failed.
//! 8. The Staff class represents a staff member of the restaurant, with properties such as ID, name, role, 
//! and contact number.
//! 9. The **Restaurant**  class is the main class that manages the restaurant operations. It follows the 
//! Singleton pattern to ensure only one instance of the restaurant exists.
//! 10. The Restaurant class provides methods for managing menu items, placing orders, updating order status, 
//! making reservations, processing payments, and managing staff.
//! 11. Multi-threading is implemented using concurrent data structures (ConcurrentHashMap and CopyOnWriteArrayList) 
//! to handle concurrent access to shared data, such as orders and reservations.
//! 12. The notifyKitchen and notifyStaff methods are placeholders for notifying relevant staff about order 
//! updates and status changes.
//! 13. The **RestaurantManagementDemo**  class demonstrates the usage of the restaurant management system by 
//! adding menu items, placing an order, making a reservation, processing a payment, updating order status, 
//! adding staff, and retrieving the menu.

//! Restaurant Management System – see `readme.md` for the full design
//! (requirements, class diagram, activity diagrams). Implements the core
//! flows: menu, tables, reservations, orders, billing, payments, staff.

use chrono::prelude::*;
use std::collections::HashMap;
use std::fmt;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use strum_macros::Display;

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum ReservationStatus {
    Requested,
    Pending,
    Confirmed,
    CheckedIn,
    Canceled,
    Abandoned,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum SeatType {
    Regular,
    Kid,
    Accessible,
    Other,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum OrderStatus {
    Received,
    Preparing,
    Completed,
    Canceled,
    None,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum TableStatus {
    Free,
    Reserved,
    Occupied,
    Other,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum AccountStatus {
    Active,
    Closed,
    Canceled,
    Blacklisted,
    Blocked,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
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

#[derive(Debug, Display, Clone, PartialEq, Eq)]
enum PaymentMethod {
    CreditCard,
    Check,
    Cash,
    MobilePayment,
}

#[derive(Debug, Clone)]
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

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}, {} {}",
            self.street_address, self.city, self.state, self.zip_code, self.country
        )
    }
}

#[derive(Debug, Clone)]
struct Person {
    name: String,
    email: String,
    phone: String,
}

impl Person {
    fn new(name: String, email: String, phone: String) -> Self {
        Person { name, email, phone }
    }
}

#[derive(Debug, Clone)]
struct Customer {
    person: Person,
    address: Address,
}

impl Customer {
    fn new(person: Person, address: Address) -> Self {
        Customer { person, address }
    }
}

#[derive(Debug, Clone)]
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
        println!("Password reset for account {} at {}", self.id, self.address);
    }
}

trait Employee {
    fn employee_id(&self) -> &str;
    fn date_joined(&self) -> &str;
    fn account(&self) -> &Account;
}

/// Fields shared by every staff role - id, account, contact info, schedule.
/// One struct, embedded via composition, so id/account/name/email/phone
/// aren't duplicated three times (Rust has no inheritance to fake this with).
#[derive(Debug, Clone)]
struct StaffProfile {
    id: String,
    account: Account,
    name: String,
    email: String,
    phone: String,
    date_joined: String,
    schedule: Vec<Shift>,
}

impl StaffProfile {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        StaffProfile {
            id,
            account,
            name,
            email,
            phone,
            date_joined: Utc::now().date_naive().to_string(),
            schedule: Vec::new(),
        }
    }

    fn add_shift(&mut self, shift: Shift) {
        println!("{} scheduled for {}", self.name, shift);
        self.schedule.push(shift);
    }
}

#[derive(Debug, Clone)]
struct Shift {
    day: NaiveDate,
    start_hour: u32,
    end_hour: u32,
}

impl Shift {
    fn new(day: NaiveDate, start_hour: u32, end_hour: u32) -> Self {
        Shift {
            day,
            start_hour,
            end_hour,
        }
    }
}

impl fmt::Display for Shift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:02}:00-{:02}:00", self.day, self.start_hour, self.end_hour)
    }
}

#[derive(Debug)]
struct Receptionist {
    profile: StaffProfile,
    customers: Vec<Customer>,
}

impl Employee for Receptionist {
    fn employee_id(&self) -> &str {
        &self.profile.id
    }

    fn date_joined(&self) -> &str {
        &self.profile.date_joined
    }

    fn account(&self) -> &Account {
        &self.profile.account
    }
}

impl Receptionist {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Receptionist {
            profile: StaffProfile::new(id, account, name, email, phone),
            customers: Vec::new(),
        }
    }

    fn add_shift(&mut self, shift: Shift) {
        self.profile.add_shift(shift);
    }

    fn create_reservation(
        &mut self,
        reservation_id: String,
        people_count: u32,
        notes: String,
        customer: Customer,
    ) -> Reservation {
        println!(
            "Receptionist {} <{}> ({}) registered {} and created reservation {}",
            self.profile.name, self.profile.email, self.profile.phone, customer.person.name, reservation_id
        );
        self.customers.push(customer.clone());
        Reservation::new(reservation_id, self.profile.id.clone(), people_count, notes, customer)
    }

    fn search_customer(&self, name: &str) -> Option<&Customer> {
        let found = self
            .customers
            .iter()
            .find(|customer| customer.person.name == name);
        if let Some(customer) = found {
            println!(
                "Found customer {} <{}> ({}) at {}",
                customer.person.name,
                customer.person.email,
                customer.person.phone,
                customer.address
            );
        }
        found
    }
}

#[derive(Debug)]
struct Manager {
    profile: StaffProfile,
    team: Vec<String>,
}

impl Employee for Manager {
    fn employee_id(&self) -> &str {
        &self.profile.id
    }

    fn date_joined(&self) -> &str {
        &self.profile.date_joined
    }

    fn account(&self) -> &Account {
        &self.profile.account
    }
}

impl Manager {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Manager {
            profile: StaffProfile::new(id, account, name, email, phone),
            team: Vec::new(),
        }
    }

    fn add_shift(&mut self, shift: Shift) {
        self.profile.add_shift(shift);
    }

    fn add_employee(&mut self, name: String) -> bool {
        if self.team.contains(&name) {
            println!(
                "Manager {} <{}> ({}) skipped {} – already on the team",
                self.profile.name, self.profile.email, self.profile.phone, name
            );
            return false;
        }
        self.team.push(name.clone());
        println!(
            "Manager {} <{}> ({}) added {} to the team",
            self.profile.name, self.profile.email, self.profile.phone, name
        );
        true
    }

    /// Derived, not stored: team.len() already is the hire count, so a
    /// separate counter would just be a second source of truth to drift.
    fn hires_made(&self) -> usize {
        self.team.len()
    }
}

#[derive(Debug, Clone)]
struct Chef {
    profile: StaffProfile,
}

impl Employee for Chef {
    fn employee_id(&self) -> &str {
        &self.profile.id
    }

    fn date_joined(&self) -> &str {
        &self.profile.date_joined
    }

    fn account(&self) -> &Account {
        &self.profile.account
    }
}

impl Chef {
    fn new(id: String, account: Account, name: String, email: String, phone: String) -> Self {
        Chef {
            profile: StaffProfile::new(id, account, name, email, phone),
        }
    }

    fn add_shift(&mut self, shift: Shift) {
        self.profile.add_shift(shift);
    }

    fn take_order(&self, order: &mut Order) -> bool {
        if order.status == OrderStatus::Received {
            order.set_status(OrderStatus::Preparing);
            order.taken_by_chef = true;
            println!(
                "Chef {} <{}> ({}) started preparing order {}",
                self.profile.name, self.profile.email, self.profile.phone, order.order_id
            );
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
struct Waiter {
    id: String,
    name: String,
}

impl Waiter {
    fn new(id: String, name: String) -> Self {
        Waiter { id, name }
    }
}

#[derive(Debug, Clone)]
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
        println!("Chef {} assigned to kitchen {}", chef.profile.name, self.name);
        self.chefs.push(chef);
    }
}

#[derive(Debug, Clone)]
struct TableSeat {
    table_seat_number: u32,
    seat_type: SeatType,
}

impl TableSeat {
    fn new(table_seat_number: u32, seat_type: SeatType) -> Self {
        TableSeat {
            table_seat_number,
            seat_type,
        }
    }

    fn update_seat_type(&mut self, seat_type: SeatType) {
        println!("Seat {} type → {:?}", self.table_seat_number, seat_type);
        self.seat_type = seat_type;
    }
}

#[derive(Debug, Clone)]
struct ReservationSlot {
    reservation_id: String,
    date: NaiveDate,
}

#[derive(Debug, Clone)]
struct Table {
    table_id: String,
    max_capacity: u32,
    location_identifier: String,
    status: TableStatus,
    seats: Vec<TableSeat>,
    reservation_slots: Vec<ReservationSlot>,
}

impl Table {
    fn new(
        table_id: String,
        max_capacity: u32,
        location_identifier: String,
        status: TableStatus,
    ) -> Self {
        Table {
            table_id,
            max_capacity,
            location_identifier,
            status,
            seats: Vec::new(),
            reservation_slots: Vec::new(),
        }
    }

    fn add_seat(&mut self, seat: TableSeat) {
        self.seats.push(seat);
    }

    fn is_table_free(&self) -> bool {
        self.status == TableStatus::Free
    }

    fn is_available_at(&self, capacity: u32, start_time: DateTime<Utc>) -> bool {
        self.is_table_free()
            && self.max_capacity >= capacity
            && !self
                .reservation_slots
                .iter()
                .any(|slot| slot.date == start_time.date_naive())
    }

    fn reserve(&mut self, reservation_id: &str, date: NaiveDate) {
        self.status = TableStatus::Reserved;
        self.reservation_slots.push(ReservationSlot {
            reservation_id: reservation_id.to_string(),
            date,
        });
    }

    fn occupy(&mut self) {
        self.status = TableStatus::Occupied;
    }

    fn free_reservation(&mut self, reservation_id: &str) {
        self.reservation_slots
            .retain(|slot| slot.reservation_id != reservation_id);
        if self.reservation_slots.is_empty() && self.status == TableStatus::Reserved {
            self.status = TableStatus::Free;
        }
    }
}

#[derive(Debug, Clone)]
struct Notification {
    id: u32,
    created_on: DateTime<Utc>,
    message: String,
}

impl Notification {
    fn new(id: u32, message: String) -> Self {
        Notification {
            id,
            created_on: Utc::now(),
            message,
        }
    }

    fn send(&self) -> bool {
        println!(
            "Notification #{} at {}: {}",
            self.id,
            self.created_on.format("%Y-%m-%d %H:%M"),
            self.message
        );
        true
    }
}

#[derive(Debug, Clone)]
struct Reservation {
    reservation_id: String,
    /// Who took this reservation - lets reservations_handled be a query
    /// (COUNT of reservations per receptionist_id) instead of a counter
    /// field that has to be kept in sync by hand.
    receptionist_id: String,
    time_of_reservation: DateTime<Utc>,
    people_count: u32,
    status: ReservationStatus,
    notes: String,
    checkin_time: DateTime<Utc>,
    customer: Customer,
    tables: Vec<Table>,
    notifications: Vec<Notification>,
}

impl Reservation {
    fn new(
        reservation_id: String,
        receptionist_id: String,
        people_count: u32,
        notes: String,
        customer: Customer,
    ) -> Self {
        Reservation {
            reservation_id,
            receptionist_id,
            time_of_reservation: Utc::now(),
            people_count,
            status: ReservationStatus::Requested,
            notes,
            checkin_time: Utc::now(),
            customer,
            tables: Vec::new(),
            notifications: Vec::new(),
        }
    }

    fn update_people_count(&mut self, count: u32) {
        println!("Reservation {} party size → {count}", self.reservation_id);
        self.people_count = count;
    }

    fn add_table(&mut self, table: Table) {
        self.tables.push(table);
    }

    fn check_in(&mut self) {
        self.status = ReservationStatus::CheckedIn;
        self.checkin_time = Utc::now();
        println!(
            "Reservation {} checked in at {}",
            self.reservation_id,
            self.checkin_time.format("%H:%M")
        );
    }

    fn cancel(&mut self) {
        self.status = ReservationStatus::Canceled;
        println!("Reservation {} canceled", self.reservation_id);
    }

    fn send_reminder(&mut self) -> bool {
        let id = self.notifications.len() as u32 + 1;
        let notification = Notification::new(
            id,
            format!(
                "Hi {}, your table for {} at {} is reserved{}",
                self.customer.person.name,
                self.people_count,
                self.time_of_reservation.format("%Y-%m-%d %H:%M"),
                if self.notes.is_empty() {
                    String::new()
                } else {
                    format!(" – {}", self.notes)
                }
            ),
        );
        self.notifications.push(notification.clone());
        notification.send()
    }
}

#[derive(Debug, Clone)]
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

    fn update_price(&mut self, price: f32) {
        println!(
            "Menu item {} ({}) – {} → price ${price:.2}",
            self.title, self.menu_item_id, self.description
        );
        self.price = price;
    }
}

#[derive(Debug)]
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
        self.menu_items.push(menu_item);
    }
}

#[derive(Debug)]
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
        self.menu_sections.push(menu_section);
    }

    fn find_item(&self, title: &str) -> Option<&MenuItem> {
        self.menu_sections
            .iter()
            .flat_map(|section| &section.menu_items)
            .find(|item| item.title == title)
    }

    fn print(&self) {
        println!(
            "Menu {} – {}: {}",
            self.menu_id, self.title, self.description
        );
        for section in &self.menu_sections {
            println!(
                "  {} {}: {}",
                section.menu_section_id, section.title, section.description
            );
            for item in &section.menu_items {
                println!("    {} – ${:.2}", item.title, item.price);
            }
        }
    }
}

#[derive(Debug, Clone)]
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

    fn update_quantity(&mut self, quantity: u32) {
        println!(
            "Meal item {} ({}) quantity → {quantity}",
            self.menu_item.title, self.meal_item_id
        );
        self.quantity = quantity;
    }
}

#[derive(Debug, Clone)]
struct Meal {
    meal_id: String,
    seat: TableSeat,
    meal_items: Vec<MealItem>,
}

impl Meal {
    fn new(meal_id: String, seat: TableSeat) -> Self {
        Meal {
            meal_id,
            seat,
            meal_items: Vec::new(),
        }
    }

    fn add_meal_item(&mut self, meal_item: MealItem) {
        println!(
            "Meal item added to meal {} at seat {}",
            self.meal_id, self.seat.table_seat_number
        );
        self.meal_items.push(meal_item);
    }
}

#[derive(Debug, Clone)]
struct BillItem {
    menu_item_id: String,
    title: String,
    quantity: u32,
    unit_price: f32,
    amount: f32,
}

impl BillItem {
    fn new(
        menu_item_id: String,
        title: String,
        quantity: u32,
        unit_price: f32,
        amount: f32,
    ) -> Self {
        BillItem {
            menu_item_id,
            title,
            quantity,
            unit_price,
            amount,
        }
    }
}

#[derive(Debug, Clone)]
struct Check {
    check_id: String,
    bill_items: Vec<BillItem>,
    total_amount: f32,
    payment_method: Option<PaymentMethod>,
    payment_status: PaymentStatus,
}

impl Check {
    fn new(check_id: String) -> Self {
        Check {
            check_id,
            bill_items: Vec::new(),
            total_amount: 0.0,
            payment_method: None,
            payment_status: PaymentStatus::Unpaid,
        }
    }

    fn generate_bill(&mut self, meals: &[Meal]) {
        self.bill_items.clear();
        self.total_amount = 0.0;
        for meal in meals {
            for meal_item in &meal.meal_items {
                let amount = meal_item.menu_item.price * meal_item.quantity as f32;
                self.bill_items.push(BillItem::new(
                    meal_item.menu_item.menu_item_id.clone(),
                    meal_item.menu_item.title.clone(),
                    meal_item.quantity,
                    meal_item.menu_item.price,
                    amount,
                ));
                self.total_amount += amount;
            }
        }
        println!(
            "Bill {} generated – {} item(s), total ${:.2}",
            self.check_id,
            self.bill_items.len(),
            self.total_amount
        );
        for item in &self.bill_items {
            println!(
                "  {} ({}) x{} @ ${:.2} = ${:.2}",
                item.title, item.menu_item_id, item.quantity, item.unit_price, item.amount
            );
        }
    }

    fn get_total(&self) -> f32 {
        self.total_amount
    }

    fn process_payment(&mut self, method: PaymentMethod) -> bool {
        if self.total_amount <= 0.0 {
            println!("Payment declined – no bill to pay");
            return false;
        }
        self.payment_method = Some(method.clone());
        self.payment_status = match method {
            PaymentMethod::Cash => PaymentStatus::Completed,
            _ => PaymentStatus::Settled,
        };
        println!(
            "{} payment of ${:.2} accepted for {}",
            method, self.total_amount, self.check_id
        );
        true
    }
}

#[derive(Debug, Clone)]
struct Order {
    order_id: String,
    status: OrderStatus,
    creation_time: DateTime<Utc>,
    meals: Vec<Meal>,
    table: Table,
    waiter: Waiter,
    chef: Chef,
    /// Set once, the moment the chef actually starts preparing (see
    /// Chef::take_order). `chef` above is just who the order was created
    /// with - a raw fact, not a maintained counter, so "orders handled"
    /// can be derived from it without drift.
    taken_by_chef: bool,
    check: Check,
    notifications: Vec<Notification>,
}

impl Order {
    fn new(
        order_id: String,
        status: OrderStatus,
        table: Table,
        waiter: Waiter,
        chef: Chef,
    ) -> Self {
        let mut order = Order {
            order_id: order_id.clone(),
            status,
            creation_time: Utc::now(),
            meals: Vec::new(),
            table,
            waiter,
            chef,
            taken_by_chef: false,
            check: Check::new(format!("CHK-{order_id}")),
            notifications: Vec::new(),
        };
        order.notify_kitchen(format!(
            "New order {} for table {} assigned to chef {}",
            order.order_id, order.table.table_id, order.chef.profile.name
        ));
        order
    }

    /// Placeholder from the design doc (item #12), now wired: pushes a
    /// notification and sends it, same mechanism `Reservation::send_reminder`
    /// already uses.
    fn notify_kitchen(&mut self, message: String) -> bool {
        let notification = Notification::new(self.notifications.len() as u32 + 1, message);
        self.notifications.push(notification.clone());
        notification.send()
    }

    fn notify_staff(&mut self, message: String) -> bool {
        let notification = Notification::new(self.notifications.len() as u32 + 1, message);
        self.notifications.push(notification.clone());
        notification.send()
    }

    fn add_meal(&mut self, meal: Meal) {
        println!(
            "Meal {} added to order {} for table {} (waiter {} {})",
            meal.meal_id, self.order_id, self.table.table_id, self.waiter.name, self.waiter.id
        );
        self.meals.push(meal);
    }

    fn remove_meal(&mut self, meal_id: &str) -> bool {
        let before = self.meals.len();
        self.meals.retain(|meal| meal.meal_id != meal_id);
        let removed = self.meals.len() < before;
        if removed {
            println!("Meal {meal_id} removed from order {}", self.order_id);
        }
        removed
    }

    fn get_status(&self) -> OrderStatus {
        self.status.clone()
    }

    fn set_status(&mut self, status: OrderStatus) {
        println!(
            "Order {} (created {}, chef {}) status → {}",
            self.order_id,
            self.creation_time.format("%H:%M:%S"),
            self.chef.profile.name,
            status
        );
        let waiter_should_be_notified = status == OrderStatus::Completed;
        self.status = status;
        if waiter_should_be_notified {
            self.notify_staff(format!(
                "Order {} ready for table {} (waiter {})",
                self.order_id, self.table.table_id, self.waiter.name
            ));
        }
    }

    fn generate_bill(&mut self) {
        self.check.generate_bill(&self.meals);
    }
}

#[derive(Debug)]
struct TableChart {
    table_chart_id: String,
    table_chart_image: Vec<u8>,
    tables: Vec<Table>,
}

impl TableChart {
    fn new(id: String, tables: Vec<Table>) -> Self {
        TableChart {
            table_chart_id: id,
            table_chart_image: Vec::new(),
            tables,
        }
    }

    fn print(&self) {
        println!(
            "Table chart {} – {} table(s), layout image {} byte(s)",
            self.table_chart_id,
            self.tables.len(),
            self.table_chart_image.len()
        );
        for table in &self.tables {
            println!(
                "  {} – capacity {} at {} – {:?}",
                table.table_id, table.max_capacity, table.location_identifier, table.status
            );
        }
    }
}

#[derive(Debug, Clone)]
struct Ingredient {
    name: String,
    unit: String,
    quantity_on_hand: f32,
}

/// Stock levels + recipes (menu item title -> ingredients consumed per
/// unit sold). Deducted when a bill is generated, i.e. once the kitchen
/// has committed to the dish.
#[derive(Debug, Default)]
struct Inventory {
    stock: HashMap<String, Ingredient>,
    recipes: HashMap<String, Vec<(String, f32)>>,
}

impl Inventory {
    fn add_ingredient(&mut self, name: &str, unit: &str, quantity_on_hand: f32) {
        self.stock.insert(
            name.to_string(),
            Ingredient {
                name: name.to_string(),
                unit: unit.to_string(),
                quantity_on_hand,
            },
        );
    }

    fn set_recipe(&mut self, menu_item_title: &str, ingredients: Vec<(&str, f32)>) {
        self.recipes.insert(
            menu_item_title.to_string(),
            ingredients
                .into_iter()
                .map(|(name, qty)| (name.to_string(), qty))
                .collect(),
        );
    }

    /// Deduct stock for a bill's items. Doesn't block the sale (that would
    /// need a purchasing workflow this demo doesn't have) - just flags
    /// ingredients that dropped below zero so ops can react.
    fn consume_for_bill(&mut self, bill_items: &[BillItem]) -> Vec<String> {
        let mut low_stock = Vec::new();
        for item in bill_items {
            let Some(recipe) = self.recipes.get(&item.title).cloned() else {
                continue;
            };
            for (ingredient_name, qty_per_unit) in recipe {
                if let Some(ingredient) = self.stock.get_mut(&ingredient_name) {
                    ingredient.quantity_on_hand -= qty_per_unit * item.quantity as f32;
                    if ingredient.quantity_on_hand < 0.0 {
                        low_stock.push(ingredient_name.clone());
                    }
                }
            }
        }
        low_stock
    }
}

#[derive(Debug)]
struct Branch {
    name: String,
    location: String,
    kitchen: Kitchen,
    menu: Menu,
    tables: RwLock<Vec<Table>>,
    reservations: RwLock<Vec<Reservation>>,
    orders: RwLock<Vec<Order>>,
    inventory: RwLock<Inventory>,
}

impl Branch {
    fn new(name: String, location: String, kitchen: Kitchen) -> Self {
        Branch {
            name: name.clone(),
            location,
            kitchen,
            menu: Menu::new(
                format!("MENU-{name}"),
                format!("{name} menu"),
                format!("Menu for {name}"),
            ),
            tables: RwLock::new(Vec::new()),
            reservations: RwLock::new(Vec::new()),
            orders: RwLock::new(Vec::new()),
            inventory: RwLock::new(Inventory::default()),
        }
    }

    fn record_order(&self, order: Order) {
        println!("Order {} recorded for {}", order.order_id, self.name);
        write_guard(&self.orders).push(order);
    }

    /// Aggregate paid orders into a revenue + best-seller summary.
    /// Requirement #6 ("generate reports...such as sales reports"); pulls
    /// from orders already recorded, no new entity needed.
    fn sales_report(&self) -> SalesReport {
        let mut report = SalesReport::default();
        for order in read_guard(&self.orders).iter() {
            let check = &order.check;
            if !matches!(check.payment_status, PaymentStatus::Completed | PaymentStatus::Settled) {
                continue;
            }
            report.paid_order_count += 1;
            report.total_revenue += check.total_amount;
            for item in &check.bill_items {
                *report.item_quantities.entry(item.title.clone()).or_insert(0) += item.quantity;
            }
        }
        report
    }

    fn add_table(&self, table: Table) {
        println!(
            "Added table {} (capacity {}) to {}",
            table.table_id, table.max_capacity, self.name
        );
        write_guard(&self.tables).push(table);
    }

    fn table_count(&self) -> usize {
        read_guard(&self.tables).len()
    }

    /// Read-only snapshot of one table by position.
    fn table_snapshot(&self, idx: usize) -> Table {
        read_guard(&self.tables)[idx].clone()
    }

    fn search_tables(&self, capacity: u32, start_time: DateTime<Utc>) -> Vec<Table> {
        read_guard(&self.tables)
            .iter()
            .filter(|table| table.is_available_at(capacity, start_time))
            .cloned()
            .collect()
    }

    /// Reserve a table for a reservation. The availability check and the
    /// reserve happen under one write lock, so two concurrent reservations
    /// can never both grab the same table – the double-booking race that
    /// bit the seat-booking systems elsewhere in this codebase is
    /// structurally impossible here.
    fn reserve_table(&self, reservation: &mut Reservation, table_id: &str) -> bool {
        let confirmed_table = {
            let mut tables = write_guard(&self.tables);
            let Some(table) = tables.iter_mut().find(|table| table.table_id == table_id) else {
                return false;
            };
            if !table.is_available_at(reservation.people_count, reservation.time_of_reservation) {
                return false;
            }
            table.reserve(
                &reservation.reservation_id,
                reservation.time_of_reservation.date_naive(),
            );
            table.clone()
        };
        reservation.add_table(confirmed_table);
        reservation.status = ReservationStatus::Confirmed;
        println!(
            "Reservation {} confirmed for table {} at {}",
            reservation.reservation_id,
            table_id,
            reservation.time_of_reservation.format("%Y-%m-%d %H:%M")
        );
        write_guard(&self.reservations).push(reservation.clone());
        true
    }

    fn check_in(&self, reservation_id: &str) -> bool {
        let table_id = {
            let mut reservations = write_guard(&self.reservations);
            let Some(reservation) = reservations
                .iter_mut()
                .find(|reservation| reservation.reservation_id == reservation_id)
            else {
                return false;
            };
            reservation.check_in();
            reservation.tables.first().map(|table| table.table_id.clone())
        };
        if let Some(table_id) = table_id
            && let Some(table) = write_guard(&self.tables)
                .iter_mut()
                .find(|table| table.table_id == table_id)
        {
            table.occupy();
        }
        true
    }

    fn cancel_reservation(&self, reservation_id: &str) -> bool {
        let table_id = {
            let mut reservations = write_guard(&self.reservations);
            let Some(reservation) = reservations
                .iter_mut()
                .find(|reservation| reservation.reservation_id == reservation_id)
            else {
                return false;
            };
            reservation.cancel();
            reservation.tables.first().map(|table| table.table_id.clone())
        };
        if let Some(table_id) = table_id
            && let Some(table) = write_guard(&self.tables)
                .iter_mut()
                .find(|table| table.table_id == table_id)
        {
            table.free_reservation(reservation_id);
        }
        true
    }

    fn add_table_chart(&self) -> TableChart {
        TableChart::new(format!("CHART-{}", self.name), read_guard(&self.tables).clone())
    }

    fn reservation(&self, reservation_id: &str) -> Option<Reservation> {
        read_guard(&self.reservations)
            .iter()
            .find(|reservation| reservation.reservation_id == reservation_id)
            .cloned()
    }

    /// Derived from recorded orders, not a stored counter on Chef - avoids
    /// a second source of truth that has to be kept in sync on every write.
    fn order_count_for_chef(&self, chef_id: &str) -> usize {
        read_guard(&self.orders)
            .iter()
            .filter(|order| order.taken_by_chef && order.chef.employee_id() == chef_id)
            .count()
    }

    /// Derived from recorded reservations, same reasoning.
    fn reservation_count_for(&self, receptionist_id: &str) -> usize {
        read_guard(&self.reservations)
            .iter()
            .filter(|reservation| reservation.receptionist_id == receptionist_id)
            .count()
    }

    /// Requirement #2: seed stock + recipes for the branch kitchen.
    fn stock_ingredient(&self, name: &str, unit: &str, quantity_on_hand: f32) {
        write_guard(&self.inventory).add_ingredient(name, unit, quantity_on_hand);
    }

    fn set_recipe(&self, menu_item_title: &str, ingredients: Vec<(&str, f32)>) {
        write_guard(&self.inventory).set_recipe(menu_item_title, ingredients);
    }

    /// Deduct stock for a billed order. Returns ingredients that dropped
    /// below zero.
    fn consume_inventory(&self, order: &Order) -> Vec<String> {
        write_guard(&self.inventory).consume_for_bill(&order.check.bill_items)
    }

    /// Requirement #6 "inventory analysis": ingredients below a threshold.
    fn low_stock(&self, threshold: f32) -> Vec<Ingredient> {
        read_guard(&self.inventory)
            .stock
            .values()
            .filter(|ingredient| ingredient.quantity_on_hand < threshold)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Default)]
struct SalesReport {
    paid_order_count: u32,
    total_revenue: f32,
    item_quantities: HashMap<String, u32>,
}

impl SalesReport {
    fn print(&self) {
        println!(
            "Sales report – {} paid order(s), ${:.2} revenue",
            self.paid_order_count, self.total_revenue
        );
        for (title, qty) in &self.item_quantities {
            println!("  {title} x{qty}");
        }
    }
}

#[derive(Debug)]
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
        println!("Branch {} added to {}", branch.name, self.name);
        self.branches.push(branch);
    }
}

fn main() {
    // Restaurant with two branches, each with its own kitchen and menu.
    let mut restaurant = Restaurant::new("Tandoor Palace".to_string());
    let kitchen = Kitchen::new("Main Kitchen".to_string());
    let mut branch = Branch::new(
        "Downtown".to_string(),
        "Bandra West, Mumbai".to_string(),
        kitchen,
    );

    let mut t1 = Table::new("T1".to_string(), 4, "Window".to_string(), TableStatus::Free);
    for n in 1..=4 {
        t1.add_seat(TableSeat::new(n, SeatType::Regular));
    }
    let mut t2 = Table::new("T2".to_string(), 2, "Patio".to_string(), TableStatus::Free);
    for n in 1..=2 {
        let seat_type = if n == 2 {
            SeatType::Kid
        } else {
            SeatType::Regular
        };
        t2.add_seat(TableSeat::new(n, seat_type));
    }
    let mut t3 = Table::new(
        "T3".to_string(),
        6,
        "Main Hall".to_string(),
        TableStatus::Free,
    );
    for n in 1..=6 {
        t3.add_seat(TableSeat::new(n, SeatType::Regular));
    }
    branch.add_table(t1);
    branch.add_table(t2);
    branch.add_table(t3);

    let chef = Chef::new(
        "C-1".to_string(),
        Account::new(
            "ACC-C1".to_string(),
            "pw".to_string(),
            Address::new(
                "5 Back Kitchen Lane".to_string(),
                "Mumbai".to_string(),
                "MH".to_string(),
                "400050".to_string(),
                "India".to_string(),
            ),
            AccountStatus::Active,
        ),
        "Gordan".to_string(),
        "gordan@tandoor.example".to_string(),
        "+91-90000-00001".to_string(),
    );
    branch.kitchen.assign_chef(chef);

    let mut starters = MenuSection::new(
        "SEC-1".to_string(),
        "Starters".to_string(),
        "Small plates".to_string(),
    );
    starters.add_menu_item(MenuItem::new(
        "MI-101".to_string(),
        "Paneer Tikka".to_string(),
        "Char-grilled paneer".to_string(),
        250.0,
    ));
    starters.add_menu_item(MenuItem::new(
        "MI-102".to_string(),
        "Veg Spring Rolls".to_string(),
        "Crispy vegetable rolls".to_string(),
        180.0,
    ));
    let mut mains = MenuSection::new(
        "SEC-2".to_string(),
        "Main Course".to_string(),
        "Hearty dishes".to_string(),
    );
    mains.add_menu_item(MenuItem::new(
        "MI-103".to_string(),
        "Butter Chicken".to_string(),
        "Creamy tomato curry".to_string(),
        420.0,
    ));
    mains.add_menu_item(MenuItem::new(
        "MI-104".to_string(),
        "Dal Makhani".to_string(),
        "Slow-cooked lentils".to_string(),
        260.0,
    ));
    branch.menu.add_menu_section(starters);
    branch.menu.add_menu_section(mains);
    branch.menu.print();

    // Ingredient stock + recipes. Paneer is stocked low on purpose, to
    // exercise the low-stock alert once an order consumes it.
    branch.stock_ingredient("Paneer", "g", 150.0);
    branch.stock_ingredient("Butter", "g", 3000.0);
    branch.set_recipe("Paneer Tikka", vec![("Paneer", 200.0)]);
    branch.set_recipe("Butter Chicken", vec![("Butter", 50.0)]);

    // Seating layout snapshot.
    let chart = branch.add_table_chart();
    chart.print();

    // Walk-in customers are seated from the free pool.
    let now = Utc::now();
    let available = branch.search_tables(4, now);
    assert_eq!(available.len(), 2);
    assert!(available.iter().any(|table| table.table_id == "T1"));
    assert!(available.iter().any(|table| table.table_id == "T3"));
    assert!(!available.iter().any(|table| table.table_id == "T2"));

    // Receptionist registers customers and makes reservations.
    let recp_account = Account::new(
        "ACC-R1".to_string(),
        "pw".to_string(),
        Address::new(
            "1 Station Road".to_string(),
            "Mumbai".to_string(),
            "MH".to_string(),
            "400050".to_string(),
            "India".to_string(),
        ),
        AccountStatus::Active,
    );
    let mut receptionist = Receptionist::new(
        "R-1".to_string(),
        recp_account,
        "Priya".to_string(),
        "priya@tandoor.example".to_string(),
        "+91-90000-00002".to_string(),
    );
    assert!(!receptionist.profile.account.password.is_empty());

    let customer_address = Address::new(
        "42 Rose Villa".to_string(),
        "Mumbai".to_string(),
        "MH".to_string(),
        "400051".to_string(),
        "India".to_string(),
    );
    let alice = Customer::new(
        Person::new(
            "Alice".to_string(),
            "alice@example.com".to_string(),
            "+91-90000-00010".to_string(),
        ),
        customer_address.clone(),
    );
    let mut res1 = receptionist.create_reservation(
        "RES-1".to_string(),
        3,
        "Window seat preferred".to_string(),
        alice,
    );
    assert!(receptionist.search_customer("Alice").is_some());
    assert_eq!(res1.status, ReservationStatus::Requested);

    assert!(branch.reserve_table(&mut res1, "T1"));
    assert_eq!(res1.status, ReservationStatus::Confirmed);
    assert_eq!(res1.tables.len(), 1);
    assert_eq!(branch.table_snapshot(0).status, TableStatus::Reserved);

    let available_after = branch.search_tables(4, now);
    assert_eq!(available_after.len(), 1);
    assert_eq!(available_after[0].table_id, "T3");

    // The system warns the customer as the reservation time approaches.
    assert!(res1.send_reminder());
    assert_eq!(res1.notifications.len(), 1);

    // A second reservation checks in; its table becomes occupied.
    let bob = Customer::new(
        Person::new(
            "Bob".to_string(),
            "bob@example.com".to_string(),
            "+91-90000-00011".to_string(),
        ),
        customer_address.clone(),
    );
    let mut res2 = receptionist.create_reservation("RES-2".to_string(), 2, String::new(), bob);
    assert!(branch.reserve_table(&mut res2, "T3"));
    assert!(branch.check_in("RES-2"));
    let checked_in = branch.reservation("RES-2").expect("RES-2 present in branch");
    assert_eq!(checked_in.status, ReservationStatus::CheckedIn);
    assert_eq!(branch.table_snapshot(2).status, TableStatus::Occupied);

    // A third reservation is canceled; its table returns to the free pool.
    let carol = Customer::new(
        Person::new(
            "Carol".to_string(),
            "carol@example.com".to_string(),
            "+91-90000-00012".to_string(),
        ),
        customer_address.clone(),
    );
    let mut res3 = receptionist.create_reservation("RES-3".to_string(), 2, String::new(), carol);
    assert!(branch.reserve_table(&mut res3, "T2"));
    assert!(branch.cancel_reservation("RES-3"));
    let canceled = branch.reservation("RES-3").expect("RES-3 present in branch");
    assert_eq!(canceled.status, ReservationStatus::Canceled);
    assert!(branch.table_snapshot(1).is_table_free());

    // A waiter builds an order with one meal per table seat.
    let waiter = Waiter::new("W-1".to_string(), "Ramesh".to_string());
    let chef = branch.kitchen.chefs[0].clone();
    let mut order = Order::new(
        "ORD-1".to_string(),
        OrderStatus::Received,
        branch.table_snapshot(1),
        waiter.clone(),
        chef.clone(),
    );
    let paneer = branch
        .menu
        .find_item("Paneer Tikka")
        .expect("paneer on the menu")
        .clone();
    let dal = branch
        .menu
        .find_item("Dal Makhani")
        .expect("dal on the menu")
        .clone();
    let rolls = branch
        .menu
        .find_item("Veg Spring Rolls")
        .expect("rolls on the menu")
        .clone();
    let mut meal1 = Meal::new("MEAL-1".to_string(), branch.table_snapshot(1).seats[0].clone());
    meal1.add_meal_item(MealItem::new("LI-1".to_string(), 1, paneer));
    let mut meal2 = Meal::new("MEAL-2".to_string(), branch.table_snapshot(1).seats[1].clone());
    meal2.add_meal_item(MealItem::new("LI-2".to_string(), 2, dal));
    let mut meal3 = Meal::new("MEAL-3".to_string(), branch.table_snapshot(1).seats[0].clone());
    meal3.add_meal_item(MealItem::new("LI-3".to_string(), 1, rolls));
    order.add_meal(meal1);
    order.add_meal(meal2);
    order.add_meal(meal3);
    assert!(order.remove_meal("MEAL-3"));
    assert_eq!(order.meals.len(), 2);

    // The chef works the order, then it is marked completed.
    assert!(branch.kitchen.chefs[0].take_order(&mut order));
    assert_eq!(order.get_status(), OrderStatus::Preparing);
    order.set_status(OrderStatus::Completed);
    assert_eq!(order.get_status(), OrderStatus::Completed);

    // The cashier generates a bill and the customer pays by credit card.
    order.generate_bill();
    assert!((order.check.get_total() - 770.0).abs() < 0.001);
    assert_eq!(order.check.bill_items.len(), 2);
    assert!(order.check.process_payment(PaymentMethod::CreditCard));
    assert_eq!(order.check.payment_method, Some(PaymentMethod::CreditCard));
    assert_eq!(order.check.payment_status, PaymentStatus::Settled);

    // Billing the order draws down ingredient stock. Paneer was stocked
    // low on purpose, so this trips the low-stock alert (200g needed,
    // 150g on hand).
    let low_stock = branch.consume_inventory(&order);
    assert_eq!(low_stock, vec!["Paneer".to_string()]);
    let alerts = branch.low_stock(0.0);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].name, "Paneer");
    println!(
        "Inventory alert: {} below threshold ({} {} on hand)",
        alerts[0].name, alerts[0].quantity_on_hand, alerts[0].unit
    );

    // A second order is paid by check, a third by cash.
    let chicken = branch
        .menu
        .find_item("Butter Chicken")
        .expect("chicken on the menu")
        .clone();
    let mut meal4 = Meal::new("MEAL-4".to_string(), branch.table_snapshot(1).seats[1].clone());
    meal4.add_meal_item(MealItem::new("LI-4".to_string(), 1, chicken));
    let mut order2 = Order::new(
        "ORD-2".to_string(),
        OrderStatus::Received,
        branch.table_snapshot(0),
        waiter.clone(),
        chef.clone(),
    );
    order2.add_meal(meal4);
    order2.generate_bill();
    assert!((order2.check.get_total() - 420.0).abs() < 0.001);
    assert!(order2.check.process_payment(PaymentMethod::MobilePayment));
    assert_eq!(order2.check.payment_status, PaymentStatus::Settled);
    branch.consume_inventory(&order2);

    let rolls2 = branch
        .menu
        .find_item("Veg Spring Rolls")
        .expect("rolls on the menu")
        .clone();
    let mut meal5 = Meal::new("MEAL-5".to_string(), branch.table_snapshot(1).seats[0].clone());
    meal5.add_meal_item(MealItem::new("LI-5".to_string(), 1, rolls2));
    let mut order3 = Order::new(
        "ORD-3".to_string(),
        OrderStatus::Received,
        branch.table_snapshot(2),
        waiter.clone(),
        chef.clone(),
    );
    order3.add_meal(meal5);
    order3.generate_bill();
    assert!(order3.check.process_payment(PaymentMethod::Cash));
    assert_eq!(order3.check.payment_status, PaymentStatus::Completed);

    // An order with no meals cannot be paid.
    let mut order4 = Order::new(
        "ORD-4".to_string(),
        OrderStatus::Received,
        branch.table_snapshot(1),
        waiter,
        chef,
    );
    assert_eq!(order4.check.payment_status, PaymentStatus::Unpaid);
    assert!(!order4.check.process_payment(PaymentMethod::Cash));
    order4.set_status(OrderStatus::Canceled);
    assert_eq!(order4.get_status(), OrderStatus::Canceled);

    // The manager hires staff (and rejects duplicates).
    let mgr_account = Account::new(
        "ACC-M1".to_string(),
        "pw".to_string(),
        customer_address.clone(),
        AccountStatus::Active,
    );
    let mut manager = Manager::new(
        "M-1".to_string(),
        mgr_account,
        "Vikram".to_string(),
        "vikram@tandoor.example".to_string(),
        "+91-90000-00003".to_string(),
    );
    assert!(manager.add_employee("Suresh".to_string()));
    assert!(!manager.add_employee("Suresh".to_string()));
    assert_eq!(manager.team.len(), 1);

    // Everyday maintenance: prices, quantities, party sizes, and seat types.
    branch.menu.menu_sections[0].menu_items[0].update_price(260.0);
    assert_eq!(branch.menu.menu_sections[0].menu_items[0].price, 260.0);
    order.meals[0].meal_items[0].update_quantity(2);
    assert_eq!(order.meals[0].meal_items[0].quantity, 2);
    res1.update_people_count(4);
    assert_eq!(res1.people_count, 4);
    let mut seat = TableSeat::new(1, SeatType::Regular);
    seat.update_seat_type(SeatType::Accessible);
    assert_eq!(seat.seat_type, SeatType::Accessible);

    // Branch retains its orders once they're settled, so performance
    // counts (below) and the sales report can be derived from real rows
    // instead of a hand-maintained counter (order3 paid, order4 never was).
    branch.record_order(order);
    branch.record_order(order2);
    branch.record_order(order3);
    branch.record_order(order4);

    // Staff schedules (requirement #5).
    let today = Utc::now().date_naive();
    branch.kitchen.chefs[0].add_shift(Shift::new(today, 9, 17));
    receptionist.add_shift(Shift::new(today, 8, 16));
    manager.add_shift(Shift::new(today, 10, 18));

    // Staff details, performance (derived, not stored), and account admin.
    let chef = branch.kitchen.chefs[0].clone();
    let chef_orders = branch.order_count_for_chef(chef.employee_id());
    println!(
        "Chef {} (joined {}) – account {:?} – {} order(s) handled, {} shift(s)",
        chef.employee_id(),
        chef.date_joined(),
        chef.account().status,
        chef_orders,
        chef.profile.schedule.len()
    );
    assert_eq!(chef_orders, 1);
    println!(
        "Manager {} joined on {} – {} hire(s) made",
        manager.employee_id(),
        manager.date_joined(),
        manager.hires_made()
    );
    assert_eq!(manager.hires_made(), 1);
    let receptionist_reservations = branch.reservation_count_for(receptionist.employee_id());
    println!(
        "Receptionist {} – {} reservation(s) handled, {} shift(s)",
        receptionist.employee_id(),
        receptionist_reservations,
        receptionist.profile.schedule.len()
    );
    assert_eq!(receptionist_reservations, 3);
    receptionist.profile.account.reset_password();

    let report = branch.sales_report();
    report.print();
    assert_eq!(report.paid_order_count, 3);
    assert!((report.total_revenue - 1370.0).abs() < 0.001);
    assert_eq!(report.item_quantities.get("Butter Chicken"), Some(&1));

    // The restaurant owns both branches; if it goes inactive so do they.
    restaurant.add_branch(branch);
    let airport_kitchen = Kitchen::new("Airport Kitchen".to_string());
    let airport = Branch::new(
        "Airport".to_string(),
        "Terminal 2, Mumbai".to_string(),
        airport_kitchen,
    );
    restaurant.add_branch(airport);
    println!(
        "{} has {} branch(es):",
        restaurant.name,
        restaurant.branches.len()
    );
    for b in &restaurant.branches {
        println!(
            "  {} at {} – kitchen {}, {} table(s), menu {}",
            b.name,
            b.location,
            b.kitchen.name,
            b.table_count(),
            b.menu.title
        );
    }

    let reservation_statuses = [ReservationStatus::Pending, ReservationStatus::Abandoned];
    let seat_types = [SeatType::Other];
    let order_statuses = [OrderStatus::None];
    let table_statuses = [TableStatus::Other];
    let account_statuses = [
        AccountStatus::Closed,
        AccountStatus::Canceled,
        AccountStatus::Blacklisted,
        AccountStatus::Blocked,
    ];
    let payment_statuses = [
        PaymentStatus::Pending,
        PaymentStatus::Filled,
        PaymentStatus::Declined,
        PaymentStatus::Cancelled,
        PaymentStatus::Abandoned,
        PaymentStatus::Settling,
        PaymentStatus::Refunded,
    ];
    let payment_methods = [
        PaymentMethod::CreditCard,
        PaymentMethod::Check,
        PaymentMethod::Cash,
    ];
    println!(
        "Other domain statuses: {reservation_statuses:?} {seat_types:?} {order_statuses:?} {table_statuses:?} {account_statuses:?}"
    );
    println!("Payment domain: {payment_statuses:?} via {payment_methods:?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn dummy_customer(name: &str) -> Customer {
        Customer::new(
            Person::new(name.to_string(), format!("{name}@example.com"), "000".to_string()),
            Address::new(
                "1 St".to_string(),
                "City".to_string(),
                "ST".to_string(),
                "00000".to_string(),
                "Country".to_string(),
            ),
        )
    }

    /// Requirement #7: concurrent reservations for the same table must not
    /// double-book it. reserve_table's check-then-reserve runs under one
    /// write lock, so exactly one thread should win.
    #[test]
    fn test_reserve_table_race_exactly_one_winner() {
        let branch = Branch::new("Race".to_string(), "Loc".to_string(), Kitchen::new("K".to_string()));
        branch.add_table(Table::new("T1".to_string(), 4, "Hall".to_string(), TableStatus::Free));
        let branch = Arc::new(branch);

        let handles: Vec<_> = (0..32)
            .map(|i| {
                let branch = Arc::clone(&branch);
                std::thread::spawn(move || {
                    let mut reservation = Reservation::new(
                        format!("RES-{i}"),
                        "R-TEST".to_string(),
                        2,
                        String::new(),
                        dummy_customer(&format!("user{i}")),
                    );
                    branch.reserve_table(&mut reservation, "T1")
                })
            })
            .collect();

        let winners = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(winners, 1, "exactly one concurrent reservation must win");
    }

    #[test]
    fn test_low_stock_alert() {
        let mut inventory = Inventory::default();
        inventory.add_ingredient("Paneer", "g", 150.0);
        inventory.set_recipe("Paneer Tikka", vec![("Paneer", 200.0)]);
        let bill_items = vec![BillItem::new(
            "MI-101".to_string(),
            "Paneer Tikka".to_string(),
            1,
            250.0,
            250.0,
        )];
        let low_stock = inventory.consume_for_bill(&bill_items);
        assert_eq!(low_stock, vec!["Paneer".to_string()]);
    }
}
