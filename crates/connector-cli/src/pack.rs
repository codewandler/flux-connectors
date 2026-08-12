//! Compiling the canonical documents into the catalog pack (C-537).
//!
//! The pack is the *distributed* form of the catalogue: one file holding every provider's
//! canonical document (C-536), fronted by an offset index so a reader serves any provider or
//! operation without parsing a byte of JSON at query time. `crates/catalog-reader` embeds it and
//! is the format's consuming side; this module is the only writer. The format decision — a plain
//! UTF-8 container over the committed document bytes, uncompressed, indexed, digest-carrying — is
//! recorded with its measurements and its rejected alternative in
//! `docs/designs/catalog-artifact.md` §2.
//!
//! # The format, in one place
//!
//! ```text
//! flux-connectors-catalog-pack 1                    ← magic + container format version
//! digest sha256 <64 lowercase hex>                  ← over every byte after this line
//! schema <n>                                        ← the documents' schema_version
//! providers <n>
//! operations <m>
//! p <id> <start> <len>                              ← one per provider, ordered by id
//! o <id> <provider> <service> <start> <len>         ← one per operation, ordered by id
//! payload <len>
//! <the canonical documents, concatenated in provider-id order>
//! ```
//!
//! Offsets are byte offsets into the payload, in decimal. A provider row's span is exactly its
//! committed `catalog/<id>.catalog.json` bytes; an operation row's span slices that operation's
//! own JSON object *out of its owning document* — the record is a substring, never a
//! re-serialization that could disagree with the reviewed artifact.
//!
//! # Why the writer re-verifies its own spans
//!
//! The operation spans come from [`operation_spans`], a structural scan of the document text
//! (JSON has no byte-offset API in `serde_json`). A scanner bug here would ship a pack whose
//! spans slice plausible-looking garbage, so [`compile`] holds each span to the standard the
//! repository holds every artifact to: the slice must parse as JSON and be **value-equal** to the
//! element `serde_json` finds at the same position. A mismatch is a refusal naming the operation,
//! not a warning — refuse ambiguous output rather than emit it.

use anyhow::{bail, Context, Result};
use connector_spec::sha256_hex;

/// The pack's magic word — the first token of the first line.
pub const MAGIC: &str = "flux-connectors-catalog-pack";

/// The container format version this writer emits. A reader refuses a version above what it
/// knows, by name, so bumping this is a coordinated change with `crates/catalog-reader`.
pub const FORMAT_VERSION: u32 = 1;

