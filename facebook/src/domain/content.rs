//! Posts, comments, messages, and privacy lists.

use std::collections::HashSet;

use crate::id::{CommentId, MemberId, MessageId, NotificationId, PostId, PrivacyListId};

#[derive(Clone, Debug)]
pub struct Post {
    pub id: PostId,
    pub owner_id: MemberId,
    pub text: String,
    pub likes: u64,
    pub shares: u64,
    /// Unrestricted when empty; otherwise visible only to these lists' members.
    pub privacy_list_ids: HashSet<PrivacyListId>,
    pub comment_ids: Vec<CommentId>,
}

impl Post {
    pub fn new(id: PostId, owner_id: MemberId, text: &str) -> Self {
        Self {
            id,
            owner_id,
            text: text.to_string(),
            likes: 0,
            shares: 0,
            privacy_list_ids: HashSet::new(),
            comment_ids: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Comment {
    pub id: CommentId,
    pub post_id: PostId,
    pub author_id: MemberId,
    pub text: String,
    pub likes: u64,
}

impl Comment {
    pub fn new(id: CommentId, post_id: PostId, author_id: MemberId, text: &str) -> Self {
        Self {
            id,
            post_id,
            author_id,
            text: text.to_string(),
            likes: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: MessageId,
    pub from_id: MemberId,
    pub to_id: MemberId,
    pub body: String,
}

impl Message {
    pub fn new(id: MessageId, from_id: MemberId, to_id: MemberId, body: &str) -> Self {
        Self {
            id,
            from_id,
            to_id,
            body: body.to_string(),
        }
    }
}

/// A named list of the owner's friends; linking a post to it restricts
/// visibility to exactly those members.
#[derive(Clone, Debug)]
pub struct PrivacyList {
    pub id: PrivacyListId,
    pub owner_id: MemberId,
    pub name: String,
    pub member_ids: HashSet<MemberId>,
}

impl PrivacyList {
    pub fn new(id: PrivacyListId, owner_id: MemberId, name: &str) -> Self {
        Self {
            id,
            owner_id,
            name: name.to_string(),
            member_ids: HashSet::new(),
        }
    }
}

/// Requirement 11. `from` is always the other party; the recipient is the
/// member the notification is filed under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    Message { from: MemberId },
    FriendRequest { from: MemberId },
    Comment { post_id: PostId, from: MemberId },
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: NotificationId,
    pub member_id: MemberId,
    pub kind: NotificationKind,
}

impl Notification {
    pub fn new(id: NotificationId, member_id: MemberId, kind: NotificationKind) -> Self {
        Self {
            id,
            member_id,
            kind,
        }
    }
}
