//! Provider-neutral types shared by the Microsoft and Google backends and the UI.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Microsoft,
    Google,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Microsoft => "microsoft",
            Kind::Google => "google",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub id: String,
    pub kind: Kind,
    pub email: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Addr {
    pub name: String,
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// inbox | drafts | sent | archive | trash | junk | starred | all | other
    pub role: String,
    pub unread: u32,
    pub total: u32,
    pub depth: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MsgSummary {
    pub id: String,
    pub subject: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub received: String,
    pub is_read: bool,
    pub preview: String,
    pub has_attachments: bool,
    pub flagged: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub content_type: String,
    pub inline: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MsgDetail {
    #[serde(flatten)]
    pub summary: MsgSummary,
    pub cc: Vec<Addr>,
    pub html: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    pub items: Vec<MsgSummary>,
    pub next: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutAttachment {
    pub name: String,
    pub path: String,
}

/// A message being composed: new, reply, forward, or an existing draft.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub html: String,
    /// New files to attach (from disk).
    pub attachments: Vec<OutAttachment>,
    /// Attachment ids already on the draft that should be kept (draft edits only).
    pub keep_attachments: Vec<String>,
    pub reply_to_id: Option<String>,
    pub reply_all: bool,
    /// Forward this message (its attachments come along).
    pub forward_of_id: Option<String>,
    pub draft_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DraftData {
    pub draft_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub html: String,
    pub attachments: Vec<Attachment>,
    pub reply_to_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub calendar_id: String,
    pub calendar_name: String,
    pub color: String,
    pub title: String,
    /// RFC 3339 UTC for timed events, `YYYY-MM-DD` for all-day events.
    pub start: String,
    /// Same format as `start`; exclusive for all-day events.
    pub end: String,
    pub all_day: bool,
    pub location: String,
    pub description: String,
    pub attendees: Vec<String>,
    pub organizer: String,
    pub web_link: Option<String>,
    pub join_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub id: Option<String>,
    pub calendar_id: Option<String>,
    pub title: String,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub location: String,
    pub description: Option<String>,
    pub attendees: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInfo {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub etag: Option<String>,
    pub name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub company: String,
    pub title: String,
}
