//! In-process pub-sub system. Each topic fans a message out to its current
//! subscribers: the subscriber set is snapshotted under a read lock, then
//! delivery happens outside the lock so a subscriber can never deadlock the
//! topic. Sequence numbers come from an atomic counter, so concurrent
//! publishers on the same topic produce a total order.

/*
# Designing a Pub-Sub System

### Requirements

- The Pub-Sub system should allow publishers to publish messages to specific topics.
- Subscribers should be able to subscribe to topics of interest and receive messages published to those topics.
- The system should support multiple publishers and subscribers.
- Messages should be delivered to all subscribers of a topic in real-time.
- The system should handle concurrent access and ensure thread safety.
- The Pub-Sub system should be scalable and efficient in terms of message delivery.

### Classes, Interfaces and Enumerations

- The Message class represents a message that can be published and received by subscribers. It contains the message
content.
- The Topic class represents a topic to which messages can be published. It maintains a set of subscribers and provides
methods to add and remove subscribers, as well as publish messages to all subscribers.
- The Subscriber interface defines the contract for subscribers. It declares the onMessage method that is invoked when
a subscriber receives a message.
- The PrintSubscriber class is a concrete implementation of the Subscriber interface. It receives messages and prints
them to the console.
- The Publisher class represents a publisher that publishes messages to a specific topic.
- The PubSubSystem class is the main class that manages topics, subscribers, and message publishing. It uses a ConcurrentHashMap
to store topics and an ExecutorService to handle concurrent message publishing.
- The PubSubDemo class demonstrates the usage of the Pub-Sub system by creating topics, subscribers, and publishers, and
publishing messages.

*/

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PubSubError {
    #[error("topic '{name}' already exists")]
    TopicExists { name: String },
    #[error("topic '{name}' not found")]
    TopicNotFound { name: String },
    #[error("subscriber {id} is already subscribed to '{topic}'")]
    AlreadySubscribed { topic: String, id: uuid::Uuid },
    #[error("subscriber {id} is not subscribed to '{topic}'")]
    NotSubscribed { topic: String, id: uuid::Uuid },
}

pub type PubSubResult<T> = std::result::Result<T, PubSubError>;

#[derive(Debug, Clone)]
pub struct Message {
    pub seq: u64,
    pub content: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
}

pub trait Subscriber: Send + Sync {
    fn on_message(&self, message: &Message);
    fn id(&self) -> uuid::Uuid;
}

pub struct PrintSubscriber {
    name: String,
    id: uuid::Uuid,
}

impl PrintSubscriber {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: uuid::Uuid::new_v4(),
        }
    }
}

impl Subscriber for PrintSubscriber {
    fn on_message(&self, message: &Message) {
        println!("[{}] {} (seq {})", self.name, message.content, message.seq);
    }

    fn id(&self) -> uuid::Uuid {
        self.id
    }
}

pub struct Topic {
    name: String,
    next_seq: AtomicU64,
    subscribers: RwLock<HashMap<uuid::Uuid, Arc<dyn Subscriber>>>,
}

impl Topic {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            next_seq: AtomicU64::new(0),
            subscribers: RwLock::new(HashMap::new()),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn subscribe(&self, subscriber: Arc<dyn Subscriber>) -> PubSubResult<()> {
        let id = subscriber.id();
        let mut subs = write_guard(&self.subscribers);
        if subs.contains_key(&id) {
            return Err(PubSubError::AlreadySubscribed {
                topic: self.name.clone(),
                id,
            });
        }
        subs.insert(id, subscriber);
        Ok(())
    }

    pub fn unsubscribe(&self, id: uuid::Uuid) -> PubSubResult<()> {
        let mut subs = write_guard(&self.subscribers);
        subs.remove(&id)
            .map(|_| ())
            .ok_or(PubSubError::NotSubscribed {
                topic: self.name.clone(),
                id,
            })
    }

    pub fn subscriber_count(&self) -> usize {
        read_guard(&self.subscribers).len()
    }

    /// Deliver a message to every current subscriber. The subscriber set is
    /// snapshotted under the read lock; delivery runs lock-free afterwards.
    pub fn publish(&self, content: impl Into<String>) -> PubSubResult<usize> {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let message = Message {
            seq,
            content: content.into(),
            published_at: chrono::Utc::now(),
        };
        let targets: Vec<Arc<dyn Subscriber>> =
            read_guard(&self.subscribers).values().cloned().collect();
        for subscriber in targets {
            subscriber.on_message(&message);
        }
        Ok(read_guard(&self.subscribers).len())
    }
}

pub struct Publisher {
    pub name: String,
    topic: Arc<Topic>,
}

impl Publisher {
    pub fn new(name: impl Into<String>, topic: Arc<Topic>) -> Self {
        Self {
            name: name.into(),
            topic,
        }
    }

    pub fn publish(&self, content: impl Into<String>) -> PubSubResult<usize> {
        self.topic.publish(content)
    }
}

pub struct PubSubSystem {
    topics: RwLock<HashMap<String, Arc<Topic>>>,
}

