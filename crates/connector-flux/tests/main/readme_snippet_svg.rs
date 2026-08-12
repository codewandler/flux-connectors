//! **The committed README image is coloured by flux's own classification** (C-45).
//!
//! `assets/readme-snippet-{light,dark}.svg` are generated artifacts a human never edits, and the
//! only thing that makes them trustworthy is that their colours come from
//! [`flux_lang::highlight::highlight`] — the walk over the lossless CST that classifies a token by
//! its kind *and its parent node's kind*. A keyword-list highlighter cannot reproduce that: it has
//! no way to know that `ticket_id` in `op f(ticket_id: Number)` is a symbol binder while `body` in
//! `{ body: $body }` is an object key.
//!
//! So this file asserts the property directly on the **committed bytes**, without naming any of this
//! repo's rendering API:
//!
//! 1. [`the_readme_image_shows_the_snippet_verbatim`] — the image's visible text *is* the snippet,
//!    so nothing can be highlighted that the reader is not also shown.
//! 2. [`every_highlight_class_in_the_snippet_has_its_own_colour`] — every span flux classifies is
//!    painted one colour, one colour per class, and no two classes share one. A class that upstream
//!    adds and this repo does not colour separately therefore collides with a neighbour and fails
//!    here, rather than shipping as uncoloured text nobody notices.
//!
//! [`class_name`] is deliberately an exhaustive `match`: a new [`HighlightClass`] variant in a
//! future flux-lang pin stops this file compiling, which is the loudest possible form of "a class
//! was added upstream".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use flux_lang::highlight::{highlight, HighlightClass};

/// The two palettes the README's `<picture>` element selects between.
const THEMES: &[&str] = &["light", "dark"];

/// `<repo>/assets` — derived from the manifest directory so the test is independent of the working
/// directory a runner happens to use.
fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("the assets directory exists")
}

fn read(name: &str) -> String {
    let path = assets_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("cannot read {}: {error}", path.display());
    })
}

/// The snippet the README shows, which is also the source both SVGs render.
fn snippet() -> String {
    read("readme-snippet.flux")
}

/// The stable name of a highlight class, for readable failures.
///
/// Exhaustive on purpose — no `_` arm. When flux-lang gains a class, this stops compiling.
fn class_name(class: HighlightClass) -> &'static str {
    match class {
        HighlightClass::Keyword => "Keyword",
        HighlightClass::Op => "Op",
        HighlightClass::Var => "Var",
        HighlightClass::Annotation => "Annotation",
        HighlightClass::String => "String",
        HighlightClass::Number => "Number",
        HighlightClass::Comment => "Comment",
        HighlightClass::Punct => "Punct",
        HighlightClass::Type => "Type",
        HighlightClass::Error => "Error",
    }
}

/// One run of visible text in a rendered SVG, and the `fill` in scope where it appears.
#[derive(Debug)]
struct Fragment {
    text: String,
    fill: Option<String>,
}

/// The visible text of an SVG, in document order, each run tagged with the colour painting it.
///
/// Only text inside a `<tspan>` counts, and a `<tspan>` carrying a `y=` attribute starts a new
/// visual line — that is how both a line-per-`<tspan>` and a line-per-`<text>` layout reduce to the
/// same sequence, so the assertions below do not depend on which one the renderer chose.
fn fragments(svg: &str) -> Vec<Fragment> {
    let mut out = Vec::new();
    let mut fills: Vec<Option<String>> = Vec::new();
    let mut seen_line = false;
    let mut rest = svg;

    while let Some(open) = rest.find('<') {
        let text = &rest[..open];
        if !text.is_empty() && !fills.is_empty() {
            out.push(Fragment {
                text: unescape(text),
                fill: fills.last().cloned().flatten(),
            });
        }
        let end = rest[open..].find('>').expect("a well-formed tag") + open;
        let tag = &rest[open + 1..end];
        rest = &rest[end + 1..];

        if tag.starts_with("/tspan") {
            fills.pop();
        } else if tag.starts_with("tspan") {
            if tag.contains(" y=") {
                if seen_line {
                    out.push(Fragment {
                        text: "\n".to_string(),
                        fill: None,
                    });
                }
                seen_line = true;
            }
            if !tag.ends_with('/') {
                let inherited = fills.last().cloned().flatten();
                fills.push(attribute(tag, "fill").map(str::to_string).or(inherited));
            }
        }
    }
    out
}

