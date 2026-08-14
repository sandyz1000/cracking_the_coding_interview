use thiserror::Error;

use crate::id::{
    AccountId, CommentId, GroupId, InvitationId, MemberId, PageId, PostId, PrivacyListId,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FbError {
    #[error("member {0} not found")]
    MemberNotFound(MemberId),
    #[error("account {0} not found")]
    AccountNotFound(AccountId),
    #[error("group {0} not found")]
    GroupNotFound(GroupId),
    #[error("page {0} not found")]
    PageNotFound(PageId),
    #[error("post {0} not found")]
    PostNotFound(PostId),
    #[error("comment {0} not found")]
    CommentNotFound(CommentId),
    #[error("invitation {0} not found")]
    InvitationNotFound(InvitationId),
    #[error("privacy list {0} not found")]
    PrivacyListNotFound(PrivacyListId),
    #[error("member {0} already connected")]
    AlreadyConnected(MemberId),
    #[error("member {0} already invited")]
    AlreadyInvited(MemberId),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("account disabled")]
    AccountDisabled,
    #[error("cannot perform on self")]
    SelfAction,
}

pub type FbResult<T> = Result<T, FbError>;
