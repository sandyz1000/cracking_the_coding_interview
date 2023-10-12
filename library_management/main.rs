#![allow(unused)]

use std::{fmt, collections::HashMap};
use strum_macros::Display;
use fmt::Display as FmtDisplay;
use chrono::prelude::*;

#[derive(Debug)]
enum BookFormat {
    Hardcover,
    Paperback,
    AudioBook,
    Ebook,
    Newspaper,
    Magazine,
    Journal,
}

#[derive(Debug)]
enum BookStatus {
    Available,
    Reserved,
    Loaned,
    Lost,
}

#[derive(Debug)]
enum ReservationStatus {
    Waiting,
    Pending,
    Canceled,
    Completed,
    None,
}

#[derive(Debug)]
enum AccountStatus {
    Active,
    Closed,
    Canceled,
    Blacklisted,
    None,
}

#[derive(Debug, Clone)]
struct Address {
    street_address: String,
    city: String,
    state: String,
    zip_code: String,
    country: String,
}

#[derive(Debug)]
struct Person {
    name: String,
    address: Address,
    email: String,
    phone: String,
}

impl Person {
    fn new(name: String, address: Address, email: String, phone: String) -> Self {
        Person {
            name,
            address,
            email,
            phone,
        }
    }
}

#[derive(Debug)]
struct Constants {
    max_books_issued_to_user: u32,
    max_lending_days: u32,
}

impl Constants {
    fn new() -> Self {
        Constants {
            max_books_issued_to_user: 5,
            max_lending_days: 10,
        }
    }
}

#[derive(Debug)]
struct Author {
    name: String,
    description: String,
}

/// The basic building block of the system. Every book will have ISBN, Title, 
/// Subject, Publishers, etc.
trait Book {
    fn get_isbn(&self) -> &str;
    fn get_title(&self) -> &str;
    fn get_subject(&self) -> &str;
    fn get_publisher(&self) -> &str;
    fn get_language(&self) -> &str;
    fn get_number_of_pages(&self) -> u32;
    fn get_authors(&self) -> Vec<&Author>;
}

/// Any book can have multiple copies, each copy will be considered a book item in our system. 
/// Each book item will have a unique barcode.
#[derive(Debug)]
struct BookItem {
    isbn: String,
    title: String,
    subject: String,
    publisher: String,
    language: String,
    number_of_pages: u32,
    authors: Vec<Author>,
    barcode: String,
    is_reference_only: bool,
    borrowed: bool,
    due_date: String,
    price: f64,
    book_format: BookFormat,
    status: BookStatus,
    date_of_purchase: chrono::DateTime<chrono::Utc>,
    publication_date: chrono::DateTime<chrono::Utc>,
    placed_at: String,
}

impl BookItem {
    fn checkout(&mut self, member_id: u32) -> bool {
        if self.is_reference_only {
            println!("This book is reference only and can't be issued");
            return false;
        }
        if !BookLending::lend_book(&self.barcode, member_id) {
            return false;
        }
        self.update_book_item_status(BookStatus::Loaned);
        true
    }

    fn update_book_item_status(&mut self, status: BookStatus) {
        self.status = status;
    }

    fn update_due_date(&self, due_date: chrono::NaiveDateTime) {

    }
}

impl Book for BookItem {
    fn get_isbn(&self) -> &str {
        &self.isbn
    }

    fn get_title(&self) -> &str {
        &self.title
    }

    fn get_subject(&self) -> &str {
        &self.subject
    }

    fn get_publisher(&self) -> &str {
        &self.publisher
    }

    fn get_language(&self) -> &str {
        &self.language
    }

    fn get_number_of_pages(&self) -> u32 {
        self.number_of_pages
    }

    fn get_authors(&self) -> Vec<&Author> {
        vec![]
    }
}

/// Books will be placed on racks. Each rack will be identified by a rack number and 
/// will have a location identifier to describe the physical location of the rack in 
/// the library.
#[derive(Debug)]
struct Rack {
    number: u32,
    location_identifier: String,
}

/// **BookReservation, BookLending, and Fine:** 
/// These classes represent a book reservation, lending, and fine collection, respectively.
#[derive(Debug)]
struct BookReservation {
    creation_date: chrono::NaiveDateTime,
    status: ReservationStatus,
    book_item_barcode: String,
    member_id: u32,
}


#[derive(Debug)]
struct BookLending {
    creation_date: chrono::NaiveDateTime,
    due_date: chrono::NaiveDateTime,
    return_date: chrono::NaiveDateTime,
    book_item_barcode: String,
    member_id: u32,
}

#[derive(Debug)]
struct Fine {
    creation_date: chrono::NaiveDateTime,
    book_item_barcode: String,
    member_id: u32,
}

impl Fine {
    fn new(creation_date: chrono::NaiveDateTime, book_item_barcode: String, member_id: u32) -> Self {
        Fine {
            creation_date, book_item_barcode, member_id
        }
    }

    fn get_amount(&self) -> f64 {
        0.0  
    }
}