/// The value of `name="…"` in a tag body, if present.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// Reverse the XML escaping a renderer applies to source text.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let Some(semi) = rest[amp..].find(';') else {
            out.push_str(&rest[amp..]);
            return out;
        };
        let entity = &rest[amp + 1..amp + semi];
        out.push(match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" | "#39" | "#x27" => '\'',
            other => panic!("unknown XML entity `&{other};`"),
        });
        rest = &rest[amp + semi + 1..];
    }
    out.push_str(rest);
    out
}

/// **The image shows the source, verbatim.** A rendering that dropped or reordered a token would
/// still look plausible; this is what makes the colour assertions below meaningful, because it pins
/// the byte offsets the two sides are compared on.
#[test]
fn the_readme_image_shows_the_snippet_verbatim() {
    let source = snippet();
    let expected = source.trim_end_matches('\n');

    for theme in THEMES {
        let svg = read(&format!("readme-snippet-{theme}.svg"));
        let visible: String = fragments(&svg).into_iter().map(|f| f.text).collect();
        assert_eq!(
            visible, expected,
            "assets/readme-snippet-{theme}.svg does not show assets/readme-snippet.flux verbatim"
        );
    }
}

/// **Every class flux distinguishes, the image distinguishes** — one colour per class, and no two
/// classes sharing one.
///
/// This is the assertion the regex highlighter this story replaced could not pass. It classified by
/// keyword list, so a parameter binder (`ticket_id`) and an object key (`body`) both fell through to
/// the plain foreground colour while `$base` — the same [`HighlightClass::Var`] as the binder — was
/// coloured. One class, two colours.
#[test]
fn every_highlight_class_in_the_snippet_has_its_own_colour() {
    let source = snippet();
    let spans = highlight(&source);
    assert!(!spans.is_empty(), "the snippet must classify to spans");

    for theme in THEMES {
        let svg = read(&format!("readme-snippet-{theme}.svg"));

        // Byte offset -> the colour painting it, from the fragments the image is built out of.
        let mut painted: Vec<Option<String>> = vec![None; source.len()];
        let mut at = 0usize;
        for fragment in fragments(&svg) {
            let len = fragment.text.len();
            // Whitespace carries no class, so whatever colour it inherited says nothing.
            if !fragment.text.trim().is_empty() {
                for slot in painted.iter_mut().skip(at).take(len) {
                    slot.clone_from(&fragment.fill);
                }
            }
            at += len;
        }

        // Every class present in the snippet, mapped to the set of colours the image paints it.
        let mut colours: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for (range, class) in &spans {
            let name = class_name(*class);
            let text = &source[*range];
            let (start, end) = (usize::from(range.start()), usize::from(range.end()));
            for (fill, byte) in painted[start..end]
                .iter()
                .zip(&source.as_bytes()[start..end])
            {
                let Some(fill) = fill.clone() else {
                    // Whitespace inside a multi-line token has no colour of its own.
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    panic!(
                        "assets/readme-snippet-{theme}.svg leaves `{text}` ({name}) uncoloured — \
                         a class the renderer does not paint reads as plain text"
                    );
                };
                let seen = colours.entry(name).or_default();
                if !seen.contains(&fill) {
                    seen.push(fill);
                }
            }
        }

        for (name, fills) in &colours {
            assert_eq!(
                fills.len(),
                1,
                "assets/readme-snippet-{theme}.svg paints {name} in {} different colours ({}) — \
                 the image is not coloured by flux's classification",
                fills.len(),
                fills.join(", ")
            );
        }

        // Distinct classes must be distinguishable, or "covered" means nothing: two classes sharing
        // a colour is exactly how an upstream addition would ship as invisible.
        let mut by_fill: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, fills) in &colours {
            by_fill.entry(&fills[0]).or_default().push(name);
        }
        for (fill, names) in &by_fill {
            assert_eq!(
                names.len(),
                1,
                "assets/readme-snippet-{theme}.svg paints {} with the same colour {fill}",
                names.join(" and ")
            );
        }
    }
}
