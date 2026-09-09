//! Account-corpus and immutable-record boundary shared by Structured discovery.

use std::collections::BTreeSet;

use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use super::types::*;
use crate::observation::{FinalizedRecordProof, ObservationError, authenticate_finalized_record};
use crate::{Finality, Observation, ObservedAccount};

/// Located refusal from account discovery and finalized record authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleDiscoveryErrorV1 {
    /// A snapshot repeated an address with possibly contradictory observations.
    DuplicateAddress([u8; 32]),
    /// The account bytes did not match their explicit full/sliced observation.
    DataWindow([u8; 32]),
    /// A required point observation has not been acquired.
    MissingAccount([u8; 32]),
    /// A required existing account is absent.
    AbsentAccount([u8; 32]),
    /// A full semantic account was substituted with a data slice.
    SlicedAccount([u8; 32]),
    /// A native query requested incompatible observations for one address.
    ConflictingRequest([u8; 32]),
    /// An inventory has invalid ordering, duplicate addresses, or a future slot.
    InvalidScan,
    /// Native Registry key validation refused.
    RecordKey(dclutch_registry::record::Error),
    /// A fetched record differs from its selected immutable digest.
    RecordDigest([u8; 32]),
    /// Canonical immutable record authentication refused at the named raw address.
    RecordAuthentication([u8; 32], ObservationError),
}

/// A canonical raw/staging pair identified by schema and content, not a report label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleRecordKeyV1 {
    /// Immutable Registry schema identity.
    pub schema: [u8; 32],
    /// Exact selected content digest.
    pub content: [u8; 32],
    /// Canonical raw-record address.
    pub raw: [u8; 32],
    /// Canonical paired staging cursor address.
    pub staging: [u8; 32],
}

/// Immutable record whose address, owner, body and finalized staging vacancy joined.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleFinalizedRecordV1 {
    /// Canonical selected identities.
    pub key: StructuredLifecycleRecordKeyV1,
    /// Exact raw observation.
    pub raw: ObservedAccount,
    /// Exact paired vacancy observation.
    pub staging: ObservedAccount,
}

/// Checked corpus shape; semantic role authentication remains mandatory per lookup.
pub struct StructuredLifecycleCorpusV1<'a> {
    snapshot: &'a StructuredLifecycleSnapshotV1,
}

impl<'a> StructuredLifecycleCorpusV1<'a> {
    /// Check duplicate identities and preserve explicit absence and data windows.
    pub fn new(
        snapshot: &'a StructuredLifecycleSnapshotV1,
    ) -> Result<Self, StructuredLifecycleDiscoveryErrorV1> {
        let mut keys = BTreeSet::new();
        for row in &snapshot.accounts {
            let address = row.request.address;
            if !keys.insert(address) {
                return Err(StructuredLifecycleDiscoveryErrorV1::DuplicateAddress(
                    address,
                ));
            }
            if let Some(value) = &row.value {
                let wanted = match row.request.data_slice {
                    None => value.space,
                    Some(slice) if slice.length != 0 => u64::from(slice.length)
                        .min(value.space.saturating_sub(u64::from(slice.offset))),
                    Some(_) => {
                        return Err(StructuredLifecycleDiscoveryErrorV1::DataWindow(address));
                    }
                };
                if u64::try_from(value.data.len()).ok() != Some(wanted) {
                    return Err(StructuredLifecycleDiscoveryErrorV1::DataWindow(address));
                }
            }
        }
        for scan in &snapshot.scans {
            let unique: BTreeSet<_> = scan.addresses.iter().copied().collect();
            if scan.slot > snapshot.slot || unique.len() != scan.addresses.len() {
                return Err(StructuredLifecycleDiscoveryErrorV1::InvalidScan);
            }
        }
        Ok(Self { snapshot })
    }

    /// Common finalized observation slot.
    pub fn slot(&self) -> u64 {
        self.snapshot.slot
    }