/// **Account, Member, and Librarian:** These classes represent various people 
/// that interact with our system: 
trait Account {
    fn reset_password(&self) -> bool;
}

impl Account for Librarian {
    fn reset_password(&self) -> bool {
        false
    }
}

impl Account for Member {
    fn reset_password(&self) -> bool {
        false
    }    
}

/// The central part of the organization for which this software has been designed. 
/// It has attributes like 'Name' to distinguish it from any other libraries and 
/// 'Address' to describe its location.
#[derive(Debug, Clone)]
struct Library {
    name: String,
    address: Address,
}

impl Library {
    fn new(name: String, address: Address) -> Self {
        Self {
            name,
            address,
        }
    }

    fn get_address(&self) -> &Address {
        unimplemented!()
    }
}

/// Library card has barcode that will be scan by the reader
#[derive(Debug, Clone)]
struct BarcodeReader {
    id: String,
    registered_at: chrono::DateTime<chrono::Utc>,
    active: bool
}

impl BarcodeReader {
    fn new(id: String, registered_at: chrono::DateTime<chrono::Utc>, active: bool) -> Self {
        Self {
            id, registered_at, active
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }
}


/// Mainly responsible for adding and modifying books, book items, and users. 
/// The Librarian can also issue, reserve, and return book items.
#[derive(Debug)]
struct Librarian {
    id: u32,
    password: String,
    person: Person,
    status: AccountStatus,
}

impl Librarian {
    // TODO: Implement all the methods for Librarian
    fn add_book_item(&self, book_item: &BookItem) -> bool {
        true
    }

    fn block_member(&self, member: &mut Member) -> bool {
        true
    }

