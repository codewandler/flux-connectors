//! The catalog pack, and the reader that serves it — dependency-free on purpose (C-537).
//!
//! The pack is one file compiled by `flux-connectors build` from the canonical per-provider
//! documents (`catalog/<name>.catalog.json`, C-536): every provider's complete published surface,
//! concatenated behind an offset index, fronted by a versioned header and a content digest. This
//! crate embeds that file and answers the four catalogue questions over it —
//! [`providers`], [`provider`], [`operation`], [`operations_of`] — without touching the network,
//! walking a filesystem, or parsing a byte of JSON at query time. A host that wants a *newer*
//! catalogue than it was built with loads one from a path with [`Pack::load`], which refuses a
//! wrong format version, schema version or digest before serving a single record.
//!
//! **Zero non-optional dependencies is the contract, not a habit.** The point of the pack is that
//! catalogue data stops riding code releases; the point of this crate is that reading it costs a
//! consumer nothing but the crate itself. The digest is SHA-256 — the one hash spelling this
//! repository records anywhere — so the check is vendored ([`sha256`]) rather than imported.
//!
//! # The format (container version 1)
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
//! Offsets are decimal byte offsets into the payload. A provider's span is its canonical document,
//! byte for byte; an operation's span slices that operation's own JSON record out of the owning
//! document. The whole file is UTF-8 text — documents are JSON — so a record is handed out as
//! `&str` and a consumer brings whatever JSON parser it already has.
//!
//! # Forward compatibility, stated once
//!
//! - **A newer container format is refused by name**: the version is the first line, checked
//!   before anything else is believed.
//! - **A newer document schema is refused by name**: the header's `schema` line is checked against
//!   [`SUPPORTED_SCHEMA`] before any record is served, because a record this reader hands out is
//!   one a consumer will act on.
//! - **Additive growth does not break this reader**: an unknown header line or an unknown index
//!   row kind is skipped, so a future pack that *adds* a record family still serves everything a
//!   version-1 consumer asks for. Anything a reader must not ignore is a format bump.
//!
//! # What this crate is not
//!
//! Not a document model — records are canonical JSON text, and interpreting them is the
//! resolver's job (C-538). Not the legacy `catalog` API — `codewandler-connector-catalog` remains
//! the typed `&'static` surface and re-exports this crate as `catalog::reader`. And not an
//! authentication boundary: the digest catches corruption and truncation, not an author who can
//! rewrite both the payload and the digest line above it.

mod sha256;

use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

/// The container format version this reader understands. A pack declaring a higher one is
/// refused by name — see [`Error::UnsupportedFormat`].
pub const FORMAT_VERSION: u32 = 1;

/// The canonical-document schema version this reader serves. A pack carrying a different one is
/// refused before any record is served — see [`Error::UnsupportedSchema`].
pub const SUPPORTED_SCHEMA: u32 = 1;

/// The magic word every pack opens with.
const MAGIC: &str = "flux-connectors-catalog-pack";

/// The pack this crate was built with: the compiled catalogue of the same repository state.
static EMBEDDED: &[u8] = include_bytes!("../catalog.pack");

/// Why a byte sequence is not a pack this reader will serve.
///
/// Every variant is a refusal *before* the first record: a reader that served half a catalogue
/// and then noticed would have already handed out records nothing verified.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The first line does not open with the pack's magic word — this is not a pack at all.
    NotAPack,
    /// The pack declares a container format this reader does not implement, named so an operator
    /// reads "upgrade the reader" rather than "the file is corrupt".
    UnsupportedFormat {
        /// The version the file declares.
        found: u32,
    },
    /// The pack's documents carry a schema version this reader does not serve — fail closed, by
    /// name, exactly as the story's forward-compatibility note requires.
    UnsupportedSchema {
        /// The schema version the header declares.
        found: u32,
    },
    /// The stated digest is not the digest of the content. Truncation, corruption or a hand-edit;
    /// whichever it was, no record is served from bytes that disagree with their own header.
    DigestMismatch {
        /// The digest the header states.
        stated: String,
        /// The digest the bytes actually have.
        computed: String,
    },
    /// The bytes are not valid UTF-8, which a pack — a text container over JSON — always is.
    NotText,
    /// Structurally not a version-1 pack: a missing header line, a malformed row, a span pointing
    /// outside the payload, a payload shorter than declared. The string names the offender.
    Malformed(String),
    /// [`Pack::load`] could not read the file. The message carries the path.
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAPack => write!(f, "not a {MAGIC} file"),
            Error::UnsupportedFormat { found } => write!(
                f,
                "the pack declares container format {found}, but this reader implements \
                 {FORMAT_VERSION}; a newer pack needs a newer reader"
            ),
            Error::UnsupportedSchema { found } => write!(
                f,
                "the pack carries document schema {found}, but this reader serves \
                 {SUPPORTED_SCHEMA}; refusing rather than handing out records it cannot vouch for"
            ),
            Error::DigestMismatch { stated, computed } => write!(
                f,
                "the pack's stated digest {stated} is not the content's digest {computed}; the \
                 file is truncated, corrupted or edited"
            ),
            Error::NotText => write!(f, "the pack is not UTF-8 text"),
            Error::Malformed(what) => write!(f, "malformed pack: {what}"),
            Error::Io(what) => write!(f, "cannot read the pack: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// The pack's bytes: borrowed from the binary for the embedded pack, owned for a loaded one.
enum Bytes {
    Embedded(&'static [u8]),
    Owned(Vec<u8>),
}

impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Embedded(bytes) => bytes,
            Bytes::Owned(bytes) => bytes,
        }
    }
}

