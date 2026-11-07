//! Forum authorization stubs (Phase 1).
//!
//! The full permission/moderation model lands with Phase 1 UI/moderation work.
//! These stubs make the API surface explicit and safe: authenticated LosOS
//! principals can read public forum content and create content; destructive/
//! administrative actions require a more privileged principal (admin/owner group
//! or spare key) following existing lososd patterns. The HTTP layer remains the
//! source of truth for authentication via `guarded`/`guarded_as`.

use serde::{Deserialize, Serialize};

/// Section/route label used when wiring into lososd's route-section checks.
pub const FORUM_SECTION: &str = "forum";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalRef {
    pub name: String,
    pub groups: Vec<String>,
    pub is_spare: bool,
}

impl PrincipalRef {
    pub fn from_name_groups(name: impl Into<String>, groups: Vec<String>, is_spare: bool) -> Self {
        Self {
            name: name.into(),
            groups,
            is_spare,
        }
    }

    pub fn is_adminish(&self) -> bool {
        self.is_spare
            || self
                .groups
                .iter()
                .any(|g| g == "owner" || g == "admin" || g == "wheel")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthzDecision {
    Allow,
    Deny,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ForumAuthz;

impl ForumAuthz {
    /// Read/list operations. Stub: any authenticated principal may read.
    pub fn can_read(_principal: &PrincipalRef) -> AuthzDecision {
        AuthzDecision::Allow
    }

    /// Create topics/posts/messages. Stub: any authenticated principal.
    pub fn can_create(_principal: &PrincipalRef) -> AuthzDecision {
        AuthzDecision::Allow
    }

    /// Space/channel/admin management. Stub: adminish principals only.
    pub fn can_admin(principal: &PrincipalRef) -> AuthzDecision {
        if principal.is_adminish() {
            AuthzDecision::Allow
        } else {
            AuthzDecision::Deny
        }
    }

    /// @LosOS assistant mention hook. Stub: allowed when caller is authenticated;
    /// the feature flag/assistant config is checked separately.
    pub fn can_mention_assistant(_principal: &PrincipalRef) -> AuthzDecision {
        AuthzDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_create_allow_authenticated_stub() {
        let p = PrincipalRef::from_name_groups("alice", vec![], false);
        assert_eq!(ForumAuthz::can_read(&p), AuthzDecision::Allow);
        assert_eq!(ForumAuthz::can_create(&p), AuthzDecision::Allow);
    }

    #[test]
    fn admin_requires_adminish_group() {
        let user = PrincipalRef::from_name_groups("alice", vec!["users".into()], false);
        let owner = PrincipalRef::from_name_groups("rooty", vec!["owner".into()], false);
        let spare = PrincipalRef::from_name_groups("spare", vec![], true);
        assert_eq!(ForumAuthz::can_admin(&user), AuthzDecision::Deny);
        assert_eq!(ForumAuthz::can_admin(&owner), AuthzDecision::Allow);
        assert_eq!(ForumAuthz::can_admin(&spare), AuthzDecision::Allow);
    }
}
