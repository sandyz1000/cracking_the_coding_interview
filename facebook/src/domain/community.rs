//! Groups, pages, and recommendations.

use std::collections::HashSet;

use crate::id::{GroupId, MemberId, PageId, RecommendationId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupStatus {
    Active,
    Archived,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub description: String,
    pub status: GroupStatus,
    pub member_ids: HashSet<MemberId>,
}

impl Group {
    pub fn new(id: GroupId, name: &str, description: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: description.to_string(),
            status: GroupStatus::Active,
            member_ids: HashSet::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == GroupStatus::Active
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageStatus {
    Active,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub id: PageId,
    pub name: String,
    pub description: String,
    pub status: PageStatus,
    pub follower_ids: HashSet<MemberId>,
    pub recommendation_ids: Vec<RecommendationId>,
}

impl Page {
    pub fn new(id: PageId, name: &str, description: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: description.to_string(),
            status: PageStatus::Active,
            follower_ids: HashSet::new(),
            recommendation_ids: Vec::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == PageStatus::Active
    }
}

#[derive(Clone, Debug)]
pub struct Recommendation {
    pub id: RecommendationId,
    pub page_id: PageId,
    pub author_id: MemberId,
    pub text: String,
}

impl Recommendation {
    pub fn new(id: RecommendationId, page_id: PageId, author_id: MemberId, text: &str) -> Self {
        Self {
            id,
            page_id,
            author_id,
            text: text.to_string(),
        }
    }
}