/// One `p` row: a provider and its document's span in the payload.
struct ProviderRow {
    id: String,
    start: usize,
    len: usize,
}

/// One `o` row: an operation, its owner, and its record's span in the payload.
struct OperationRow {
    id: String,
    provider: String,
    service: String,
    start: usize,
    len: usize,
}

/// A parsed, digest-verified pack.
///
/// Constructed by [`Pack::load`], [`Pack::from_bytes`], or once for the whole process by
/// [`embedded`]. Every span was bounds- and boundary-checked at construction, so the accessors on
/// [`Provider`] and [`Operation`] cannot fail.
pub struct Pack {
    bytes: Bytes,
    payload_start: usize,
    schema_version: u32,
    digest: String,
    /// Sorted by id — restored on parse, so lookups may binary-search regardless of the file.
    providers: Vec<ProviderRow>,
    /// Sorted by id, same rule.
    operations: Vec<OperationRow>,
}

impl fmt::Debug for Pack {
    /// The identity and the shape, never the 9-MB payload: a `{:?}` in a log must not print the
    /// catalogue at an operator.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pack")
            .field("schema_version", &self.schema_version)
            .field("digest", &self.digest)
            .field("providers", &self.providers.len())
            .field("operations", &self.operations.len())
            .finish_non_exhaustive()
    }
}

/// One provider served from a pack: its id and its canonical document.
#[derive(Clone, Copy)]
pub struct Provider<'a> {
    pack: &'a Pack,
    row: &'a ProviderRow,
}

impl fmt::Debug for Provider<'_> {
    /// The id, never the document — same rule as [`Pack`]'s `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("id", &self.row.id)
            .finish_non_exhaustive()
    }
}

impl<'a> Provider<'a> {
    /// The provider id, e.g. `zendesk`.
    pub fn id(&self) -> &'a str {
        &self.row.id
    }

    /// The provider's canonical document — the exact bytes of its committed
    /// `catalog/<id>.catalog.json`, as JSON text.
    pub fn document(&self) -> &'a str {
        self.pack.payload_slice(self.row.start, self.row.len)
    }

    /// Every operation this provider publishes, in id order.
    pub fn operations(self) -> impl Iterator<Item = Operation<'a>> + 'a {
        let pack = self.pack;
        let id = self.row.id.as_str();
        pack.operations
            .iter()
            .filter(move |row| row.provider == id)
            .map(move |row| Operation { pack, row })
    }
}

/// One operation served from a pack: its index facts and its record.
#[derive(Clone, Copy)]
pub struct Operation<'a> {
    pack: &'a Pack,
    row: &'a OperationRow,
}

impl fmt::Debug for Operation<'_> {
    /// The index facts, never the record — same rule as [`Pack`]'s `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Operation")
            .field("id", &self.row.id)
            .field("provider", &self.row.provider)
            .field("service", &self.row.service)
            .finish_non_exhaustive()
    }
}

impl<'a> Operation<'a> {
    /// The operation id, e.g. `zendesk-ticket-show` — unique across the whole pack.
    pub fn id(&self) -> &'a str {
        &self.row.id
    }

    /// The id of the provider that declares it.
    pub fn provider(&self) -> &'a str {
        &self.row.provider
    }

    /// The service the operation belongs to — `default` for a single-surface provider, spelled
    /// out rather than elided, exactly as the document spells it.
    pub fn service(&self) -> &'a str {
        &self.row.service
    }

    /// The operation's record: its own JSON object, sliced byte for byte out of the owning
    /// canonical document.
    pub fn record(&self) -> &'a str {
        self.pack.payload_slice(self.row.start, self.row.len)
    }

    /// The whole canonical document of the operation's provider.
    pub fn document(&self) -> &'a str {
        self.pack
            .provider(&self.row.provider)
            .expect("an operation's provider row exists; verified at construction")
            .document()
    }
}

