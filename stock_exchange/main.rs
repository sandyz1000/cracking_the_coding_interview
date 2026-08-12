use chrono::prelude::*;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
enum ReturnStatus {
    Success,
    Fail,
    InsufficientFunds,
    InsufficientQuantity,
    NoStockPosition,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum OrderStatus {
    Open,
    Filled,
    PartiallyFilled,
    Cancelled,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum TimeEnforcementType {
    GoodTillCancelled,
    FillOrKill,
    ImmediateOrCancel,
    OnTheOpen,
    OnTheClose,
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum AccountStatus {
    Active,
    Closed,
    Cancelled,
    Blacklisted,
    None,
}

#[derive(Debug, Clone)]
struct Location {
    street_address: String,
    city: String,
    state: String,
    zip_code: String,
    country: String,
}

impl Location {
    fn new(street: String, city: String, state: String, zip_code: String, country: String) -> Self {
        Location {
            street_address: street,
            city,
            state,
            zip_code,
            country,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}, {} {}",
            self.street_address, self.city, self.state, self.zip_code, self.country
        )
    }
}

struct Constants {
    money_transfer_limit: i32,
}

impl Constants {
    fn new() -> Self {
        Constants {
            money_transfer_limit: 100000,
        }
    }
}

/// The part of an order that was executed at a single price, e.g. a market
/// order for 100 shares can fill as 70 at $10 and 30 at $10.05.
#[derive(Debug, Clone)]
struct OrderPart {
    price: f64,
    quantity: i32,
    execution_date: DateTime<Utc>,
}

/// The order-type-specific behavior, composed into an `Order` as its kind.
///
/// * `Market` fills immediately at the current market price.
/// * `Limit` fills at the limit price or better.
/// * `StopLoss` triggers once the stop price is reached.
/// * `StopLimit` becomes a limit order at `limit_price` once `stop_price` is
///   reached.
#[derive(Debug, Clone)]
enum OrderKind {
    Market(f64),
    Limit(f64),
    StopLoss(f64),
    StopLimit { stop_price: f64, limit_price: f64 },
}

/// An order is a composition of the data shared by every order and the
/// `OrderKind` that gives it its pricing behavior.
#[derive(Debug, Clone)]
struct Order {
    order_id: u32,
    symbol: String,
    quantity: i32,
    is_buy_order: bool,
    status: OrderStatus,
    time_enforcement: TimeEnforcementType,
    parts: Vec<OrderPart>,
    creation_time: DateTime<Utc>,
    kind: OrderKind,
}

impl Order {
    fn new(
        order_id: u32,
        symbol: &str,
        quantity: i32,
        kind: OrderKind,
        time_enforcement: TimeEnforcementType,
    ) -> Self {
        Order {
            order_id,
            symbol: symbol.to_string(),
            quantity,
            is_buy_order: true,
            status: OrderStatus::Open,
            time_enforcement,
            parts: Vec::new(),
            creation_time: Utc::now(),
            kind,
        }
    }

    /// The price at which this order executes.
    fn get_price(&self) -> f64 {
        match &self.kind {
            OrderKind::Market(price) => *price,
            OrderKind::Limit(price) => *price,
            OrderKind::StopLoss(stop_price) => *stop_price,
            OrderKind::StopLimit { limit_price, .. } => *limit_price,
        }
    }

    fn describe(&self) -> String {
        match &self.kind {
            OrderKind::Market(price) => format!("market order executing at ${price:.2}"),
            OrderKind::Limit(price) => format!("limit order at ${price:.2}"),
            OrderKind::StopLoss(stop_price) => {
                format!("stop-loss order triggering at ${stop_price:.2}")
            }
            OrderKind::StopLimit {
                stop_price,
                limit_price,
            } => format!(
                "stop-limit order becoming a limit order at ${limit_price:.2} once ${stop_price:.2} is reached"
            ),
        }
    }

    fn set_status(&mut self, status: OrderStatus) {
        self.status = status;
    }

    fn save_in_db(&self) {
        println!(
            "Order {} for {} ({} share(s), {:?}) created at {} – saved to DB",
            self.order_id,
            self.symbol,
            self.quantity,
            self.time_enforcement,
            self.creation_time.format("%Y-%m-%d %H:%M:%S")
        );
    }

    fn update_in_db(&mut self) {
        let filled_value: f64 = self
            .parts
            .iter()
            .map(|part| part.price * part.quantity as f64)
            .sum();
        let last_execution = self.parts.iter().map(|part| part.execution_date).max();
        println!(
            "Order {} is now {:?} with ${filled_value:.2} filled (last execution {last_execution:?}) – DB updated",
            self.order_id, self.status
        );
    }

    fn add_order_parts(&mut self, parts: Vec<OrderPart>) {
        self.parts.extend(parts);
    }
}

/// Lazy singleton holding the order book, mirroring the `__OnlyOne` inner
/// class of the reference design.
#[derive(Debug, Clone)]
struct OnlyOne {
    order_book: RefCell<HashMap<u32, Order>>,
}

impl OnlyOne {
    fn new() -> Self {
        OnlyOne {
            order_book: RefCell::new(HashMap::new()),
        }
    }

    fn submit_order(&self, order: &Order) -> ReturnStatus {
        self.order_book
            .borrow_mut()
            .insert(order.order_id, order.clone());
        println!("Order {} submitted to the stock exchange", order.order_id);
        ReturnStatus::Success
    }

    fn get_order_count(&self) -> usize {
        self.order_book.borrow().len()
    }
}

#[derive(Debug, Clone)]
struct StockExchange {
    instance: RefCell<Option<OnlyOne>>,
}

impl StockExchange {
    fn new() -> Self {
        StockExchange {
            instance: RefCell::new(None),
        }
    }

    fn get_instance(&self) -> RefMut<'_, OnlyOne> {
        RefMut::map(self.instance.borrow_mut(), |slot| {
            slot.get_or_insert_with(OnlyOne::new)
        })
    }

    fn place_order(&self, order: &Order) -> ReturnStatus {
        self.get_instance().submit_order(order)
    }
}

trait Account {
    fn reset_password(&self);
}

struct Admin {
    id: u32,
    password: String,
    name: String,
    address: Location,
    email: String,
    phone: String,
    status: AccountStatus,
    members: Vec<Member>,
}

impl Account for Admin {
    fn reset_password(&self) {
        println!(
            "Password reset for admin {} <{}> ({}) at {}",
            self.name, self.email, self.phone, self.address
        );
    }
}

impl Admin {
    fn new(
        id: u32,
        password: String,
        name: String,
        address: Location,
        email: String,
        phone: String,
    ) -> Self {
        Admin {
            id,
            password,
            name,
            address,
            email,
            phone,
            status: AccountStatus::Active,
            members: Vec::new(),
        }
    }

