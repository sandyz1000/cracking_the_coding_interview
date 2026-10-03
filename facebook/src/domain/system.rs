//! The `FacebookSystem`: composition root and home of every mutation rule.

use std::collections::{HashMap, HashSet};

use crate::domain::accounts::{
    Account, AccountStatus, Admin, ConnectionInvitation, Education, InvitationStatus, Member,
    Profile, Work,
};
use crate::domain::community::{Group, Page, PageStatus, Recommendation};
use crate::domain::content::{Comment, Message, Notification, NotificationKind, Post, PrivacyList};
use crate::domain::search::SearchIndex;
use crate::error::{FbError, FbResult};
use crate::id::{
    AccountId, AdminId, CommentId, GroupId, InvitationId, MemberId, MessageId, NotificationId,
    PageId, PostId, PrivacyListId, RecommendationId,
};
use crate::locks::{rd, wr};
use crate::registry::Registry;

/// Composition root: one `RwLock<HashMap<_>>` per registry, each with its own
/// id counter. See DESIGN.md.
#[derive(Default)]
pub struct FacebookSystem {
    accounts: Registry<AccountId, Account>,
    members: Registry<MemberId, Member>,
    admins: Registry<AdminId, Admin>,
    requests: Registry<InvitationId, ConnectionInvitation>,
    groups: Registry<GroupId, Group>,
    pages: Registry<PageId, Page>,
    recommendations: Registry<RecommendationId, Recommendation>,
    posts: Registry<PostId, Post>,
    comments: Registry<CommentId, Comment>,
    messages: Registry<MessageId, Message>,
    privacy_lists: Registry<PrivacyListId, PrivacyList>,
    notifications: Registry<NotificationId, Notification>,
    search: SearchIndex,
}

impl FacebookSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Gates every member-initiated action on the account status.
    fn require_active(&self, member_id: MemberId) -> FbResult<()> {
        let account_id = rd(&self.members.items)
            .get(&member_id)
            .map(|member| member.account_id)
            .ok_or(FbError::MemberNotFound(member_id))?;
        match rd(&self.accounts.items).get(&account_id) {
            Some(account) if account.is_active() => Ok(()),
            _ => Err(FbError::AccountDisabled),
        }
    }

    fn require_admin(&self, admin_id: AdminId) -> FbResult<()> {
        if rd(&self.admins.items).contains_key(&admin_id) {
            Ok(())
        } else {
            Err(FbError::PermissionDenied)
        }
    }
}

/// Accounts and lookups.
impl FacebookSystem {
    pub fn create_member(&self, name: &str, password: &str) -> FbResult<Member> {
        if name.trim().is_empty() || password.is_empty() {
            return Err(FbError::InvalidState("name and password required".into()));
        }
        let account = self.accounts.insert(|id| Account::new(id, password));
        let member = self.members.insert(|id| Member::new(id, name, account.id));
        self.search.members.add(member.id, name);
        Ok(member)
    }

    pub fn create_admin(&self, name: &str, password: &str) -> FbResult<Admin> {
        if name.trim().is_empty() || password.is_empty() {
            return Err(FbError::InvalidState("name and password required".into()));
        }
        let account = self.accounts.insert(|id| Account::new(id, password));
        let admin = self.admins.insert(|id| Admin::new(id, name, account.id));
        Ok(admin)
    }

    pub fn member(&self, id: MemberId) -> Option<Member> {
        rd(&self.members.items).get(&id).cloned()
    }

    pub fn group(&self, id: GroupId) -> Option<Group> {
        rd(&self.groups.items).get(&id).cloned()
    }

    pub fn page(&self, id: PageId) -> Option<Page> {
        rd(&self.pages.items).get(&id).cloned()
    }

    pub fn post(&self, id: PostId) -> Option<Post> {
        rd(&self.posts.items).get(&id).cloned()
    }

    pub fn recommendation(&self, id: RecommendationId) -> Option<Recommendation> {
        rd(&self.recommendations.items).get(&id).cloned()
    }

    pub fn account_status(&self, member_id: MemberId) -> FbResult<AccountStatus> {
        let account_id = rd(&self.members.items)
            .get(&member_id)
            .map(|member| member.account_id)
            .ok_or(FbError::MemberNotFound(member_id))?;
        rd(&self.accounts.items)
            .get(&account_id)
            .map(|account| account.status)
            .ok_or(FbError::AccountNotFound(account_id))
    }
}

