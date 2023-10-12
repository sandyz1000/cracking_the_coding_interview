#![allow(unused)]

use std::cmp::Eq;
use std::fmt;
use std::marker::PhantomData;
use chrono::prelude::*;
use std::collections::HashMap;

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

#[derive(Debug, PartialEq, Eq)]
enum AccountStatus {
    Active,
    Closed,
    Cancelled,
    Blacklisted,
    None,
}

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

#[derive(Debug, Clone)]
struct StockOrder {
    order_id: u32,
    quantity: i32,
    is_buy_order: bool,
    status: OrderStatus,
    time_enforcement: TimeEnforcementType,
    parts: Vec<Part>,
    creation_time: DateTime<Utc>,
}

/// ### Order
/// Members can place stock trading orders whenever they would like to sell or buy stock positions. 
/// The system would support multiple types of orders:
trait Order {
    fn set_status(&mut self, status: OrderStatus) -> bool;
    fn save_in_db(&self) -> bool;
    fn add_order_parts(&mut self, parts: Vec<Part>);
    fn update_in_db(&mut self ) -> bool;
    // fn get_order_id(&self) -> u32;
}

#[derive(Debug, Clone)]
struct Part {
    // Define the Part struct as needed
}

/// An order to buy or sell once the stock reaches a certain price.
#[derive(Debug, Clone)]
struct StopLossOrder {
    order: StockOrder,
    price_limit: f64,
}

impl StopLossOrder {
    fn new(
        order_id: u32, quantity: i32, price_limit: f64, status: OrderStatus, is_buy_order: bool
    ) -> Self {
        Self {
            order: StockOrder {
                order_id, 
                quantity, 
                is_buy_order, 
                status, 
                time_enforcement: TimeEnforcementType::GoodTillCancelled, 
                parts: Vec::new(), 
                creation_time: Utc::now()
            },
            price_limit
        }
    }
}

impl Order for StopLossOrder {
    fn set_status(&mut self, status: OrderStatus) -> bool {
        false
    }

    fn save_in_db(&self) -> bool {
        false
    }

    fn add_order_parts(&mut self, parts: Vec<Part>) {
        todo!()
    }

    fn update_in_db(&mut self ) -> bool {
        false
    }
}

/// The stop-limit order will be executed at a specified price or better after a given stop price has been reached. 
/// Once the stop price is reached, the stop-limit order becomes a limit order to buy or sell at the limit price or 
/// better.
#[derive(Debug, Clone)]
struct StopLimitOrder {
    order: StockOrder,
    price_limit: f64,
}

impl StopLimitOrder {
    fn new(
        order_id: u32, quantity: i32, price_limit: f64, status: OrderStatus, is_buy_order: bool
    ) -> Self {
        Self {
            order: StockOrder {
                order_id, 
                quantity, 
                is_buy_order, 
                status, 
                time_enforcement: TimeEnforcementType::GoodTillCancelled, 
                parts: Vec::new(), 
                creation_time: Utc::now()
            },
            price_limit
        }
    }
}

impl Order for StopLimitOrder {
    fn set_status(&mut self, status: OrderStatus) -> bool {
        todo!()
    }

    fn save_in_db(&self) -> bool {
        todo!()
    }

    fn add_order_parts(&mut self, parts: Vec<Part>) {
        todo!()
    }

    fn update_in_db(&mut self ) -> bool {
        false
    }
}

/// Market order will enable users to buy or sell stocks immediately at the current market price.
#[derive(Debug, Clone)]
struct MarketOrder {
    order: StockOrder,
}

impl MarketOrder {
    fn new(
        order_id: u32, quantity: i32, status: OrderStatus, is_buy_order: bool
    ) -> Self {
        Self {
            order: StockOrder {
                order_id, 
                quantity, 
                is_buy_order, 
                status, 
                time_enforcement: TimeEnforcementType::GoodTillCancelled, 
                parts: Vec::new(), 
                creation_time: Utc::now()
            }
        }
    }
}

impl Order for MarketOrder {
    fn set_status(&mut self, status: OrderStatus) -> bool {
        todo!()
    }

    fn save_in_db(&self) -> bool {
        todo!()
    }

    fn add_order_parts(&mut self, parts: Vec<Part>) {
        todo!()
    }

    fn update_in_db(&mut self ) -> bool {
        todo!()
    }

}

