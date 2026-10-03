//! Facebook-like social network.
//!
//! Spec (numbered requirements, class diagram, reference implementation):
//! `readme.md`. Design decisions taken on top of it: `DESIGN.md`.

pub mod domain;
pub mod error;
pub mod id;
pub(crate) mod locks;
pub mod registry;

pub use domain::accounts::{
    Account, AccountStatus, Admin, ConnectionInvitation, Education, InvitationStatus, Member,
    Profile, Work,
};
pub use domain::community::{Group, GroupStatus, Page, PageStatus, Recommendation};
pub use domain::content::{Comment, Message, Notification, NotificationKind, Post, PrivacyList};
pub use domain::search::SearchIndex;
pub use domain::system::FacebookSystem;
pub use error::{FbError, FbResult};
pub use id::{
    AccountId, AdminId, CommentId, GroupId, InvitationId, MemberId, MessageId, NotificationId,
    PageId, PostId, PrivacyListId, RecommendationId,
};
