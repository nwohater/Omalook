//! Gmail, Google Calendar and Google Contacts (People API).
use crate::http::{self, Ctx};
use crate::model::*;
use crate::util::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures::{future::join_all, stream, StreamExt};
use mail_builder::MessageBuilder;
use mail_parser::{MessageParser, MimeHeaders};
use reqwest::{Method, RequestBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;

const GMAIL: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const CAL: &str = "https://www.googleapis.com/calendar/v3";
const PEOPLE: &str = "https://people.googleapis.com/v1";

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn req(ctx: &Ctx, m: Method, url: &str) -> RequestBuilder {
    ctx.http.request(m, url).bearer_auth(ctx.token)
}

async fn get(ctx: &Ctx<'_>, url: &str) -> Result<Value, String> {
    http::json(req(ctx, Method::GET, url)).await
}

fn b64(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn unb64(s: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).map_err(|e| format!("bad message encoding: {e}"))
}

pub struct Profile {
    pub email: String,
    pub name: String,
}

pub async fn profile(ctx: &Ctx<'_>) -> Result<Profile, String> {
    let v = get(ctx, "https://openidconnect.googleapis.com/v1/userinfo").await?;
    Ok(Profile { email: s(&v["email"]), name: s(&v["name"]) })
}

// ───────────────────────────── mail ─────────────────────────────

pub async fn folders(ctx: &Ctx<'_>) -> Result<Vec<Folder>, String> {
    let system = [
        ("INBOX", "Inbox", "inbox"),
        ("STARRED", "Starred", "starred"),
        ("DRAFT", "Drafts", "drafts"),
        ("SENT", "Sent", "sent"),
        ("ALL", "All Mail", "all"),
        ("SPAM", "Spam", "junk"),
        ("TRASH", "Trash", "trash"),
    ];
    let labels = get(ctx, &format!("{GMAIL}/labels")).await?;
    let mut user: Vec<(String, String)> = labels["labels"]
        .as_array()
        .map(|a| a.iter().filter(|l| l["type"].as_str() == Some("user")).map(|l| (s(&l["id"]), s(&l["name"]))).collect())
        .unwrap_or_default();
    user.sort_by_key(|(_, n)| n.to_lowercase());

    let mut entries: Vec<(String, String, String, u32)> = system.iter().map(|(i, n, r)| (i.to_string(), n.to_string(), r.to_string(), 0)).collect();
    for (id, name) in user.iter().take(60) {
        let depth = name.matches('/').count() as u32;
        entries.push((id.clone(), name.rsplit('/').next().unwrap_or(name).to_string(), "other".to_string(), depth));
    }

    let ids: Vec<String> = entries.iter().map(|e| e.0.clone()).collect();
    let counts: Vec<Value> = stream::iter(ids)
        .map(|id| async move {
            if id == "ALL" {
                return Value::Null;
            }
            get(ctx, &format!("{GMAIL}/labels/{}", enc(&id))).await.unwrap_or(Value::Null)
        })
        .buffered(8)
        .collect()
        .await;

    Ok(entries
        .into_iter()
        .zip(counts)
        .map(|((id, name, role, depth), c)| Folder {
            id,
            name,
            role,
            // for drafts Gmail reports thread counts, which is what the UI shows
            unread: if depth == 0 && c["messagesUnread"].is_null() { 0 } else { u(&c["messagesUnread"]) },
            total: u(&c["messagesTotal"]),
            depth,
        })
        .collect())
}

fn header(v: &Value, name: &str) -> String {
    v["payload"]["headers"]
        .as_array()
        .and_then(|a| a.iter().find(|h| s(&h["name"]).eq_ignore_ascii_case(name)))
        .map(|h| s(&h["value"]))
        .unwrap_or_default()
}

fn has_label(v: &Value, label: &str) -> bool {
    v["labelIds"].as_array().map(|a| a.iter().any(|l| l.as_str() == Some(label))).unwrap_or(false)
}

fn received_of(v: &Value) -> String {
    v["internalDate"]
        .as_str()
        .and_then(|ms| ms.parse::<i64>().ok())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

fn summary(v: &Value) -> MsgSummary {
    MsgSummary {
        id: s(&v["id"]),
        subject: decode_header(&header(v, "Subject")),
        from: parse_addr_list(&header(v, "From")).into_iter().next().unwrap_or_default(),
        to: parse_addr_list(&header(v, "To")),
        received: received_of(v),
        is_read: !has_label(v, "UNREAD"),
        preview: s(&v["snippet"]).replace("&#39;", "'").replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"),
        has_attachments: v["payload"]["mimeType"].as_str() == Some("multipart/mixed"),
        flagged: has_label(v, "STARRED"),
    }
}

async fn meta(ctx: &Ctx<'_>, id: &str) -> Result<Value, String> {
    get(ctx, &format!("{GMAIL}/messages/{}?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Subject", enc(id))).await
}

pub async fn messages(ctx: &Ctx<'_>, folder: &str, page: Option<String>, search: Option<String>) -> Result<MessagePage, String> {
    let mut url = format!("{GMAIL}/messages?maxResults=50");
    if folder != "ALL" {
        url += &format!("&labelIds={}", enc(folder));
    }
    if folder == "SPAM" || folder == "TRASH" {
        url += "&includeSpamTrash=true";
    }
    if let Some(q) = search.filter(|q| !q.trim().is_empty()) {
        url += &format!("&q={}", enc(q.trim()));
    }
    if let Some(p) = page {
        url += &format!("&pageToken={}", enc(&p));
    }
    let v = get(ctx, &url).await?;
    let ids: Vec<String> = v["messages"].as_array().map(|a| a.iter().map(|m| s(&m["id"])).collect()).unwrap_or_default();
    let items = stream::iter(ids)
        .map(|id| async move { meta(ctx, &id).await })
        .buffered(12)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|m| summary(&m))
        .collect();
    Ok(MessagePage { items, next: v["nextPageToken"].as_str().map(String::from) })
}

async fn raw_message(ctx: &Ctx<'_>, id: &str) -> Result<(Value, Vec<u8>), String> {
    let v = get(ctx, &format!("{GMAIL}/messages/{}?format=raw", enc(id))).await?;
    let raw = unb64(&s(&v["raw"]))?;
    Ok((v, raw))
}

struct Parsed {
    html: String,
    attachments: Vec<Attachment>,
}

fn parse_body(msg: &mail_parser::Message) -> Parsed {
    let html = msg.body_html(0).map(|h| h.to_string()).unwrap_or_default();
    let mut images: HashMap<String, (String, Vec<u8>)> = HashMap::new();
    let mut attachments = vec![];
    for (i, part) in msg.attachments().enumerate() {
        let cid = part.content_id().map(|c| c.trim_matches(|ch| ch == '<' || ch == '>').to_string());
        let inline = cid.as_ref().map(|c| html.contains(&format!("cid:{c}"))).unwrap_or(false);
        let ctype = part
            .content_type()
            .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream")))
            .unwrap_or_else(|| "application/octet-stream".into());
        if inline {
            images.insert(cid.clone().unwrap(), (ctype.clone(), part.contents().to_vec()));
        }
        attachments.push(Attachment {
            id: i.to_string(),
            name: part.attachment_name().map(String::from).unwrap_or_else(|| format!("attachment-{}", i + 1)),
            size: part.contents().len() as u64,
            content_type: ctype,
            inline,
        });
    }
    Parsed { html: replace_cids(&html, &images), attachments }
}

fn addrs(a: Option<&mail_parser::Address>) -> Vec<Addr> {
    a.map(|a| a.iter().map(|x| addr(x.name().unwrap_or_default(), x.address().unwrap_or_default())).collect()).unwrap_or_default()
}

pub async fn message(ctx: &Ctx<'_>, id: &str) -> Result<MsgDetail, String> {
    let (v, raw) = raw_message(ctx, id).await?;
    let msg = MessageParser::default().parse(&raw).ok_or("could not parse message")?;
    let p = parse_body(&msg);
    Ok(MsgDetail {
        summary: MsgSummary {
            id: id.to_string(),
            subject: msg.subject().unwrap_or_default().to_string(),
            from: addrs(msg.from()).into_iter().next().unwrap_or_default(),
            to: addrs(msg.to()),
            received: received_of(&v),
            is_read: !has_label(&v, "UNREAD"),
            preview: s(&v["snippet"]),
            has_attachments: p.attachments.iter().any(|a| !a.inline),
            flagged: has_label(&v, "STARRED"),
        },
        cc: addrs(msg.cc()),
        html: p.html,
        attachments: p.attachments,
    })
}

async fn modify(ctx: &Ctx<'_>, id: &str, add: &[&str], remove: &[&str]) -> Result<(), String> {
    http::json(req(ctx, Method::POST, &format!("{GMAIL}/messages/{}/modify", enc(id))).json(&json!({ "addLabelIds": add, "removeLabelIds": remove }))).await?;
    Ok(())
}

pub async fn set_read(ctx: &Ctx<'_>, id: &str, read: bool) -> Result<(), String> {
    if read { modify(ctx, id, &[], &["UNREAD"]).await } else { modify(ctx, id, &["UNREAD"], &[]).await }
}

pub async fn set_flag(ctx: &Ctx<'_>, id: &str, flagged: bool) -> Result<(), String> {
    if flagged { modify(ctx, id, &["STARRED"], &[]).await } else { modify(ctx, id, &[], &["STARRED"]).await }
}

/// `target`: archive | trash | junk | inbox | <label id>; `from` is the folder the message is currently shown in.
pub async fn move_to(ctx: &Ctx<'_>, id: &str, target: &str, from: Option<&str>) -> Result<(), String> {
    match target {
        "archive" => modify(ctx, id, &[], &["INBOX"]).await,
        "trash" => {
            http::json(req(ctx, Method::POST, &format!("{GMAIL}/messages/{}/trash", enc(id)))).await?;
            Ok(())
        }
        "junk" => modify(ctx, id, &["SPAM"], &["INBOX"]).await,
        "inbox" => {
            if from == Some("TRASH") {
                http::json(req(ctx, Method::POST, &format!("{GMAIL}/messages/{}/untrash", enc(id)))).await?;
            }
            modify(ctx, id, &["INBOX"], &["SPAM"]).await
        }
        label => {
            let remove: Vec<&str> = from.filter(|f| !matches!(*f, "ALL" | "STARRED" | "DRAFT" | "SENT")).into_iter().collect();
            modify(ctx, id, &[label], &remove).await
        }
    }
}

pub async fn delete_forever(_ctx: &Ctx<'_>, _id: &str) -> Result<(), String> {
    Err("Gmail empties the Trash automatically after 30 days (Omalook can't delete mail permanently with its limited access).".into())
}

pub async fn attachment_bytes(ctx: &Ctx<'_>, msg: &str, att: &str) -> Result<Vec<u8>, String> {
    let (_, raw) = raw_message(ctx, msg).await?;
    let parsed = MessageParser::default().parse(&raw).ok_or("could not parse message")?;
    let idx: usize = att.parse().map_err(|_| "bad attachment id")?;
    let found = parsed.attachments().nth(idx).map(|p| p.contents().to_vec());
    found.ok_or_else(|| "attachment not found".to_string())
}

fn clean(list: &[String]) -> Vec<String> {
    list.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
}

struct Source {
    thread_id: Option<String>,
    kept: Vec<(String, String, Vec<u8>)>,
}

async fn existing_draft(ctx: &Ctx<'_>, draft_id: &str, keep: &[String]) -> Result<Source, String> {
    let v = get(ctx, &format!("{GMAIL}/drafts/{}?format=raw", enc(draft_id))).await?;
    let raw = unb64(&s(&v["message"]["raw"]))?;
    let msg = MessageParser::default().parse(&raw).ok_or("could not parse draft")?;
    let kept = msg
        .attachments()
        .enumerate()
        .filter(|(i, _)| keep.contains(&i.to_string()))
        .map(|(i, p)| {
            let ctype = p.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream"))).unwrap_or_else(|| "application/octet-stream".into());
            (p.attachment_name().map(String::from).unwrap_or_else(|| format!("attachment-{}", i + 1)), ctype, p.contents().to_vec())
        })
        .collect();
    Ok(Source { thread_id: v["message"]["threadId"].as_str().map(String::from), kept })
}

async fn build(ctx: &Ctx<'_>, out: &Outgoing) -> Result<(Vec<u8>, Option<String>), String> {
    let src = match &out.draft_id {
        Some(d) => existing_draft(ctx, d, &out.keep_attachments).await?,
        None => Source { thread_id: None, kept: vec![] },
    };
    let mut thread_id = src.thread_id.clone();

    let from_name = ctx.account.name.clone();
    let from_addr = ctx.account.email.clone();
    let mut b = MessageBuilder::new()
        .from((from_name, from_addr))
        .subject(out.subject.clone())
        .html_body(out.html.clone())
        .text_body(strip_tags(&out.html));
    let (to, cc, bcc) = (clean(&out.to), clean(&out.cc), clean(&out.bcc));
    if !to.is_empty() {
        b = b.to(to);
    }
    if !cc.is_empty() {
        b = b.cc(cc);
    }
    if !bcc.is_empty() {
        b = b.bcc(bcc);
    }

    if let (Some(rid), None) = (&out.reply_to_id, &thread_id) {
        let m = get(ctx, &format!("{GMAIL}/messages/{}?format=metadata&metadataHeaders=Message-ID&metadataHeaders=References", enc(rid))).await?;
        thread_id = m["threadId"].as_str().map(String::from);
        let mid = header(&m, "Message-ID");
        let id = mid.trim().trim_matches(|c| c == '<' || c == '>').to_string();
        if !id.is_empty() {
            let mut refs: Vec<String> = header(&m, "References")
                .split_whitespace()
                .map(|r| r.trim_matches(|c| c == '<' || c == '>').to_string())
                .filter(|r| !r.is_empty())
                .collect();
            refs.push(id.clone());
            b = b.in_reply_to(id).references(refs);
        }
    }

    if let (Some(fid), None) = (&out.forward_of_id, &out.draft_id) {
        let (_, raw) = raw_message(ctx, fid).await?;
        if let Some(orig) = MessageParser::default().parse(&raw) {
            let info = parse_body(&orig).attachments;
            for (i, part) in orig.attachments().enumerate() {
                if info.get(i).map(|a| !a.inline).unwrap_or(false) {
                    b = b.attachment(info[i].content_type.clone(), info[i].name.clone(), part.contents().to_vec());
                }
            }
        }
    }

    for (name, ctype, data) in src.kept {
        b = b.attachment(ctype, name, data);
    }
    for a in &out.attachments {
        let data = tokio::fs::read(&a.path).await.map_err(|e| format!("can't read {}: {e}", a.name))?;
        b = b.attachment(mime_for(&a.name), a.name.clone(), data);
    }
    let raw = b.write_to_vec().map_err(|e| format!("could not build message: {e}"))?;
    Ok((raw, thread_id))
}

fn message_json(raw: &[u8], thread: Option<String>) -> Value {
    let mut m = json!({ "raw": b64(raw) });
    if let Some(t) = thread {
        m["threadId"] = json!(t);
    }
    m
}

pub async fn save_draft(ctx: &Ctx<'_>, out: &Outgoing) -> Result<String, String> {
    let (raw, thread) = build(ctx, out).await?;
    let body = json!({ "message": message_json(&raw, thread) });
    let v = match &out.draft_id {
        Some(d) => http::json(req(ctx, Method::PUT, &format!("{GMAIL}/drafts/{}", enc(d))).json(&body)).await?,
        None => http::json(req(ctx, Method::POST, &format!("{GMAIL}/drafts")).json(&body)).await?,
    };
    Ok(s(&v["id"]))
}

pub async fn send(ctx: &Ctx<'_>, out: &Outgoing) -> Result<(), String> {
    if out.draft_id.is_some() {
        let id = save_draft(ctx, out).await?;
        http::json(req(ctx, Method::POST, &format!("{GMAIL}/drafts/send")).json(&json!({ "id": id }))).await?;
    } else {
        let (raw, thread) = build(ctx, out).await?;
        http::json(req(ctx, Method::POST, &format!("{GMAIL}/messages/send")).json(&message_json(&raw, thread))).await?;
    }
    Ok(())
}

pub async fn discard_draft(ctx: &Ctx<'_>, draft_id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("{GMAIL}/drafts/{}", enc(draft_id)))).await?;
    Ok(())
}

/// `message_id` is the id shown in the Drafts folder; Gmail's draft id is looked up from it.
pub async fn open_draft(ctx: &Ctx<'_>, id: &str) -> Result<DraftData, String> {
    // `id` is either a Gmail draft id (after saving) or a message id from the Drafts folder
    let message_id = id;
    let mut page: Option<String> = None;
    let mut draft_id = get(ctx, &format!("{GMAIL}/drafts/{}?format=minimal", enc(id))).await.ok().map(|_| id.to_string());
    for _ in 0..10 {
        if draft_id.is_some() {
            break;
        }
        let mut url = format!("{GMAIL}/drafts?maxResults=100");
        if let Some(p) = &page {
            url += &format!("&pageToken={}", enc(p));
        }
        let v = get(ctx, &url).await?;
        if let Some(d) = v["drafts"].as_array().and_then(|a| a.iter().find(|d| d["message"]["id"].as_str() == Some(message_id))) {
            draft_id = Some(s(&d["id"]));
            break;
        }
        match v["nextPageToken"].as_str() {
            Some(n) => page = Some(n.to_string()),
            None => break,
        }
    }
    let draft_id = draft_id.ok_or("draft not found")?;
    let v = get(ctx, &format!("{GMAIL}/drafts/{}?format=raw", enc(&draft_id))).await?;
    let raw = unb64(&s(&v["message"]["raw"]))?;
    let msg = MessageParser::default().parse(&raw).ok_or("could not parse draft")?;
    let p = parse_body(&msg);
    let list = |a: Option<&mail_parser::Address>| addrs(a).into_iter().map(|x| x.address).collect::<Vec<_>>();
    Ok(DraftData {
        draft_id,
        to: list(msg.to()),
        cc: list(msg.cc()),
        bcc: list(msg.bcc()),
        subject: msg.subject().unwrap_or_default().to_string(),
        html: p.html,
        attachments: p.attachments.into_iter().filter(|a| !a.inline).collect(),
        reply_to_id: None,
    })
}

// ──────────────────────────── calendar ───────────────────────────

pub async fn calendars(ctx: &Ctx<'_>) -> Result<Vec<CalendarInfo>, String> {
    let v = get(ctx, &format!("{CAL}/users/me/calendarList?minAccessRole=reader&maxResults=50")).await?;
    Ok(v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["selected"].as_bool().unwrap_or(false) || c["primary"].as_bool().unwrap_or(false))
                .map(|c| CalendarInfo {
                    id: s(&c["id"]),
                    name: if s(&c["summaryOverride"]).is_empty() { s(&c["summary"]) } else { s(&c["summaryOverride"]) },
                    color: if s(&c["backgroundColor"]).is_empty() { "#7aa2f7".into() } else { s(&c["backgroundColor"]) },
                })
                .collect()
        })
        .unwrap_or_default())
}