/// Administration.
impl FacebookSystem {
    /// Blacklists the account; the member row survives. See DESIGN.md.
    pub fn block_member(&self, admin_id: AdminId, member_id: MemberId) -> FbResult<()> {
        self.set_account_status(admin_id, member_id, AccountStatus::Blacklisted)
    }

    pub fn unblock_member(&self, admin_id: AdminId, member_id: MemberId) -> FbResult<()> {
        self.set_account_status(admin_id, member_id, AccountStatus::Active)
    }

    fn set_account_status(
        &self,
        admin_id: AdminId,
        member_id: MemberId,
        status: AccountStatus,
    ) -> FbResult<()> {
        self.require_admin(admin_id)?;
        let account_id = rd(&self.members.items)
            .get(&member_id)
            .map(|member| member.account_id)
            .ok_or(FbError::MemberNotFound(member_id))?;
        let mut accounts = wr(&self.accounts.items);
        let account = accounts
            .get_mut(&account_id)
            .ok_or(FbError::AccountNotFound(account_id))?;
        account.status = status;
        Ok(())
    }

    pub fn enable_page(&self, admin_id: AdminId, page_id: PageId) -> FbResult<()> {
        self.set_page_status(admin_id, page_id, PageStatus::Active)
    }

    pub fn disable_page(&self, admin_id: AdminId, page_id: PageId) -> FbResult<()> {
        self.set_page_status(admin_id, page_id, PageStatus::Disabled)
    }

    fn set_page_status(
        &self,
        admin_id: AdminId,
        page_id: PageId,
        status: PageStatus,
    ) -> FbResult<()> {
        self.require_admin(admin_id)?;
        let mut pages = wr(&self.pages.items);
        let page = pages
            .get_mut(&page_id)
            .ok_or(FbError::PageNotFound(page_id))?;
        page.status = status;
        Ok(())
    }
}

/// Profiles (requirement 1).
impl FacebookSystem {
    pub fn add_work(&self, member_id: MemberId, work: Work) -> FbResult<()> {
        self.require_active(member_id)?;
        let mut members = wr(&self.members.items);
        let member = members
            .get_mut(&member_id)
            .ok_or(FbError::MemberNotFound(member_id))?;
        member.profile.work.push(work);
        Ok(())
    }

    pub fn add_education(&self, member_id: MemberId, education: Education) -> FbResult<()> {
        self.require_active(member_id)?;
        let mut members = wr(&self.members.items);
        let member = members
            .get_mut(&member_id)
            .ok_or(FbError::MemberNotFound(member_id))?;
        member.profile.education.push(education);
        Ok(())
    }

    pub fn profile_of(&self, member_id: MemberId) -> FbResult<Profile> {
        rd(&self.members.items)
            .get(&member_id)
            .map(|member| member.profile.clone())
            .ok_or(FbError::MemberNotFound(member_id))
    }
}

/// Connections (requirement 3) and follows (requirement 4).
impl FacebookSystem {
    pub fn send_connection_invitation(
        &self,
        from_id: MemberId,
        to_id: MemberId,
    ) -> FbResult<InvitationId> {
        if from_id == to_id {
            return Err(FbError::SelfAction);
        }
        self.require_active(from_id)?;
        self.require_active(to_id)?;
        if self
            .member(from_id)
            .is_some_and(|member| member.is_connected_to(to_id))
        {
            return Err(FbError::AlreadyConnected(to_id));
        }
        // Only a pending invitation blocks a new one, so a rejection is not final.
        let already_pending = rd(&self.requests.items).values().any(|request| {
            request.is_pending()
                && ((request.from == from_id && request.to == to_id)
                    || (request.from == to_id && request.to == from_id))
        });
        if already_pending {
            return Err(FbError::AlreadyInvited(to_id));
        }
        let invitation = self
            .requests
            .insert(|id| ConnectionInvitation::new(id, from_id, to_id));
        self.notify(to_id, NotificationKind::FriendRequest { from: from_id });
        Ok(invitation.id)
    }