/// Compile the canonical documents into the pack's exact bytes.
///
/// `documents` is `(provider id, document text)` — the same planned contents a build writes to
/// `catalog/<id>.catalog.json`, so the pack is a fixed point together with the documents rather
/// than a reading of whatever is on disk. Order does not matter; the pack sorts by provider id.
///
/// # Errors
///
/// A duplicate provider or operation id, a document whose `schema_version` disagrees with the
/// others, an operation without an `id` or `service`, or a span that fails the value-equality
/// check — each is a loud refusal naming the offender.
pub fn compile(documents: &[(&str, &str)]) -> Result<String> {
    let mut sorted: Vec<(&str, &str)> = documents.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for window in sorted.windows(2) {
        if window[0].0 == window[1].0 {
            bail!("provider `{}` has two canonical documents", window[0].0);
        }
    }

    let mut schema_version: Option<u64> = None;
    let mut provider_rows: Vec<String> = Vec::new();
    let mut operation_rows: Vec<(String, String)> = Vec::new(); // (operation id, rendered row)
    let mut payload = String::new();

    for (provider, document) in &sorted {
        let context = || format!("provider `{provider}`'s canonical document");

        let value: serde_json::Value = serde_json::from_str(document).with_context(context)?;
        let declared = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .with_context(|| format!("{} declares no schema_version", context()))?;
        match schema_version {
            None => schema_version = Some(declared),
            Some(previous) if previous != declared => bail!(
                "provider `{provider}`'s document declares schema_version {declared}, but the \
                 documents before it declare {previous}; one pack cannot carry two schemas"
            ),
            Some(_) => {}
        }

        let base = payload.len();
        provider_rows.push(format!("p {provider} {base} {}", document.len()));

        let operations = value
            .get("operations")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("{} has no operations array", context()))?;
        let spans = operation_spans(document).with_context(context)?;
        if spans.len() != operations.len() {
            bail!(
                "{}: the span scan found {} operations where the document carries {}",
                context(),
                spans.len(),
                operations.len()
            );
        }
        for ((start, end), operation) in spans.iter().zip(operations) {
            let slice = &document[*start..*end];
            let reparsed: serde_json::Value = serde_json::from_str(slice)
                .with_context(|| format!("{}: an operation span is not JSON", context()))?;
            if &reparsed != operation {
                bail!(
                    "{}: an operation span does not slice the record `serde_json` reads at the \
                     same position — the scanner and the parser disagree, so the pack would ship \
                     a wrong record",
                    context()
                );
            }
            let id = operation
                .get("id")
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("{}: an operation has no id", context()))?;
            let service = operation
                .get("service")
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("{}: operation `{id}` names no service", context()))?;
            operation_rows.push((
                id.to_owned(),
                format!(
                    "o {id} {provider} {service} {} {}",
                    base + start,
                    end - start
                ),
            ));
        }

        payload.push_str(document);
    }

    let schema_version =
        schema_version.context("cannot compile a pack from zero canonical documents")?;
    operation_rows.sort();
    for window in operation_rows.windows(2) {
        if window[0].0 == window[1].0 {
            bail!(
                "operation id `{}` appears in more than one document; the pack's index keys on it",
                window[0].0
            );
        }
    }

    // Everything after the digest line, assembled first so the digest can state it.
    let mut body = String::with_capacity(payload.len() + 64 * operation_rows.len());
    body.push_str(&format!("schema {schema_version}\n"));
    body.push_str(&format!("providers {}\n", provider_rows.len()));
    body.push_str(&format!("operations {}\n", operation_rows.len()));
    for row in &provider_rows {
        body.push_str(row);
        body.push('\n');
    }
    for (_, row) in &operation_rows {
        body.push_str(row);
        body.push('\n');
    }
    body.push_str(&format!("payload {}\n", payload.len()));
    body.push_str(&payload);

    Ok(format!(
        "{MAGIC} {FORMAT_VERSION}\ndigest sha256 {}\n{body}",
        sha256_hex(body.as_bytes())
    ))
}

