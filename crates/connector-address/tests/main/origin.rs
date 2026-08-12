//! The published HTTPS-origin contract: normalization, identity, and value-free refusal (C-523).

use std::collections::HashSet;

use connector_address::{HttpsOrigin, OriginRefusal};

use crate::origin_corpus::{Outcome, ORIGIN_CASES};

/// The corpus, read as what it is for this crate: parsing produces exactly this canonical text, or
/// exactly this refusal class.
#[test]
fn every_corpus_case_parses_to_its_recorded_outcome() {
    for case in ORIGIN_CASES {
        match case.outcome {
            Outcome::Canonical(canonical) => {
                let origin = HttpsOrigin::parse(case.input).unwrap_or_else(|refusal| {
                    panic!(
                        "{:?} is a safe origin but was refused: {refusal}",
                        case.input
                    )
                });
                assert_eq!(
                    origin.as_str(),
                    canonical,
                    "{:?} normalized to the wrong destination",
                    case.input
                );
            }
            Outcome::Refused(expected) => {
                let Err(refusal) = HttpsOrigin::parse(case.input) else {
                    panic!("{:?} must be refused", case.input)
                };
                assert_eq!(
                    refusal, expected,
                    "{:?} was refused as the wrong class",
                    case.input
                );
            }
        }
    }
}

/// Canonical text is a fixed point, or "already canonical" would not be a decidable question and
/// [`HttpsOrigin::parse_canonical`] would refuse values the runtime had produced.
#[test]
fn normalization_is_idempotent_and_canonical_declarations_round_trip() {
    for case in ORIGIN_CASES {
        let Some(canonical) = case.canonical() else {
            continue;
        };
        let reparsed = HttpsOrigin::parse(canonical).expect("canonical text is a safe origin");
        assert_eq!(reparsed.as_str(), canonical);
        assert_eq!(
            HttpsOrigin::parse_canonical(canonical)
                .expect("canonical text is a canonical declaration")
                .as_str(),
            canonical
        );
        assert_eq!(
            HttpsOrigin::parse_canonical(case.input).is_ok(),
            case.is_canonical_declaration(),
            "{:?} is {}canonical",
            case.input,
            if case.is_canonical_declaration() {
                ""
            } else {
                "not "
            }
        );
    }
}

/// The property the type exists for: equivalent spellings are **one value**, so equality, ordering
/// and hashing compare destinations rather than caller text. Exchange compares proposal revisions
/// with these, and the pack compares a supplied origin against the reviewed default.
#[test]
fn equivalent_spellings_are_one_value_and_a_different_origin_is_not() {
    let equivalent = [
        "https://gitlab.com",
        "HTTPS://gitlab.com",
        "https://GitLab.COM",
        "https://gitlab.com:443",
        "https://gitlab.com:0443",
    ]
    .map(|spelling| HttpsOrigin::parse(spelling).expect("a safe spelling of one origin"));

    let distinct: HashSet<&HttpsOrigin> = equivalent.iter().collect();
    assert_eq!(distinct.len(), 1, "one destination must hash as one value");
    for origin in &equivalent {
        assert_eq!(origin, &equivalent[0]);
        assert_eq!(origin.cmp(&equivalent[0]), std::cmp::Ordering::Equal);
    }

    // A port, a host and a subdomain each remain a real authority change.
    for other in [
        "https://gitlab.com:8443",
        "https://gitlab.example",
        "https://api.gitlab.com",
    ] {
        let other = HttpsOrigin::parse(other).expect("a safe origin");
        assert_ne!(other, equivalent[0]);
    }
}