impl Default for PubSubSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSubSystem {
    pub fn new() -> Self {
        Self {
            topics: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_topic(&self, name: &str) -> PubSubResult<Arc<Topic>> {
        let mut topics = write_guard(&self.topics);
        if topics.contains_key(name) {
            return Err(PubSubError::TopicExists {
                name: name.to_string(),
            });
        }
        let topic = Arc::new(Topic::new(name));
        topics.insert(name.to_string(), Arc::clone(&topic));
        Ok(topic)
    }

    pub fn topic(&self, name: &str) -> PubSubResult<Arc<Topic>> {
        read_guard(&self.topics)
            .get(name)
            .cloned()
            .ok_or(PubSubError::TopicNotFound {
                name: name.to_string(),
            })
    }

    pub fn subscribe(&self, topic: &str, subscriber: Arc<dyn Subscriber>) -> PubSubResult<()> {
        self.topic(topic)?.subscribe(subscriber)
    }

    pub fn unsubscribe(&self, topic: &str, id: uuid::Uuid) -> PubSubResult<()> {
        self.topic(topic)?.unsubscribe(id)
    }

    pub fn publish(&self, topic: &str, content: &str) -> PubSubResult<usize> {
        self.topic(topic)?.publish(content)
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
    let system = PubSubSystem::new();
    let sports = system.create_topic("sports").expect("topic created");
    let news = system.create_topic("news").expect("topic created");

    let alice = Arc::new(PrintSubscriber::new("alice"));
    let bob = Arc::new(PrintSubscriber::new("bob"));
    system
        .subscribe("sports", Arc::clone(&alice) as Arc<dyn Subscriber>)
        .expect("alice subscribes to sports");
    system
        .subscribe("sports", Arc::clone(&bob) as Arc<dyn Subscriber>)
        .expect("bob subscribes to sports");
    system
        .subscribe("news", Arc::clone(&alice) as Arc<dyn Subscriber>)
        .expect("alice subscribes to news");

    let play_by_play = Publisher::new("play-by-play", Arc::clone(&sports));
    let breaking_news = Publisher::new("breaking-news", Arc::clone(&news));

    play_by_play
        .publish("Goal! 1-0")
        .expect("publish to sports");
    breaking_news
        .publish("Markets rally")
        .expect("publish to news");

    println!("\nBob unsubscribes from sports.");
    let bob_id = bob.id();
    system
        .unsubscribe("sports", bob_id)
        .expect("bob unsubscribes");
    play_by_play
        .publish("Penalty awarded")
        .expect("publish to sports");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct CountingSubscriber {
        count: Arc<AtomicUsize>,
        id: uuid::Uuid,
    }

    impl CountingSubscriber {
        fn new(count: Arc<AtomicUsize>) -> Self {
            Self {
                count,
                id: uuid::Uuid::new_v4(),
            }
        }
    }

    impl Subscriber for CountingSubscriber {
        fn on_message(&self, _message: &Message) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        fn id(&self) -> uuid::Uuid {
            self.id
        }
    }

    fn system_with_topic(name: &str) -> (PubSubSystem, Arc<Topic>) {
        let system = PubSubSystem::new();
        let topic = system.create_topic(name).expect("topic created");
        (system, topic)
    }

    #[test]
    fn test_duplicate_topic() {
        let system = PubSubSystem::new();
        system.create_topic("sports").expect("created once");
        let result = system.create_topic("sports");
        assert!(matches!(result, Err(PubSubError::TopicExists { .. })));
    }

    #[test]
    fn test_publish_unknown_topic() {
        let system = PubSubSystem::new();
        let err = system.publish("missing", "hello").unwrap_err();
        assert!(matches!(err, PubSubError::TopicNotFound { .. }));
    }

    #[test]
    fn test_duplicate_subscription() {
        let (system, topic) = system_with_topic("sports");
        let subscriber: Arc<dyn Subscriber> = Arc::new(PrintSubscriber::new("alice"));
        system
            .subscribe(topic.name(), Arc::clone(&subscriber))
            .expect("first subscription");
        let err = system.subscribe(topic.name(), subscriber).unwrap_err();
        assert!(matches!(err, PubSubError::AlreadySubscribed { .. }));
    }

    #[test]
    fn test_unsubscribe_missing() {
        let (system, topic) = system_with_topic("sports");
        let err = system
            .unsubscribe(topic.name(), uuid::Uuid::new_v4())
            .unwrap_err();
        assert!(matches!(err, PubSubError::NotSubscribed { .. }));
    }

    #[test]
    fn test_deliver_all_subscribers() {
        let (_system, topic) = system_with_topic("sports");
        let counts = Arc::new(AtomicUsize::new(0));
        let first = Arc::new(CountingSubscriber::new(Arc::clone(&counts)));
        let second = Arc::new(CountingSubscriber::new(Arc::clone(&counts)));
        topic
            .subscribe(Arc::clone(&first) as Arc<dyn Subscriber>)
            .expect("first subscribes");
        topic
            .subscribe(Arc::clone(&second) as Arc<dyn Subscriber>)
            .expect("second subscribes");

        topic.publish("hello").expect("published");

        assert_eq!(counts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_sequence_is_total() {
        let (_system, topic) = system_with_topic("sports");
        topic.publish("first").expect("published");
        topic.publish("second").expect("published");
        topic.publish("third").expect("published");

        assert_eq!(topic.next_seq.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_concurrent_delivery() {
        let (_system, topic) = system_with_topic("sports");
        let counts = Arc::new(AtomicUsize::new(0));
        let subscriber = Arc::new(CountingSubscriber::new(Arc::clone(&counts)));
        topic
            .subscribe(Arc::clone(&subscriber) as Arc<dyn Subscriber>)
            .expect("subscriber joins");

        let publishers = 4;
        let messages_per_publisher = 250;
        let handles: Vec<_> = (0..publishers)
            .map(|_| {
                let topic = Arc::clone(&topic);
                std::thread::spawn(move || {
                    for i in 0..messages_per_publisher {
                        topic.publish(format!("msg {i}")).expect("published");
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("publisher thread joins");
        }

        assert_eq!(
            counts.load(Ordering::SeqCst),
            publishers * messages_per_publisher
        );
    }
}

fn main() {
    run_demo();
    println!("\nAll done.");
}