    fn add_member(&mut self, member: Member) {
        self.members.push(member);
    }

    fn block_member(&mut self, member_id: u32) -> bool {
        match self
            .members
            .iter_mut()
            .find(|member| member.id == member_id)
        {
            Some(member) => {
                member.status = AccountStatus::Blacklisted;
                println!("Member {member_id} blocked");
                true
            }
            None => false,
        }
    }

    fn unblock_member(&mut self, member_id: u32) -> bool {
        match self
            .members
            .iter_mut()
            .find(|member| member.id == member_id)
        {
            Some(member) => {
                member.status = AccountStatus::Active;
                println!("Member {member_id} unblocked");
                true
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone)]
struct Member {
    id: u32,
    password: String,
    name: String,
    address: Location,
    email: String,
    phone: String,
    status: AccountStatus,
    available_funds_for_trading: f64,
    date_of_membership: DateTime<Utc>,
    stock_positions: HashMap<u32, StockPosition>,
    active_orders: HashMap<u32, Order>,
}

impl Account for Member {
    fn reset_password(&self) {
        println!(
            "Password reset for member {} <{}> ({}) at {}",
            self.name, self.email, self.phone, self.address
        );
    }
}

impl Member {
    fn new(
        id: u32,
        password: String,
        name: String,
        address: Location,
        email: String,
        phone: String,
    ) -> Self {
        Member {
            id,
            password,
            name,
            address,
            email,
            phone,
            status: AccountStatus::Active,
            available_funds_for_trading: 0.0,
            date_of_membership: Utc::now(),
            stock_positions: HashMap::new(),
            active_orders: HashMap::new(),
        }
    }

    fn deposit(&mut self, amount: f64) -> ReturnStatus {
        if amount <= 0.0 {
            return ReturnStatus::Fail;
        }
        self.available_funds_for_trading += amount;
        println!(
            "Deposited ${amount:.2}; available for trading: ${:.2}",
            self.available_funds_for_trading
        );
        ReturnStatus::Success
    }

    fn withdraw(&mut self, amount: f64) -> ReturnStatus {
        if amount <= 0.0 || amount > self.available_funds_for_trading {
            return ReturnStatus::Fail;
        }
        self.available_funds_for_trading -= amount;
        println!(
            "Withdrew ${amount:.2}; available for trading: ${:.2}",
            self.available_funds_for_trading
        );
        ReturnStatus::Success
    }

    fn add_stock_position(&mut self, stock_id: u32, symbol: String, quantity: i32) {
        self.stock_positions
            .entry(stock_id)
            .and_modify(|position| position.quantity += quantity)
            .or_insert(StockPosition::new(symbol, quantity));
    }

    fn remove_stock_position(&mut self, stock_id: u32, quantity: i32) {
        let remaining = match self.stock_positions.get_mut(&stock_id) {
            Some(position) => {
                position.quantity -= quantity;
                position.quantity
            }
            None => 0,
        };
        if remaining <= 0 {
            self.stock_positions.remove(&stock_id);
        }
    }

    fn place_buy_limit_order(
        &mut self,
        stock_id: u32,
        symbol: &str,
        quantity: i32,
        limit_price: f64,
        enforcement_type: TimeEnforcementType,
    ) -> ReturnStatus {
        if self.available_funds_for_trading < quantity as f64 * limit_price {
            return ReturnStatus::InsufficientFunds;
        }

        let order = Order::new(
            stock_id,
            symbol,
            quantity,
            OrderKind::Limit(limit_price),
            enforcement_type,
        );
        order.save_in_db();
        let stock_exchange = StockExchange::new();
        if stock_exchange.place_order(&order) == ReturnStatus::Success {
            self.active_orders.insert(stock_id, order);
            ReturnStatus::Success
        } else {
            ReturnStatus::Fail
        }
    }

    fn place_sell_limit_order(
        &mut self,
        stock_id: u32,
        symbol: &str,
        quantity: i32,
        limit_price: f64,
        enforcement_type: TimeEnforcementType,
    ) -> ReturnStatus {
        let Some(position) = self.stock_positions.get(&stock_id) else {
            return ReturnStatus::NoStockPosition;
        };
        if position.get_quantity() < quantity {
            return ReturnStatus::InsufficientQuantity;
        }

        let mut order = Order::new(
            stock_id,
            symbol,
            quantity,
            OrderKind::Limit(limit_price),
            enforcement_type,
        );
        order.is_buy_order = false;
        order.save_in_db();
        let stock_exchange = StockExchange::new();
        if stock_exchange.place_order(&order) == ReturnStatus::Success {
            self.active_orders.insert(stock_id, order);
            ReturnStatus::Success
        } else {
            ReturnStatus::Fail
        }
    }

    /// Invoked by the stock exchange whenever an open order gets a fill update.
    fn callback_stock_exchange(
        &mut self,
        order_id: u32,
        order_parts: Vec<OrderPart>,
        status: OrderStatus,
    ) {
        let Some(order) = self.active_orders.get_mut(&order_id) else {
            return;
        };
        order.add_order_parts(order_parts);
        order.set_status(status.clone());
        order.update_in_db();

        // Positions change only when an order reaches Filled, not on partial fills.
        if status == OrderStatus::Filled {
            let symbol = order.symbol.clone();
            let quantity = order.quantity;
            if order.is_buy_order {
                self.add_stock_position(order_id, symbol, quantity);
            } else {
                self.remove_stock_position(order_id, quantity);
            }
        }

        if status == OrderStatus::Filled || status == OrderStatus::Cancelled {
            self.active_orders.remove(&order_id);
        }
    }
}

#[derive(Debug, Clone)]
struct StockPosition {
    symbol: String,
    quantity: i32,
}

impl StockPosition {
    fn new(symbol: String, quantity: i32) -> Self {
        StockPosition { symbol, quantity }
    }

    fn get_quantity(&self) -> i32 {
        self.quantity
    }
}

#[derive(Debug, Clone)]
struct StockLot {
    lot_number: String,
    buying_order: Order,
}

impl StockLot {
    fn new(lot_number: String, buying_order: Order) -> Self {
        StockLot {
            lot_number,
            buying_order,
        }
    }

    fn get_buying_price(&self) -> f64 {
        self.buying_order.get_price()
    }
}

#[derive(Debug, Clone)]
struct WatchList {
    name: String,
    stocks: Vec<Stock>,
}

impl WatchList {
    fn new(name: String) -> Self {
        WatchList {
            name,
            stocks: Vec::new(),
        }
    }

    fn add_stock(&mut self, stock: Stock) {
        self.stocks.push(stock);
    }

    fn remove_stock(&mut self, symbol: &str) -> bool {
        let before = self.stocks.len();
        self.stocks.retain(|stock| stock.symbol != symbol);
        self.stocks.len() < before
    }

    fn get_stocks(&self) -> &[Stock] {
        &self.stocks
    }
}

#[derive(Debug, Clone)]
struct Stock {
    symbol: String,
    price: f64,
}

impl Stock {
    fn new(symbol: String, price: f64) -> Self {
        Stock { symbol, price }
    }

    fn get_symbol(&self) -> &str {
        &self.symbol
    }

    fn get_price(&self) -> f64 {
        self.price
    }

    fn set_price(&mut self, price: f64) {
        self.price = price;
    }
}

#[derive(Debug, Clone)]
struct StockInventory {
    inventory_name: String,
    last_updated_date: DateTime<Utc>,
    stocks: Vec<Stock>,
}

impl StockInventory {
    fn new(name: String, last_updated_date: DateTime<Utc>) -> Self {
        StockInventory {
            inventory_name: name,
            last_updated_date,
            stocks: Vec::new(),
        }
    }

    fn add_stock(&mut self, stock: Stock) {
        self.stocks.push(stock);
    }

    fn search_symbol(&self, symbol: &str) -> Option<Stock> {
        self.stocks
            .iter()
            .find(|stock| stock.symbol == symbol)
            .cloned()
    }
}

#[derive(Debug, Clone)]
struct Statement {
    name: String,
    description: String,
}

impl Statement {
    fn get_statement(&self) -> String {
        format!("{} – {}", self.name, self.description)
    }
}

struct QuarterlyStatement {
    statement: Statement,
    quarter_no: u32,
    year: u32,
}

impl QuarterlyStatement {
    fn get_statement(&self) -> String {
        format!(
            "Q{} {} report: {}",
            self.quarter_no,
            self.year,
            self.statement.get_statement()
        )
    }
}

struct AnnualStatement {
    statement: Statement,
    year: u32,
}

impl AnnualStatement {
    fn get_statement(&self) -> String {
        format!(
            "FY{} tax statement: {}",
            self.year,
            self.statement.get_statement()
        )
    }
}

#[derive(Debug, Clone)]
struct TransferMoney {
    amount: f64,
    from_account_id: u32,
    to_account_id: u32,
    created_on: DateTime<Utc>,
}

impl TransferMoney {
    fn new(amount: f64, from_account_id: u32, to_account_id: u32) -> Self {
        TransferMoney {
            amount,
            from_account_id,
            to_account_id,
            created_on: Utc::now(),
        }
    }

    fn transfer(&self) -> ReturnStatus {
        if self.amount <= 0.0 || self.amount > Constants::new().money_transfer_limit as f64 {
            println!(
                "Transfer of ${:.2} rejected – over the ${} limit",
                self.amount,
                Constants::new().money_transfer_limit
            );
            return ReturnStatus::Fail;
        }
        println!(
            "Transferred ${:.2} from account {} to account {} on {}",
            self.amount,
            self.from_account_id,
            self.to_account_id,
            self.created_on.format("%Y-%m-%d %H:%M")
        );
        ReturnStatus::Success
    }
}

struct ElectronicWireTransfer {
    transfer: TransferMoney,
    bank_name: String,
    wire_id: u64,
}

impl ElectronicWireTransfer {
    fn new(
        amount: f64,
        from_account_id: u32,
        to_account_id: u32,
        bank_name: String,
        wire_id: u64,
    ) -> Self {
        ElectronicWireTransfer {
            transfer: TransferMoney::new(amount, from_account_id, to_account_id),
            bank_name,
            wire_id,
        }
    }

    fn transfer(&self) -> ReturnStatus {
        println!("Wire #{} via {}", self.wire_id, self.bank_name);
        self.transfer.transfer()
    }
}

struct CheckTransfer {
    transfer: TransferMoney,
    bank_name: String,
    check_number: String,
}

impl CheckTransfer {
    fn new(
        amount: f64,
        from_account_id: u32,
        to_account_id: u32,
        bank_name: String,
        check_number: String,
    ) -> Self {
        CheckTransfer {
            transfer: TransferMoney::new(amount, from_account_id, to_account_id),
            bank_name,
            check_number,
        }
    }

    fn transfer(&self) -> ReturnStatus {
        println!("Check {} drawn on {}", self.check_number, self.bank_name);
        self.transfer.transfer()
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
            "Notification #{} on {}: {}",
            self.id,
            self.created_on.format("%Y-%m-%d %H:%M"),
            self.message
        );
        true
    }
}

struct SMSNotification {
    notification: Notification,
    phone_number: String,
}

impl SMSNotification {
    fn new(id: u32, message: String, phone_number: String) -> Self {
        SMSNotification {
            notification: Notification::new(id, message),
            phone_number,
        }
    }