impl Pack {
    /// Read and verify a pack at `path` — the constructor for a host loading a newer catalogue
    /// than it was built with. Refuses a wrong format version, schema version or digest before
    /// serving any record.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the file cannot be read; otherwise everything
    /// [`from_bytes`](Self::from_bytes) refuses.
    pub fn load(path: impl AsRef<Path>) -> Result<Pack, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|error| Error::Io(format!("{}: {error}", path.display())))?;
        Self::from_bytes(bytes)
    }

    /// Verify and parse a pack from bytes already in hand.
    ///
    /// # Errors
    ///
    /// Every [`Error`] variant except [`Error::Io`]; see each variant for what it refuses.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Pack, Error> {
        Self::parse(Bytes::Owned(bytes))
    }

    /// The document schema version the pack carries.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// The verified content digest, as lowercase hex — the pack's identity, usable as a cache key.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Every provider in the pack, in id order.
    pub fn providers(&self) -> impl ExactSizeIterator<Item = Provider<'_>> {
        self.providers
            .iter()
            .map(|row| Provider { pack: self, row })
    }

    /// One provider by id.
    pub fn provider(&self, id: &str) -> Option<Provider<'_>> {
        self.providers
            .binary_search_by(|row| row.id.as_str().cmp(id))
            .ok()
            .map(|index| Provider {
                pack: self,
                row: &self.providers[index],
            })
    }

    /// Every operation in the pack, in id order.
    pub fn operations(&self) -> impl ExactSizeIterator<Item = Operation<'_>> {
        self.operations
            .iter()
            .map(|row| Operation { pack: self, row })
    }

    /// One operation by id.
    pub fn operation(&self, id: &str) -> Option<Operation<'_>> {
        self.operations
            .binary_search_by(|row| row.id.as_str().cmp(id))
            .ok()
            .map(|index| Operation {
                pack: self,
                row: &self.operations[index],
            })
    }

    /// Every operation of one provider, in id order. An unknown provider yields nothing, exactly
    /// as the legacy catalogue's `operations_of` does.
    pub fn operations_of<'s>(&'s self, provider: &str) -> impl Iterator<Item = Operation<'s>> + 's {
        // Owned, so the returned iterator borrows the pack alone — a caller may drop the id
        // string it looked up with while still walking the answer.
        let provider = provider.to_owned();
        self.operations
            .iter()
            .filter(move |row| row.provider == provider)
            .map(move |row| Operation { pack: self, row })
    }

    /// A verified span of the payload, as text.
    fn payload_slice(&self, start: usize, len: usize) -> &str {
        let bytes = &self.bytes.as_slice()[self.payload_start + start..][..len];
        std::str::from_utf8(bytes).expect("every span was boundary-checked at construction")
    }

    /// Parse and verify, in the order the refusals are promised: format, digest, schema,
    /// structure. Nothing is served until all four hold.
    fn parse(bytes: Bytes) -> Result<Pack, Error> {
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| Error::NotText)?;

        let mut offset = 0usize;
        let mut next_line = |what: &'static str| -> Result<(&str, usize), Error> {
            let rest = &text[offset..];
            let end = rest
                .find('\n')
                .ok_or_else(|| Error::Malformed(format!("the file ends before its {what} line")))?;
            let line = &rest[..end];
            offset += end + 1;
            Ok((line, offset))
        };

        // 1. The format line, believed before anything else is.
        let (magic_line, _) = next_line("magic")?;
        let mut words = magic_line.split(' ');
        if words.next() != Some(MAGIC) {
            return Err(Error::NotAPack);
        }
        let found: u32 = words
            .next()
            .and_then(|version| version.parse().ok())
            .ok_or_else(|| Error::Malformed(format!("no format version in `{magic_line}`")))?;
        if found != FORMAT_VERSION {
            return Err(Error::UnsupportedFormat { found });
        }

        // 2. The digest, verified over everything after its own line before any of it is parsed.
        let (digest_line, digested_from) = next_line("digest")?;
        let stated = digest_line
            .strip_prefix("digest sha256 ")
            .ok_or_else(|| Error::Malformed(format!("not a digest line: `{digest_line}`")))?
            .to_owned();
        let computed = sha256::hex_digest(&bytes.as_slice()[digested_from..]);
        if stated != computed {
            return Err(Error::DigestMismatch { stated, computed });
        }

        // 3. The header and index: known keys parsed, unknown lines skipped (additive growth),
        //    `payload` terminating.
        let mut schema_version: Option<u32> = None;
        let mut declared_providers: Option<usize> = None;
        let mut declared_operations: Option<usize> = None;
        let mut providers: Vec<ProviderRow> = Vec::new();
        let mut operations: Vec<OperationRow> = Vec::new();
        let payload_start;
        let declared_payload;
        loop {
            let (line, after) = next_line("payload")?;
            let mut fields = line.split(' ');
            match fields.next() {
                Some("schema") => {
                    let found = parse_field(line, fields.next())?;
                    if found != SUPPORTED_SCHEMA {
                        return Err(Error::UnsupportedSchema { found });
                    }
                    schema_version = Some(found);
                }
                Some("providers") => declared_providers = Some(parse_field(line, fields.next())?),
                Some("operations") => declared_operations = Some(parse_field(line, fields.next())?),
                Some("p") => {
                    let id = required(line, fields.next())?.to_owned();
                    let start = parse_field(line, fields.next())?;
                    let len = parse_field(line, fields.next())?;
                    providers.push(ProviderRow { id, start, len });
                }
                Some("o") => {
                    let id = required(line, fields.next())?.to_owned();
                    let provider = required(line, fields.next())?.to_owned();
                    let service = required(line, fields.next())?.to_owned();
                    let start = parse_field(line, fields.next())?;
                    let len = parse_field(line, fields.next())?;
                    operations.push(OperationRow {
                        id,
                        provider,
                        service,
                        start,
                        len,
                    });
                }
                Some("payload") => {
                    declared_payload = parse_field(line, fields.next())?;
                    payload_start = after;
                    break;
                }
                // An unknown line is additive growth within this format version, not corruption:
                // the digest above already vouched for the bytes.
                _ => {}
            }
        }

        // 4. Structure: everything promised must be present, sized and aligned.
        if schema_version.is_none() {
            return Err(Error::Malformed("no schema line".into()));
        }
        let payload = &text[payload_start..];
        if payload.len() != declared_payload {
            return Err(Error::Malformed(format!(
                "the payload is {} bytes where the header declares {declared_payload}",
                payload.len()
            )));
        }
        if declared_providers != Some(providers.len()) {
            return Err(Error::Malformed(format!(
                "{} provider rows where the header declares {declared_providers:?}",
                providers.len()
            )));
        }
        if declared_operations != Some(operations.len()) {
            return Err(Error::Malformed(format!(
                "{} operation rows where the header declares {declared_operations:?}",
                operations.len()
            )));
        }
        for (start, len, what) in providers
            .iter()
            .map(|row| (row.start, row.len, row.id.as_str()))
            .chain(
                operations
                    .iter()
                    .map(|row| (row.start, row.len, row.id.as_str())),
            )
        {
            let span = start
                .checked_add(len)
                .and_then(|end| payload.get(start..end));
            if span.is_none() {
                return Err(Error::Malformed(format!(
                    "`{what}`'s span {start}+{len} is not a slice of the {declared_payload}-byte \
                     payload"
                )));
            }
        }
        providers.sort_by(|a, b| a.id.cmp(&b.id));
        operations.sort_by(|a, b| a.id.cmp(&b.id));
        for row in &operations {
            if providers
                .binary_search_by(|held| held.id.cmp(&row.provider))
                .is_err()
            {
                return Err(Error::Malformed(format!(
                    "operation `{}` names provider `{}`, which has no row",
                    row.id, row.provider
                )));
            }
        }

        Ok(Pack {
            bytes,
            payload_start,
            schema_version: schema_version.expect("checked above"),
            digest: stated,
            providers,
            operations,
        })
    }
}

