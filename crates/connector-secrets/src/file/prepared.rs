//! Durable v2 transaction ledger and fixed prepared-image grammar.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;

use crate::{
    batch, Layout, PreparedSecretError, PreparedSecretStore, Secret, SecretBatch,
    SecretProposalDigest, SecretTransactionGeneration, SecretTransactionId, SecretTransactionState,
    MAX_TERMINAL_TRANSACTIONS,
};

use super::{
    fixed_sibling, hex_decode, hex_encode, read_bounded, remove_fixed, FileStore, StoreError,
    MAX_ENTRIES, MAX_FILE_BYTES, MAX_VALUE_BYTES,
};

pub(super) const HEADER_V2: &str = "# codewandler-connector-secrets file store, v2";
const RETIRED_PREFIX: &str = "# retired-through ";
const TRANSACTION_PREFIX: &str = "# transaction ";

#[derive(Clone, PartialEq, Eq)]
pub(super) enum FileRecord {
    Prepared(SecretProposalDigest),
    Committed(SecretProposalDigest),
    Aborted,
}

pub(super) struct Candidate {
    pub(super) id: SecretTransactionId,
    pub(super) digest: SecretProposalDigest,
    pub(super) entries: BTreeMap<String, Secret>,
}

#[derive(Default)]
pub(super) struct FileTransactions {
    pub(super) version_two: bool,
    pub(super) retired_through: u64,
    pub(super) records: BTreeMap<[u8; 32], FileRecord>,
    pub(super) candidate: Option<Candidate>,
}

impl FileTransactions {
    pub(super) fn prepared(&self) -> Option<(SecretTransactionId, SecretProposalDigest)> {
        self.records.iter().find_map(|(id, record)| match record {
            FileRecord::Prepared(digest) => Some((
                SecretTransactionId::from_protocol_bytes(*id)
                    .expect("v2 parsing rejects zero-generation ids"),
                *digest,
            )),
            FileRecord::Committed(_) | FileRecord::Aborted => None,
        })
    }

    pub(super) fn terminal_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| !matches!(record, FileRecord::Prepared(_)))
            .count()
    }
}

pub(super) fn encode_v2(
    entries: &BTreeMap<String, Secret>,
    transactions: &FileTransactions,
) -> Result<String, String> {
    if transactions.terminal_count() > MAX_TERMINAL_TRANSACTIONS {
        return Err("the terminal transaction ledger exceeds its fixed capacity".to_owned());
    }
    if transactions
        .records
        .values()
        .filter(|record| matches!(record, FileRecord::Prepared(_)))
        .count()
        > 1
    {
        return Err("the transaction ledger contains more than one prepared record".to_owned());
    }
    validate_entries(entries)?;

    let mut rendered = String::new();
    rendered.push_str(HEADER_V2);
    rendered.push('\n');
    rendered.push_str(RETIRED_PREFIX);
    rendered.push_str(&format!("{:016x}", transactions.retired_through));
    rendered.push('\n');
    for (id, record) in &transactions.records {
        if generation_of(*id) <= transactions.retired_through {
            return Err(
                "the transaction ledger contains a record at or below the inclusive retirement fence"
                    .to_owned(),
            );
        }
        rendered.push_str(TRANSACTION_PREFIX);
        rendered.push_str(&hex_encode(id));
        match record {
            FileRecord::Prepared(digest) => {
                rendered.push_str(" prepared ");
                rendered.push_str(&hex_encode(&digest.protocol_bytes()));
            }
            FileRecord::Committed(digest) => {
                rendered.push_str(" committed ");
                rendered.push_str(&hex_encode(&digest.protocol_bytes()));
            }
            FileRecord::Aborted => rendered.push_str(" aborted"),
        }
        rendered.push('\n');
    }
    for (address, secret) in entries {
        rendered.push_str(address);
        rendered.push(' ');
        rendered.push_str(&hex_encode(secret.expose_secret().as_bytes()));
        rendered.push('\n');
    }
    if rendered.len() > MAX_FILE_BYTES {
        return Err(format!(
            "the encoded store would exceed the {MAX_FILE_BYTES}-byte limit"
        ));
    }
    Ok(rendered)
}