    pub fn accept_connection(&self, to_id: MemberId, invite_id: InvitationId) -> FbResult<()> {
        let (from, to) = {
            let mut requests = wr(&self.requests.items);
            let request = requests
                .get_mut(&invite_id)
                .ok_or(FbError::InvitationNotFound(invite_id))?;
            if request.to != to_id {
                return Err(FbError::PermissionDenied);
            }
            if !request.is_pending() {
                return Err(FbError::InvalidState(format!(
                    "invitation {invite_id} is already {:?}",
                    request.status
                )));
            }
            request.status = InvitationStatus::Accepted;
            (request.from, request.to)
        };
        self.link(from, to);
        Ok(())
    }

    pub fn reject_connection(&self, to_id: MemberId, invite_id: InvitationId) -> FbResult<()> {
        let mut requests = wr(&self.requests.items);
        let request = requests
            .get_mut(&invite_id)
            .ok_or(FbError::InvitationNotFound(invite_id))?;
        if request.to != to_id {
            return Err(FbError::PermissionDenied);
        }
        request.status = InvitationStatus::Rejected;
        Ok(())
    }

    /// Installs the two-way edge under one guard, so it is never half-visible.
    fn link(&self, a: MemberId, b: MemberId) {
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&a) {
            member.connections.insert(b);
        }
        if let Some(member) = members.get_mut(&b) {
            member.connections.insert(a);
        }
    }

    // Mutual: a stale reverse edge would still grant friends-only visibility.
    pub fn remove_connection(&self, actor_id: MemberId, target_id: MemberId) -> FbResult<()> {
        let members = rd(&self.members.items);
        if !members.contains_key(&actor_id) {
            return Err(FbError::MemberNotFound(actor_id));
        }
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&actor_id) {
            member.connections.remove(&target_id);
        }
        if let Some(member) = members.get_mut(&target_id) {
            member.connections.remove(&actor_id);
        }
        Ok(())
    }

    pub fn follow_member(&self, follower: MemberId, target: MemberId) -> FbResult<()> {
        if follower == target {
            return Err(FbError::SelfAction);
        }
        self.require_active(follower)?;
        self.require_active(target)?;
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&follower) {
            member.members_followed.insert(target);
        }
        Ok(())
    }

    pub fn unfollow_member(&self, follower: MemberId, target: MemberId) -> FbResult<()> {
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&follower) {
            member.members_followed.remove(&target);
        }
        Ok(())
    }
}

/// Groups and pages (requirement 5).
impl FacebookSystem {
    pub fn create_group(
        &self,
        owner_id: MemberId,
        name: &str,
        description: &str,
    ) -> FbResult<Group> {
        self.require_active(owner_id)?;
        let group = self.groups.insert(|id| {
            let mut group = Group::new(id, name, description);
            group.member_ids.insert(owner_id);
            group
        });
        self.record_group_membership(owner_id, group.id, true);
        self.search.groups.add(group.id, name);
        Ok(group)
    }

    pub fn join_group(&self, member_id: MemberId, group_id: GroupId) -> FbResult<()> {
        self.require_active(member_id)?;
        {
            let mut groups = wr(&self.groups.items);
            let group = groups
                .get_mut(&group_id)
                .ok_or(FbError::GroupNotFound(group_id))?;
            if !group.is_active() {
                return Err(FbError::InvalidState(format!(
                    "group {group_id} is archived"
                )));
            }
            group.member_ids.insert(member_id);
        }
        self.record_group_membership(member_id, group_id, true);
        Ok(())
    }

    pub fn leave_group(&self, member_id: MemberId, group_id: GroupId) -> FbResult<()> {
        {
            let mut groups = wr(&self.groups.items);
            let group = groups
                .get_mut(&group_id)
                .ok_or(FbError::GroupNotFound(group_id))?;
            group.member_ids.remove(&member_id);
        }
        self.record_group_membership(member_id, group_id, false);
        Ok(())
    }