/// A required text field of an index row, or the malformed-line refusal naming the row.
fn required<'a>(line: &str, field: Option<&'a str>) -> Result<&'a str, Error> {
    field
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Malformed(format!("a field is missing in `{line}`")))
}

/// A required numeric field of a header line or index row.
fn parse_field<T: std::str::FromStr>(line: &str, field: Option<&str>) -> Result<T, Error> {
    field
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| Error::Malformed(format!("not a number where one is required: `{line}`")))
}

/// The pack this crate embeds, parsed and digest-verified once per process.
///
/// # Panics
///
/// If the embedded bytes do not verify — a state no released crate can be in, because the pack is
/// committed beside the reader and CI holds the pair to a fixed point of a build. Panicking beats
/// returning a `Result` every caller of a compile-time constant would have to invent a story for.
pub fn embedded() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| {
        Pack::parse(Bytes::Embedded(EMBEDDED))
            .expect("the embedded catalog.pack verifies; it is committed beside this crate")
    })
}

/// Every provider in the embedded pack, in id order.
pub fn providers() -> impl ExactSizeIterator<Item = Provider<'static>> {
    embedded().providers()
}

/// One provider of the embedded pack, by id.
pub fn provider(id: &str) -> Option<Provider<'static>> {
    embedded().provider(id)
}

/// One operation of the embedded pack, by id.
pub fn operation(id: &str) -> Option<Operation<'static>> {
    embedded().operation(id)
}

/// Every operation of one provider in the embedded pack, in id order.
pub fn operations_of(provider: &str) -> impl Iterator<Item = Operation<'static>> {
    embedded().operations_of(provider)
}