pub(super) fn parse_v2<L: Layout>(
    contents: &str,
    layout: &L,
    file: &Path,
) -> Result<(BTreeMap<String, Secret>, FileTransactions), StoreError> {
    let mut lines = contents.lines().enumerate();
    let Some((_, header)) = lines.next() else {
        return Err(backend(file, "the v2 store is empty"));
    };
    if header != HEADER_V2 {
        return Err(backend(file, "the v2 header is not canonical"));
    }
    let Some((_, retired)) = lines.next() else {
        return Err(backend(file, "the v2 store has no retirement fence"));
    };
    let Some(encoded_fence) = retired.strip_prefix(RETIRED_PREFIX) else {
        return Err(backend(file, "line 2 is not the retirement fence"));
    };
    if encoded_fence.len() != 16
        || encoded_fence
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(backend(
            file,
            "line 2 carries a non-canonical retirement fence",
        ));
    }
    let retired_through = u64::from_str_radix(encoded_fence, 16)
        .map_err(|_| backend(file, "line 2 carries an invalid retirement fence"))?;

    let mut entries = BTreeMap::new();
    let mut records = BTreeMap::new();
    let mut saw_entry = false;
    let mut prior_id: Option<[u8; 32]> = None;
    let mut prior_address: Option<String> = None;
    let mut prepared_count = 0usize;

    for (index, raw) in lines {
        let number = index + 1;
        if raw.is_empty() {
            return Err(backend(
                file,
                &format!("line {number} is unexpectedly blank"),
            ));
        }
        if let Some(transaction) = raw.strip_prefix(TRANSACTION_PREFIX) {
            if saw_entry {
                return Err(backend(
                    file,
                    &format!("line {number} places a transaction after credential entries"),
                ));
            }
            let mut fields = transaction.split(' ');
            let encoded_id = fields.next().unwrap_or_default();
            let state = fields.next().unwrap_or_default();
            let digest = fields.next();
            if fields.next().is_some() || encoded_id.len() != 64 {
                return Err(backend(
                    file,
                    &format!("line {number} has an invalid transaction record"),
                ));
            }
            let id_bytes = decode_array::<32>(encoded_id)
                .ok_or_else(|| backend(file, &format!("line {number} has a non-canonical id")))?;
            SecretTransactionId::from_protocol_bytes(id_bytes).ok_or_else(|| {
                backend(
                    file,
                    &format!("line {number} uses the reserved zero generation"),
                )
            })?;
            if generation_of(id_bytes) <= retired_through {
                return Err(backend(
                    file,
                    &format!(
                        "line {number} contradicts the inclusive retirement fence with an already-retired transaction record"
                    ),
                ));
            }
            if prior_id.is_some_and(|prior| prior >= id_bytes) {
                return Err(backend(
                    file,
                    &format!("line {number} is not in raw-id order"),
                ));
            }
            prior_id = Some(id_bytes);
            let record = match (state, digest) {
                ("prepared", Some(encoded)) => {
                    prepared_count += 1;
                    FileRecord::Prepared(SecretProposalDigest::from_protocol_bytes(
                        decode_array::<32>(encoded).ok_or_else(|| {
                            backend(file, &format!("line {number} has an invalid digest"))
                        })?,
                    ))
                }
                ("committed", Some(encoded)) => {
                    FileRecord::Committed(SecretProposalDigest::from_protocol_bytes(
                        decode_array::<32>(encoded).ok_or_else(|| {
                            backend(file, &format!("line {number} has an invalid digest"))
                        })?,
                    ))
                }
                ("aborted", None) => FileRecord::Aborted,
                _ => {
                    return Err(backend(
                        file,
                        &format!("line {number} has an invalid transaction state grammar"),
                    ))
                }
            };
            if records.insert(id_bytes, record).is_some() {
                return Err(backend(
                    file,
                    &format!("line {number} repeats a transaction id"),
                ));
            }
            continue;
        }

        saw_entry = true;
        if raw.starts_with('#') {
            return Err(backend(
                file,
                &format!("line {number} is an unknown v2 record"),
            ));
        }
        let (address, encoded) = raw
            .split_once(' ')
            .ok_or_else(|| backend(file, &format!("line {number} expected `<address> <hex>`")))?;
        if prior_address
            .as_deref()
            .is_some_and(|prior| prior >= address)
        {
            return Err(backend(
                file,
                &format!("line {number} is not in address order"),
            ));
        }
        prior_address = Some(address.to_owned());
        let reference = layout
            .parse(address)
            .map_err(|_| backend(file, &format!("line {number} carries an invalid address")))?;
        if layout.render(&reference) != address {
            return Err(backend(
                file,
                &format!("line {number} carries a non-canonical address"),
            ));
        }
        if encoded.len() > MAX_VALUE_BYTES * 2 {
            return Err(backend(
                file,
                &format!("line {number} exceeds the value bound"),
            ));
        }
        let value = hex_decode(encoded)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(|| {
                backend(
                    file,
                    &format!("line {number} carries an invalid value encoding"),
                )
            })?;
        entries.insert(address.to_owned(), Secret::new(value));
    }

    if prepared_count > 1 {
        return Err(backend(
            file,
            "the v2 ledger contains more than one prepared record",
        ));
    }
    let terminal_count = records.len().saturating_sub(prepared_count);
    if terminal_count > MAX_TERMINAL_TRANSACTIONS {
        return Err(backend(
            file,
            "the terminal transaction ledger exceeds capacity",
        ));
    }
    validate_entries(&entries).map_err(|reason| backend(file, &reason))?;
    let transactions = FileTransactions {
        version_two: true,
        retired_through,
        records,
        candidate: None,
    };
    let canonical = encode_v2(&entries, &transactions).map_err(|reason| backend(file, &reason))?;
    if canonical != contents {
        return Err(backend(file, "the v2 store is not in canonical byte form"));
    }
    Ok((entries, transactions))
}