    /// Group membership is stored on both sides.
    fn record_group_membership(&self, member_id: MemberId, group_id: GroupId, joined: bool) {
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&member_id) {
            if joined {
                member.groups_joined.insert(group_id);
            } else {
                member.groups_joined.remove(&group_id);
            }
        }
    }

    pub fn create_page(&self, owner_id: MemberId, name: &str, description: &str) -> FbResult<Page> {
        self.require_active(owner_id)?;
        let page = self.pages.insert(|id| Page::new(id, name, description));
        self.search.pages.add(page.id, name);
        Ok(page)
    }

    pub fn follow_page(&self, member_id: MemberId, page_id: PageId) -> FbResult<()> {
        self.require_active(member_id)?;
        {
            let mut pages = wr(&self.pages.items);
            let page = pages
                .get_mut(&page_id)
                .ok_or(FbError::PageNotFound(page_id))?;
            if !page.is_active() {
                return Err(FbError::InvalidState(format!("page {page_id} is disabled")));
            }
            page.follower_ids.insert(member_id);
        }
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&member_id) {
            member.pages_followed.insert(page_id);
        }
        Ok(())
    }

    pub fn unfollow_page(&self, member_id: MemberId, page_id: PageId) -> FbResult<()> {
        {
            let mut pages = wr(&self.pages.items);
            if let Some(page) = pages.get_mut(&page_id) {
                page.follower_ids.remove(&member_id);
            }
        }
        let mut members = wr(&self.members.items);
        if let Some(member) = members.get_mut(&member_id) {
            member.pages_followed.remove(&page_id);
        }
        Ok(())
    }

    /// Requirement 10.
    pub fn recommend_page(
        &self,
        member_id: MemberId,
        page_id: PageId,
        text: &str,
    ) -> FbResult<Recommendation> {
        self.require_active(member_id)?;
        // Reject before allocating, so a bad page leaves no orphan row behind.
        match rd(&self.pages.items).get(&page_id) {
            None => return Err(FbError::PageNotFound(page_id)),
            Some(page) if !page.is_active() => {
                return Err(FbError::InvalidState(format!("page {page_id} is disabled")));
            }
            Some(_) => {}
        }
        let recommendation = self
            .recommendations
            .insert(|id| Recommendation::new(id, page_id, member_id, text));
        if let Some(page) = wr(&self.pages.items).get_mut(&page_id) {
            page.recommendation_ids.push(recommendation.id);
        }
        Ok(recommendation)
    }
}

/// Posts, comments and privacy (requirements 6-8).
impl FacebookSystem {
    pub fn create_post(&self, owner_id: MemberId, text: &str) -> FbResult<Post> {
        self.require_active(owner_id)?;
        if text.trim().is_empty() {
            return Err(FbError::InvalidState("post text required".into()));
        }
        let post = self.posts.insert(|id| Post::new(id, owner_id, text));
        self.search.posts.add(post.id, text);
        Ok(post)
    }

    pub fn like_post(&self, post_id: PostId) -> FbResult<()> {
        let mut posts = wr(&self.posts.items);
        let post = posts
            .get_mut(&post_id)
            .ok_or(FbError::PostNotFound(post_id))?;
        post.likes += 1;
        Ok(())
    }

    /// The reshare belongs to the sharer, not to the original author.
    pub fn share_post(&self, sharer_id: MemberId, post_id: PostId) -> FbResult<Post> {
        self.require_active(sharer_id)?;
        let original = {
            let mut posts = wr(&self.posts.items);
            let post = posts
                .get_mut(&post_id)
                .ok_or(FbError::PostNotFound(post_id))?;
            post.shares += 1;
            post.text.clone()
        };
        self.create_post(sharer_id, &original)
    }

    pub fn create_privacy_list(&self, owner_id: MemberId, name: &str) -> FbResult<PrivacyList> {
        self.require_active(owner_id)?;
        Ok(self
            .privacy_lists
            .insert(|id| PrivacyList::new(id, owner_id, name)))
    }

    /// A privacy list holds the owner's friends, so a non-connection is refused.
    pub fn add_to_privacy_list(
        &self,
        owner_id: MemberId,
        list_id: PrivacyListId,
        member_id: MemberId,
    ) -> FbResult<()> {
        if !self
            .member(owner_id)
            .is_some_and(|owner| owner.is_connected_to(member_id))
        {
            return Err(FbError::InvalidState(format!(
                "member {member_id} is not connected to {owner_id}"
            )));
        }
        let mut lists = wr(&self.privacy_lists.items);
        let list = lists
            .get_mut(&list_id)
            .ok_or(FbError::PrivacyListNotFound(list_id))?;
        if list.owner_id != owner_id {
            return Err(FbError::PermissionDenied);
        }
        list.member_ids.insert(member_id);
        Ok(())
    }

