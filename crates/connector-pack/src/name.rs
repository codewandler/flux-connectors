//! The naming asymmetry, in one function.
//!
//! This repository emits `zendesk-ticket-show`, because a **dotted name is not a legal Flux
//! declaration** — `connector-flux` refuses an operation id that
//! [`flux_lang::ast::is_valid_decl_name`] rejects, and that predicate's character set has no `.` in
//! it. But every flux **tool** is dotted: `http.request`, `op.register`, `skill.load`. flux's
//! reference flow calls `zendesk.ticket.show`, and only a tool surface can spell that.
//!
//! So the two names are the same operation seen from either side of the seam, and this module is
//! the seam. It is deliberately one small function with its own tests: flux's reference flow
//! resolves through it, and a wrong answer here is an operation the flow cannot call at all.
//!
//! # The rule
//!
//! Replace every `-` with `.`. That is the whole mapping — the id's remaining characters (ASCII
//! alphanumerics and `_`) are legal in a tool name unchanged, so nothing else moves.
//!
//! # Why it is fallible
//!
//! Because the two ends are governed by **flux's own predicates**, not by a character set invented
//! here, and they do not agree on everything:
//!
//! | | accepts |
//! |---|---|
//! | [`is_valid_decl_name`] — the id | non-empty ASCII alphanumerics, `_`, `-` |
//! | [`is_valid_op_name`] — the tool name | first char ASCII alphabetic or `_`, then alphanumerics, `_`, `.`, `-` |
//!
//! An id may therefore be perfectly declarable and still project onto something flux's call grammar
//! will not accept (`3d-print` → `3d.print` starts with a digit), and a doubled separator projects
//! onto an empty level (`a--b` → `a..b`) that both predicates accept and no host should be asked to
//! resolve. Each is refused by name rather than quietly repaired: `connector-flux` already refuses
//! an unspellable id rather than rewriting it, and silently rewriting a name flux's reference flow
//! spells out is how a flow stops resolving with nothing to point at.

use flux_lang::ast::{is_valid_decl_name, is_valid_op_name};

/// The separator an operation id uses between levels — the character a Flux declaration may carry
/// and a dotted tool name may not.
const ID_SEPARATOR: char = '-';

/// The separator a flux tool name uses between levels.
const TOOL_SEPARATOR: char = '.';

/// Why an operation id has no dotted tool name.
///
/// Every variant is a refusal, never a repair. See the module docs for why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The id is not something this repository could have emitted a declaration for.
    ///
    /// `connector-flux` refuses an id [`is_valid_decl_name`] rejects, so an id reaching here in
    /// this state means the input did not come from a generated catalogue entry — an
    /// already-dotted name being the likely case, and one worth refusing loudly rather than
    /// passing through as a no-op that looks like it worked.
    #[error(
        "`{id}` is not a declarable Flux symbol, so it is not an operation id this repository \
         emits — a Flux declaration name is ASCII alphanumerics, `_` and `-`, which is why the \
         dotted spelling exists on the tool side only"
    )]
    NotDeclarable {
        /// The rejected id.
        id: String,
    },

    /// A separator with nothing on one side of it, which projects onto an empty level.
    ///
    /// `zendesk--show` becomes `zendesk..show`: a name flux's grammar accepts and no host can
    /// meaningfully resolve, because one of its levels names nothing.
    #[error(
        "`{id}` has an empty level (a leading, trailing or doubled `{separator}`), which would \
         project onto the tool name `{dotted}` — every level between dots names something"
    )]
    EmptyLevel {
        /// The rejected id.
        id: String,
        /// The separator that produced the empty level.
        separator: char,
        /// What the id would have projected onto.
        dotted: String,
    },

    /// The projection is not a name flux's call grammar accepts.
    ///
    /// The reachable case is a leading digit: `is_valid_decl_name` admits one, `is_valid_op_name`
    /// does not, so `3d-print` is a declarable id with no tool name.
    #[error(
        "`{id}` projects onto `{dotted}`, which flux's call grammar does not accept — an op name \
         starts with an ASCII letter or `_`"
    )]
    NotCallable {
        /// The rejected id.
        id: String,
        /// The projection flux refused.
        dotted: String,
    },
}

