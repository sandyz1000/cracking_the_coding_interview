// Simple rate‑based pricing or promotion engine
// Given a basket and a set of discount rules, compute final price; often tests decomposition, data modelling, and handling overlapping

// For this prompt, the expectation is that you:

//     Model the domain cleanly (basket, line items, discounts).

//     Make discount rules composable.

//     Explain how you handle overlapping promotions and the complexity of your approach.

// Here’s a compact  sketch you could type out and then discuss.

// 1. Data model

// ## Time and Space Complexity
// Let I = number of items, R = number of rules.
// Each rule scans items at worst, so computing final price is O(R · I) time.
// Space is O(I + R) for basket and rules; no extra asymptotic overhead.

#[derive(Clone, Debug)]
struct Item {
    sku: String,
    unit_price: f64,
    quantity: u32,
}

#[derive(Clone, Debug)]
struct Basket {
    items: Vec<Item>,
}

impl Basket {
    fn subtotal(&self) -> f64 {
        self.items
            .iter()
            .map(|i| i.unit_price * i.quantity as f64)
            .sum()
    }
}

// 2. Discount rules


// SKU stands for Stock Keeping Unit.
// It’s a short code (like "APPLE", "MILK-1L") that uniquely identifies a specific 
// product in inventory so you can track prices, stock levels, and discounts per product. 
// In the promotion engine example, sku is just the product ID you match rules against.
// In your own projects, what identifier do you usually use for products—database ID, 
// human-readable code, or something else?
#[derive(Clone, Debug)]
enum DiscountRule {
    // e.g. 10% off whole basket over a threshold
    PercentageOverTotal { threshold: f64, percent: f64 },

    // e.g. 3 for 2 on a SKU (buy N get M free)
    BuyNGetMFree { sku: String, buy: u32, free: u32 },

    // e.g. fixed amount off per unit of a SKU
    FixedPerItem { sku: String, amount: f64 },
}

impl DiscountRule {
    fn discount(&self, basket: &Basket) -> f64 {
        match self {
            DiscountRule::PercentageOverTotal { threshold, percent } => {
                let sub = basket.subtotal();
                if sub >= *threshold {
                    sub * (percent / 100.0)
                } else {
                    0.0
                }
            }

            DiscountRule::BuyNGetMFree { sku, buy, free } => {
                let mut disc = 0.0;
                for item in &basket.items {
                    if &item.sku != sku || *buy == 0 {
                        continue;
                    }
                    let group = buy + free;
                    let times = item.quantity / group;
                    let free_qty = times * free;
                    disc += free_qty as f64 * item.unit_price;
                }
                disc
            }

            DiscountRule::FixedPerItem { sku, amount } => {
                basket
                    .items
                    .iter()
                    .filter(|i| &i.sku == sku)
                    .map(|i| *amount * i.quantity as f64)
                    .sum()
            }
        }
    }
}

// 3. Promotion engine and overlapping

// For a first pass, apply all rules and clamp at zero. In an interview you’d explicitly say “this stacks discounts; a real system might choose the best subset or mark some discounts as non‑stackable.”


#[derive(Debug)]
struct PromotionEngine {
    rules: Vec<DiscountRule>,
}

impl PromotionEngine {
    fn new(rules: Vec<DiscountRule>) -> Self {
        Self { rules }
    }

    fn final_price(&self, basket: &Basket) -> f64 {
        let sub = basket.subtotal();
        let mut total_discount = 0.0;

        for rule in &self.rules {
            total_discount += rule.discount(basket);
        }

        let final_price = (sub - total_discount).max(0.0);
        // round to cents
        (final_price * 100.0).round() / 100.0
    }


    // Use this when you want to apply stackable and best from non-stackable discount
    // fn final_price(&self, basket: &Basket) -> f64 {
    //     let sub = basket.subtotal();

    //     let (stackable, non_stackable): (Vec<_>, Vec<_>) =
    //         self.rules.iter().partition(|r| r.stackable);

    //     // Always apply stackable rules
    //     let mut base_discount: f64 = stackable
    //         .iter()
    //         .map(|r| r.discount(basket))
    //         .sum();

    //     // Brute‑force search over all subsets of non‑stackable rules to find best extra discount.
    //     let n = non_stackable.len();
    //     let mut best_extra = 0.0;

    //     for mask in 0..(1u64 << n) {
    //         let mut d = 0.0;
    //         for i in 0..n {
    //             if (mask & (1 << i)) != 0 {
    //                 d += non_stackable[i].discount(basket);
    //             }
    //         }
    //         if d > best_extra {
    //             best_extra = d;
    //         }
    //     }

    //     let total_discount = base_discount + best_extra;
    //     let final_price = (sub - total_discount).max(0.0);
    //     (final_price * 100.0).round() / 100.0
    // }
}

// 4. Example usage


fn main() {
    let basket = Basket {
        items: vec![
            Item { sku: "APPLE".into(), unit_price: 1.0, quantity: 5 },
            Item { sku: "MILK".into(),  unit_price: 2.5, quantity: 2 },
        ],
    };

    let rules = vec![
        DiscountRule::PercentageOverTotal { threshold: 10.0, percent: 10.0 },
        DiscountRule::BuyNGetMFree { sku: "APPLE".into(), buy: 2, free: 1 },
    ];

    let engine = PromotionEngine::new(rules);
    let subtotal = basket.subtotal();
    let final_price = engine.final_price(&basket);

    println!("Subtotal:    {subtotal:.2}");
    println!("Final price: {final_price:.2}");
}



// Follow up:
// Adding a stackable: bool flag and exploring “best combination of non‑stackable 
// rules” (knapsack‑like).
// Pre‑indexing items by SKU to reduce rule cost from O(I) to O(#items for that SKU).

// If you were in the room right now, which extension would you pitch first: 
// non‑stackable rules, or a way to simulate different rule orders to show how stacking 
// changes the final price?

