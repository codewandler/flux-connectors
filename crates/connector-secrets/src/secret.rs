//! The value a store hands back, in a wrapper that resists reaching a place it should not.

use std::fmt;

/// A credential value.
///
/// A newtype rather than a `String`, for the same reason `flux_secret::Material` is one: the
/// guarantee is at compile time, not at review time.
///
/// - **[`Debug`] does not leak.** It prints `Secret(<redacted>)` — no value, and no length, since a
///   length is enough to fingerprint which credential a log line is about.
/// - **There is no [`Display`](fmt::Display)**, so a secret cannot reach a format string by
///   accident. `{}` on one does not compile.
/// - **There is no `Serialize`.** It cannot be serialised into a tool result, a manifest, a
///   catalogue entry or a model-visible symbol. Asserted, not merely stated:
///
///   ```compile_fail
///   fn requires_serialize<T: serde::Serialize>() {}
///   requires_serialize::<connector_secrets::Secret>();
///   ```
///
///   The control, so the test above is known to fail for the right reason:
///
///   ```
///   fn requires_serialize<T: serde::Serialize>() {}
///   requires_serialize::<String>();
///   ```
///
/// Reading the value out is [`expose_secret`](Self::expose_secret) — a name chosen to be greppable,
/// so an audit of "where does a credential become a plain `&str`" is one search.
///
/// # What this is not
///
/// It is not zeroised on drop, and it does not claim to be: doing that honestly needs control over
/// every buffer the value was ever copied into, which a `String` does not give. It also carries no
/// expiry — managing or refreshing an expiring token is out of scope for this crate, and flux
/// already owns that machinery.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wrap a value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value.
    ///
    /// Named to be conspicuous. Every call site is a place a credential leaves its wrapper, and a
    /// host should register the result with its redactor before it goes anywhere else.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and take the value.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Whether the value is empty.
    ///
    /// Useful without exposing anything: a store that answered with an empty string is a
    /// misconfiguration worth reporting, and reporting it should not require unwrapping.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Redacted, and without a length — a length is a fingerprint.
impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

/// Compared without an early return.
///
/// A signing secret is declared in `[[auth]]` like any other credential (see the authentication
/// contract in `AGENTS.md`), and comparing one is a verification step, so the obvious `==` on the
/// inner `String` would be the wrong default to hand a host.
impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        let (left, right) = (self.0.as_bytes(), other.0.as_bytes());
        if left.len() != right.len() {
            return false;
        }
        // No early return past this point: the number of leading bytes that matched is the thing
        // worth not reporting.
        let mut difference = 0u8;
        for (a, b) in left.iter().zip(right) {
            difference |= a ^ b;
        }
        difference == 0
    }
}

impl Eq for Secret {}

#[cfg(test)]
mod tests {
    use super::*;

    /// An obvious non-credential. Nothing in this repository may commit a value shaped like a real
    /// token — a plausible placeholder has tripped push protection here before.
    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

    #[test]
    fn debug_leaks_neither_the_value_nor_its_length() {
        let secret = Secret::new(SENTINEL);
        let rendered = format!("{secret:?}");

        assert_eq!(rendered, "Secret(<redacted>)");
        assert!(!rendered.contains(SENTINEL));
        assert!(!rendered.contains(&SENTINEL.len().to_string()));
    }

    /// The wrapper is only worth anything if it survives being put inside something else — a store
    /// error, an `Option`, a tuple — which is where a derived `Debug` would otherwise print it.
    #[test]
    fn debug_still_does_not_leak_when_nested() {
        let nested = Some((Secret::new(SENTINEL), "context"));
        assert!(!format!("{nested:?}").contains(SENTINEL));
    }

    #[test]
    fn the_value_is_readable_through_one_conspicuous_call() {
        assert_eq!(Secret::new(SENTINEL).expose_secret(), SENTINEL);
        assert_eq!(Secret::new(SENTINEL).into_inner(), SENTINEL);
        assert!(Secret::new("").is_empty());
        assert!(!Secret::new(SENTINEL).is_empty());
    }

    #[test]
    fn equality_holds_for_the_ordinary_cases() {
        assert_eq!(Secret::new(SENTINEL), Secret::new(SENTINEL));
        assert_ne!(Secret::new(SENTINEL), Secret::new("SENTINEL-OTHER-VALUE"));
        assert_ne!(Secret::new(SENTINEL), Secret::new(""));
        assert_ne!(Secret::new(""), Secret::new(SENTINEL));
        assert_eq!(Secret::new(""), Secret::new(""));
        // Same length, one byte apart — the case a length-only comparison would pass.
        assert_ne!(Secret::new("aaaa"), Secret::new("aaab"));
    }
}