fn to_utc(dt: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(dt)
        .map(|d| d.with_timezone(&chrono::Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|_| dt.to_string())
}

fn event_from(v: &Value, cal: &CalendarInfo) -> Event {
    let all_day = v["start"]["date"].is_string();
    let (start, end) = if all_day {
        (s(&v["start"]["date"]), s(&v["end"]["date"]))
    } else {
        (to_utc(&s(&v["start"]["dateTime"])), to_utc(&s(&v["end"]["dateTime"])))
    };
    Event {
        id: s(&v["id"]),
        calendar_id: cal.id.clone(),
        calendar_name: cal.name.clone(),
        color: cal.color.clone(),
        title: s(&v["summary"]),
        start,
        end,
        all_day,
        location: s(&v["location"]),
        description: strip_tags(&s(&v["description"])).trim().to_string(),
        attendees: v["attendees"].as_array().map(|a| a.iter().map(|x| s(&x["email"])).collect()).unwrap_or_default(),
        organizer: { let n = s(&v["organizer"]["displayName"]); if n.is_empty() { s(&v["organizer"]["email"]) } else { n } },
        web_link: v["htmlLink"].as_str().map(String::from),
        join_url: v["hangoutLink"].as_str().map(String::from).or_else(|| v["conferenceData"]["entryPoints"][0]["uri"].as_str().map(String::from)),
    }
}

pub async fn events(ctx: &Ctx<'_>, from: &str, to: &str) -> Result<Vec<Event>, String> {
    let cals = calendars(ctx).await?;
    let results = join_all(cals.iter().take(12).map(|c| async move {
        get(ctx, &format!("{CAL}/calendars/{}/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=250", enc(&c.id), enc(from), enc(to))).await
    }))
    .await;
    let mut out = vec![];
    for (c, r) in cals.iter().zip(results) {
        let v = r?;
        for e in v["items"].as_array().cloned().unwrap_or_default() {
            if e["status"].as_str() != Some("cancelled") {
                out.push(event_from(&e, c));
            }
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(out)
}

fn google_time(iso: &str, all_day: bool) -> Value {
    if all_day { json!({ "date": &iso[..10] }) } else { json!({ "dateTime": iso, "timeZone": "UTC" }) }
}

pub async fn save_event(ctx: &Ctx<'_>, e: &EventInput) -> Result<(), String> {
    let mut body = json!({
        "summary": e.title,
        "location": e.location,
        "start": google_time(&e.start, e.all_day),
        "end": google_time(&e.end, e.all_day),
        "attendees": e.attendees.iter().filter(|a| !a.trim().is_empty()).map(|a| json!({ "email": a.trim() })).collect::<Vec<_>>(),
    });
    if let Some(d) = &e.description {
        body["description"] = json!(d);
    }
    let cal = enc(e.calendar_id.as_deref().unwrap_or("primary"));
    match &e.id {
        Some(id) => http::json(req(ctx, Method::PATCH, &format!("{CAL}/calendars/{cal}/events/{}?sendUpdates=all", enc(id))).json(&body)).await?,
        None => http::json(req(ctx, Method::POST, &format!("{CAL}/calendars/{cal}/events?sendUpdates=all")).json(&body)).await?,
    };
    Ok(())
}

pub async fn delete_event(ctx: &Ctx<'_>, calendar_id: &str, id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("{CAL}/calendars/{}/events/{}?sendUpdates=all", enc(calendar_id), enc(id)))).await?;
    Ok(())
}

// ──────────────────────────── contacts ───────────────────────────

const FIELDS: &str = "names,emailAddresses,phoneNumbers,organizations";

fn values(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().map(|x| s(&x["value"])).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
}

fn contact_from(v: &Value) -> Contact {
    Contact {
        id: s(&v["resourceName"]),
        etag: v["etag"].as_str().map(String::from),
        name: s(&v["names"][0]["displayName"]),
        emails: values(v, "emailAddresses"),
        phones: values(v, "phoneNumbers"),
        company: s(&v["organizations"][0]["name"]),
        title: s(&v["organizations"][0]["title"]),
    }
}

pub async fn contacts(ctx: &Ctx<'_>) -> Result<Vec<Contact>, String> {
    let mut out = vec![];
    let mut page: Option<String> = None;
    for _ in 0..10 {
        let mut url = format!("{PEOPLE}/people/me/connections?personFields={FIELDS}&pageSize=1000&sortOrder=FIRST_NAME_ASCENDING");
        if let Some(p) = &page {
            url += &format!("&pageToken={}", enc(p));
        }
        let v = get(ctx, &url).await?;
        out.extend(v["connections"].as_array().map(|a| a.iter().map(contact_from).collect::<Vec<_>>()).unwrap_or_default());
        match v["nextPageToken"].as_str() {
            Some(n) => page = Some(n.to_string()),
            None => break,
        }
    }
    Ok(out)
}

pub async fn save_contact(ctx: &Ctx<'_>, c: &Contact) -> Result<(), String> {
    let (given, family) = split_name(&c.name);
    let mut body = json!({
        "names": [{ "givenName": given, "familyName": family }],
        "emailAddresses": c.emails.iter().filter(|e| !e.trim().is_empty()).map(|e| json!({ "value": e.trim() })).collect::<Vec<_>>(),
        "phoneNumbers": c.phones.iter().filter(|p| !p.trim().is_empty()).map(|p| json!({ "value": p.trim() })).collect::<Vec<_>>(),
        "organizations": if c.company.is_empty() && c.title.is_empty() { vec![] } else { vec![json!({ "name": c.company, "title": c.title })] },
    });
    if c.id.is_empty() {
        http::json(req(ctx, Method::POST, &format!("{PEOPLE}/people:createContact")).json(&body)).await?;
    } else {
        body["etag"] = json!(c.etag);
        http::json(req(ctx, Method::PATCH, &format!("{PEOPLE}/{}:updateContact?updatePersonFields={FIELDS}", c.id)).json(&body)).await?;
    }
    Ok(())
}

pub async fn delete_contact(ctx: &Ctx<'_>, id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("{PEOPLE}/{id}:deleteContact"))).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_roundtrip_with_attachment_and_inline_image() {
        let raw = MessageBuilder::new()
            .from(("Me", "me@example.com"))
            .to(vec!["a@example.com", "b@example.com"])
            .subject("Héllo")
            .html_body("<p>Hi <img src=\"cid:logo@x\"></p>")
            .text_body("Hi")
            .inline("image/png", "logo@x", vec![1u8, 2, 3])
            .attachment("text/plain", "notes.txt", b"hello".to_vec())
            .write_to_vec()
            .unwrap();
        let msg = MessageParser::default().parse(&raw).unwrap();
        let p = parse_body(&msg);
        assert!(p.html.contains("data:image/png;base64,AQID"), "cid should become a data URI: {}", p.html);
        let files: Vec<_> = p.attachments.iter().filter(|a| !a.inline).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "notes.txt");
        assert_eq!(addrs(msg.to()).len(), 2);
        assert_eq!(msg.subject(), Some("Héllo"));
    }

    #[test]
    fn summary_reads_gmail_metadata() {
        let v = json!({
            "id": "abc", "snippet": "it&#39;s here", "internalDate": "1790000000000",
            "labelIds": ["INBOX", "UNREAD", "STARRED"],
            "payload": { "mimeType": "multipart/mixed", "headers": [
                {"name": "From", "value": "=?UTF-8?Q?Jos=C3=A9?= <jose@x.com>"},
                {"name": "Subject", "value": "Plan"}, {"name": "To", "value": "me@x.com"}] }
        });
        let m = summary(&v);
        assert_eq!(m.from.name, "José");
        assert!(!m.is_read && m.flagged && m.has_attachments);
        assert_eq!(m.preview, "it's here");
        assert!(m.received.starts_with("2026-"));
    }
}