/// The byte span of every element of the document's top-level `operations` array, in order.
///
/// A single structural pass: string-aware (escapes included), depth-tracked, and interested in
/// exactly one place — the array whose *key at the root level* is `operations`. Everything else,
/// including an `"operations"` string anywhere deeper, is opaque payload. Returns `start..end`
/// spans where `end` is one past the element's closing brace.
fn operation_spans(text: &str) -> Result<Vec<(usize, usize)>> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_start = 0usize;
    // The contents span of the most recently *completed* string literal — at root level, the key
    // a following `:` belongs to.
    let mut last_string: Option<(usize, usize)> = None;
    // Set when the root-level key `operations` was seen and its `:` consumed; the next `[` opens
    // the array this function exists to walk.
    let mut expecting_array = false;
    // The depth at which the operations array's *elements* sit, while inside it.
    let mut element_depth: Option<usize> = None;
    let mut element_start: Option<usize> = None;

    for (i, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                last_string = Some((string_start, i));
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                string_start = i + 1;
            }
            b':' => {
                if depth == 1 {
                    expecting_array = last_string
                        .map(|(start, end)| &text[start..end] == "operations")
                        .unwrap_or(false);
                }
            }
            b'[' => {
                depth += 1;
                if depth == 2 && expecting_array && element_depth.is_none() {
                    element_depth = Some(depth);
                    expecting_array = false;
                }
            }
            b'{' => {
                if element_depth == Some(depth) && element_start.is_none() {
                    element_start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth = depth
                    .checked_sub(1)
                    .context("unbalanced `}` in a canonical document")?;
                if element_depth == Some(depth) {
                    if let Some(start) = element_start.take() {
                        spans.push((start, i + 1));
                    }
                }
            }
            b']' => {
                if element_depth == Some(depth) {
                    element_depth = None;
                }
                depth = depth
                    .checked_sub(1)
                    .context("unbalanced `]` in a canonical document")?;
            }
            _ => {}
        }
    }

    if in_string || depth != 0 {
        bail!("the canonical document ends inside a string or an open bracket");
    }
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal but structurally honest document: nested braces, brackets and quoted braces all
    /// inside operation records, exactly where a naive scan goes wrong.
    const DOCUMENT: &str = r#"{
  "connector": "acme",
  "schema_version": 1,
  "operations": [
    {
      "id": "acme-thing-get",
      "service": "default",
      "request": {
        "method": "GET",
        "url": "{base}/v1/things/{thing_id}"
      },
      "params": [
        {
          "name": "thing_id",
          "schema": { "type": "integer" }
        }
      ],
      "description": "Braces in strings: } ] \" \\ {"
    },
    {
      "id": "acme-thing-list",
      "service": "default",
      "request": { "method": "GET", "url": "{base}/v1/things" }
    }
  ]
}
"#;

    #[test]
    fn spans_slice_each_operation_record_exactly() {
        let spans = operation_spans(DOCUMENT).expect("the document scans");
        assert_eq!(spans.len(), 2);
        for (span, id) in spans.iter().zip(["acme-thing-get", "acme-thing-list"]) {
            let slice = &DOCUMENT[span.0..span.1];
            assert!(slice.starts_with('{') && slice.ends_with('}'));
            let value: serde_json::Value = serde_json::from_str(slice).expect("a span is JSON");
            assert_eq!(value["id"], id);
        }
    }

    /// An `"operations"` *string* deeper in the document is payload, not the index's array — the
    /// scanner keys on the root-level key alone.
    #[test]
    fn a_nested_operations_key_is_not_the_array() {
        let document = r#"{
  "schema_version": 1,
  "config": [ { "help": "operations", "operations": [ { "id": "decoy" } ] } ],
  "operations": [ { "id": "real-op", "service": "default" } ]
}
"#;
        let spans = operation_spans(document).expect("the document scans");
        assert_eq!(spans.len(), 1);
        let value: serde_json::Value =
            serde_json::from_str(&document[spans[0].0..spans[0].1]).expect("JSON");
        assert_eq!(value["id"], "real-op");
    }

    #[test]
    fn the_compiled_pack_round_trips_its_own_spans() {
        let pack = compile(&[("acme", DOCUMENT)]).expect("the pack compiles");
        let mut lines = pack.lines();
        assert_eq!(lines.next(), Some("flux-connectors-catalog-pack 1"));
        let digest = lines.next().expect("a digest line");
        let stated = digest
            .strip_prefix("digest sha256 ")
            .expect("the digest spelling");
        let body_start = pack.find('\n').expect("a newline") + 1 + digest.len() + 1;
        assert_eq!(stated, sha256_hex(&pack.as_bytes()[body_start..]));
        assert_eq!(lines.next(), Some("schema 1"));
        assert_eq!(lines.next(), Some("providers 1"));
        assert_eq!(lines.next(), Some("operations 2"));
    }

    #[test]
    fn a_duplicate_operation_id_is_refused_by_name() {
        let twin = DOCUMENT.replace("\"connector\": \"acme\"", "\"connector\": \"other\"");
        let error = compile(&[("acme", DOCUMENT), ("other", &twin)])
            .expect_err("two documents declaring one operation id must refuse");
        assert!(
            format!("{error:#}").contains("acme-thing-get"),
            "the refusal names the colliding id: {error:#}"
        );
    }

    #[test]
    fn disagreeing_schema_versions_are_refused() {
        let newer = DOCUMENT.replace("\"schema_version\": 1", "\"schema_version\": 2");
        let error = compile(&[("acme", DOCUMENT), ("beacon", &newer)])
            .expect_err("one pack cannot carry two schema versions");
        assert!(
            format!("{error:#}").contains("schema_version 2"),
            "the refusal names the disagreement: {error:#}"
        );
    }
}
