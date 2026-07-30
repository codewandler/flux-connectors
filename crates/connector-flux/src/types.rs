//! JSON Schema → Flux [`TypeRef`], and the one fallback that makes the mapping total.
//!
//! # The type surface, and what it cannot hold
//!
//! Flux's `TypeRef` is deliberately small: `Any`, `Bool`, `Number`, `String`, a homogeneous
//! `List<T>`, and a `Named` reference to a registered schema. JSON Schema is not small. So the
//! mapping is a *narrowing*, and the interesting part is what happens at the edge.
//!
//! | JSON Schema | Flux |
//! |---|---|
//! | `{"type": "string"}` | `String` |
//! | `{"type": "integer"}`, `{"type": "number"}` | `Number` |
//! | `{"type": "boolean"}` | `Bool` |
//! | `{"type": "array", "items": S}` | `List<`map(S)`>` |
//! | **anything else** | **`Any`** |
//!
//! # The `Any` fallback
//!
//! Everything Flux cannot express becomes `Any`, and that is a deliberate choice rather than a gap:
//!
//! - **Unions** — `oneOf`/`anyOf`, or a `type` that is itself a list (`["string", "null"]`).
//!   babelforce's `agentId` is `uuid | array[uuid]`
//!   (`docs/designs/provider-operation-inventory.md` §6.5); Flux has no sum type to hold it.
//! - **Objects** — `{"type": "object"}` with or without properties. `Named` exists, but naming a
//!   type would mean *registering* a schema with flux, which no story has designed yet.
//! - **`$ref`, `allOf`, and an absent `type`** — resolvable in principle, not resolved here.
//! - **Every constraint keyword** — `minimum`, `maxLength`, `enum`, `format`. Flux's type surface
//!   has nowhere to put them, so they are dropped from the *declaration* only.
//!
//! Two properties keep this honest. First, `Any` is the **top type**, so the failure mode is a
//! missing check, never a false rejection: an op whose parameter is `Any` still accepts every value
//! the vendor accepts. Narrowing later is a compatible change; guessing a narrower type now and
//! being wrong is not. Second, **nothing is lost** — the full [`JsonSchema`] stays on the IR
//! [`connector_spec::Param`], so the manifest (C-10) and any later response work read the vendor's
//! schema verbatim rather than this projection of it.

use connector_spec::JsonSchema;
use flux_lang::ast::TypeRef;

/// The Flux type for a parameter's JSON Schema. Total: an unmappable shape lands on `Any` — see the
/// module documentation for exactly which shapes those are and why.
pub(crate) fn flux_type(schema: &JsonSchema) -> TypeRef {
    // A `type` that is not a single string — absent, or the `["string","null"]` union form — has no
    // Flux spelling and takes the fallback.
    let Some(kind) = schema.get("type").and_then(|t| t.as_str()) else {
        return TypeRef::Any;
    };
    match kind {
        "string" => TypeRef::String,
        "integer" | "number" => TypeRef::Number,
        "boolean" => TypeRef::Bool,
        // An array with no declared `items` is a list of unknowns, not a non-list.
        "array" => TypeRef::List(Box::new(
            schema.get("items").map(flux_type).unwrap_or(TypeRef::Any),
        )),
        _ => TypeRef::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn label(schema: serde_json::Value) -> String {
        flux_type(&schema).label()
    }

    #[test]
    fn scalars_map_across() {
        assert_eq!(label(json!({"type": "string"})), "String");
        assert_eq!(label(json!({"type": "integer"})), "Number");
        assert_eq!(label(json!({"type": "number"})), "Number");
        assert_eq!(label(json!({"type": "boolean"})), "Bool");
    }

    /// Constraint keywords do not change the type — they simply have nowhere to live.
    #[test]
    fn constraints_are_dropped_not_promoted() {
        assert_eq!(
            label(json!({"type": "integer", "format": "int64", "minimum": 1})),
            "Number"
        );
        assert_eq!(
            label(json!({"type": "string", "enum": ["inbound", "outbound"]})),
            "String"
        );
    }

    #[test]
    fn arrays_carry_their_item_type() {
        assert_eq!(
            label(json!({"type": "array", "items": {"type": "string"}})),
            "List<String>"
        );
        assert_eq!(label(json!({"type": "array"})), "List<Any>");
    }

    /// The fallback, one case per documented reason.
    #[test]
    fn shapes_flux_cannot_express_fall_back_to_any() {
        // A `oneOf` union — babelforce's scalar-or-array filters.
        assert_eq!(
            label(json!({"oneOf": [{"type": "string"}, {"type": "array"}]})),
            "Any"
        );
        // A nullable union written as a `type` list.
        assert_eq!(label(json!({"type": ["string", "null"]})), "Any");
        // A free-form object — babelforce's session-variable bodies.
        assert_eq!(label(json!({"type": "object"})), "Any");
        // An unresolved reference, and a schema with no `type` at all.
        assert_eq!(
            label(json!({"$ref": "#/components/schemas/CallState"})),
            "Any"
        );
        assert_eq!(label(json!({})), "Any");
    }

    /// The fallback is recursive: a list of unmappable things is still a list.
    #[test]
    fn the_fallback_reaches_inside_a_list() {
        assert_eq!(
            label(json!({"type": "array", "items": {"oneOf": [{"type": "string"}]}})),
            "List<Any>"
        );
    }
}
