//! Typed entity identifiers, so a `PostId` cannot be passed where a `MemberId`
//! belongs. Each registry counts its own ids from 1.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdminId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InvitationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecommendationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PostId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrivacyListId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotificationId(pub u64);

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for MemberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for InvitationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for CommentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for PrivacyListId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for AccountId {
    fn from(raw: u64) -> Self {
        AccountId(raw)
    }
}

impl From<u64> for MemberId {
    fn from(raw: u64) -> Self {
        MemberId(raw)
    }
}

impl From<u64> for AdminId {
    fn from(raw: u64) -> Self {
        AdminId(raw)
    }
}

impl From<u64> for InvitationId {
    fn from(raw: u64) -> Self {
        InvitationId(raw)
    }
}

impl From<u64> for GroupId {
    fn from(raw: u64) -> Self {
        GroupId(raw)
    }
}

impl From<u64> for PageId {
    fn from(raw: u64) -> Self {
        PageId(raw)
    }
}

impl From<u64> for RecommendationId {
    fn from(raw: u64) -> Self {
        RecommendationId(raw)
    }
}

impl From<u64> for PostId {
    fn from(raw: u64) -> Self {
        PostId(raw)
    }
}

impl From<u64> for CommentId {
    fn from(raw: u64) -> Self {
        CommentId(raw)
    }
}

impl From<u64> for MessageId {
    fn from(raw: u64) -> Self {
        MessageId(raw)
    }
}

impl From<u64> for PrivacyListId {
    fn from(raw: u64) -> Self {
        PrivacyListId(raw)
    }
}

impl From<u64> for NotificationId {
    fn from(raw: u64) -> Self {
        NotificationId(raw)
    }
}