/// Limit orders will allow a user to set a price at which they want to buy or sell a stock.
#[derive(Debug, Clone)]
struct LimitOrder {
    order: StockOrder,
    price_limit: f64,
}

impl Order for LimitOrder {
    
    fn set_status(&mut self, status: OrderStatus) -> bool {
        self.order.status = status;
        true
    }

    fn save_in_db(&self) -> bool {
        // Implement the logic to save the order in the database
        false
    }

    fn add_order_parts(&mut self, parts: Vec<Part>) {
        self.order.parts.extend(parts);
    }

    fn update_in_db(&mut self ) -> bool {
        false
    }
}

impl LimitOrder {
    fn get_order_id(&self) -> u32 {
        unimplemented!()
    }

    fn new(
        id: u32, 
        quantity: i32, 
        price_limit: f64,
        time_enforcement: TimeEnforcementType, 
    ) -> Self {
        LimitOrder {
            order: StockOrder {
                order_id: id,
                quantity,
                is_buy_order: true,
                status: OrderStatus::Open,
                time_enforcement,
                parts: Vec::new(),
                creation_time: Utc::now(),
            },
            price_limit,
        }
    }
}

#[derive(Debug, Clone)]
struct StockInventory {
    inventory_name: String,
    last_updated_date: chrono::DateTime<Utc>
}

impl StockInventory {
    fn new(name: String, last_updated_date: chrono::DateTime<Utc>) -> Self {
        StockInventory {
            inventory_name: name,
            last_updated_date
        }
    }

    fn search_symbol(&self, symbol: &str) -> Stock {
        unimplemented!()
    }
}

/// The basic building block of the system. Every stock will have a symbol, current trading price, etc.
#[derive(Debug, Clone)]
struct Stock {
    symbol: String,
    price: f64,
}

impl Stock {
    fn new(symbol: String, price: f64) -> Self {
        Stock {
            symbol, price
        }
    }

    fn get_price(&self) -> f64 {
        self.price
    }
}

/// An order could be fulfilled in multiple parts. For example, a market order to buy 100 
/// stocks could have one part containing 70 stocks at $10 and another part with 30 stocks at $10.05.
#[derive(Debug, Clone)]
struct OrderPart {
    price: f64,
    quantity: i32,
    execution_date: chrono::DateTime<Utc>,
}

/// ### StockExchange system 
/// The stockbroker system will fetch all stocks and their current prices from the stock 
/// exchange. StockExchange will be a singleton class encapsulating all interactions with 
/// the stock exchange. This class will also be used to place stock trading orders on the 
/// stock exchange.
#[derive(Debug, Clone)]
struct StockExchange<O> {
    instance: Option<Box<OnlyOne<O>>>,
}

#[derive(Debug, Clone)]
struct OnlyOne<O> {
    _marker: PhantomData<O>,
    instance: StockExchange<O>
    // Define the OnlyOne struct as needed
}

impl<O: Order> StockExchange<O> {
    fn new() -> Self {
        StockExchange {
            instance: None
        }
    }

    fn get_instance(&self) -> &OnlyOne<O> {
        self.instance.as_ref().unwrap()
    }

    fn place_order(&self, order: &O) -> ReturnStatus {
        let return_status = self.get_instance().submit_order(order);
        return_status
    }
}

impl<O: Order> Default for StockExchange<O> {
    fn default() -> Self {
        StockExchange::new()
    }
}

impl<O: Order> OnlyOne<O> {
    fn new(instance: StockExchange<O>) -> Self {
        Self {
            instance,
            _marker: PhantomData,
            // Initialize the OnlyOne struct as needed
        }
    }

    fn submit_order(&self, order: &O) -> ReturnStatus {
        // Implement the logic to submit the order
        ReturnStatus::Success
    }
}

/// ### Account 
/// Consists of the member’s name, address, e-mail, phone, total funds, funds that are 
/// available for trading, etc. We’ll have two types of accounts in the system: one will 
/// be a general member, and the other will be an Admin. The Account class will also 
/// contain all the stocks the member is holding.
trait Account {
    fn reset_password(&self) -> bool;
}

impl AccountStatus {
    const NONE: u32 = 1;
}

struct Admin {
    id: u32,
    password: String,
    name: String,
    address: String,
    email: String,
    phone: String,
    status: u32,
    // available_funds_for_trading: f64,
    // date_of_membership: chrono::DateTime<Utc>,
    stock_positions: HashMap<u32, StockPosition>,
    active_orders: HashMap<u32, Box<dyn Order>>,
}

