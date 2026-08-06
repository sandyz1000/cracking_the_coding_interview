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


fn main() {
    
}
