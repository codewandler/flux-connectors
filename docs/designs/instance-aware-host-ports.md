# Design: instance-aware host ports

**Status:** accepted for C-494 · **Pillar:** Bridge · **Consumer:** flux-exchange X-14

## Boundary

C-406 already owns the durable address:

```
tenants/<tenant>/<authority>/@instances/<uuid>[/<service>]/<credential>
```

The host owns mutable labels and decides when a tenant has one or several connections. This change
does not move labels, selection policy or tenancy into flux-connectors. It gives the host two missing
mechanisms: bind that already-validated UUID to the pack, and migrate the old sole-connection
addresses without exposing values or committing only half a credential set.

## Scoped inventory

`CredentialScope` is the validated pair `(tenant, authority)`. `SecretStore::references(scope)`
returns only `CredentialRef`s in that scope, in deterministic order. It never returns a secret and
does not imply that arbitrary stores can list: the default is `StoreError::Unsupported`, and a Vault
adapter without a proven list policy refuses on the same terms.

The scope is deliberately narrower than a tenant and broader than an instance. It is exactly the
set a host must inspect when the second connection to one connector appears, while preventing a
batch assembled for that migration from reaching another connector or tenant.

## Atomic mutation

`SecretBatch` contains checked moves, puts and idempotent deletes under one `CredentialScope`.
Construction refuses an address outside the scope. Applying a move refuses a missing source or an
occupied destination; it never repairs either case. All operations are evaluated against a copy of
the store state and become visible together.

`MemoryStore` swaps the checked copy while holding its mutex. `FileStore` writes the checked copy
through its existing durable temporary-file-and-rename path before swapping the in-memory map. A
write failure therefore leaves both views unchanged. Stores that cannot provide this guarantee say
`Unsupported`; callers must not decompose a refused batch into point writes.

## Pack binding

`Credentials::new` and `Configuration::new` remain the sole-connection constructors.
`for_instance(..., uuid)` validates and holds the existing `InstanceId`. Credential references use
`CredentialRef::for_instance`; configuration snapshots call
`ConfigStore::get_for_instance(..., instance, ...)`.

The new config method has a compatibility default: no instance delegates to `get`, while a named
instance returns `None`. Existing host implementations therefore compile and remain sole-connection
only; they cannot silently serve the unscoped value for a named connection.

## Release boundary

This is an additive host-contract release across the normal connector publish closure. It is cut as
v0.18.0 by CI only. flux-exchange consumes the crates.io release and never a path or Git dependency,
so its instance migration lands only after the published contract is observable in the registry.
