//! Microsoft 365 / Outlook.com through the Microsoft Graph API.
use crate::http::{self, Ctx};
use crate::model::*;
use crate::util::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::future::{join_all, BoxFuture};
use reqwest::{Method, RequestBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;

const BASE: &str = "https://graph.microsoft.com/v1.0";
const SIMPLE_ATTACH_LIMIT: usize = 3_000_000;
const CHUNK: usize = 320 * 1024 * 12;

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn req(ctx: &Ctx, m: Method, path: &str) -> RequestBuilder {
    let url = if path.starts_with("https://") { path.to_string() } else { format!("{BASE}{path}") };
    ctx.http.request(m, url).bearer_auth(ctx.token)
}

async fn get(ctx: &Ctx<'_>, path: &str) -> Result<Value, String> {
    http::json(req(ctx, Method::GET, path)).await
}

pub struct Profile {
    pub email: String,
    pub name: String,
}

pub async fn profile(ctx: &Ctx<'_>) -> Result<Profile, String> {
    let v = get(ctx, "/me?$select=displayName,mail,userPrincipalName").await?;
    let email = if s(&v["mail"]).is_empty() { s(&v["userPrincipalName"]) } else { s(&v["mail"]) };
    Ok(Profile { email, name: s(&v["displayName"]) })
}

// ───────────────────────────── mail ─────────────────────────────

const FOLDER_SELECT: &str = "id,displayName,unreadItemCount,totalItemCount,childFolderCount";

async fn children(ctx: &Ctx<'_>, parent: Option<&str>) -> Result<Vec<Value>, String> {
    let mut url = match parent {
        None => format!("/me/mailFolders?$top=100&$select={FOLDER_SELECT}"),
        Some(p) => format!("/me/mailFolders/{}/childFolders?$top=100&$select={FOLDER_SELECT}", enc(p)),
    };
    let mut out = vec![];
    for _ in 0..10 {
        let v = get(ctx, &url).await?;
        out.extend(v["value"].as_array().cloned().unwrap_or_default());
        match v["@odata.nextLink"].as_str() {
            Some(n) => url = n.to_string(),
            None => break,
        }
    }
    Ok(out)
}

fn role_rank(role: &str) -> u8 {
    ["inbox", "drafts", "sent", "archive", "trash", "junk"].iter().position(|r| *r == role).unwrap_or(99) as u8
}

fn walk<'a>(
    ctx: &'a Ctx<'a>,
    parent: Option<String>,
    depth: u32,
    roles: &'a HashMap<String, &'static str>,
    out: &'a mut Vec<Folder>,
) -> BoxFuture<'a, Result<(), String>> {
    Box::pin(async move {
        let mut items = children(ctx, parent.as_deref()).await?;
        if depth == 0 {
            items.sort_by_key(|v| (role_rank(roles.get(&s(&v["id"])).copied().unwrap_or("other")), s(&v["displayName"]).to_lowercase()));
        } else {
            items.sort_by_key(|v| s(&v["displayName"]).to_lowercase());
        }
        for v in items {
            let id = s(&v["id"]);
            out.push(Folder {
                id: id.clone(),
                name: s(&v["displayName"]),
                role: roles.get(&id).copied().unwrap_or("other").to_string(),
                unread: u(&v["unreadItemCount"]),
                total: u(&v["totalItemCount"]),
                depth,
            });
            if u(&v["childFolderCount"]) > 0 && depth < 4 {
                walk(ctx, Some(id), depth + 1, roles, out).await?;
            }
        }
        Ok(())
    })
}

pub async fn folders(ctx: &Ctx<'_>) -> Result<Vec<Folder>, String> {
    let aliases = [("inbox", "inbox"), ("drafts", "drafts"), ("sentitems", "sent"), ("archive", "archive"), ("deleteditems", "trash"), ("junkemail", "junk")];
    let paths: Vec<String> = aliases.iter().map(|(a, _)| format!("/me/mailFolders/{a}?$select=id")).collect();
    let results = join_all(paths.iter().map(|p| get(ctx, p))).await;
    let mut roles: HashMap<String, &'static str> = HashMap::new();
    for ((_, role), r) in aliases.iter().zip(results) {
        if let Ok(v) = r {
            roles.insert(s(&v["id"]), role);
        }
    }
    let mut out = vec![];
    walk(ctx, None, 0, &roles, &mut out).await?;
    Ok(out)
}