    fn un_block_member(&self, member: &mut Member) -> bool {
        true
    }
}

/// Each library user will be issued a library card, which will be used to identify 
/// users while issuing or returning books.
#[derive(Debug, Clone)]
struct LibraryCard {
    card_number: String,
    barcode: String,
    active: bool,
    issued_date: chrono::DateTime<chrono::Utc>,
}

impl LibraryCard {
    fn new(card_number: String, barcode: String, active: bool, issued_date: chrono::DateTime<chrono::Utc>) -> Self {
        LibraryCard {
            card_number,
            barcode,
            active,
            issued_date,
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// All members can search the catalog, as well as check-out, reserve, renew, and return a book.
/// Member borrow and reserve books
/// Member also make book reservations
#[derive(Debug)]
struct Member {
    id: u32,
    password: String,
    person: Person,
    status: AccountStatus,
    date_of_membership: chrono::NaiveDate,
    total_books_checkedout: u32,
}

impl Member {
    const MAX_BOOKS_ISSUED_TO_A_USER: u32 = 5;
    const MAX_LENDING_DAYS: i64 = 10;

    fn get_total_books_checkedout(&self) -> u32 {
        self.total_books_checkedout
    }

    fn reserve_book_item(&self, book_item: &BookItem) {
        todo!()
    }

    fn increment_total_books_checkedout(&mut self) {
        todo!()
    }

    fn checkout_book_item(&mut self, book_item: &mut BookItem) -> bool {
        if self.get_total_books_checkedout() >= Self::MAX_BOOKS_ISSUED_TO_A_USER {
            println!("The user has already checked-out maximum number of books");
            return false;
        }
        let book_reservation = BookReservation::fetch_reservation_details(book_item.barcode.as_str());
        if let Some(mut reservation) = book_reservation {
            if reservation.member_id != self.id {
                println!("This book is reserved by another member");
                return false;
            } else {
                // Update reservation status to COMPLETED
                reservation.update_status(ReservationStatus::Completed);
            }
        }

        if !book_item.checkout(self.id) {
            return false;
        }

        self.increment_total_books_checkedout();
        true
    }

    fn check_for_fine(&self, book_item_barcode: &str) {
        let book_lending = BookLending::fetch_lending_details(book_item_barcode);
        let due_date = book_lending.due_date;
        let today: chrono::NaiveDateTime = chrono::Utc::now().naive_utc();
        if today > due_date {
            let diff = today - due_date;
            let diff_days = diff.num_days();
            Fine::collect_fine(self.id, diff_days as u32);
        }
    }

    fn return_book_item(&self, book_item: &mut BookItem) {
        self.check_for_fine(book_item.barcode.as_str());
        let book_reservation = BookReservation::fetch_reservation_details(book_item.barcode.as_str());
        if let Some(reservation) = book_reservation {
            book_item.update_book_item_status(BookStatus::Reserved);
            reservation.send_book_available_notification();
            book_item.update_book_item_status(BookStatus::Available);
        }
    }

    fn renew_book_item(&self, book_item: BookItem) -> bool {
        self.check_for_fine(book_item.barcode.as_str());
        let book_reservation = BookReservation::fetch_reservation_details(book_item.barcode.as_str());
        if let Some(mut reservation) = book_reservation {
            if reservation.member_id != self.id {
                println!("This book is reserved by another member");
                return false;
            } else {
                // Update reservation status to COMPLETED
                reservation.update_status(ReservationStatus::Completed);
            }
        }

        BookLending::lend_book(book_item.barcode.as_str(), self.id);
        let max_lending_days = chrono::Utc::now().naive_utc() + chrono::Duration::days(Self::MAX_LENDING_DAYS);
        book_item.update_due_date(max_lending_days);
        true
    }
}


impl AccountStatus {
    fn reset_password(&self) -> bool {
        unimplemented!()
    }    
}

impl BookReservation {
    fn fetch_reservation_details(barcode: &str) -> Option<BookReservation> {
        None
    }

    fn update_status(&mut self, status: ReservationStatus) -> bool {
        false
    }

    fn send_book_available_notification(&self) {
        unimplemented!()
    }
}


impl BookLending {
    fn fetch_lending_details(barcode: &str) -> BookLending {
        unimplemented!()
    }

    fn lend_book(barcode: &str, member_id: u32) -> bool {
        unimplemented!()
    }

    fn get_due_date(&self) -> chrono::NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }
}

impl Fine {
    fn collect_fine(member_id: u32, days: u32) -> bool {
        unimplemented!()
    }
}

/* **************
### Search and Catalog
* ***************/
/// **Search interface and Catalog:** The Catalog class will implement the Search interface to 
/// facilitate searching of books.
trait Search {
    fn search_by_title(&self, title: &str) -> Option<&Vec<BookItem>>;
    
    fn search_by_author(&self, author: &str) -> Option<&Vec<BookItem>>;
    
    fn search_by_subject(&self, subject: &str) -> Option<&Vec<BookItem>>;
    
    fn search_by_pub_date(&self, publish_date: &NaiveDateTime) -> Option<&Vec<BookItem>>;
}

/// Catalogs contain list of books sorted on certain criteria. Our system will support 
/// searching through four catalogs: Title, Author, Subject, and Publish-date.
struct Catalog {
    created_at: chrono::DateTime<chrono::Utc>,
    total_books: u32,
    book_titles: HashMap<String, Vec<BookItem>>,
    book_authors: HashMap<String, Vec<BookItem>>,
    book_subjects: HashMap<String, Vec<BookItem>>,
    book_publication_dates: HashMap<chrono::NaiveDateTime, Vec<BookItem>>,
}

impl Catalog {
    fn new(total_books: u32) -> Self {
        Catalog {
            total_books,
            created_at: chrono::Utc::now(),
            book_titles: HashMap::new(),
            book_authors: HashMap::new(),
            book_subjects: HashMap::new(),
            book_publication_dates: HashMap::new(),
        }
    }

    fn update_catalog(&mut self) -> bool {
        unimplemented!()
    }
}

impl Search for Catalog {
    fn search_by_title(&self, title: &str) -> Option<&Vec<BookItem>> {
        self.book_titles.get(title)
    }

    fn search_by_author(&self, author: &str) -> Option<&Vec<BookItem>> {
        self.book_authors.get(author)
    }

    fn search_by_subject(&self, subject: &str) -> Option<&Vec<BookItem>> {
        self.book_subjects.get(subject)
    }

    fn search_by_pub_date(&self, publish_date: &NaiveDateTime) -> Option<&Vec<BookItem>> {
        unimplemented!()
    }
}

/* *****
Fine and Transaction
* ******/
#[derive(Debug, Clone)]
struct FineTransaction {
    creation_date: chrono::NaiveDateTime,
    fine_amount: f64
}

impl FineTransaction {
    fn new(
        creation_date: chrono::NaiveDateTime, fine_amount: f64
    ) -> Self {
        Self { creation_date, fine_amount }
    }

    fn initiate_transaction(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct CreditCardTransaction {
    fine_transaction: FineTransaction,
    name_on_card: String,
    card_number: String,
    expiration_date: String,
    cvv: String
}

#[derive(Debug, Clone)]
struct CashTransaction {
    fine_transaction: FineTransaction,
    cash_amount: f64
}

#[derive(Debug, Clone)]
struct Notification {
    notification_id: i32,
    created_on: chrono::DateTime<Utc>,
    content: String
}

impl Notification {
    fn new(notification_id: i32, created_on: chrono::DateTime<Utc>, content: String) -> Self {
        Notification {
            notification_id,
            created_on,
            content
        }
    }

    fn send_notification(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct PostalNotification {
    address: Address,
    notification: Notification
}

#[derive(Debug, Clone)]
struct EmailNotification {
    notification: Notification,
    email: String
}


fn main() {
    let address = Address {
        street_address: String::from("123 Main St"),
        city: String::from("City"),
        state: String::from("State"),
        zip_code: String::from("12345"),
        country: String::from("Country"),
    };

    let person = Person::new(
        String::from("John Doe"),
        address,
        String::from("johndoe@example.com"),
        String::from("123-456-7890"),
    );

    let constants = Constants::new();

    println!("Person: {:?}", person);
    println!("Constants: {:?}", constants);
}

