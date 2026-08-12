//! **The request**: `{ method, url, headers, body }`, exactly `http.request`'s own input.
//!
//! Moved from `connector-pack` unchanged (C-538), including the hand-written redacting `Debug`. The
//! type is the unit the differential gate compares, so it has to be the same type on both sides of
//! the comparison rather than two structs that happen to have the same fields.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::auth::query_encode;

/// The header this software identifies itself in.
const USER_AGENT: &str = "User-Agent";

/// **What this software calls itself on the wire** (C-223).
///
/// A product token and its version, per RFC 9110 §10.1.5, with the repository as the comment a
/// vendor can act on. Both halves are read from the manifest rather than typed, so neither can go
/// stale at a release — and every crate in this workspace inherits the same workspace `version` and
/// `repository`, so moving this here did not move the value.
pub const DEFAULT_USER_AGENT: &str = concat!(
    "flux-connectors/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// **The request**: `{ method, url, headers, body }`, exactly `http.request`'s own input.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// The HTTP method, as `http.request` spells it (`GET`, `PUT`, …).
    pub method: String,
    /// The absolute request URL, query string included.
    pub url: String,
    /// The request headers. `BTreeMap` because the emitted record is one, so the order is the
    /// document's order rather than a hash seed's.
    pub headers: BTreeMap<String, String>,
    /// The request body as the text `http.request` sends, or `None` for a request that has none.
    pub body: Option<String>,
}

/// **Hand-written, and no value prints** (C-159, finding 1).
///
/// A `Request` carries the assembled credential *after* placement — in a header value, and for a
/// query placement in the URL itself — and this type is `pub`, so a host can hold one and format it.
///
/// The rule is **shape without values**: the method, the host, the path, the header *names* and the
/// query-parameter *names* stay, and every value is `<redacted>`. A body prints as present or
/// absent and never as content, and never as a length: a length is a fingerprint.
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &redacted_url(&self.url))
            .field("headers", &HeaderNames(&self.headers))
            .field("body", &self.body.as_ref().map(|_| Redacted))
            .finish()
    }
}

/// A value that does not print.
struct Redacted;

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The headers as their names, every value [`Redacted`].
struct HeaderNames<'a>(&'a BTreeMap<String, String>);

impl std::fmt::Debug for HeaderNames<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.keys().map(|name| (name, Redacted)))
            .finish()
    }
}

/// A URL with every query-parameter *value* redacted and every parameter *name* kept.
fn redacted_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let mut out = String::with_capacity(url.len());
    out.push_str(base);
    out.push('?');
    for (index, pair) in query.split('&').enumerate() {
        if index > 0 {
            out.push('&');
        }
        match pair.split_once('=') {
            Some((name, _)) => {
                out.push_str(name);
                out.push_str("=<redacted>");
            }
            None => out.push_str("<redacted>"),
        }
    }
    out
}

impl Request {
    /// **Give this request this software's identity, unless the connector already stated one**
    /// (C-223).
    ///
    /// Applied at the one point every derivation shares, so a rehearsal and the wire cannot
    /// disagree about what a vendor's rate limit, allow-list and support desk are shown.
    ///
    /// The check is **case-insensitive**, which is not fastidiousness: [`Request::headers`] is a
    /// `BTreeMap`, so a document setting `user-agent` and a default inserting `User-Agent` would be
    /// two entries, two JSON keys, and — depending on how the transport folds them — two headers on
    /// the wire or a silent overwrite. The connector's own value wins.
    pub fn identify(&mut self) {
        if self
            .headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case(USER_AGENT))
        {
            return;
        }
        self.headers
            .insert(USER_AGENT.to_owned(), DEFAULT_USER_AGENT.to_owned());
    }

    /// The params `http.request` is called with.
    ///
    /// `headers` and `body` are omitted when empty rather than sent as `{}`/`""`, so a request this
    /// crate derives is the same JSON a hand-written `http.request` call would carry.
    pub fn to_params(&self) -> Value {
        let mut params = serde_json::Map::new();
        params.insert("url".to_string(), Value::String(self.url.clone()));
        params.insert("method".to_string(), Value::String(self.method.clone()));
        if !self.headers.is_empty() {
            params.insert(
                "headers".to_string(),
                Value::Object(
                    self.headers
                        .iter()
                        .map(|(name, value)| (name.clone(), Value::String(value.clone())))
                        .collect(),
                ),
            );
        }
        if let Some(body) = &self.body {
            params.insert("body".to_string(), Value::String(body.clone()));
        }
        Value::Object(params)
    }
}