fn recipients_of(v: &Value) -> Vec<Addr> {
    v.as_array()
        .map(|a| a.iter().map(|r| addr(&s(&r["emailAddress"]["name"]), &s(&r["emailAddress"]["address"]))).collect())
        .unwrap_or_default()
}

fn summary(v: &Value) -> MsgSummary {
    MsgSummary {
        id: s(&v["id"]),
        subject: s(&v["subject"]),
        from: addr(&s(&v["from"]["emailAddress"]["name"]), &s(&v["from"]["emailAddress"]["address"])),
        to: recipients_of(&v["toRecipients"]),
        received: s(&v["receivedDateTime"]),
        is_read: v["isRead"].as_bool().unwrap_or(true),
        preview: s(&v["bodyPreview"]),
        has_attachments: v["hasAttachments"].as_bool().unwrap_or(false),
        flagged: v["flag"]["flagStatus"].as_str() == Some("flagged"),
    }
}

const LIST_SELECT: &str = "id,subject,from,toRecipients,receivedDateTime,isRead,bodyPreview,hasAttachments,flag";

pub async fn messages(ctx: &Ctx<'_>, folder: &str, page: Option<String>, search: Option<String>) -> Result<MessagePage, String> {
    let path = match (page, search.filter(|q| !q.trim().is_empty())) {
        // paging links come back from Graph itself; never send the token anywhere else
        (Some(next), _) if !next.starts_with(BASE) => return Err("invalid page link".into()),
        (Some(next), _) => next,
        // $search can't be combined with $orderby
        (None, Some(q)) => format!(
            "/me/mailFolders/{}/messages?$top=50&$select={LIST_SELECT}&$search=%22{}%22",
            enc(folder),
            enc(q.replace('"', "").trim())
        ),
        (None, None) => format!(
            "/me/mailFolders/{}/messages?$top=50&$orderby=receivedDateTime%20desc&$select={LIST_SELECT}",
            enc(folder)
        ),
    };
    let v = get(ctx, &path).await?;
    Ok(MessagePage {
        items: v["value"].as_array().map(|a| a.iter().map(summary).collect()).unwrap_or_default(),
        next: v["@odata.nextLink"].as_str().map(String::from),
    })
}

const DETAIL_SELECT: &str = "id,subject,from,toRecipients,ccRecipients,bccRecipients,receivedDateTime,body,hasAttachments,isRead,flag,bodyPreview";
const ATT_EXPAND: &str = "attachments($select=id,name,size,contentType,isInline,contentId)";

async fn message_value(ctx: &Ctx<'_>, id: &str) -> Result<Value, String> {
    http::json(
        req(ctx, Method::GET, &format!("/me/messages/{}?$select={DETAIL_SELECT}&$expand={ATT_EXPAND}", enc(id)))
            .header("Prefer", "outlook.body-content-type=\"html\""),
    )
    .await
}

async fn html_with_inline(ctx: &Ctx<'_>, id: &str, v: &Value) -> String {
    let content = s(&v["body"]["content"]);
    let html = if v["body"]["contentType"].as_str() == Some("text") { format!("<pre>{}</pre>", escape_html(&content)) } else { content };
    if !html.contains("cid:") {
        return html;
    }
    let atts: Vec<&Value> = v["attachments"]
        .as_array()
        .map(|a| a.iter().filter(|x| x["contentId"].as_str().is_some() && x["size"].as_u64().unwrap_or(0) < 6_000_000).collect())
        .unwrap_or_default();
    let fetched = join_all(atts.iter().map(|a| {
        let url = format!("/me/messages/{}/attachments/{}/$value", enc(id), enc(&s(&a["id"])));
        async move { http::bytes(req(ctx, Method::GET, &url)).await }
    }))
    .await;
    let mut map = HashMap::new();
    for (a, data) in atts.iter().zip(fetched) {
        if let Ok(d) = data {
            map.insert(s(&a["contentId"]).trim_matches(|c| c == '<' || c == '>').to_string(), (s(&a["contentType"]), d));
        }
    }
    replace_cids(&html, &map)
}