    /// Restrict a post to the members of a privacy list (requirement 8).
    pub fn share_post_to_list(
        &self,
        owner_id: MemberId,
        post_id: PostId,
        list_id: PrivacyListId,
    ) -> FbResult<()> {
        if !rd(&self.privacy_lists.items)
            .get(&list_id)
            .ok_or(FbError::PrivacyListNotFound(list_id))?
            .owner_id
            .eq(&owner_id)
        {
            return Err(FbError::PermissionDenied);
        }
        let mut posts = wr(&self.posts.items);
        let post = posts
            .get_mut(&post_id)
            .ok_or(FbError::PostNotFound(post_id))?;
        if post.owner_id != owner_id {
            return Err(FbError::PermissionDenied);
        }
        post.privacy_list_ids.insert(list_id);
        Ok(())
    }

    /// Requirement 8's visibility rule. See DESIGN.md.
    pub fn can_view(&self, viewer_id: MemberId, post_id: PostId) -> FbResult<bool> {
        let post = self.post(post_id).ok_or(FbError::PostNotFound(post_id))?;
        if post.owner_id == viewer_id {
            return Ok(true);
        }
        if post.privacy_list_ids.is_empty() {
            return Ok(self
                .member(post.owner_id)
                .is_some_and(|owner| owner.is_connected_to(viewer_id)));
        }
        let lists = rd(&self.privacy_lists.items);
        Ok(post.privacy_list_ids.iter().any(|list_id| {
            lists
                .get(list_id)
                .is_some_and(|list| list.member_ids.contains(&viewer_id))
        }))
    }

    /// Every post `viewer_id` is allowed to see, newest id first.
    pub fn visible_posts(&self, viewer_id: MemberId) -> Vec<Post> {
        let ids: Vec<PostId> = rd(&self.posts.items).keys().copied().collect();
        let mut posts: Vec<Post> = ids
            .into_iter()
            .filter(|id| self.can_view(viewer_id, *id).unwrap_or(false))
            .filter_map(|id| self.post(id))
            .collect();
        posts.sort_by_key(|post| std::cmp::Reverse(post.id));
        posts
    }

    pub fn add_comment(
        &self,
        post_id: PostId,
        author_id: MemberId,
        text: &str,
    ) -> FbResult<Comment> {
        self.require_active(author_id)?;
        let owner_id = rd(&self.posts.items)
            .get(&post_id)
            .map(|post| post.owner_id)
            .ok_or(FbError::PostNotFound(post_id))?;
        let comment = self
            .comments
            .insert(|id| Comment::new(id, post_id, author_id, text));
        if let Some(post) = wr(&self.posts.items).get_mut(&post_id) {
            post.comment_ids.push(comment.id);
        }
        if owner_id != author_id {
            self.notify(
                owner_id,
                NotificationKind::Comment {
                    post_id,
                    from: author_id,
                },
            );
        }
        Ok(comment)
    }

    pub fn like_comment(&self, comment_id: CommentId) -> FbResult<()> {
        let mut comments = wr(&self.comments.items);
        let comment = comments
            .get_mut(&comment_id)
            .ok_or(FbError::CommentNotFound(comment_id))?;
        comment.likes += 1;
        Ok(())
    }
}

/// Messages (requirement 9) and notifications (requirement 11).
impl FacebookSystem {
    pub fn send_message(
        &self,
        from_id: MemberId,
        to_id: MemberId,
        body: &str,
    ) -> FbResult<Message> {
        self.require_active(from_id)?;
        self.require_active(to_id)?;
        let message = self
            .messages
            .insert(|id| Message::new(id, from_id, to_id, body));
        self.notify(to_id, NotificationKind::Message { from: from_id });
        Ok(message)
    }

    pub fn messages_to(&self, to_id: MemberId) -> Vec<Message> {
        rd(&self.messages.items)
            .values()
            .filter(|message| message.to_id == to_id)
            .cloned()
            .collect()
    }

