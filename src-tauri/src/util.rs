use crate::model::Addr;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;
use std::collections::HashMap;

pub fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

pub fn u(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}

pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // drop <style>/<script> contents entirely
            for tag in ["style", "script"] {
                if lower[i..].starts_with(&format!("<{tag}")) {
                    if let Some(end) = lower[i..].find(&format!("</{tag}>")) {
                        i += end + tag.len() + 3;
                        break;
                    }
                }
            }
            let end = html[i..].find('>').map(|e| i + e + 1).unwrap_or(bytes.len());
            let tag = lower[i..end].trim_start_matches('<');
            if tag.starts_with("br") || tag.starts_with("/p") || tag.starts_with("/div") || tag.starts_with("/li") || tag.starts_with("/tr") || tag.starts_with("/h") {
                out.push('\n');
            }
            i = end;
        } else {
            let ch = html[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Swap `cid:` references for data URIs so inline images render.
pub fn replace_cids(html: &str, images: &HashMap<String, (String, Vec<u8>)>) -> String {
    if images.is_empty() || !html.contains("cid:") {
        return html.to_string();
    }
    let mut out = html.to_string();
    for (cid, (mime, data)) in images {
        let uri = format!("data:{mime};base64,{}", STANDARD.encode(data));
        out = out.replace(&format!("cid:{cid}"), &uri);
    }
    out
}

pub fn addr(name: &str, address: &str) -> Addr {
    Addr { name: name.to_string(), address: address.to_string() }
}

/// Parse raw header values ("Name <a@b>, c@d") using the MIME parser so encoded words are decoded.
pub fn parse_addr_list(raw: &str) -> Vec<Addr> {
    if raw.trim().is_empty() {
        return vec![];
    }
    let msg = format!("To: {raw}\r\n\r\n");
    let parsed = mail_parser::MessageParser::default().parse(msg.as_bytes());
    parsed
        .and_then(|m| m.to().map(|a| a.iter().map(|x| Addr { name: x.name().unwrap_or_default().to_string(), address: x.address().unwrap_or_default().to_string() }).collect()))
        .unwrap_or_default()
}

pub fn decode_header(raw: &str) -> String {
    let msg = format!("Subject: {raw}\r\n\r\n");
    mail_parser::MessageParser::default()
        .parse(msg.as_bytes())
        .and_then(|m| m.subject().map(String::from))
        .unwrap_or_else(|| raw.to_string())
}

pub fn split_name(full: &str) -> (String, String) {
    let t: Vec<&str> = full.split_whitespace().collect();
    match t.len() {
        0 => (String::new(), String::new()),
        1 => (t[0].to_string(), String::new()),
        _ => (t[..t.len() - 1].join(" "), t[t.len() - 1].to_string()),
    }
}

pub fn escape_html(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn mime_for(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html() {
        assert_eq!(strip_tags("<style>x{}</style><p>Hi &amp; bye</p><br>there").trim(), "Hi & bye\n\nthere");
    }

    #[test]
    fn parses_addresses() {
        let a = parse_addr_list("\"Doe, Jane\" <jane@x.com>, bob@y.org");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].name, "Doe, Jane");
        assert_eq!(a[1].address, "bob@y.org");
    }

    #[test]
    fn replaces_cid() {
        let mut m = HashMap::new();
        m.insert("a@b".to_string(), ("image/png".to_string(), vec![1, 2, 3]));
        assert!(replace_cids("<img src=\"cid:a@b\">", &m).contains("data:image/png;base64,AQID"));
    }

    #[test]
    fn splits_names() {
        assert_eq!(split_name("Mary Ann Smith"), ("Mary Ann".into(), "Smith".into()));
    }
}
