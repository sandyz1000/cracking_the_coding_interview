// the order / delivery matching problem, with a simple greedy + priority‑queue algorithm you can code and reason about.
// 1. Data model


#[derive(Debug, Clone)]
struct Order {
    id: u32,
    x: f64,
    y: f64,
    ready_time: u32,   // minutes since some epoch
}

#[derive(Debug, Clone)]
struct Rider {
    id: u32,
    x: f64,
    y: f64,
    capacity: u32,     // how many orders they can carry
}

#[derive(Debug, Clone)]
struct Match {
    rider_id: u32,
    order_id: u32,
    distance: f64,
}

// Assume Euclidean distance and that all orders are “ready now” or will be by the time we assign.


fn distance(a_x: f64, a_y: f64, b_x: f64, b_y: f64) -> f64 {
    let dx = a_x - b_x;
    let dy = a_y - b_y;
    (dx * dx + dy * dy).sqrt()
}

// 2. Greedy matching algorithm (minimize distance)

// High‑level idea (classic greedy that often comes up in interviews and in simplified versions of real systems ):

//     Put all riders into a min‑heap keyed by current load and optionally by their distance to the next order.

//     Sort orders by some priority, e.g. earlier ready_time or just by ID.

//     For each order:

//  Find the rider with remaining capacity whose incremental distance to that order is minimal (here: distance rider → order).

//  Assign that order to that rider, decrement capacity.

// For simplicity, below is O(R · O) greedy: for each order, scan all riders and pick the closest with remaining capacity. 
// You can mention in the interview that this is not globally optimal but is a reasonable heuristic.


fn match_orders_to_riders(
    mut riders: Vec<Rider>,
    mut orders: Vec<Order>,
) -> Vec<Match> {
    // Sort orders by ready_time (earlier first)
    orders.sort_by_key(|o| o.ready_time);

    let mut matches = Vec::new();

    for order in orders {
        let mut best_rider_idx: Option<usize> = None;
        let mut best_dist = f64::INFINITY;

        for (idx, rider) in riders.iter().enumerate() {
            if rider.capacity == 0 {
                continue;
            }
            let d = distance(rider.x, rider.y, order.x, order.y);
            if d < best_dist {
                best_dist = d;
                best_rider_idx = Some(idx);
            }
        }

        if let Some(idx) = best_rider_idx {
            // assign
            riders[idx].capacity -= 1;
            matches.push(Match {
                rider_id: riders[idx].id,
                order_id: order.id,
                distance: best_dist,
            });
        } else {
            // no rider with free capacity; in a real system you might queue the order
        }
    }

    matches
}

// 3. Example usage


fn main() {
    let riders = vec![
        Rider { id: 1, x: 0.0, y: 0.0, capacity: 2 },
        Rider { id: 2, x: 5.0, y: 5.0, capacity: 1 },
    ];

    let orders = vec![
        Order { id: 101, x: 1.0, y: 1.0, ready_time: 0 },
        Order { id: 102, x: 6.0, y: 5.0, ready_time: 0 },
        Order { id: 103, x: 2.0, y: 0.0, ready_time: 0 },
    ];

    let matches = match_orders_to_riders(riders, orders);

    for m in matches {
        println!(
            "Rider {} assigned to order {} (distance {:.2})",
            m.rider_id, m.order_id, m.distance
        );
    }
}

// 4. Complexity talking points

//     Let R = riders, O = orders.

//     For each order, we scan riders: O(R · O) time.

//     Space: O(R + O).

//     You can mention improvements:

//         Use a spatial index (k‑d tree / grid) to find nearest riders faster.

//         Use a min‑heap keyed by “next‑available time” to balance lateness vs distance.

// If a follow‑up asked you to prioritize minimizing lateness (difference between promised and actual delivery time) instead of raw distance, how would you tweak the scoring for choosing the “best” rider for each order?