//! Accounts, members, admins, profiles, and connection invitations.

use std::collections::HashSet;

use crate::id::{AccountId, AdminId, GroupId, InvitationId, MemberId, PageId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountStatus {
    Active,
    Closed,
    Canceled,
    Blacklisted,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub id: AccountId,
    pub password: String,
    pub status: AccountStatus,
}

impl Account {
    pub fn new(id: AccountId, password: &str) -> Self {
        Self {
            id,
            password: password.to_string(),
            status: AccountStatus::Active,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }
}

#[derive(Clone, Debug)]
pub struct Admin {
    pub id: AdminId,
    pub name: String,
    pub account_id: AccountId,
}

impl Admin {
    pub fn new(id: AdminId, name: &str, account_id: AccountId) -> Self {
        Self {
            id,
            name: name.to_string(),
            account_id,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Work {
    pub title: String,
    pub company: String,
}

#[derive(Clone, Debug, Default)]
pub struct Education {
    pub school: String,
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub work: Vec<Work>,
    pub education: Vec<Education>,
}

/// Core member: connections, follows, and a profile.
#[derive(Clone, Debug)]
pub struct Member {
    pub id: MemberId,
    pub name: String,
    pub account_id: AccountId,
    pub profile: Profile,
    pub members_followed: HashSet<MemberId>,
    /// ids of connected members (the "friends" of this member).
    pub connections: HashSet<MemberId>,
    pub pages_followed: HashSet<PageId>,
    pub groups_joined: HashSet<GroupId>,
    pub suggestions: Vec<MemberId>,
}

impl Member {
    pub fn new(id: MemberId, name: &str, account_id: AccountId) -> Self {
        Self {
            id,
            name: name.to_string(),
            account_id,
            profile: Profile::default(),
            members_followed: HashSet::new(),
            connections: HashSet::new(),
            pages_followed: HashSet::new(),
            groups_joined: HashSet::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn is_connected_to(&self, other: MemberId) -> bool {
        self.connections.contains(&other)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Rejected,
    Canceled,
}

#[derive(Clone, Debug)]
pub struct ConnectionInvitation {
    pub id: InvitationId,
    pub from: MemberId,
    pub to: MemberId,
    pub status: InvitationStatus,
}

impl ConnectionInvitation {
    pub fn new(id: InvitationId, from: MemberId, to: MemberId) -> Self {
        Self {
            id,
            from,
            to,
            status: InvitationStatus::Pending,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.status == InvitationStatus::Pending
    }
}
