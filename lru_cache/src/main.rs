// ## Algorithm
// An lru cache has a hashmap that contain the key value pair, here the value is pointer to node in `doubly_linked`
// When the item is inserted or accessed the node will be moved to the head of the DLL
// When the item is removed we will removed from the hashmap and the `doubly_linked`
// When the item in the cached exceed the capacity remove the item from the tail

pub mod domain;
pub mod doubly_linked;

fn main() {
    println!("Hello, world!");
}