    fn send(&self) -> bool {
        println!(
            "SMS to {}: {}",
            self.phone_number, self.notification.message
        );
        true
    }
}

struct EmailNotification {
    notification: Notification,
    email_address: String,
}

impl EmailNotification {
    fn new(id: u32, message: String, email_address: String) -> Self {
        EmailNotification {
            notification: Notification::new(id, message),
            email_address,
        }
    }

    fn send(&self) -> bool {
        println!(
            "Email to {}: {}",
            self.email_address, self.notification.message
        );
        true
    }
}

struct PushNotification {
    notification: Notification,
    phone_number: String,
}

impl PushNotification {
    fn new(id: u32, message: String, phone_number: String) -> Self {
        PushNotification {
            notification: Notification::new(id, message),
            phone_number,
        }
    }

    fn send(&self) -> bool {
        println!(
            "Push to {}: {}",
            self.phone_number, self.notification.message
        );
        true
    }
}

fn main() {
    // Stock inventory maintained by the system.
    let mut inventory = StockInventory::new("NSE".to_string(), Utc::now());
    inventory.add_stock(Stock::new("AAPL".to_string(), 175.25));
    inventory.add_stock(Stock::new("GOOG".to_string(), 2750.0));
    inventory.add_stock(Stock::new("TSLA".to_string(), 245.8));
    if let Some(aapl) = inventory
        .stocks
        .iter_mut()
        .find(|stock| stock.symbol == "AAPL")
    {
        aapl.set_price(178.0);
    }
    println!(
        "Inventory {} last updated at {}",
        inventory.inventory_name,
        inventory.last_updated_date.format("%Y-%m-%d %H:%M:%S")
    );
    match inventory.search_symbol("AAPL") {
        Some(stock) => println!(
            "Latest quote for {}: ${:.2}",
            stock.get_symbol(),
            stock.get_price()
        ),
        None => println!("AAPL not found"),
    }
    println!(
        "Quote for unknown symbol MSFT: {:?}",
        inventory.search_symbol("MSFT")
    );

    // Member account with funds for trading.
    let address = Location::new(
        "1 Market Street".to_string(),
        "San Francisco".to_string(),
        "CA".to_string(),
        "94105".to_string(),
        "USA".to_string(),
    );
    let mut member = Member::new(
        101,
        "s3cret".to_string(),
        "Alice".to_string(),
        address,
        "alice@example.com".to_string(),
        "+1-555-0100".to_string(),
    );
    assert!(!member.password.is_empty());
    assert_eq!(member.deposit(100_000.0), ReturnStatus::Success);
    assert_eq!(member.withdraw(1_000.0), ReturnStatus::Success);
    println!(
        "{} has been a member since {} and has ${:.2} available for trading",
        member.name,
        member.date_of_membership.format("%Y-%m-%d"),
        member.available_funds_for_trading
    );

    // Watchlists track multiple stock quotes.
    let mut watchlist = WatchList::new("Tech picks".to_string());
    watchlist.add_stock(Stock::new("AAPL".to_string(), 178.0));
    watchlist.add_stock(Stock::new("GOOG".to_string(), 2750.0));
    println!(
        "Watchlist \"{}\" tracks {} stock(s)",
        watchlist.name,
        watchlist.get_stocks().len()
    );
    assert!(watchlist.remove_stock("GOOG"));
    println!(
        "After removing GOOG: {} stock(s) left",
        watchlist.get_stocks().len()
    );

    // A buy order that fills in multiple parts updates the position only when filled.
    assert_eq!(
        member.place_buy_limit_order(
            1,
            "AAPL",
            100,
            175.0,
            TimeEnforcementType::GoodTillCancelled
        ),
        ReturnStatus::Success
    );
    member.callback_stock_exchange(
        1,
        vec![OrderPart {
            price: 175.0,
            quantity: 70,
            execution_date: Utc::now(),
        }],
        OrderStatus::PartiallyFilled,
    );
    assert!(!member.stock_positions.contains_key(&1));
    member.callback_stock_exchange(
        1,
        vec![OrderPart {
            price: 175.05,
            quantity: 30,
            execution_date: Utc::now(),
        }],
        OrderStatus::Filled,
    );
    assert_eq!(
        member
            .stock_positions
            .get(&1)
            .map(StockPosition::get_quantity),
        Some(100)
    );
    assert!(member.active_orders.is_empty());

    // Buying beyond available funds is rejected.
    assert_eq!(
        member.place_buy_limit_order(
            2,
            "GOOG",
            100,
            2750.0,
            TimeEnforcementType::GoodTillCancelled
        ),
        ReturnStatus::InsufficientFunds
    );

    // Selling a stock with no position is rejected.
    assert_eq!(
        member.place_sell_limit_order(2, "GOOG", 10, 2800.0, TimeEnforcementType::OnTheClose),
        ReturnStatus::NoStockPosition
    );

    // A buy order can also be cancelled before execution.
    assert_eq!(
        member.place_buy_limit_order(
            2,
            "GOOG",
            10,
            2700.0,
            TimeEnforcementType::GoodTillCancelled
        ),
        ReturnStatus::Success
    );
    member.callback_stock_exchange(2, vec![], OrderStatus::Cancelled);
    assert!(!member.stock_positions.contains_key(&2));
    assert!(member.active_orders.is_empty());

    // Selling 40 of the 100 AAPL shares leaves a position of 60.
    assert_eq!(
        member.place_sell_limit_order(1, "AAPL", 40, 180.0, TimeEnforcementType::OnTheClose),
        ReturnStatus::Success
    );
    member.callback_stock_exchange(
        1,
        vec![OrderPart {
            price: 180.0,
            quantity: 40,
            execution_date: Utc::now(),
        }],
        OrderStatus::Filled,
    );
    assert_eq!(
        member
            .stock_positions
            .get(&1)
            .map(StockPosition::get_quantity),
        Some(60)
    );

    // Selling more than held is rejected.
    assert_eq!(
        member.place_sell_limit_order(1, "AAPL", 1_000, 180.0, TimeEnforcementType::OnTheClose),
        ReturnStatus::InsufficientQuantity
    );
    if let Some(position) = member.stock_positions.get(&1) {
        println!(
            "{} position: {} share(s)",
            position.symbol,
            position.get_quantity()
        );
    }

    // Lots distinguish purchases of the same stock made at different times.
    let lot = StockLot::new(
        "LOT-001".to_string(),
        Order::new(
            1,
            "AAPL",
            100,
            OrderKind::Limit(175.0),
            TimeEnforcementType::OnTheOpen,
        ),
    );
    println!(
        "Lot {} was bought at ${:.2}",
        lot.lot_number,
        lot.get_buying_price()
    );

    // The other supported order kinds.
    let market_order = Order::new(
        20,
        "TSLA",
        50,
        OrderKind::Market(246.0),
        TimeEnforcementType::ImmediateOrCancel,
    );
    let stop_loss_order = Order::new(
        21,
        "AAPL",
        30,
        OrderKind::StopLoss(160.0),
        TimeEnforcementType::GoodTillCancelled,
    );
    let stop_limit_order = Order::new(
        22,
        "GOOG",
        10,
        OrderKind::StopLimit {
            stop_price: 2700.0,
            limit_price: 2600.0,
        },
        TimeEnforcementType::FillOrKill,
    );
    market_order.save_in_db();
    stop_loss_order.save_in_db();
    stop_limit_order.save_in_db();
    println!("{}", market_order.describe());
    println!("{}", stop_loss_order.describe());
    println!("{}", stop_limit_order.describe());

    // The StockExchange singleton accepts orders into its order book.
    let exchange = StockExchange::new();
    let order_a = Order::new(
        30,
        "AAPL",
        25,
        OrderKind::Limit(174.0),
        TimeEnforcementType::GoodTillCancelled,
    );
    let order_b = Order::new(
        31,
        "TSLA",
        5,
        OrderKind::Limit(240.0),
        TimeEnforcementType::GoodTillCancelled,
    );
    assert_eq!(exchange.place_order(&order_a), ReturnStatus::Success);
    assert_eq!(exchange.place_order(&order_b), ReturnStatus::Success);
    println!(
        "The StockExchange singleton has recorded {} open order(s)",
        exchange.get_instance().get_order_count()
    );

    // Notifications for executed orders and system events.
    let notice = Notification::new(1, "System maintenance at 02:00 UTC".to_string());
    assert!(notice.send());
    let email = EmailNotification::new(
        2,
        "Your buy order #1 was filled".to_string(),
        "alice@example.com".to_string(),
    );
    assert!(email.send());
    let sms = SMSNotification::new(
        3,
        "AAPL crossed your target price".to_string(),
        "+1-555-0100".to_string(),
    );
    assert!(sms.send());
    let push = PushNotification::new(
        4,
        "Your portfolio changed".to_string(),
        "+1-555-0100".to_string(),
    );
    assert!(push.send());

    // Quarterly and annual statements.
    let quarterly = QuarterlyStatement {
        statement: Statement {
            name: "Q3 2025 portfolio update".to_string(),
            description: "Quarterly performance report".to_string(),
        },
        quarter_no: 3,
        year: 2025,
    };
    let annual = AnnualStatement {
        statement: Statement {
            name: "FY 2025 tax statement".to_string(),
            description: "Capital gains and losses".to_string(),
        },
        year: 2025,
    };
    println!("{}", quarterly.get_statement());
    println!("{}", annual.get_statement());

    // Deposits and withdrawals via wire or check, capped by the transfer limit.
    let wire = ElectronicWireTransfer::new(50_000.0, 101, 202, "ICICI Bank".to_string(), 9001);
    assert_eq!(wire.transfer(), ReturnStatus::Success);
    let check = CheckTransfer::new(
        500_000.0,
        101,
        202,
        "Chase".to_string(),
        "CHK-0042".to_string(),
    );
    assert_eq!(check.transfer(), ReturnStatus::Fail);

    // Admin manages members.
    member.reset_password();
    let admin_address = Location::new(
        "5 Admin Road".to_string(),
        "New York".to_string(),
        "NY".to_string(),
        "10001".to_string(),
        "USA".to_string(),
    );
    let mut admin = Admin::new(
        1,
        "root".to_string(),
        "Bob".to_string(),
        admin_address,
        "bob@example.com".to_string(),
        "+1-555-0199".to_string(),
    );
    assert!(!admin.password.is_empty());
    admin.add_member(member);
    assert!(admin.block_member(101));
    assert_eq!(admin.members[0].status, AccountStatus::Blacklisted);
    assert!(admin.unblock_member(101));
    assert_eq!(admin.members[0].status, AccountStatus::Active);
    admin.reset_password();
    println!(
        "Admin #{} {} is {:?} and manages {} member(s)",
        admin.id,
        admin.name,
        admin.status,
        admin.members.len()
    );
    println!(
        "{} holds ${:.2} in trading funds",
        admin.members[0].name, admin.members[0].available_funds_for_trading
    );

    let statuses = [
        AccountStatus::Active,
        AccountStatus::Closed,
        AccountStatus::Cancelled,
        AccountStatus::Blacklisted,
        AccountStatus::None,
    ];
    println!("Account statuses in the domain: {statuses:?}");
}