    fn notify(&self, member_id: MemberId, kind: NotificationKind) {
        self.notifications
            .insert(|id| Notification::new(id, member_id, kind));
    }

    /// A member's notifications, oldest first.
    pub fn notifications_for(&self, member_id: MemberId) -> Vec<Notification> {
        let mut out: Vec<Notification> = rd(&self.notifications.items)
            .values()
            .filter(|notification| notification.member_id == member_id)
            .cloned()
            .collect();
        out.sort_by_key(|notification| notification.id);
        out
    }
}

/// Search (requirements 2 and 12) and connection suggestions.
impl FacebookSystem {
    pub fn search_members(&self, name: &str) -> Vec<Member> {
        let ids = self.search.members.look(name);
        let members = rd(&self.members.items);
        ids.iter()
            .filter_map(|id| members.get(id).cloned())
            .collect()
    }

    pub fn search_groups(&self, name: &str) -> Vec<Group> {
        let ids = self.search.groups.look(name);
        let groups = rd(&self.groups.items);
        ids.iter()
            .filter_map(|id| groups.get(id).cloned())
            .collect()
    }

    pub fn search_pages(&self, name: &str) -> Vec<Page> {
        let ids = self.search.pages.look(name);
        let pages = rd(&self.pages.items);
        ids.iter().filter_map(|id| pages.get(id).cloned()).collect()
    }

    pub fn search_posts(&self, word: &str) -> Vec<Post> {
        let ids = self.search.posts.look(word);
        let posts = rd(&self.posts.items);
        ids.iter().filter_map(|id| posts.get(id).cloned()).collect()
    }

    /// Members two hops away, ranked by mutual friends. See DESIGN.md.
    pub fn connection_suggestions(&self, member_id: MemberId) -> FbResult<Vec<Member>> {
        let suggestions = {
            let members = rd(&self.members.items);
            let ranked = rank_suggestions(&members, member_id)?;
            ranked
                .into_iter()
                .filter_map(|id| members.get(&id).cloned())
                .collect::<Vec<Member>>()
        };
        if let Some(member) = wr(&self.members.items).get_mut(&member_id) {
            member.suggestions = suggestions.iter().map(|s| s.id).collect();
        }
        Ok(suggestions)
    }
}

