//! Canonical forum entities (Phase 1).
//!
//! These types are intentionally explicit and serde-friendly so they can be
//! stored in SQLite, returned by API scaffolding, and evolved for federation/
//! bridge adapters in later phases.

use serde::{Deserialize, Serialize};

pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: Id,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    Topic,
    Channel,
    Dm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: Id,
    pub space_id: Option<Id>,
    pub slug: String,
    pub name: String,
    pub kind: ChannelKind,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub id: Id,
    pub channel_id: Id,
    pub title: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed: bool,
}

/// A post in a Discourse-style topic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: Id,
    pub topic_id: Id,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A message in a channel/real-time timeline (Slack-style mode).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: Id,
    pub channel_id: Id,
    pub thread_id: Option<Id>,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub subject: String,
    pub space_id: Option<Id>,
    pub channel_id: Option<Id>,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub id: Id,
    pub target_kind: String,
    pub target_id: Id,
    pub actor: String,
    pub emoji: String,
    pub created_at: String,
}

/// Mention hook output. `is_assistant` marks `@LosOS` for Phase 2 handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    pub target: String,
    pub is_assistant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpace {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateChannel {
    pub space_id: Option<Id>,
    pub slug: String,
    pub name: String,
    pub kind: ChannelKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateTopic {
    pub channel_id: Id,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreatePost {
    pub topic_id: Id,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessage {
    pub channel_id: Id,
    pub thread_id: Option<Id>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateReaction {
    pub target_kind: String,
    pub target_id: Id,
    pub emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateMembership {
    pub subject: String,
    pub space_id: Option<Id>,
    pub channel_id: Option<Id>,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_serializes_camel_case() {
        let s = Space {
            id: 1,
            slug: "general".into(),
            name: "General".into(),
            description: "d".into(),
            created_by: "alice".into(),
            created_at: "t".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["createdBy"], "alice");
        assert_eq!(v["createdAt"], "t");
    }

    #[test]
    fn channel_kind_roundtrips() {
        let v = serde_json::to_value(ChannelKind::Topic).unwrap();
        assert_eq!(v, "topic");
        let k: ChannelKind = serde_json::from_value(serde_json::json!("channel")).unwrap();
        assert_eq!(k, ChannelKind::Channel);
    }

    #[test]
    fn create_topic_deserializes() {
        let t: CreateTopic = serde_json::from_value(serde_json::json!({
            "channelId": 7, "title": "Hi", "body": "Body"
        }))
        .unwrap();
        assert_eq!(t.channel_id, 7);
        assert_eq!(t.title, "Hi");
    }
}
