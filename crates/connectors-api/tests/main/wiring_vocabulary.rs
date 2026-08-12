//! **The wiring vocabulary cannot drift** (C-239).
//!
//! [`Wiring`] serializes C-206's catalogue tokens, and `src/index.html` keys its `WIRING` table on
//! them. Two surfaces describing one fact, joined by a string neither of them declares to the
//! other: rename a variant and the server sends `no_credential_required` while the page looks up
//! `no-credential-required`, finds nothing, and falls through to printing the raw token at an
//! operator. No compiler sees that edge, and no HTTP test does either — the page is where the
//! lookup happens.
//!
//! # Why this is a Rust test and not part of the JS harness
//!
//! It is a claim about `Wiring`, not about the page's behaviour, and the JS harness cannot make it:
//! the enum is Rust, and enumerating its variants is the whole assertion. Here it costs no Node, no
//! browser and no network — the page is read as bytes and the tokens come from `serde`, which is
//! the same code path that puts them on the wire.
//!
//! The exhaustiveness tripwire is [`every_variant`]'s `match`. A fifth variant does not make this
//! test fail; it makes it **not compile**, which is the right moment to be told, because the author
//! adding it is the one person who knows what the page should call it.

use connectors_api::api::Wiring;

/// The page an operator receives, read the way `src/ui.rs` compiles it in.
const PAGE: &str = include_str!("../../src/index.html");

/// Every variant of [`Wiring`], exhaustively.
///
/// The `match` binds nothing and does nothing; it exists so that adding a variant is a compile
/// error here rather than a token that silently reaches a page which has never heard of it.
fn every_variant() -> Vec<Wiring> {
    let all = vec![
        Wiring::NoCredentialRequired,
        Wiring::NoCredential,
        Wiring::Wired,
        Wiring::PartlyWired,
        Wiring::NotWired,
    ];
    for wiring in &all {
        match wiring {
            Wiring::NoCredentialRequired
            | Wiring::NoCredential
            | Wiring::Wired
            | Wiring::PartlyWired
            | Wiring::NotWired => {}
        }
    }
    all
}

/// The token a variant travels as, taken from `serde` rather than restated.
fn token(wiring: Wiring) -> String {
    let value = serde_json::to_value(wiring).expect("Wiring serializes");
    value
        .as_str()
        .unwrap_or_else(|| panic!("{wiring:?} no longer serializes as a string: {value}"))
        .to_owned()
}

#[test]
fn every_wiring_variant_is_a_token_the_operator_page_knows() {
    for wiring in every_variant() {
        let token = token(wiring);

        // As a key of the page's own lookup table, not merely somewhere in the file. `'wired'`
        // appears inside `'partly-wired'` as a substring and the quote is what separates them, so
        // the quoted form is the assertion — a page that had dropped the `wired` key entirely
        // would still contain the letters.
        assert!(
            PAGE.contains(&format!("'{token}'")),
            "the host serves the wiring token `{token}` ({wiring:?}) and src/index.html does not \
             know it — the page falls through to printing the raw token at an operator"
        );
    }
}

#[test]
fn the_page_invents_no_wiring_state_the_host_cannot_send() {
    // The other direction, which is the one that rots quietly: a key left behind by a rename is
    // dead code that looks like coverage, and the sentence it holds is the one nobody will ever
    // read again.
    let known: Vec<String> = every_variant().into_iter().map(token).collect();

    let table = PAGE
        .split_once("const WIRING = {")
        .expect("src/index.html no longer declares a WIRING table")
        .1;
    let table = table
        .split_once("\n};")
        .expect("the WIRING table is no longer closed on a line of its own")
        .0;

    for line in table.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix('\'') else {
            continue;
        };
        let Some((key, _)) = rest.split_once('\'') else {
            continue;
        };
        assert!(
            known.contains(&key.to_owned()),
            "src/index.html answers for the wiring state `{key}`, which no Wiring variant \
             serializes as — either the state was renamed and the page kept the old key, or the \
             page is describing a state the host cannot send"
        );
    }
}