fn attachments_of(v: &Value) -> Vec<Attachment> {
    v["attachments"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| Attachment {
                    id: s(&x["id"]),
                    name: s(&x["name"]),
                    size: x["size"].as_u64().unwrap_or(0),
                    content_type: s(&x["contentType"]),
                    inline: x["isInline"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn message(ctx: &Ctx<'_>, id: &str) -> Result<MsgDetail, String> {
    let v = message_value(ctx, id).await?;
    Ok(MsgDetail {
        summary: summary(&v),
        cc: recipients_of(&v["ccRecipients"]),
        html: html_with_inline(ctx, id, &v).await,
        attachments: attachments_of(&v),
    })
}

pub async fn set_read(ctx: &Ctx<'_>, id: &str, read: bool) -> Result<(), String> {
    http::json(req(ctx, Method::PATCH, &format!("/me/messages/{}", enc(id))).json(&json!({ "isRead": read }))).await?;
    Ok(())
}

pub async fn set_flag(ctx: &Ctx<'_>, id: &str, flagged: bool) -> Result<(), String> {
    let status = if flagged { "flagged" } else { "notFlagged" };
    http::json(req(ctx, Method::PATCH, &format!("/me/messages/{}", enc(id))).json(&json!({ "flag": { "flagStatus": status } }))).await?;
    Ok(())
}

/// `target`: archive | trash | junk | inbox | <folder id>
pub async fn move_to(ctx: &Ctx<'_>, id: &str, target: &str) -> Result<(), String> {
    let dest = match target {
        "archive" => "archive",
        "trash" => "deleteditems",
        "junk" => "junkemail",
        "inbox" => "inbox",
        other => other,
    };
    http::json(req(ctx, Method::POST, &format!("/me/messages/{}/move", enc(id))).json(&json!({ "destinationId": dest }))).await?;
    Ok(())
}

pub async fn delete_forever(ctx: &Ctx<'_>, id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("/me/messages/{}", enc(id)))).await?;
    Ok(())
}

pub async fn attachment_bytes(ctx: &Ctx<'_>, msg: &str, att: &str) -> Result<Vec<u8>, String> {
    http::bytes(req(ctx, Method::GET, &format!("/me/messages/{}/attachments/{}/$value", enc(msg), enc(att)))).await
}

fn recips(list: &[String]) -> Vec<Value> {
    list.iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(|a| json!({ "emailAddress": { "address": a } }))
        .collect()
}

fn draft_json(out: &Outgoing) -> Value {
    json!({
        "subject": out.subject,
        "body": { "contentType": "HTML", "content": out.html },
        "toRecipients": recips(&out.to),
        "ccRecipients": recips(&out.cc),
        "bccRecipients": recips(&out.bcc),
    })
}

async fn upload_attachment(ctx: &Ctx<'_>, draft: &str, a: &OutAttachment) -> Result<(), String> {
    let data = tokio::fs::read(&a.path).await.map_err(|e| format!("can't read {}: {e}", a.name))?;
    let ctype = mime_for(&a.name);
    if data.len() <= SIMPLE_ATTACH_LIMIT {
        http::json(req(ctx, Method::POST, &format!("/me/messages/{}/attachments", enc(draft))).json(&json!({
            "@odata.type": "#microsoft.graph.fileAttachment",
            "name": a.name,
            "contentType": ctype,
            "contentBytes": STANDARD.encode(&data),
        })))
        .await?;
        return Ok(());
    }
    let session = http::json(
        req(ctx, Method::POST, &format!("/me/messages/{}/attachments/createUploadSession", enc(draft)))
            .json(&json!({ "AttachmentItem": { "attachmentType": "file", "name": a.name, "size": data.len(), "contentType": ctype } })),
    )
    .await?;
    let url = s(&session["uploadUrl"]);
    let total = data.len();
    let mut start = 0;
    while start < total {
        let end = (start + CHUNK).min(total);
        // the upload URL is pre-authorized; sending the bearer token there is rejected
        let r = ctx
            .http
            .put(&url)
            .header("Content-Range", format!("bytes {start}-{}/{total}", end - 1))
            .header("Content-Length", end - start)
            .body(data[start..end].to_vec());
        let (status, body) = http::exec(r).await?;
        if !status.is_success() {
            return Err(format!("attachment upload failed ({status}): {}", String::from_utf8_lossy(&body)));
        }
        start = end;
    }
    Ok(())
}

/// Create or update the server-side draft and sync its attachments. Returns the draft's message id.
pub async fn save_draft(ctx: &Ctx<'_>, out: &Outgoing) -> Result<String, String> {
    let id = if let Some(d) = &out.draft_id {
        http::json(req(ctx, Method::PATCH, &format!("/me/messages/{}", enc(d))).json(&draft_json(out))).await?;
        d.clone()
    } else if let Some(r) = &out.reply_to_id {
        let action = if out.reply_all { "createReplyAll" } else { "createReply" };
        let v = http::json(req(ctx, Method::POST, &format!("/me/messages/{}/{action}", enc(r))).json(&json!({}))).await?;
        let id = s(&v["id"]);
        http::json(req(ctx, Method::PATCH, &format!("/me/messages/{}", enc(&id))).json(&draft_json(out))).await?;
        id
    } else if let Some(f) = &out.forward_of_id {
        // createForward carries the original attachments over
        let v = http::json(req(ctx, Method::POST, &format!("/me/messages/{}/createForward", enc(f))).json(&json!({}))).await?;
        let id = s(&v["id"]);
        http::json(req(ctx, Method::PATCH, &format!("/me/messages/{}", enc(&id))).json(&draft_json(out))).await?;
        id
    } else {
        let v = http::json(req(ctx, Method::POST, "/me/messages").json(&draft_json(out))).await?;
        s(&v["id"])
    };

    if out.draft_id.is_some() {
        let v = get(ctx, &format!("/me/messages/{}/attachments?$select=id,isInline", enc(&id))).await?;
        for a in v["value"].as_array().cloned().unwrap_or_default() {
            let aid = s(&a["id"]);
            if a["isInline"].as_bool() != Some(true) && !out.keep_attachments.contains(&aid) {
                http::json(req(ctx, Method::DELETE, &format!("/me/messages/{}/attachments/{}", enc(&id), enc(&aid)))).await?;
            }
        }
    }
    for a in &out.attachments {
        upload_attachment(ctx, &id, a).await?;
    }
    Ok(id)
}

pub async fn send(ctx: &Ctx<'_>, out: &Outgoing) -> Result<(), String> {
    let id = save_draft(ctx, out).await?;
    http::json(req(ctx, Method::POST, &format!("/me/messages/{}/send", enc(&id)))).await?;
    Ok(())
}

pub async fn open_draft(ctx: &Ctx<'_>, id: &str) -> Result<DraftData, String> {
    let v = message_value(ctx, id).await?;
    let list = |k: &str| recipients_of(&v[k]).into_iter().map(|a| a.address).collect::<Vec<_>>();
    Ok(DraftData {
        draft_id: id.to_string(),
        to: list("toRecipients"),
        cc: list("ccRecipients"),
        bcc: list("bccRecipients"),
        subject: s(&v["subject"]),
        html: html_with_inline(ctx, id, &v).await,
        attachments: attachments_of(&v).into_iter().filter(|a| !a.inline).collect(),
        reply_to_id: None,
    })
}

// ──────────────────────────── calendar ───────────────────────────

fn color_for(v: &Value) -> String {
    let hex = s(&v["hexColor"]);
    if hex.starts_with('#') && hex.len() == 7 {
        return hex;
    }
    match v["color"].as_str().unwrap_or("") {
        "lightBlue" => "#4aa3df",
        "lightGreen" => "#6cc070",
        "lightOrange" => "#f2a65a",
        "lightGray" => "#9aa0a6",
        "lightYellow" => "#e8c547",
        "lightTeal" => "#4fc3b0",
        "lightPink" => "#e885b4",
        "lightBrown" => "#a98467",
        "lightRed" => "#e06c75",
        _ => "#7aa2f7",
    }
    .to_string()
}

pub async fn calendars(ctx: &Ctx<'_>) -> Result<Vec<CalendarInfo>, String> {
    let v = get(ctx, "/me/calendars?$top=50&$select=id,name,hexColor,color").await?;
    Ok(v["value"]
        .as_array()
        .map(|a| a.iter().map(|c| CalendarInfo { id: s(&c["id"]), name: s(&c["name"]), color: color_for(c) }).collect())
        .unwrap_or_default())
}

fn utc_iso(dt: &str) -> String {
    format!("{}Z", &dt[..dt.len().min(19)])
}

fn event_from(v: &Value, cal: &CalendarInfo) -> Event {
    let all_day = v["isAllDay"].as_bool().unwrap_or(false);
    let (start, end) = if all_day {
        (s(&v["start"]["dateTime"])[..10].to_string(), s(&v["end"]["dateTime"])[..10].to_string())
    } else {
        (utc_iso(&s(&v["start"]["dateTime"])), utc_iso(&s(&v["end"]["dateTime"])))
    };
    let body = v["body"]["content"].as_str().unwrap_or("");
    let description = if v["body"]["contentType"].as_str() == Some("html") { strip_tags(body) } else { body.to_string() };
    Event {
        id: s(&v["id"]),
        calendar_id: cal.id.clone(),
        calendar_name: cal.name.clone(),
        color: cal.color.clone(),
        title: s(&v["subject"]),
        start,
        end,
        all_day,
        location: s(&v["location"]["displayName"]),
        description: description.trim().to_string(),
        attendees: v["attendees"].as_array().map(|a| a.iter().map(|x| s(&x["emailAddress"]["address"])).collect()).unwrap_or_default(),
        organizer: { let n = s(&v["organizer"]["emailAddress"]["name"]); if n.is_empty() { s(&v["organizer"]["emailAddress"]["address"]) } else { n } },
        web_link: v["webLink"].as_str().map(String::from),
        join_url: v["onlineMeeting"]["joinUrl"].as_str().map(String::from),
    }
}

pub async fn events(ctx: &Ctx<'_>, from: &str, to: &str) -> Result<Vec<Event>, String> {
    let cals = calendars(ctx).await?;
    let results = join_all(cals.iter().take(12).map(|c| async move {
        let url = format!(
            "/me/calendars/{}/calendarView?startDateTime={}&endDateTime={}&$top=500&$select=id,subject,start,end,isAllDay,location,body,organizer,attendees,webLink,onlineMeeting,isCancelled",
            enc(&c.id), enc(from), enc(to)
        );
        http::json(req(ctx, Method::GET, &url).header("Prefer", "outlook.timezone=\"UTC\"")).await
    }))
    .await;
    let mut out = vec![];
    for (c, r) in cals.iter().zip(results) {
        let v = r?;
        for e in v["value"].as_array().cloned().unwrap_or_default() {
            if e["isCancelled"].as_bool() != Some(true) {
                out.push(event_from(&e, c));
            }
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(out)
}

fn graph_time(iso: &str, all_day: bool) -> Value {
    let dt = if all_day { format!("{}T00:00:00", &iso[..10]) } else { iso.trim_end_matches('Z').split('.').next().unwrap_or(iso).to_string() };
    json!({ "dateTime": dt, "timeZone": "UTC" })
}

pub async fn save_event(ctx: &Ctx<'_>, e: &EventInput) -> Result<(), String> {
    let mut body = json!({
        "subject": e.title,
        "isAllDay": e.all_day,
        "start": graph_time(&e.start, e.all_day),
        "end": graph_time(&e.end, e.all_day),
        "location": { "displayName": e.location },
        "attendees": e.attendees.iter().filter(|a| !a.trim().is_empty()).map(|a| json!({ "emailAddress": { "address": a.trim() }, "type": "required" })).collect::<Vec<_>>(),
    });
    match &e.id {
        Some(id) => {
            // only touch the body when it was edited, so meeting links in the original survive
            if let Some(d) = &e.description {
                body["body"] = json!({ "contentType": "text", "content": d });
            }
            http::json(req(ctx, Method::PATCH, &format!("/me/events/{}", enc(id))).json(&body)).await?;
        }
        None => {
            body["body"] = json!({ "contentType": "text", "content": e.description.clone().unwrap_or_default() });
            let path = match &e.calendar_id {
                Some(c) => format!("/me/calendars/{}/events", enc(c)),
                None => "/me/events".to_string(),
            };
            http::json(req(ctx, Method::POST, &path).json(&body)).await?;
        }
    }
    Ok(())
}

pub async fn delete_event(ctx: &Ctx<'_>, id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("/me/events/{}", enc(id)))).await?;
    Ok(())
}

// ──────────────────────────── contacts ───────────────────────────

fn contact_from(v: &Value) -> Contact {
    let mut phones: Vec<String> = vec![];
    if let Some(m) = v["mobilePhone"].as_str().filter(|p| !p.is_empty()) {
        phones.push(m.to_string());
    }
    for k in ["businessPhones", "homePhones"] {
        phones.extend(v[k].as_array().map(|a| a.iter().map(s).filter(|p| !p.is_empty()).collect::<Vec<_>>()).unwrap_or_default());
    }
    Contact {
        id: s(&v["id"]),
        etag: None,
        name: s(&v["displayName"]),
        emails: v["emailAddresses"].as_array().map(|a| a.iter().map(|e| s(&e["address"])).filter(|e| !e.is_empty()).collect()).unwrap_or_default(),
        phones,
        company: s(&v["companyName"]),
        title: s(&v["jobTitle"]),
    }
}

pub async fn contacts(ctx: &Ctx<'_>) -> Result<Vec<Contact>, String> {
    let mut url = "/me/contacts?$top=200&$orderby=displayName&$select=id,displayName,emailAddresses,mobilePhone,businessPhones,homePhones,companyName,jobTitle".to_string();
    let mut out = vec![];
    for _ in 0..15 {
        let v = get(ctx, &url).await?;
        out.extend(v["value"].as_array().map(|a| a.iter().map(contact_from).collect::<Vec<_>>()).unwrap_or_default());
        match v["@odata.nextLink"].as_str() {
            Some(n) => url = n.to_string(),
            None => break,
        }
    }
    Ok(out)
}

pub async fn save_contact(ctx: &Ctx<'_>, c: &Contact) -> Result<(), String> {
    let (given, family) = split_name(&c.name);
    let phones: Vec<&String> = c.phones.iter().filter(|p| !p.trim().is_empty()).collect();
    let body = json!({
        "displayName": c.name,
        "givenName": given,
        "surname": family,
        "emailAddresses": c.emails.iter().filter(|e| !e.trim().is_empty()).map(|e| json!({ "address": e.trim(), "name": c.name })).collect::<Vec<_>>(),
        "mobilePhone": phones.first().map(|p| p.as_str()),
        "businessPhones": phones.iter().skip(1).collect::<Vec<_>>(),
        "homePhones": Vec::<String>::new(),
        "companyName": c.company,
        "jobTitle": c.title,
    });
    if c.id.is_empty() {
        http::json(req(ctx, Method::POST, "/me/contacts").json(&body)).await?;
    } else {
        http::json(req(ctx, Method::PATCH, &format!("/me/contacts/{}", enc(&c.id))).json(&body)).await?;
    }
    Ok(())
}

pub async fn delete_contact(ctx: &Ctx<'_>, id: &str) -> Result<(), String> {
    http::json(req(ctx, Method::DELETE, &format!("/me/contacts/{}", enc(id)))).await?;
    Ok(())
}