impl Account for Admin {
    fn reset_password(&self) -> bool {
        false
    }
}

impl Admin {
    fn block_member(&self) -> bool {
        // Implement the logic to block the member
        false
    }

    fn unblock_member(&self) -> bool {
        // Implement the logic to unblock the member
        false
    }
}

#[derive(Debug, Clone)]
struct Member {
    id: u32,
    password: String,
    name: String,
    address: String,
    email: String,
    phone: String,
    status: u32,
    available_funds_for_trading: f64,
    date_of_membership: chrono::DateTime<Utc>,
    stock_positions: HashMap<u32, StockPosition>,
    active_orders: HashMap<u32, LimitOrder>,
}

impl Account for Member {
    fn reset_password(&self) -> bool {
        // Implement the logic to reset the password
        false
    }
}

impl Member {
    fn new(id: u32, password: String, name: String, address: String, email: String, phone: String) -> Self {
        Member {
            id,
            password,
            name,
            address,
            email,
            phone,
            status: AccountStatus::NONE,
            available_funds_for_trading: 0.0,
            date_of_membership: chrono::Utc::now(),
            stock_positions: HashMap::new(),
            active_orders: HashMap::new(),
        }
    }

    fn place_sell_limit_order(
        &mut self, stock_id: u32, quantity: i32, limit_price: f64, enforcement_type: TimeEnforcementType
    ) -> ReturnStatus {
        // check if member has this stock position
        if !self.stock_positions.contains_key(&stock_id) {
            return ReturnStatus::NoStockPosition;
        }

        let stock_position = self.stock_positions.get(&stock_id).unwrap();
        // check if the member has enough quantity available to sell
        if stock_position.get_quantity() < quantity {
            return ReturnStatus::InsufficientQuantity;
        }

        let mut order = LimitOrder::new(stock_id, quantity, limit_price, enforcement_type);
        order.order.is_buy_order = false;
        order.save_in_db();
        let stock_exchange = StockExchange::<LimitOrder>::new();
        let success = stock_exchange.place_order(&order);
        if success == ReturnStatus::Success {
            order.set_status(OrderStatus::Filled);
            order.save_in_db();
        } else {
            self.active_orders.insert(order.get_order_id(), order);
        }
        success
    }

    fn place_buy_limit_order(
        &mut self, 
        stock_id: u32, quantity: i32, 
        limit_price: f64, enforcement_type: TimeEnforcementType
    ) -> ReturnStatus {
        // check if the member has enough funds to buy this stock
        if self.available_funds_for_trading < quantity as f64 * limit_price {
            return ReturnStatus::InsufficientFunds;
        }

        let mut order = LimitOrder::new(stock_id, quantity, limit_price, enforcement_type);
        order.order.is_buy_order = true;
        order.save_in_db();
        let stock_exchange = StockExchange::new();
        let success = stock_exchange.place_order(&order);
        if success != ReturnStatus::Success {
            order.set_status(OrderStatus::Filled);
            order.save_in_db();
        } else {
            self.active_orders.insert(order.get_order_id(), order);
        }
        success
    }

    fn callback_stock_exchange(&mut self, order_id: u32, order_parts: Vec<Part>, status: OrderStatus) {
        let order = self.active_orders.get_mut(&order_id).unwrap();
        order.add_order_parts(order_parts);
        order.set_status(status.clone());
        order.update_in_db();

        if status == OrderStatus::Filled || status == OrderStatus::Cancelled {
            self.active_orders.remove(&order_id);
        }
    }
}

/// This class will contain all the stocks that the user holds.
#[derive(Debug, Clone)]
struct StockPosition {
    // Define the StockPosition struct as needed
    symbol: String,
    quantity: f64,
}

impl StockPosition {
    fn get_quantity(&self) -> i32 {
        unimplemented!()
    }
}

/// ### StockLot
/// Any member can buy multiple lots of the same stock at different times. 
/// This class will represent these individual lots. For example, the user could have 
/// purchased 100 shares of AAPL yesterday and 50 more stocks of AAPL today. 
/// While selling, users will be able to select which lot they want to sell first.
#[derive(Debug, Clone)]
struct StockLot<O: Order> {
    lot_number: String,
    buying_order: O
}