    /// Actual supplied observation, distinguishing missing from explicit absence.
    pub fn get(&self, address: [u8; 32]) -> Option<&'a StructuredLifecycleAccountV1> {
        self.snapshot
            .accounts
            .iter()
            .find(|row| row.request.address == address)
    }

    /// Require complete observed account bytes; absence remains an explicit result.
    pub fn full(
        &self,
        address: [u8; 32],
    ) -> Result<Option<&'a StructuredLifecycleAccountValueV1>, StructuredLifecycleDiscoveryErrorV1>
    {
        let row = self
            .get(address)
            .ok_or(StructuredLifecycleDiscoveryErrorV1::MissingAccount(address))?;
        if row.request.data_slice.is_some() {
            return Err(StructuredLifecycleDiscoveryErrorV1::SlicedAccount(address));
        }
        Ok(row.value.as_ref())
    }

    /// Require one existing complete account.
    pub fn present(
        &self,
        address: [u8; 32],
    ) -> Result<&'a StructuredLifecycleAccountValueV1, StructuredLifecycleDiscoveryErrorV1> {
        self.full(address)?
            .ok_or(StructuredLifecycleDiscoveryErrorV1::AbsentAccount(address))
    }

    /// Canonical observation adapter. Explicit RPC absence becomes a zero System vacancy.
    pub fn observed(
        &self,
        address: [u8; 32],
        unix_timestamp: i64,
    ) -> Result<ObservedAccount, StructuredLifecycleDiscoveryErrorV1> {
        let value = self.full(address)?;
        Ok(ObservedAccount {
            observation: Observation {
                slot: self.snapshot.slot,
                unix_timestamp,
                finality: Finality::Finalized,
            },
            key: Pubkey::new_from_array(address),
            owner: value.map_or(system_program::ID, |account| {
                Pubkey::new_from_array(account.owner)
            }),
            lamports: value.map_or(0, |account| account.lamports),
            executable: value.is_some_and(|account| account.executable),
            data: value.map_or_else(Vec::new, |account| account.data.clone()),
        })
    }

    /// Deduplicate native requests and return precisely the observations still needed.
    pub fn missing(
        &self,
        requests: &[StructuredLifecycleAccountRequestV1],
    ) -> Result<Vec<StructuredLifecycleAccountRequestV1>, StructuredLifecycleDiscoveryErrorV1> {
        let mut canonical: Vec<StructuredLifecycleAccountRequestV1> = Vec::new();
        for request in requests {
            if let Some(prior) = canonical.iter().find(|row| row.address == request.address) {
                if prior != request {
                    return Err(StructuredLifecycleDiscoveryErrorV1::ConflictingRequest(
                        request.address,
                    ));
                }
            } else {
                canonical.push(*request);
            }
        }
        let mut missing = Vec::new();
        for request in canonical {
            match self.get(request.address) {
                None => missing.push(request),
                Some(row) if row.request == request => {}
                // A full observed account can satisfy a header request; a truncated
                // header can never satisfy a full semantic-account request.
                Some(row) if row.request.data_slice.is_none() => {}
                Some(_) => missing.push(request),
            }
        }
        Ok(missing)
    }

    /// Native-requested untrusted candidate inventory, if one was acquired.
    pub fn scan(
        &self,
        request: &StructuredLifecycleProgramScanV1,
    ) -> Option<&'a StructuredLifecycleScanResultV1> {
        self.snapshot
            .scans
            .iter()
            .find(|scan| scan.request == *request)
    }

    /// Authenticate exact raw bytes and their content-derived staging vacancy.
    pub fn finalized_record(
        &self,
        registry: [u8; 32],
        key: StructuredLifecycleRecordKeyV1,
        unix_timestamp: i64,
    ) -> Result<StructuredLifecycleFinalizedRecordV1, StructuredLifecycleDiscoveryErrorV1> {
        let raw = self.observed(key.raw, unix_timestamp)?;
        let staging = self.observed(key.staging, unix_timestamp)?;
        if hash(&raw.data).to_bytes() != key.content {
            return Err(StructuredLifecycleDiscoveryErrorV1::RecordDigest(key.raw));
        }
        authenticate_finalized_record(
            Pubkey::new_from_array(registry),
            &raw,
            &FinalizedRecordProof {
                schema_release_id: key.schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            StructuredLifecycleDiscoveryErrorV1::RecordAuthentication(key.raw, error)
        })?;
        Ok(StructuredLifecycleFinalizedRecordV1 { key, raw, staging })
    }
}

/// Derive raw/staging coordinates through the retained Registry seed owner.
pub fn structured_lifecycle_record_key_v1(
    registry: [u8; 32],
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<StructuredLifecycleRecordKeyV1, StructuredLifecycleDiscoveryErrorV1> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).map_err(StructuredLifecycleDiscoveryErrorV1::RecordKey)?,
        ContentDigest::new(content).map_err(StructuredLifecycleDiscoveryErrorV1::RecordKey)?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &Pubkey::new_from_array(registry),
        )
        .0
        .to_bytes()
    };
    Ok(StructuredLifecycleRecordKeyV1 {
        schema,
        content,
        raw: derive(key.raw_record_pda_seeds()),
        staging: derive(key.staging_cursor_pda_seeds()),
    })
}