fn rank_suggestions(
    all: &HashMap<MemberId, Member>,
    member_id: MemberId,
) -> FbResult<Vec<MemberId>> {
    let me = all
        .get(&member_id)
        .ok_or(FbError::MemberNotFound(member_id))?;
    let direct: &HashSet<MemberId> = &me.connections;
    let mut score: HashMap<MemberId, usize> = HashMap::new();
    for friend in direct {
        let Some(friend) = all.get(friend) else {
            continue;
        };
        for candidate in &friend.connections {
            if *candidate == member_id || direct.contains(candidate) {
                continue;
            }
            *score.entry(*candidate).or_insert(0) += 1;
        }
    }
    let mut out: Vec<MemberId> = score.keys().copied().collect();
    // Most mutual friends first, then by id so the order is deterministic.
    out.sort_by(|a, b| score[b].cmp(&score[a]).then(a.cmp(b)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(fb: &FacebookSystem, a: MemberId, b: MemberId) {
        let invite = fb.send_connection_invitation(a, b).unwrap();
        fb.accept_connection(b, invite).unwrap();
    }

    #[test]
    fn test_profile_roundtrip() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        fb.add_work(
            alice.id,
            Work {
                title: "Eng".into(),
                company: "Acme".into(),
            },
        )
        .unwrap();
        let profile = fb.profile_of(alice.id).unwrap();
        assert_eq!(profile.work.len(), 1);
        assert_eq!(profile.work[0].company, "Acme");
    }

    #[test]
    fn test_invite_self() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("A", "pw").unwrap();
        assert_eq!(
            fb.send_connection_invitation(alice.id, alice.id)
                .unwrap_err(),
            FbError::SelfAction
        );
    }

    #[test]
    fn test_duplicate_invite() {
        let fb = FacebookSystem::new();
        let a = fb.create_member("A", "pw").unwrap();
        let b = fb.create_member("B", "pw").unwrap();
        fb.send_connection_invitation(a.id, b.id).unwrap();
        assert!(matches!(
            fb.send_connection_invitation(a.id, b.id),
            Err(FbError::AlreadyInvited(_))
        ));
        assert!(
            matches!(
                fb.send_connection_invitation(b.id, a.id),
                Err(FbError::AlreadyInvited(_))
            ),
            "the reverse direction is the same pending pair"
        );
    }

    #[test]
    fn test_reinvite_after_reject() {
        let fb = FacebookSystem::new();
        let a = fb.create_member("A", "pw").unwrap();
        let b = fb.create_member("B", "pw").unwrap();
        let invite = fb.send_connection_invitation(a.id, b.id).unwrap();
        fb.reject_connection(b.id, invite).unwrap();
        assert!(fb.send_connection_invitation(a.id, b.id).is_ok());
    }

    #[test]
    fn test_accept_links() {
        let fb = FacebookSystem::new();
        let a = fb.create_member("A", "pw").unwrap();
        let b = fb.create_member("B", "pw").unwrap();
        let invite = fb.send_connection_invitation(a.id, b.id).unwrap();
        fb.accept_connection(b.id, invite).unwrap();
        assert!(fb.member(a.id).unwrap().is_connected_to(b.id));
        assert!(fb.member(b.id).unwrap().is_connected_to(a.id));
        assert!(
            fb.accept_connection(b.id, invite).is_err(),
            "an accepted invitation cannot be accepted twice"
        );
    }

    #[test]
    fn test_unfriend_mutual() {
        let fb = FacebookSystem::new();
        let a = fb.create_member("A", "pw").unwrap();
        let b = fb.create_member("B", "pw").unwrap();
        pair(&fb, a.id, b.id);
        fb.remove_connection(a.id, b.id).unwrap();
        assert!(!fb.member(a.id).unwrap().is_connected_to(b.id));
        assert!(!fb.member(b.id).unwrap().is_connected_to(a.id));
    }

    #[test]
    fn test_suggestions_ranked() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let carol = fb.create_member("Carol", "pw").unwrap();
        let dave = fb.create_member("Dave", "pw").unwrap();
        // Dave is two hops away through both Bob and Carol; Carol through Bob.
        pair(&fb, alice.id, bob.id);
        pair(&fb, alice.id, carol.id);
        pair(&fb, bob.id, dave.id);
        pair(&fb, carol.id, dave.id);
        let suggestions = fb.connection_suggestions(alice.id).unwrap();
        assert_eq!(suggestions.first().map(|m| m.id), Some(dave.id));
        assert!(
            suggestions
                .iter()
                .all(|s| s.id != alice.id && s.id != bob.id && s.id != carol.id)
        );
        assert_eq!(fb.member(alice.id).unwrap().suggestions, vec![dave.id]);
    }

    #[test]
    fn test_group_both_sides() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let group = fb.create_group(alice.id, "Rust", "about rust").unwrap();
        fb.join_group(bob.id, group.id).unwrap();
        assert!(fb.group(group.id).unwrap().member_ids.contains(&bob.id));
        assert!(fb.member(bob.id).unwrap().groups_joined.contains(&group.id));
        fb.leave_group(bob.id, group.id).unwrap();
        assert!(!fb.group(group.id).unwrap().member_ids.contains(&bob.id));
        assert!(!fb.member(bob.id).unwrap().groups_joined.contains(&group.id));
    }

    #[test]
    fn test_disabled_page() {
        let fb = FacebookSystem::new();
        let admin = fb.create_admin("root", "pw").unwrap();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let page = fb.create_page(alice.id, "Acme", "co").unwrap();
        fb.disable_page(admin.id, page.id).unwrap();
        assert!(fb.follow_page(bob.id, page.id).is_err());
        assert!(fb.recommend_page(bob.id, page.id, "great").is_err());
        fb.enable_page(admin.id, page.id).unwrap();
        assert!(fb.follow_page(bob.id, page.id).is_ok());
    }

    #[test]
    fn test_share_attribution() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let post = fb.create_post(alice.id, "I love Rust").unwrap();
        let reshare = fb.share_post(bob.id, post.id).unwrap();
        assert_eq!(reshare.owner_id, bob.id);
        assert_eq!(fb.post(post.id).unwrap().shares, 1);
    }

    #[test]
    fn test_privacy_restricts() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let carol = fb.create_member("Carol", "pw").unwrap();
        pair(&fb, alice.id, bob.id);
        pair(&fb, alice.id, carol.id);
        let post = fb.create_post(alice.id, "secret").unwrap();
        assert!(
            fb.can_view(bob.id, post.id).unwrap(),
            "friends see by default"
        );

        let list = fb.create_privacy_list(alice.id, "close").unwrap();
        fb.add_to_privacy_list(alice.id, list.id, bob.id).unwrap();
        fb.share_post_to_list(alice.id, post.id, list.id).unwrap();
        assert!(fb.can_view(bob.id, post.id).unwrap());
        assert!(
            !fb.can_view(carol.id, post.id).unwrap(),
            "friend, not listed"
        );
        assert!(fb.can_view(alice.id, post.id).unwrap(), "the author always");
    }

    #[test]
    fn test_privacy_needs_friend() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let stranger = fb.create_member("Stranger", "pw").unwrap();
        let list = fb.create_privacy_list(alice.id, "close").unwrap();
        assert!(
            fb.add_to_privacy_list(alice.id, list.id, stranger.id)
                .is_err()
        );
    }

    #[test]
    fn test_stranger_cannot_view() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let stranger = fb.create_member("Stranger", "pw").unwrap();
        let post = fb.create_post(alice.id, "friends only").unwrap();
        assert!(!fb.can_view(stranger.id, post.id).unwrap());
        assert!(fb.visible_posts(stranger.id).is_empty());
    }

    #[test]
    fn test_notifications_raised() {
        let fb = FacebookSystem::new();
        let alice = fb.create_member("Alice", "pw").unwrap();
        let bob = fb.create_member("Bob", "pw").unwrap();
        let invite = fb.send_connection_invitation(bob.id, alice.id).unwrap();
        fb.accept_connection(alice.id, invite).unwrap();
        fb.send_message(bob.id, alice.id, "hi").unwrap();
        let post = fb.create_post(alice.id, "hello world").unwrap();
        fb.add_comment(post.id, bob.id, "nice").unwrap();

        let kinds: Vec<NotificationKind> = fb
            .notifications_for(alice.id)
            .into_iter()
            .map(|n| n.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                NotificationKind::FriendRequest { from: bob.id },
                NotificationKind::Message { from: bob.id },
                NotificationKind::Comment {
                    post_id: post.id,
                    from: bob.id
                },
            ]
        );
        assert!(
            fb.notifications_for(bob.id).is_empty(),
            "the actor is not notified of their own actions"
        );
    }

    #[test]
    fn test_search_prefix() {
        let fb = FacebookSystem::new();
        fb.create_member("Alice", "pw").unwrap();
        assert_eq!(fb.search_members("Al").len(), 1);
        assert!(fb.search_members("zoe").is_empty());
    }

    #[test]
    fn test_block_disables() {
        let fb = FacebookSystem::new();
        let admin = fb.create_admin("root", "pw").unwrap();
        let victim = fb.create_member("V", "pw").unwrap();
        let post = fb.create_post(victim.id, "before the ban").unwrap();

        fb.block_member(admin.id, victim.id).unwrap();
        assert_eq!(
            fb.account_status(victim.id).unwrap(),
            AccountStatus::Blacklisted
        );
        assert_eq!(
            fb.create_post(victim.id, "after").unwrap_err(),
            FbError::AccountDisabled
        );
        assert!(
            fb.post(post.id).is_some(),
            "existing content survives the block"
        );
        assert!(fb.member(victim.id).is_some());

        fb.unblock_member(admin.id, victim.id).unwrap();
        assert!(fb.create_post(victim.id, "after").is_ok());
    }

    #[test]
    fn test_block_needs_admin() {
        // Passing a MemberId here no longer compiles, so the only reachable
        // case is an id that names no admin at all.
        let fb = FacebookSystem::new();
        let victim = fb.create_member("V", "pw").unwrap();
        assert_eq!(
            fb.block_member(AdminId(404), victim.id).unwrap_err(),
            FbError::PermissionDenied
        );
    }
}