impl <O: Order> StockLot<O> {
    fn new(lot_number: String, buying_order: O) -> Self {
        StockLot {
            lot_number, buying_order
        }
    }

    fn get_buying_price(&self) -> f64 {
        0.0
    }
}

/// A watchlist will contain a list of stocks that the member wants to follow.
#[derive(Debug, Clone)]
struct WatchList {
    name: String,
    stocks: Vec<Stock>,
}

impl WatchList {
    fn new(name: String, stocks: Vec<Stock>) -> Self {
        WatchList {
            name, stocks
        }
    }
    
    /// Add a stock to the watchlist 
    fn add_stock(&mut self, stock: Stock) {
        self.stocks.push(stock);
    }

    fn get_stocks(&self) -> Vec<Stock> {
        vec![]
    }
}
/// ### Statement
/// All members will have reports for quarterly updates and yearly tax statements.
#[derive(Debug, Clone)]
struct Statement {
    name: String,
    description: String
}

impl Statement {
    fn get_statement(&self) -> String {
        unimplemented!()
    }
}

struct QuarterlyStatement {
    statement: Statement,
    quarter_no: u32,
    year: u32,
}

impl QuarterlyStatement {
    fn new(name: String, description: String, quarter_no: u32, year: u32) -> Self {
        QuarterlyStatement {
            statement: Statement {
                name, description
            },
            quarter_no,
            year
        }
    }
    fn get_statement(&self) -> String {
        unimplemented!()
    }
}

struct AnnualStatement {
    statement: Statement,
    year: u32,
}

impl AnnualStatement {
    fn new(name: String, description: String, year: u32) -> Self {
        AnnualStatement {
            statement: Statement {
                name, description
            },
            year
        }
    }
    fn get_statement(&self) -> String {
        unimplemented!()
    }
}

/// ### DepositMoney & WithdrawMoney:
/// Members will be able to move money through check, wire or electronic bank transfers.
#[derive(Debug, Clone)]
struct TransferMoney {
    amount: f64,
    from_account_id: u32,
    to_account_id: u32,
    created_on: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct ElectronicWireTransfer {
    transfer: TransferMoney,
    bank_name: String,
    wire_id: u64
}

impl ElectronicWireTransfer {
    fn new(amount: f64, from_account_id: u32, to_account_id: u32, bank_name: String, wire_id: u64) -> Self {
        ElectronicWireTransfer {
            transfer: TransferMoney {
                amount, from_account_id, to_account_id, created_on: chrono::Utc::now()
            },
            bank_name,
            wire_id
        }    
    }

    fn get_wire_id(&self) -> u64 {
        unimplemented!()
    }
    
    fn initiate_transfer(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
struct CheckTransfer {
    transfer: TransferMoney,
    bank_name: String,
    check_number: String,
}

impl CheckTransfer {
    fn new(amount: f64, from_account_id: u32, to_account_id: u32, bank_name: String, check_number: String) -> Self {
        CheckTransfer {
            transfer: TransferMoney {
                amount, from_account_id, to_account_id, created_on: chrono::Utc::now()
            },
            bank_name,
            check_number
        }
    }    
}

#[derive(Debug, Clone)]
struct DepositMoney {
    amount: f64,
    from_account_id: u32,
    created_on: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct WithdrawMoney {
    amount: f64,
    from_account_id: u32,
}

impl TransferMoney {
    fn initiate_transfer(&self) -> bool {
        false
    }
}
/// ### Notification
/// Will take care of sending notifications to members.
#[derive(Debug, Clone)]
struct Notification {
    id: u32,
    created_on: chrono::DateTime<Utc>,
    message: String,
}

impl Notification {
    fn send(&self) -> bool {
        false
    }
}

struct SMSNotification {
    _notification: Notification,
    phone_number: String,
}

struct EmailNotification {
    _notification: Notification,
    email_address: String,
}

struct PushNotification {
    _notification: Notification,
    phone_number: String,
}

trait MyTrait {
    fn do_something(&self);
}

struct MyStruct {
    name: String,
    age: u32,
}

impl MyTrait for MyStruct {
    fn do_something(&self) {
        println!("My name is {} and I am {} years old.", self.name, self.age);
    }
}

fn main() {
    let my_trait_object: Box<dyn MyTrait> = Box::new(MyStruct {
        name: String::from("John"),
        age: 25,
    });
    
    my_trait_object.do_something();
}