impl StructuredLifecycleRecordKeyV1 {
    /// Both full observations required to authenticate this immutable record.
    pub fn requests(self) -> [StructuredLifecycleAccountRequestV1; 2] {
        [self.raw, self.staging].map(|address| StructuredLifecycleAccountRequestV1 {
            address,
            data_slice: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(address: [u8; 32], owner: [u8; 32], data: Vec<u8>) -> StructuredLifecycleAccountV1 {
        StructuredLifecycleAccountV1 {
            request: StructuredLifecycleAccountRequestV1 {
                address,
                data_slice: None,
            },
            value: Some(StructuredLifecycleAccountValueV1 {
                owner,
                lamports: 1,
                executable: false,
                space: u64::try_from(data.len()).expect("fixture width"),
                data,
            }),
        }
    }

    #[test]
    fn snapshot_duplicates_and_incomplete_full_images_refuse_exactly() {
        let row = account([1; 32], [2; 32], vec![3]);
        let mut snapshot = StructuredLifecycleSnapshotV1 {
            slot: 10,
            accounts: vec![row.clone(), row],
            scans: vec![],
        };
        assert!(
            matches!(StructuredLifecycleCorpusV1::new(&snapshot), Err(StructuredLifecycleDiscoveryErrorV1::DuplicateAddress(key)) if key == [1; 32])
        );
        snapshot.accounts.pop();
        snapshot.accounts[0].value.as_mut().expect("present").space = 2;
        assert!(
            matches!(StructuredLifecycleCorpusV1::new(&snapshot), Err(StructuredLifecycleDiscoveryErrorV1::DataWindow(key)) if key == [1; 32])
        );
        snapshot.accounts[0].value.as_mut().expect("present").space = 1;
        StructuredLifecycleCorpusV1::new(&snapshot).expect("complete unique corpus");
    }

    #[test]
    fn sliced_state_never_satisfies_full_semantic_account_request() {
        let mut row = account([1; 32], [2; 32], vec![3]);
        row.value.as_mut().expect("present").space = 20;
        row.request.data_slice = Some(StructuredLifecycleDataSliceV1 {
            offset: 0,
            length: 1,
        });
        let snapshot = StructuredLifecycleSnapshotV1 {
            slot: 10,
            accounts: vec![row],
            scans: vec![],
        };
        let corpus = StructuredLifecycleCorpusV1::new(&snapshot).expect("explicit slice");
        let full = StructuredLifecycleAccountRequestV1 {
            address: [1; 32],
            data_slice: None,
        };
        assert_eq!(
            corpus.full([1; 32]),
            Err(StructuredLifecycleDiscoveryErrorV1::SlicedAccount([1; 32]))
        );
        assert_eq!(corpus.missing(&[full, full]), Ok(vec![full]));
        assert_eq!(
            corpus.missing(&[full, snapshot.accounts[0].request]),
            Err(StructuredLifecycleDiscoveryErrorV1::ConflictingRequest(
                [1; 32]
            ))
        );
        assert_eq!(corpus.missing(&[snapshot.accounts[0].request]), Ok(vec![]));
    }

    #[test]
    fn finalized_record_checks_selected_owner_digest_and_staging_vacancy() {
        let registry = [1; 32];
        let data = vec![7, 8, 9];
        let key = structured_lifecycle_record_key_v1(registry, [2; 32], hash(&data).to_bytes())
            .expect("record seeds");
        let mut snapshot = StructuredLifecycleSnapshotV1 {
            slot: 10,
            accounts: vec![
                account(key.raw, registry, data),
                StructuredLifecycleAccountV1 {
                    request: key.requests()[1],
                    value: None,
                },
            ],
            scans: vec![],
        };
        StructuredLifecycleCorpusV1::new(&snapshot)
            .expect("corpus")
            .finalized_record(registry, key, 100)
            .expect("exact raw and absent staging");
        snapshot.accounts[0].value.as_mut().expect("raw").owner = [3; 32];
        assert_eq!(
            StructuredLifecycleCorpusV1::new(&snapshot)
                .expect("corpus")
                .finalized_record(registry, key, 100),
            Err(StructuredLifecycleDiscoveryErrorV1::RecordAuthentication(
                key.raw,
                ObservationError::InvalidOwner
            ))
        );
        snapshot.accounts[0].value.as_mut().expect("raw").owner = registry;
        snapshot.accounts[1] = account(key.staging, system_program::ID.to_bytes(), vec![1]);
        assert_eq!(
            StructuredLifecycleCorpusV1::new(&snapshot)
                .expect("corpus")
                .finalized_record(registry, key, 100),
            Err(StructuredLifecycleDiscoveryErrorV1::RecordAuthentication(
                key.raw,
                ObservationError::AddressMismatch
            ))
        );
        snapshot.accounts[1].value = None;
        snapshot.accounts[0].value.as_mut().expect("raw").data[0] ^= 1;
        assert_eq!(
            StructuredLifecycleCorpusV1::new(&snapshot)
                .expect("corpus")
                .finalized_record(registry, key, 100),
            Err(StructuredLifecycleDiscoveryErrorV1::RecordDigest(key.raw))
        );
    }
}
