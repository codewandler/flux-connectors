//! Wire names and Flux symbol names are **not** the same namespace, and this module is the seam
//! between them.
//!
//! A vendor is free to name a query parameter `time.start`; babelforce does exactly that
//! (`docs/designs/provider-operation-inventory.md` §6.5). Flux is not: a session symbol name is
//! ASCII alphanumerics and `_` only, and a dot in particular is *never* valid in a declared name —
//! in expression position `$a.b` is field-access sugar for `jq(".b", $a)`, so a dotted symbol would
//! silently change meaning rather than fail (`flux_lang::ast::SymbolName::is_identifier`).
//!
//! So each parameter carries two names: the **wire name**, which is what the vendor sees in the
//! query string, and the **symbol name**, which is what the generated `op` declares and references.
//! Nothing is mangled silently — the mapping is total, deterministic, and only ever changes the
//! symbol side.

use std::collections::BTreeSet;

use crate::Error;

/// Symbols the emitter binds inside an op body. A vendor parameter that would map onto one of these
/// is disambiguated instead of being allowed to shadow it — a parameter named `url` must not quietly
/// overwrite the URL the request is built from.
///
/// The list is reserved unconditionally, not per operation: a `GET` binds neither `payload` nor
/// `content_type`, but reserving them everywhere keeps a parameter's symbol independent of which
/// other parameters its operation happens to declare, so adding a body field later cannot rename a
/// symbol that already travelled.
const RESERVED: &[&str] = &["base", "url", "sep", "content_type", "payload", "response"];

/// Hands out a unique Flux symbol name for each vendor parameter name, in declaration order.
pub(crate) struct Symbols {
    taken: BTreeSet<String>,
    /// An extra veto, consulted like [`taken`](Self::taken). See [`Symbols::guarded`].
    forbidden: fn(&str) -> bool,
}

fn nothing_forbidden(_: &str) -> bool {
    false
}

impl Symbols {
    /// A fresh allocator, pre-seeded with the emitter's own [`RESERVED`] symbols.
    pub(crate) fn new() -> Self {
        Self {
            taken: RESERVED.iter().map(|s| (*s).to_string()).collect(),
            forbidden: nothing_forbidden,
        }
    }

    /// A fresh allocator that additionally refuses any name `forbidden` vetoes.
    ///
    /// The operation emitter binds a fixed set of its own symbols and reserves exactly those. The
    /// graph lowering binds none of them and instead has to dodge **flux's own reserved words**,
    /// because a graph's symbols are generated from author-chosen node ids and port names.
    ///
    /// That veto is a *predicate*, deliberately, so it can be `flux_lang::ast::is_reserved_word`
    /// itself rather than a list transcribed from flux's parser. A transcribed list is wrong the
    /// moment flux adds a word — and under flux-lang 0.39, where a local binding is spelled without
    /// the `$` sigil unless it collides with a keyword, being wrong means emitting a name that
    /// reads as a statement keyword.
    pub(crate) fn guarded(forbidden: fn(&str) -> bool) -> Self {
        Self {
            taken: BTreeSet::new(),
            forbidden,
        }
    }

    /// The Flux symbol for `wire`, reserving it so no later parameter can collide with it.
    ///
    /// The mapping, in order:
    ///
    /// 1. every character outside `[A-Za-z0-9_]` becomes `_` (`time.start` → `time_start`);
    /// 2. a name that would start with a digit is prefixed with `p_`, because a leading digit is
    ///    not a spellable identifier start;
    /// 3. a name already taken — by [`RESERVED`] or by an earlier parameter that normalized to the
    ///    same thing — gains a `_2`, `_3`, … suffix.
    ///
    /// Step 3 is why this is a stateful allocator rather than a pure function: `time.start` and
    /// `time_start` are distinct wire names that normalize identically, and collapsing them would
    /// send one parameter's value under the other's name.
    pub(crate) fn allocate(&mut self, operation: &str, wire: &str) -> Result<String, Error> {
        if wire.is_empty() {
            return Err(Error::BadParamName {
                operation: operation.to_string(),
                name: wire.to_string(),
                reason: "it is empty",
            });
        }
        // A brace would be read as an interpolation delimiter by the `fmt` template the URL is
        // built from, corrupting the query string rather than escaping it.
        if wire.contains('{') || wire.contains('}') {
            return Err(Error::BadParamName {
                operation: operation.to_string(),
                name: wire.to_string(),
                reason: "it contains a brace, which would corrupt the URL template",
            });
        }

        let mut symbol: String = wire
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        if symbol.starts_with(|c: char| c.is_ascii_digit()) {
            symbol.insert_str(0, "p_");
        }

        if self.is_unavailable(&symbol) {
            let base = symbol.clone();
            let mut n = 2;
            while self.is_unavailable(&symbol) {
                symbol = format!("{base}_{n}");
                n += 1;
            }
        }

        self.taken.insert(symbol.clone());
        Ok(symbol)
    }

    /// Whether `symbol` is already handed out or vetoed by the guard.
    fn is_unavailable(&self, symbol: &str) -> bool {
        self.taken.contains(symbol) || (self.forbidden)(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_safe_name_is_left_alone() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("op", "per_page").unwrap(), "per_page");
        assert_eq!(symbols.allocate("op", "agentId").unwrap(), "agentId");
    }

    /// The babelforce case: dots are not identifier-safe, and `$time.start` would reparse as field
    /// access on `$time` rather than failing.
    #[test]
    fn a_dotted_vendor_name_becomes_an_identifier() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("op", "time.start").unwrap(), "time_start");
    }

    #[test]
    fn two_names_that_normalize_alike_stay_distinct() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("op", "time.start").unwrap(), "time_start");
        assert_eq!(
            symbols.allocate("op", "time_start").unwrap(),
            "time_start_2"
        );
    }

    /// The emitter's own symbols are not up for grabs — a vendor parameter called `url` must not
    /// overwrite the URL the request is built from.
    #[test]
    fn the_emitters_own_symbols_are_reserved() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("op", "url").unwrap(), "url_2");
        assert_eq!(symbols.allocate("op", "base").unwrap(), "base_2");
        assert_eq!(symbols.allocate("op", "sep").unwrap(), "sep_2");
        assert_eq!(symbols.allocate("op", "payload").unwrap(), "payload_2");
        assert_eq!(symbols.allocate("op", "response").unwrap(), "response_2");
        assert_eq!(
            symbols.allocate("op", "content_type").unwrap(),
            "content_type_2"
        );
    }

    #[test]
    fn a_leading_digit_is_prefixed() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("op", "2fa").unwrap(), "p_2fa");
    }

    #[test]
    fn an_empty_or_brace_bearing_name_is_refused() {
        let mut symbols = Symbols::new();
        assert!(symbols.allocate("op", "").is_err());
        assert!(symbols.allocate("op", "a{b}").is_err());
    }
}