/// Apply Flux 0.54's structured-query wire contract to the pairs a request template declares — C-30.
///
/// The pairs arrive already rendered and already ordered; what this owns is the wire contract:
/// every key and value encoded exactly once, a key that duplicates one already embedded in the URL
/// refused rather than sent twice, and a fragment preserved on the end.
pub(crate) fn append_query(url: &mut String, pairs: &[(String, String)]) -> Result<(), String> {
    if pairs.is_empty() {
        return Ok(());
    }
    let (without_fragment, fragment) = match url.split_once('#') {
        Some((head, tail)) => (head.to_owned(), Some(tail.to_owned())),
        None => (url.clone(), None),
    };
    let existing_names: BTreeSet<String> = without_fragment
        .split_once('?')
        .map(|(_, query)| {
            query
                .split('&')
                .filter(|pair| !pair.is_empty())
                .map(|pair| pair.split_once('=').map_or(pair, |(name, _)| name))
                .map(query_decode)
                .collect()
        })
        .unwrap_or_default();

    let mut appended = Vec::with_capacity(pairs.len());
    for (name, value) in pairs {
        if existing_names.contains(name) {
            return Err(format!(
                "field {name:?} duplicates a key already embedded in its URL"
            ));
        }
        appended.push(format!("{}={}", query_encode(name), query_encode(value)));
    }

    *url = without_fragment;
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str(&appended.join("&"));
    if let Some(fragment) = fragment {
        url.push('#');
        url.push_str(&fragment);
    }
    Ok(())
}

/// Decode just enough of an existing URL key to enforce Flux's duplicate-key refusal.
fn query_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let pair = &value[index + 1..index + 3];
                match u8::from_str_radix(pair, 16) {
                    Ok(byte) => {
                        decoded.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        decoded.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Request {
        Request {
            method: "GET".to_string(),
            url: "https://vendor.example/things?api_key=SENTINEL".to_string(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer SENTINEL".to_string())]),
            body: Some("{\"a\":\"SENTINEL\"}".to_string()),
        }
    }

    #[test]
    fn debug_prints_shape_and_never_a_value() {
        let printed = format!("{:?}", request());
        assert!(!printed.contains("SENTINEL"), "{printed}");
        assert!(printed.contains("api_key=<redacted>"), "{printed}");
        assert!(printed.contains("Authorization"), "{printed}");
    }

    #[test]
    fn the_params_omit_what_is_absent() {
        let bare = Request {
            method: "GET".to_string(),
            url: "https://vendor.example/things".to_string(),
            headers: BTreeMap::new(),
            body: None,
        };
        assert_eq!(
            bare.to_params(),
            serde_json::json!({"url": "https://vendor.example/things", "method": "GET"})
        );
    }

    #[test]
    fn a_duplicate_query_key_is_refused_rather_than_sent_twice() {
        let mut url = "https://vendor.example/things?page=1".to_string();
        assert!(append_query(&mut url, &[("page".to_string(), "2".to_string())]).is_err());
    }

    #[test]
    fn a_fragment_survives_the_appended_query() {
        let mut url = "https://vendor.example/things#top".to_string();
        append_query(&mut url, &[("page".to_string(), "2".to_string())]).expect("it appends");
        assert_eq!(url, "https://vendor.example/things?page=2#top");
    }

    #[test]
    fn the_default_identity_names_this_software_and_this_repository() {
        assert!(DEFAULT_USER_AGENT.starts_with("flux-connectors/"));
        assert!(DEFAULT_USER_AGENT.contains("github.com/codewandler/flux-connectors"));
    }
}