fn validate_entries(entries: &BTreeMap<String, Secret>) -> Result<(), String> {
    if entries.len() > MAX_ENTRIES {
        return Err(format!("the store exceeds the {MAX_ENTRIES}-entry limit"));
    }
    if entries
        .values()
        .any(|secret| secret.expose_secret().len() > MAX_VALUE_BYTES)
    {
        return Err(format!(
            "a credential exceeds the {MAX_VALUE_BYTES}-byte value limit"
        ));
    }
    Ok(())
}

fn decode_array<const N: usize>(encoded: &str) -> Option<[u8; N]> {
    if encoded
        .bytes()
        .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return None;
    }
    let decoded = hex_decode(encoded)?;
    decoded.try_into().ok()
}

fn generation_of(id: [u8; 32]) -> u64 {
    let mut generation = [0; 8];
    generation.copy_from_slice(&id[..8]);
    u64::from_be_bytes(generation)
}

fn backend(file: &Path, reason: &str) -> StoreError {
    StoreError::Backend {
        path: file.display().to_string(),
        reason: reason.to_owned(),
    }
}

#[async_trait]
impl<L: Layout + Send + Sync> PreparedSecretStore for FileStore<L> {
    async fn prepare(
        &self,
        id: SecretTransactionId,
        digest: SecretProposalDigest,
        mutations: &SecretBatch,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut transactions = self.locked_transactions();
        if retired(&transactions, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some((prepared_id, prepared_digest)) = transactions.prepared() {
            if prepared_id != id {
                return Err(PreparedSecretError::Busy);
            }
            return if prepared_digest == digest {
                Ok(SecretTransactionState::Prepared)
            } else {
                Err(PreparedSecretError::DigestMismatch)
            };
        }
        if let Some(record) = transactions.records.get(&id.key()) {
            return match record {
                FileRecord::Committed(existing) if *existing == digest => {
                    Ok(SecretTransactionState::Committed)
                }
                FileRecord::Committed(_) => Err(PreparedSecretError::DigestMismatch),
                FileRecord::Aborted => Err(PreparedSecretError::TransactionIdReused),
                FileRecord::Prepared(_) => unreachable!("prepared() handled this record"),
            };
        }
        if transactions.terminal_count() >= MAX_TERMINAL_TRANSACTIONS {
            return Err(PreparedSecretError::Capacity);
        }

        let entries = self.locked();
        let mut candidate = entries.clone();
        batch::apply_to(&mut candidate, &self.layout, mutations)
            .map_err(|_| PreparedSecretError::InvalidBatch)?;
        super::validate_transactional_bounds(&candidate, transactions.terminal_count() + 1)
            .map_err(|_| PreparedSecretError::Capacity)?;

        let mut live_records = transactions.records.clone();
        live_records.insert(id.key(), FileRecord::Prepared(digest));
        let live = ledger(transactions.retired_through, live_records.clone());
        let live_rendered = encode_v2(&entries, &live).map_err(capacity_or_invalid)?;

        let mut stage_records = live_records;
        stage_records.insert(id.key(), FileRecord::Committed(digest));
        let stage = ledger(transactions.retired_through, stage_records);
        let stage_rendered = encode_v2(&candidate, &stage).map_err(capacity_or_invalid)?;
        let stage_path = fixed_sibling(&self.path, "prepared");

        self.write_rendered_to(&stage_path, &stage_rendered, true)
            .map_err(|_| PreparedSecretError::Backend)?;
        self.write_rendered_to(&self.path, &live_rendered, true)
            .map_err(|_| PreparedSecretError::Backend)?;

        transactions.version_two = true;
        transactions.records = live.records;
        transactions.candidate = Some(Candidate {
            id,
            digest,
            entries: candidate,
        });
        Ok(SecretTransactionState::Prepared)
    }

    async fn state(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut transactions = self.locked_transactions();
        let mut entries = self.locked();
        let mut file = super::platform::open_existing(&self.path)
            .map_err(|_| PreparedSecretError::Backend)?
            .ok_or(PreparedSecretError::Backend)?;
        let contents =
            read_bounded(&mut file, &self.path).map_err(|_| PreparedSecretError::Backend)?;
        let (durable_entries, mut durable_transactions) =
            if contents.lines().next() == Some(HEADER_V2) {
                parse_v2(&contents, &self.layout, &self.path)
                    .map_err(|_| PreparedSecretError::Backend)?
            } else {
                (
                    super::parse(&contents, &self.layout, &self.path)
                        .map_err(|_| PreparedSecretError::Backend)?,
                    FileTransactions::default(),
                )
            };
        super::recover_stage(&self.path, &self.layout, &mut durable_transactions)
            .map_err(|_| PreparedSecretError::Backend)?;
        *entries = durable_entries;
        *transactions = durable_transactions;
        if retired(&transactions, id) {
            return Err(PreparedSecretError::Retired);
        }
        Ok(match transactions.records.get(&id.key()) {
            Some(FileRecord::Prepared(_)) => SecretTransactionState::Prepared,
            Some(FileRecord::Committed(_)) => SecretTransactionState::Committed,
            Some(FileRecord::Aborted) | None => SecretTransactionState::Absent,
        })
    }

    async fn commit(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut transactions = self.locked_transactions();
        if retired(&transactions, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some(record) = transactions.records.get(&id.key()) {
            match record {
                FileRecord::Committed(_) => return Ok(SecretTransactionState::Committed),
                FileRecord::Aborted => return Err(PreparedSecretError::TransactionIdReused),
                FileRecord::Prepared(_) => {}
            }
        } else {
            return Err(PreparedSecretError::NotPrepared);
        }
        if transactions
            .prepared()
            .is_some_and(|(prepared, _)| prepared != id)
        {
            return Err(PreparedSecretError::NotPrepared);
        }
        let Some(candidate) = transactions.candidate.as_ref() else {
            return Err(PreparedSecretError::Backend);
        };
        if candidate.id != id {
            return Err(PreparedSecretError::Backend);
        }

        let mut entries = self.locked();
        verify_live(self, &entries, &transactions)?;
        let mut committed_records = transactions.records.clone();
        committed_records.insert(id.key(), FileRecord::Committed(candidate.digest));
        let committed = ledger(transactions.retired_through, committed_records.clone());
        let expected_stage =
            encode_v2(&candidate.entries, &committed).map_err(|_| PreparedSecretError::Backend)?;
        let stage_path = fixed_sibling(&self.path, "prepared");
        let stage_bytes = read_exact_stage(&stage_path, &self.path)?;
        if stage_bytes != expected_stage {
            return Err(PreparedSecretError::Backend);
        }
        self.write_rendered_to(&self.path, &stage_bytes, true)
            .map_err(|_| PreparedSecretError::Backend)?;

        let candidate = transactions.candidate.take().expect("checked above");
        *entries = candidate.entries;
        transactions.records = committed_records;
        transactions.version_two = true;
        if remove_fixed(&stage_path, &self.path).is_err() {
            return Err(PreparedSecretError::Backend);
        }
        Ok(SecretTransactionState::Committed)
    }

    async fn abort(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError> {
        let mut transactions = self.locked_transactions();
        if retired(&transactions, id) {
            return Err(PreparedSecretError::Retired);
        }
        if let Some(record) = transactions.records.get(&id.key()) {
            match record {
                FileRecord::Committed(_) => return Err(PreparedSecretError::AlreadyCommitted),
                FileRecord::Aborted => return Ok(SecretTransactionState::Absent),
                FileRecord::Prepared(_) => {}
            }
        }
        if let Some((prepared, _)) = transactions.prepared() {
            if prepared != id {
                return Err(PreparedSecretError::Busy);
            }
        }
        if transactions.terminal_count() >= MAX_TERMINAL_TRANSACTIONS {
            return Err(PreparedSecretError::Capacity);
        }

        let entries = self.locked();
        let was_prepared = matches!(
            transactions.records.get(&id.key()),
            Some(FileRecord::Prepared(_))
        );
        let mut records = transactions.records.clone();
        records.insert(id.key(), FileRecord::Aborted);
        let next = ledger(transactions.retired_through, records.clone());
        let rendered = encode_v2(&entries, &next).map_err(capacity_or_invalid)?;
        self.write_rendered_to(&self.path, &rendered, true)
            .map_err(|_| PreparedSecretError::Backend)?;

        transactions.version_two = true;
        transactions.records = records;
        transactions.candidate = None;
        if was_prepared && remove_fixed(&fixed_sibling(&self.path, "prepared"), &self.path).is_err()
        {
            return Err(PreparedSecretError::Backend);
        }
        Ok(SecretTransactionState::Absent)
    }

    async fn reclaim(
        &self,
        through: SecretTransactionGeneration,
    ) -> Result<(), PreparedSecretError> {
        let mut transactions = self.locked_transactions();
        if transactions.prepared().is_some() {
            return Err(PreparedSecretError::Busy);
        }
        let next_fence = transactions.retired_through.max(through.value());
        if next_fence == transactions.retired_through {
            return Ok(());
        }
        let mut records = transactions.records.clone();
        records.retain(|id, _| generation_of(*id) > next_fence);
        let next = ledger(next_fence, records.clone());
        let entries = self.locked();
        let rendered = encode_v2(&entries, &next).map_err(capacity_or_invalid)?;
        self.write_rendered_to(&self.path, &rendered, true)
            .map_err(|_| PreparedSecretError::Backend)?;
        transactions.version_two = true;
        transactions.retired_through = next_fence;
        transactions.records = records;
        Ok(())
    }
}

fn ledger(retired_through: u64, records: BTreeMap<[u8; 32], FileRecord>) -> FileTransactions {
    FileTransactions {
        version_two: true,
        retired_through,
        records,
        candidate: None,
    }
}

fn retired(transactions: &FileTransactions, id: SecretTransactionId) -> bool {
    id.generation().value() <= transactions.retired_through
}

fn capacity_or_invalid(reason: String) -> PreparedSecretError {
    if reason.contains("terminal transaction ledger") || reason.contains("encoded store") {
        PreparedSecretError::Capacity
    } else {
        PreparedSecretError::InvalidBatch
    }
}

fn verify_live<L: Layout>(
    store: &FileStore<L>,
    entries: &BTreeMap<String, Secret>,
    transactions: &FileTransactions,
) -> Result<(), PreparedSecretError> {
    let expected = encode_v2(entries, transactions).map_err(|_| PreparedSecretError::Backend)?;
    let mut file = super::platform::open_existing(&store.path)
        .map_err(|_| PreparedSecretError::Backend)?
        .ok_or(PreparedSecretError::Backend)?;
    let found = read_bounded(&mut file, &store.path).map_err(|_| PreparedSecretError::Backend)?;
    if found == expected {
        Ok(())
    } else {
        Err(PreparedSecretError::Backend)
    }
}

fn read_exact_stage(stage: &Path, store: &Path) -> Result<String, PreparedSecretError> {
    let mut file = super::platform::open_existing(stage)
        .map_err(|_| PreparedSecretError::Backend)?
        .ok_or(PreparedSecretError::Backend)?;
    read_bounded(&mut file, store).map_err(|_| PreparedSecretError::Backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SecretTransactionGeneration, TenantLayout};

    #[test]
    fn retired_prepared_and_terminal_records_are_refused_by_parser_and_encoder() {
        let id = format!("{}{}", "0000000000000001", "aa".repeat(24));
        let digest = "bb".repeat(32);
        for record in [
            format!("# transaction {id} prepared {digest}"),
            format!("# transaction {id} committed {digest}"),
            format!("# transaction {id} aborted"),
        ] {
            let fixture = format!("{HEADER_V2}\n# retired-through 0000000000000001\n{record}\n");
            let error = parse_v2(&fixture, &TenantLayout, Path::new("retired.fixture"))
                .err()
                .expect("retired record must contradict the inclusive fence");
            assert!(error.to_string().contains("retirement fence"), "{error}");
        }

        let generation = SecretTransactionGeneration::from_protocol_bytes([0, 0, 0, 0, 0, 0, 0, 1])
            .expect("non-zero");
        let transaction = SecretTransactionId::new(generation, [0xaa; 24]);
        let proposal = SecretProposalDigest::from_protocol_bytes([0xbb; 32]);
        for record in [
            FileRecord::Prepared(proposal),
            FileRecord::Committed(proposal),
            FileRecord::Aborted,
        ] {
            let mut records = BTreeMap::new();
            records.insert(transaction.key(), record);
            let ledger = FileTransactions {
                version_two: true,
                retired_through: 1,
                records,
                candidate: None,
            };
            assert!(encode_v2(&BTreeMap::new(), &ledger)
                .expect_err("encoder must refuse every retired record state")
                .contains("inclusive retirement fence"));
        }
    }
}