/// Project an operation id onto the dotted name flux resolves a tool by.
///
/// `zendesk-ticket-comment-add` → `zendesk.ticket.comment.add`.
///
/// ```
/// assert_eq!(
///     connector_pack::dotted_name("zendesk-ticket-comment-add").unwrap(),
///     "zendesk.ticket.comment.add"
/// );
/// ```
///
/// # Errors
///
/// Returns [`NameError`] when the id is not a declarable Flux symbol, when it has an empty level,
/// or when the projection is not a name flux's call grammar accepts. See the module docs.
pub fn dotted_name(id: &str) -> Result<String, NameError> {
    if !is_valid_decl_name(id) {
        return Err(NameError::NotDeclarable { id: id.to_owned() });
    }

    let dotted = id.replace(ID_SEPARATOR, &TOOL_SEPARATOR.to_string());

    if dotted.split(TOOL_SEPARATOR).any(str::is_empty) {
        return Err(NameError::EmptyLevel {
            id: id.to_owned(),
            separator: ID_SEPARATOR,
            dotted,
        });
    }

    // Checked against flux's own predicate rather than re-derived: `is_valid_decl_name` admits a
    // leading digit and `is_valid_op_name` does not, and that gap is exactly the kind of thing a
    // hand-written charset here would miss.
    if !is_valid_op_name(&dotted) {
        return Err(NameError::NotCallable {
            id: id.to_owned(),
            dotted,
        });
    }

    Ok(dotted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Acceptance example, spelled out.
    #[test]
    fn every_separator_becomes_a_level() {
        assert_eq!(
            dotted_name("zendesk-ticket-comment-add").unwrap(),
            "zendesk.ticket.comment.add"
        );
    }

    /// The four names `examples/zendesk.triage.flux` calls. These are the reason this function is
    /// its own module: flux's retained-but-unrunnable reference flow resolves through exactly
    /// these, and a wrong answer for any one of them is what keeps it unrunnable.
    #[test]
    fn the_reference_flows_names_project_exactly() {
        assert_eq!(dotted_name("zendesk-test").unwrap(), "zendesk.test");
        assert_eq!(
            dotted_name("zendesk-ticket-show").unwrap(),
            "zendesk.ticket.show"
        );
        assert_eq!(
            dotted_name("zendesk-ticket-search").unwrap(),
            "zendesk.ticket.search"
        );
        assert_eq!(
            dotted_name("zendesk-ticket-comment-list").unwrap(),
            "zendesk.ticket.comment.list"
        );
    }

    /// One level is a whole name. The catalogue has no such operation today, but the mapping must
    /// not need a separator to be present in order to be total.
    #[test]
    fn a_single_level_is_left_alone() {
        assert_eq!(dotted_name("zendesk").unwrap(), "zendesk");
    }

    /// `_` is legal in both a declaration name and a tool name, so it is **not** a separator. An
    /// implementation that normalised "word separators" generally would turn `chat_post` into two
    /// levels and change the name flux resolves.
    #[test]
    fn an_underscore_is_not_a_separator() {
        assert_eq!(
            dotted_name("slack-chat_post-message").unwrap(),
            "slack.chat_post.message"
        );
        assert_eq!(dotted_name("_internal-probe").unwrap(), "_internal.probe");
    }

    /// A digit inside a level is ordinary — only a *leading* digit is a problem, and it is a
    /// problem for the whole name rather than for the level it sits in.
    #[test]
    fn a_digit_inside_a_level_is_ordinary() {
        assert_eq!(
            dotted_name("google-drive-v3-file-get").unwrap(),
            "google.drive.v3.file.get"
        );
        assert_eq!(
            dotted_name("acme-oauth2-token").unwrap(),
            "acme.oauth2.token"
        );
    }

    /// `google-calendar-calendar-get` is real. A projection that deduplicated levels — or that
    /// stopped at the first repeat — would silently rename it.
    #[test]
    fn a_repeated_level_survives() {
        assert_eq!(
            dotted_name("google-calendar-calendar-get").unwrap(),
            "google.calendar.calendar.get"
        );
    }

    /// Distinct ids must stay distinct, because two ids projecting onto one tool name is one
    /// operation silently shadowing another inside a host's registry.
    #[test]
    fn the_projection_does_not_merge_two_ids() {
        assert_ne!(
            dotted_name("fly-machine-get").unwrap(),
            dotted_name("fly-machines-list").unwrap()
        );
        assert_ne!(
            dotted_name("zendesk-ticket-comment-add").unwrap(),
            dotted_name("zendesk-ticket-comment-list").unwrap()
        );
    }

    #[test]
    fn an_empty_id_has_no_tool_name() {
        assert!(matches!(
            dotted_name(""),
            Err(NameError::NotDeclarable { .. })
        ));
    }

    /// An already-dotted name is refused rather than returned unchanged. Passing it through would
    /// make a double projection look successful, and `connector-flux` refuses a dotted id at
    /// emission for the same reason: a name is repaired by its author, not by the compiler.
    #[test]
    fn an_already_dotted_name_is_refused_rather_than_passed_through() {
        assert!(matches!(
            dotted_name("zendesk.ticket.show"),
            Err(NameError::NotDeclarable { .. })
        ));
    }

    /// A leading, trailing or doubled separator each produce an empty level, and each is named as
    /// that rather than as a generic parse failure.
    #[test]
    fn a_separator_with_nothing_beside_it_is_refused() {
        for id in ["-zendesk", "zendesk-", "zendesk--show", "-", "--"] {
            assert!(
                matches!(dotted_name(id), Err(NameError::EmptyLevel { .. })),
                "`{id}` must be refused as an empty level, got {:?}",
                dotted_name(id)
            );
        }
    }

    /// `is_valid_decl_name` admits a leading digit; flux's call grammar does not. The id is
    /// declarable and has no tool name, which is precisely why this function returns a `Result`.
    #[test]
    fn a_leading_digit_is_declarable_and_not_callable() {
        assert!(
            is_valid_decl_name("3d-print"),
            "the premise of this test is that the id is declarable"
        );
        assert!(matches!(
            dotted_name("3d-print"),
            Err(NameError::NotCallable { .. })
        ));
    }

    /// Every projection this function returns must satisfy the predicate flux's analyzer applies
    /// to a call's op name. This is the invariant the error variants exist to preserve.
    #[test]
    fn every_accepted_projection_is_a_name_flux_can_call() {
        for id in [
            "zendesk-test",
            "zendesk-ticket-comment-add",
            "google-calendar-calendar-get",
            "slack-chat_post-message",
            "_internal-probe",
            "airtable",
        ] {
            let dotted = dotted_name(id).unwrap_or_else(|error| panic!("`{id}`: {error}"));
            assert!(
                is_valid_op_name(&dotted),
                "`{id}` projected onto `{dotted}`, which flux's call grammar rejects"
            );
            assert!(
                !dotted.contains(ID_SEPARATOR),
                "`{dotted}` kept a separator"
            );
        }
    }
}
