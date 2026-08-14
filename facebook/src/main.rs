//! Demo driver for the Facebook-like social network.

use facebook::domain::accounts::{Education, Work};
use facebook::{FacebookSystem, NotificationKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let system = FacebookSystem::new();

    let alice = system.create_member("Alice", "pw1")?;
    let bob = system.create_member("Bob", "pw2")?;
    let carol = system.create_member("Carol", "pw3")?;
    let dave = system.create_member("Dave", "pw4")?;
    let eve = system.create_member("Eve", "pw5")?;
    let admin = system.create_admin("Root Admin", "rootpw")?;

    // Requirement 1: profile.
    system.add_work(
        alice.id,
        Work {
            title: "Engineer".into(),
            company: "Acme".into(),
        },
    )?;
    system.add_education(
        alice.id,
        Education {
            school: "MIT".into(),
        },
    )?;
    println!(
        "Alice's education: {}",
        system.profile_of(alice.id)?.education[0].school
    );

    // Requirement 3: friend requests, building a graph for the suggestions.
    for (from, to) in [
        (alice.id, bob.id),
        (alice.id, carol.id),
        (bob.id, dave.id),
        (carol.id, dave.id),
        (dave.id, eve.id),
    ] {
        let invite = system.send_connection_invitation(from, to)?;
        system.accept_connection(to, invite)?;
    }
    println!(
        "Alice is connected to Bob: {}",
        system.member(alice.id).unwrap().is_connected_to(bob.id)
    );

    // Requirement 5: groups + pages.
    let group = system.create_group(alice.id, "Rustaceans", "All about Rust")?;
    system.join_group(bob.id, group.id)?;
    let page = system.create_page(alice.id, "Acme Co", "Products")?;
    system.follow_page(bob.id, page.id)?;
    system.recommend_page(carol.id, page.id, "Great company")?;
    println!(
        "Page {} followers: {} | Bob's groups: {}",
        page.name,
        system.page(page.id).unwrap().follower_ids.len(),
        system.member(bob.id).unwrap().groups_joined.len()
    );

    // Requirements 6-8: posts, comments, privacy.
    let post = system.create_post(alice.id, "I love Rust")?;
    system.like_post(post.id)?;
    let comment = system.add_comment(post.id, bob.id, "Same!")?;
    system.like_comment(comment.id)?;
    let reshare = system.share_post(bob.id, post.id)?;
    println!(
        "Post #{} likes: {} | reshare #{} belongs to {}",
        post.id,
        system.post(post.id).unwrap().likes,
        reshare.id,
        system.member(reshare.owner_id).unwrap().name
    );

    let list = system.create_privacy_list(alice.id, "Close friends")?;
    system.add_to_privacy_list(alice.id, list.id, carol.id)?;
    system.share_post_to_list(alice.id, post.id, list.id)?;
    println!(
        "Restricted post #{} — Carol sees it: {}, Bob sees it: {}, Eve sees it: {}",
        post.id,
        system.can_view(carol.id, post.id)?,
        system.can_view(bob.id, post.id)?,
        system.can_view(eve.id, post.id)?
    );

    // Requirement 9: messages.
    system.send_message(bob.id, alice.id, "Nice post!")?;
    println!("Alice's messages: {}", system.messages_to(alice.id).len());

    // Requirement 11: notifications.
    for notification in system.notifications_for(alice.id) {
        let line = match notification.kind {
            NotificationKind::Message { from } => {
                format!("message from {}", system.member(from).unwrap().name)
            }
            NotificationKind::FriendRequest { from } => {
                format!("friend request from {}", system.member(from).unwrap().name)
            }
            NotificationKind::Comment { post_id, from } => format!(
                "{} commented on post #{post_id}",
                system.member(from).unwrap().name
            ),
        };
        println!("  notification: {line}");
    }

    // Requirement 12 + extended: search and connection suggestions.
    let hits = system.search_members("Al");
    println!(
        "Search 'Al': {}",
        hits.iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let suggestions = system.connection_suggestions(alice.id)?;
    println!(
        "Alice's suggestions (most mutual friends first): {}",
        suggestions
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Admin: block, then unblock.
    system.block_member(admin.id, dave.id)?;
    println!(
        "Dave blocked — status {:?}, can post: {}, posts retained: {}",
        system.account_status(dave.id)?,
        system.create_post(dave.id, "hello?").is_ok(),
        system.member(dave.id).is_some()
    );
    system.unblock_member(admin.id, dave.id)?;
    println!(
        "Dave unblocked — can post: {}",
        system.create_post(dave.id, "back online").is_ok()
    );

    Ok(())
}
