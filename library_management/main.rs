//! Library management system. The catalog and the member registry are each
//! behind an `RwLock`; `borrow_book`/`return_book` take the catalog write
//! lock and then the member write lock (always in that order, so two threads
//! can never deadlock), which makes a checkout atomic: a book is either
//! loaned to exactly one member or available.

/*
Designing a Library Management System

### Requirements

- The library management system should allow librarians to manage books, members, and borrowing activities.
- The system should support adding, updating, and removing books from the library catalog.
- Each book should have details such as title, author, ISBN, publication year, and availability status.
- The system should allow members to borrow and return books.
- Each member should have details such as name, member ID, contact information, and borrowing history.
- The system should enforce borrowing rules, such as a maximum number of books that can be borrowed at a
time and loan duration.
- The system should handle concurrent access to the library catalog and member records.
- The system should be extensible to accommodate future enhancements and new features.

### Classes, Interfaces and Enumerations

- The Book class represents a book in the library catalog, with properties such as ISBN, title, author,
publication year, and availability status.
- The Member class represents a library member, with properties like member ID, name, contact information,
and a list of borrowed books.
- The LibraryManager class is the core of the library management system and follows the Singleton pattern to
ensure a single instance of the library manager.
- The LibraryManager class uses concurrent data structures (ConcurrentHashMap) to handle concurrent access to
the library catalog and member records.
- The LibraryManager class provides methods for adding and removing books, registering and unregistering members,
borrowing and returning books, and searching for books based on keywords.
- The LibraryManagementSystemDemo class serves as the entry point of the application and demonstrates the usage
of the library management system.

*/

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum BookStatus {
    Available,
    Borrowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LibraryError {
    #[error("book with ISBN '{isbn}' already in the catalog")]
    BookExists { isbn: String },
    #[error("book with ISBN '{isbn}' not found")]
    BookNotFound { isbn: String },
    #[error("book with ISBN '{isbn}' is currently borrowed")]
    BookBorrowed { isbn: String },
    #[error("member {id} not found")]
    MemberNotFound { id: uuid::Uuid },
    #[error("member {id} has outstanding loans")]
    MemberHasLoans { id: uuid::Uuid },
    #[error("member {id} reached the limit of {max} borrowed books")]
    BorrowLimitReached { id: uuid::Uuid, max: usize },
    #[error("book with ISBN '{isbn}' is not on loan to member {id}")]
    BookNotOnLoan { isbn: String, id: uuid::Uuid },
}

pub type LibraryResult<T> = std::result::Result<T, LibraryError>;

#[derive(Debug, Clone)]
pub struct Book {
    pub isbn: String,
    pub title: String,
    pub author: String,
    pub publication_year: u32,
    status: BookStatus,
}

impl Book {
    pub fn new(
        isbn: impl Into<String>,
        title: impl Into<String>,
        author: impl Into<String>,
        publication_year: u32,
    ) -> Self {
        Self {
            isbn: isbn.into(),
            title: title.into(),
            author: author.into(),
            publication_year,
            status: BookStatus::Available,
        }
    }

    fn set_borrowed(&mut self) {
        self.status = BookStatus::Borrowed;
    }

    fn set_available(&mut self) {
        self.status = BookStatus::Available;
    }
}

#[derive(Debug, Clone)]
pub struct Loan {
    pub book_isbn: String,
    pub borrowed_at: chrono::NaiveDateTime,
    pub due_date: chrono::NaiveDateTime,
    pub returned_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct Member {
    pub id: uuid::Uuid,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub joined_at: chrono::NaiveDate,
    loans: Vec<Loan>,
}

impl Member {
    pub fn new(
        name: impl Into<String>,
        email: impl Into<String>,
        phone: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            email: email.into(),
            phone: phone.into(),
            joined_at: chrono::Utc::now().date_naive(),
            loans: Vec::new(),
        }
    }

    pub fn loans(&self) -> &[Loan] {
        &self.loans
    }

    fn active_loans(&self) -> usize {
        self.loans
            .iter()
            .filter(|l| l.returned_at.is_none())
            .count()
    }
}

pub struct LibraryManager {
    catalog: RwLock<HashMap<String, Book>>,
    members: RwLock<HashMap<uuid::Uuid, Member>>,
}

impl LibraryManager {
    const MAX_BOOKS_PER_MEMBER: usize = 5;
    const MAX_LENDING_DAYS: i64 = 10;

    /// The process-wide single instance.
    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<LibraryManager>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(LibraryManager::new()))
    }

    pub fn new() -> Self {
        Self {
            catalog: RwLock::new(HashMap::new()),
            members: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_book(&self, book: Book) -> LibraryResult<()> {
        let mut catalog = write_guard(&self.catalog);
        if catalog.contains_key(&book.isbn) {
            return Err(LibraryError::BookExists {
                isbn: book.isbn.clone(),
            });
        }
        catalog.insert(book.isbn.clone(), book);
        Ok(())
    }

    pub fn remove_book(&self, isbn: &str) -> LibraryResult<()> {
        let mut catalog = write_guard(&self.catalog);
        let book = catalog.get(isbn).ok_or(LibraryError::BookNotFound {
            isbn: isbn.to_string(),
        })?;
        if book.status != BookStatus::Available {
            return Err(LibraryError::BookBorrowed {
                isbn: isbn.to_string(),
            });
        }
        catalog.remove(isbn);
        Ok(())
    }

    pub fn register_member(&self, name: &str, email: &str, phone: &str) -> LibraryResult<Member> {
        let member = Member::new(name, email, phone);
        write_guard(&self.members).insert(member.id, member.clone());
        Ok(member)
    }

    pub fn unregister_member(&self, id: uuid::Uuid) -> LibraryResult<()> {
        let mut members = write_guard(&self.members);
        let member = members
            .get(&id)
            .ok_or(LibraryError::MemberNotFound { id })?;
        if member.active_loans() > 0 {
            return Err(LibraryError::MemberHasLoans { id });
        }
        members.remove(&id);
        Ok(())
    }

    /// Check out a book to a member. The catalog lock is taken before the
    /// member lock, the same order as `return_book`, so concurrent checkouts
    /// of the same book can never both succeed.
    pub fn borrow_book(&self, member_id: uuid::Uuid, isbn: &str) -> LibraryResult<Loan> {
        let mut catalog = write_guard(&self.catalog);
        let mut members = write_guard(&self.members);

        let book = catalog.get_mut(isbn).ok_or(LibraryError::BookNotFound {
            isbn: isbn.to_string(),
        })?;
        if book.status != BookStatus::Available {
            return Err(LibraryError::BookBorrowed {
                isbn: isbn.to_string(),
            });
        }
        let member = members
            .get_mut(&member_id)
            .ok_or(LibraryError::MemberNotFound { id: member_id })?;
        if member.active_loans() >= Self::MAX_BOOKS_PER_MEMBER {
            return Err(LibraryError::BorrowLimitReached {
                id: member_id,
                max: Self::MAX_BOOKS_PER_MEMBER,
            });
        }

        let borrowed_at = chrono::Utc::now().naive_utc();
        let loan = Loan {
            book_isbn: isbn.to_string(),
            borrowed_at,
            due_date: borrowed_at + chrono::Duration::days(Self::MAX_LENDING_DAYS),
            returned_at: None,
        };
        book.set_borrowed();
        member.loans.push(loan.clone());
        Ok(loan)
    }

    /// Return a loaned book. The outstanding loan gets a return date and the
    /// book becomes available again.
    pub fn return_book(&self, member_id: uuid::Uuid, isbn: &str) -> LibraryResult<Loan> {
        let mut catalog = write_guard(&self.catalog);
        let mut members = write_guard(&self.members);

        let book = catalog.get_mut(isbn).ok_or(LibraryError::BookNotFound {
            isbn: isbn.to_string(),
        })?;
        let member = members
            .get_mut(&member_id)
            .ok_or(LibraryError::MemberNotFound { id: member_id })?;
        let loan = member
            .loans
            .iter_mut()
            .find(|l| l.book_isbn == isbn && l.returned_at.is_none())
            .ok_or(LibraryError::BookNotOnLoan {
                isbn: isbn.to_string(),
                id: member_id,
            })?;
        loan.returned_at = Some(chrono::Utc::now().naive_utc());
        book.set_available();
        Ok(loan.clone())
    }

    /// Case-insensitive keyword search across title, author, and ISBN. An
    /// empty keyword returns the whole catalog.
    pub fn search_books(&self, keyword: &str) -> Vec<Book> {
        let keyword = keyword.to_lowercase();
        let catalog = read_guard(&self.catalog);
        catalog.values()
            .filter(|b| {
                b.title.to_lowercase().contains(&keyword)
                    || b.author.to_lowercase().contains(&keyword)
                    || b.isbn.to_lowercase().contains(&keyword)
            })
            .cloned()
            .collect()
    }
}

impl Default for LibraryManager {
    fn default() -> Self {
        Self::new()
    }
}

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

fn run_demo() {
    let system = LibraryManager::instance();

    let alice = system
        .register_member("Alice", "alice@example.com", "555-0100")
        .expect("member registered");
    system
        .add_book(Book::new(
            "978-0-13-110362-7",
            "The C Programming Language",
            "Kernighan & Ritchie",
            1978,
        ))
        .expect("book added");
    system
        .add_book(Book::new(
            "978-1-492-05559-3",
            "Programming Rust",
            "Blandy, Orendorff & Tindall",
            2021,
        ))
        .expect("book added");

    println!("== Search 'programming' ==");
    for book in system.search_books("programming") {
        println!(
            "  {} — {} ({}, {})",
            book.title, book.author, book.publication_year, book.isbn
        );
    }

    println!("\n== Borrow ==");
    let loan = system
        .borrow_book(alice.id, "978-0-13-110362-7")
        .expect("borrow succeeds");
    println!(
        "  {} loaned until {} ({} days)",
        loan.book_isbn,
        loan.due_date,
        LibraryManager::MAX_LENDING_DAYS
    );
    match system.borrow_book(alice.id, "978-0-13-110362-7") {
        Err(LibraryError::BookBorrowed { isbn }) => {
            println!("  second borrow of {isbn} rejected — no double lending")
        }
        other => panic!("expected BookBorrowed, got {other:?}"),
    }

    println!("\n== Return ==");
    let returned = system
        .return_book(alice.id, "978-0-13-110362-7")
        .expect("return succeeds");
    println!(
        "  returned on {}",
        returned.returned_at.expect("return date is set")
    );

    system
        .remove_book("978-0-13-110362-7")
        .expect("book removed");
    system
        .unregister_member(alice.id)
        .expect("member unregistered");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> LibraryManager {
        LibraryManager::new()
    }

    fn book(isbn: &str, title: &str, author: &str) -> Book {
        Book::new(isbn, title, author, 2020)
    }

    fn seed(manager: &LibraryManager) -> (uuid::Uuid, String) {
        let member = manager
            .register_member("Alice", "a@x.io", "555")
            .expect("member registered");
        let isbn = "isbn-1".to_string();
        manager
            .add_book(book(&isbn, "Rust Book", "A. Author"))
            .expect("book added");
        (member.id, isbn)
    }

    #[test]
    fn test_add_duplicate_book() {
        let manager = manager();
        manager
            .add_book(book("isbn-1", "Title", "Author"))
            .expect("first add");
        let result = manager.add_book(book("isbn-1", "Title", "Author"));
        assert!(matches!(result, Err(LibraryError::BookExists { .. })));
    }

    #[test]
    fn test_remove_missing_book() {
        let manager = manager();
        let result = manager.remove_book("missing");
        assert!(matches!(result, Err(LibraryError::BookNotFound { .. })));
    }

    #[test]
    fn test_remove_borrowed_book() {
        let manager = manager();
        let (member_id, isbn) = seed(&manager);
        manager
            .borrow_book(member_id, &isbn)
            .expect("borrow succeeds");
        let result = manager.remove_book(&isbn);
        assert!(matches!(result, Err(LibraryError::BookBorrowed { .. })));
    }

    #[test]
    fn test_borrow_missing_book() {
        let manager = manager();
        let member = manager
            .register_member("Alice", "a@x.io", "555")
            .expect("member registered");
        let result = manager.borrow_book(member.id, "missing");
        assert!(matches!(result, Err(LibraryError::BookNotFound { .. })));
    }

    #[test]
    fn test_borrow_missing_member() {
        let manager = manager();
        let isbn = "isbn-1".to_string();
        manager
            .add_book(book(&isbn, "Rust Book", "A. Author"))
            .expect("book added");
        let result = manager.borrow_book(uuid::Uuid::new_v4(), &isbn);
        assert!(matches!(result, Err(LibraryError::MemberNotFound { .. })));
    }

    #[test]
    fn test_borrow_unavailable() {
        let manager = manager();
        let (member_id, isbn) = seed(&manager);
        manager.borrow_book(member_id, &isbn).expect("first borrow");
        let result = manager.borrow_book(member_id, &isbn);
        assert!(matches!(result, Err(LibraryError::BookBorrowed { .. })));
    }

    #[test]
    fn test_borrow_limit() {
        let manager = manager();
        let member = manager
            .register_member("Alice", "a@x.io", "555")
            .expect("member registered");
        for i in 0..LibraryManager::MAX_BOOKS_PER_MEMBER {
            let isbn = format!("isbn-{i}");
            manager
                .add_book(book(&isbn, "Book", "Author"))
                .expect("book added");
            manager
                .borrow_book(member.id, &isbn)
                .expect("borrow within limit");
        }
        manager
            .add_book(book("isbn-x", "Book", "Author"))
            .expect("book added");
        let result = manager.borrow_book(member.id, "isbn-x");
        assert!(matches!(
            result,
            Err(LibraryError::BorrowLimitReached { .. })
        ));
    }

    #[test]
    fn test_return_releases_book() {
        let manager = manager();
        let (member_id, isbn) = seed(&manager);
        manager
            .borrow_book(member_id, &isbn)
            .expect("borrow succeeds");
        manager
            .return_book(member_id, &isbn)
            .expect("return succeeds");
        manager
            .borrow_book(member_id, &isbn)
            .expect("borrow succeeds again");
    }

    #[test]
    fn test_return_not_on_loan() {
        let manager = manager();
        let (member_id, isbn) = seed(&manager);
        let result = manager.return_book(member_id, &isbn);
        assert!(matches!(result, Err(LibraryError::BookNotOnLoan { .. })));
    }

    #[test]
    fn test_unregister_with_loans() {
        let manager = manager();
        let (member_id, isbn) = seed(&manager);
        manager
            .borrow_book(member_id, &isbn)
            .expect("borrow succeeds");
        let result = manager.unregister_member(member_id);
        assert!(matches!(result, Err(LibraryError::MemberHasLoans { .. })));
    }

    #[test]
    fn test_search_keyword() {
        let manager = manager();
        manager
            .add_book(book("isbn-1", "Rust in Action", "Blandy"))
            .expect("book added");
        manager
            .add_book(book("isbn-2", "The Rust Book", "Community"))
            .expect("book added");
        manager
            .add_book(book("isbn-3", "Go Programming", "Donovan"))
            .expect("book added");
        assert_eq!(manager.search_books("rust").len(), 2);
    }

    #[test]
    fn test_search_miss() {
        let manager = manager();
        manager
            .add_book(book("isbn-1", "Rust Book", "A. Author"))
            .expect("book added");
        assert!(manager.search_books("cobol").is_empty());
    }

    #[test]
    fn test_concurrent_borrow() {
        let manager = Arc::new(manager());
        let isbn = "isbn-1".to_string();
        manager
            .add_book(book(&isbn, "Rust Book", "A. Author"))
            .expect("book added");
        let members: Vec<uuid::Uuid> = (0..8)
            .map(|i| {
                manager
                    .register_member(&format!("user{i}"), "u@x.io", "555")
                    .expect("member registered")
                    .id
            })
            .collect();

        let handles: Vec<_> = members
            .into_iter()
            .map(|member_id| {
                let manager = Arc::clone(&manager);
                let isbn = isbn.clone();
                std::thread::spawn(move || manager.borrow_book(member_id, &isbn))
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    }
}

fn main() {
    run_demo();
    println!("\nAll done.");
}