/// The parts a consumer reads instead of re-parsing the canonical text.
#[test]
fn the_components_agree_with_the_canonical_text() {
    let default_port = HttpsOrigin::parse("https://gitlab.com:443").expect("a safe origin");
    assert_eq!(default_port.host(), "gitlab.com");
    assert_eq!(default_port.port(), None);
    assert_eq!(default_port.effective_port(), 443);

    let explicit = HttpsOrigin::parse("https://GITLAB.example:08443").expect("a safe origin");
    assert_eq!(explicit.host(), "gitlab.example");
    assert_eq!(explicit.port(), Some(8443));
    assert_eq!(explicit.effective_port(), 8443);
    assert_eq!(explicit.as_str(), "https://gitlab.example:8443");

    let ipv6 = HttpsOrigin::parse("https://[2001:0DB8::1]:8443").expect("a safe origin");
    assert_eq!(ipv6.host(), "[2001:db8::1]");
    assert_eq!(ipv6.port(), Some(8443));
    assert_eq!(
        HttpsOrigin::parse("https://[2001:db8::1]")
            .expect("a safe origin")
            .host(),
        "[2001:db8::1]",
        "an unported IPv6 host still renders its brackets, which is what composes back into a URL"
    );

    assert_eq!(
        HttpsOrigin::parse("https://gitlab.com")
            .expect("a safe origin")
            .into_string(),
        "https://gitlab.com"
    );
}

/// **A refusal carries no value, and neither does the accepted one's `Debug`.** A configured origin
/// is a private installation's deployment detail; a refusal is exactly the moment it would otherwise
/// be copied into a log, an error chain and a test failure at once.
#[test]
fn no_refusal_and_no_debug_rendering_reproduces_the_supplied_text() {
    for case in ORIGIN_CASES {
        match HttpsOrigin::parse(case.input) {
            Ok(origin) => {
                let debug = format!("{origin:?}");
                assert!(
                    !debug.contains(origin.as_str()) && !debug.contains(case.input),
                    "a normalized origin rendered its text through `Debug`: {debug}"
                );
            }
            Err(refusal) => {
                // The third rendering is what a consumer walking a `thiserror` chain sees, which is
                // where a retained value would otherwise resurface.
                let chained: &dyn std::error::Error = &refusal;
                for rendered in [
                    refusal.to_string(),
                    format!("{refusal:?}"),
                    chained.to_string(),
                ] {
                    assert!(
                        !rendered.contains(case.input),
                        "the refusal of {:?} reproduced it: {rendered}",
                        case.input
                    );
                }
            }
        }
    }

    // The refusal is a plain unit-variant enum: there is no field for a value to be retained in, so
    // this holds by construction rather than by every message being written carefully.
    assert_eq!(std::mem::size_of::<OriginRefusal>(), 1);
}

/// The corpus is only a contract if it covers the classes it claims to. Stated as a property over
/// the corpus so a class added to the type without a case fails here rather than silently.
#[test]
fn the_corpus_covers_every_refusal_class_and_both_declaration_answers() {
    let refused: HashSet<String> = ORIGIN_CASES
        .iter()
        .filter_map(|case| match case.outcome {
            Outcome::Refused(refusal) => Some(format!("{refusal:?}")),
            Outcome::Canonical(_) => None,
        })
        .collect();
    for class in [
        OriginRefusal::NotHttps,
        OriginRefusal::Userinfo,
        OriginRefusal::Path,
        OriginRefusal::Query,
        OriginRefusal::Fragment,
        OriginRefusal::Whitespace,
        OriginRefusal::Placeholder,
        OriginRefusal::MissingHost,
        OriginRefusal::UnknownHost,
        OriginRefusal::UnbracketedIpv6,
        OriginRefusal::InvalidIpv6,
        OriginRefusal::InvalidPort,
    ] {
        assert!(
            refused.contains(&format!("{class:?}")),
            "no corpus case is refused as {class:?}"
        );
    }
    // `NotCanonical` is not a `parse` outcome — it is what `parse_canonical` adds — so it is covered
    // by the accepted half instead, which must contain both answers to be worth reading.
    assert!(
        ORIGIN_CASES
            .iter()
            .any(|case| case.is_canonical_declaration()),
        "the corpus records no declarable origin"
    );
    assert!(
        ORIGIN_CASES
            .iter()
            .any(|case| case.canonical().is_some() && !case.is_canonical_declaration()),
        "the corpus records no safe spelling that a declaration must not use"
    );
    assert_eq!(
        HttpsOrigin::parse_canonical("https://GitLab.com"),
        Err(OriginRefusal::NotCanonical)
    );
}
