//! Per-occurrence derivation of the canonical Series Prepare child bank.
//!
//! The immutable Prepare Effect carries scalar words derived by this module.
//! Callers supply authenticated records and physical observations, never child
//! request bytes or their digests. The physical runtime must still prove that
//! the supplied root header, Product/Source projection, and immediate Core
//! receipt came from the current transaction accounts.

use dclutch_market::{Action, ProjectFoundReceiptV2, Request};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, admit_occurrence, admit_ticket,
    escrow::prepare_series_escrow_v3, pre_founding_series_escrow,
};
use solana_program::hash::hash;

use super::{
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PREPARE_IR_REQUEST_BYTES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    custody_v3::{SeriesCustodyPhysicalV3, project_prepare_custody_v3},
    operator::{SeriesOccurrenceSnapshotV3, build_prepare_v3},
    projected_custody_v3::{
        SeriesProjectedCustodyPhysicalV3, project_prepare_initialize_v3,
        project_prepare_open_hoard_v3,
    },
};

/// Exact number of little-endian scalar words carrying the Prepare child bank.
pub const SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1: usize =
    SERIES_PREPARE_IR_REQUEST_BYTES_V3 / 8;

const _: () = assert!(SERIES_PREPARE_IR_REQUEST_BYTES_V3 % 8 == 0);

/// Authenticated inputs required to derive one occurrence's Prepare children.
#[derive(Clone, Copy)]
pub struct SeriesPrepareDerivedRequestInputV1<'a> {
    /// Exact family request that the current Trading instruction executes.
    pub family_request: &'a [u8],
    /// Same-snapshot immutable records, replay state, proof, and Clock guard.
    pub snapshot: SeriesOccurrenceSnapshotV3<'a>,
    /// Product graph projection authenticated against the occurrence.
    pub product: AuthenticatedProductProjectionV2,
    /// Current Registry program identity used by the future-Market projection.
    pub registry_program: AccountKeyV3,
    /// Current authenticated Trading capability-root account identity.
    pub parent_root: [u8; 32],
    /// Normal Custody observations. Its parent digest is derived here.
    pub custody: SeriesCustodyPhysicalV3,
    /// Projected Custody observations. Its receipt digest is derived here.
    pub projected_custody: SeriesProjectedCustodyPhysicalV3,
    /// Immediate typed Core ProjectFound receipt.
    pub project_found_receipt: ProjectFoundReceiptV2,
    /// Source-owner projection of the canonical principal ceiling.
    pub principal_cap_sets: u64,
}

/// Stable refusal from authenticated Prepare child derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPrepareDerivedRequestErrorV1 {
    /// The supplied family bytes were not the canonical request for the snapshot.
    FamilyRequest,
    /// Template, occurrence, Ticket, replay, schedule, or Product admission refused.
    Content,
    /// The two physical Custody projections did not describe one execution.
    Physical,
    /// The ProjectFound receipt did not bind the canonical future Market.
    ProjectFoundReceipt,
    /// A canonical Custody request could not be projected or encoded.
    Custody,
    /// The scalar destination had another exact width.
    ScalarGeometry,
}

/// Result alias for Prepare request derivation.
pub type Result<T> = core::result::Result<T, SeriesPrepareDerivedRequestErrorV1>;

/// Private authenticated bank of the five canonical Prepare child requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesPrepareDerivedRequestsV1 {
    family_request_digest: [u8; 32],
    projected_initialize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    projected_open: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    replay_initialize: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    escrow_open: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    escrow_lock: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
}

impl SeriesPrepareDerivedRequestsV1 {
    /// Borrow the projected-replay initialization request.
    pub const fn projected_initialize(&self) -> &[u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3] {
        &self.projected_initialize
    }

    /// Borrow the projected-Hoard open request.
    pub const fn projected_open(&self) -> &[u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3] {
        &self.projected_open
    }

    /// Borrow the normal Custody replay initialization request.
    pub const fn replay_initialize(&self) -> &[u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3] {
        &self.replay_initialize
    }

    /// Borrow the normal SeriesEscrow Vault-open request.
    pub const fn escrow_open(&self) -> &[u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3] {
        &self.escrow_open
    }

    /// Borrow the founder-to-SeriesEscrow transfer request.
    pub const fn escrow_lock(&self) -> &[u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3] {
        &self.escrow_lock
    }

    /// Write the complete request bank as ordered little-endian scalar words.
    ///
    /// `output` is exactly the appended request-word region, excluding the
    /// pre-existing Prepare scalars. Failure leaves the destination unchanged.
    pub fn write_scalar_words(&self, family_request: &[u8], output: &mut [u64]) -> Result<()> {
        if hash(family_request).to_bytes() != self.family_request_digest {
            return Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest);
        }
        if output.len() != SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1 {
            return Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry);
        }
        let mut scratch = [0_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
        let mut destination = scratch.iter_mut();
        for request in self.requests() {
            for chunk in request.chunks_exact(8) {
                let word = destination
                    .next()
                    .ok_or(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)?;
                let bytes: [u8; 8] = chunk
                    .try_into()
                    .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)?;
                *word = u64::from_le_bytes(bytes);
            }
        }
        if destination.next().is_some() {
            return Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry);
        }
        output.copy_from_slice(&scratch);
        Ok(())
    }

    fn requests(&self) -> [&[u8]; 5] {
        [
            &self.projected_initialize,
            &self.projected_open,
            &self.replay_initialize,
            &self.escrow_open,
            &self.escrow_lock,
        ]
    }
}

/// Derive the exact five-route Prepare request bank from authenticated facts.
pub fn derive_series_prepare_requests_v1(
    input: SeriesPrepareDerivedRequestInputV1<'_>,
) -> Result<SeriesPrepareDerivedRequestsV1> {
    let expected = build_prepare_v3(input.snapshot)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    if expected.as_bytes() != input.family_request {
        return Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest);
    }
    let occurrence = admit_occurrence(
        input.snapshot.template_bytes,
        input.snapshot.occurrence_bytes,
        input.snapshot.siblings,
    )
    .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    let ticket = admit_ticket(input.snapshot.ticket_bytes)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    let escrow =
        pre_founding_series_escrow(occurrence, ticket, input.product, input.registry_program)
            .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;

    require_physical_agreement(input)?;
    require_project_found_receipt(input, escrow.future_market().identity())?;

    let family_digest = hash(input.family_request).to_bytes();
    let receipt_bytes = input
        .project_found_receipt
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)?;
    let receipt_digest = hash(&receipt_bytes).to_bytes();
    let mut custody = input.custody;
    custody.parent_request_digest = family_digest;
    let mut projected = input.projected_custody;
    projected.projection_receipt_digest = receipt_digest;
    let expiry = occurrence
        .template()
        .retry_through(escrow.occurrence())
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;

    let projected_initialize = project_prepare_initialize_v3(escrow, expiry, projected)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    let projected_open = project_prepare_open_hoard_v3(escrow, expiry, projected)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    let [replay_initialize, escrow_open, escrow_lock] =
        project_prepare_custody_v3(prepare_series_escrow_v3(escrow), custody)
            .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;

    Ok(SeriesPrepareDerivedRequestsV1 {
        family_request_digest: family_digest,
        projected_initialize,
        projected_open,
        replay_initialize: replay_initialize
            .to_bytes()
            .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?,
        escrow_open: escrow_open
            .to_bytes()
            .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?,
        escrow_lock: escrow_lock
            .to_bytes()
            .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?,
    })
}

fn require_physical_agreement(input: SeriesPrepareDerivedRequestInputV1<'_>) -> Result<()> {
    let custody = input.custody;
    let projected = input.projected_custody;
    if input.parent_root == [0; 32]
        || projected.parent_capability_root != input.parent_root
        || custody.caller_program != projected.caller_program
        || custody.payer != projected.payer
        || custody.mint != projected.mint
        || custody.token_program != projected.token_program
        || custody.escrow_vault != projected.escrow_vault
        || custody.hoard_vault != projected.hoard_vault
        || custody.rent_credit != projected.rent_credit
        || custody.replay_rent_lamports != projected.escrow_replay_rent_lamports
        || custody.vault_rent_lamports != projected.escrow_vault_rent_lamports
    {
        return Err(SeriesPrepareDerivedRequestErrorV1::Physical);
    }
    Ok(())
}

fn require_project_found_receipt(
    input: SeriesPrepareDerivedRequestInputV1<'_>,
    future: dclutch_market::MarketIdentity,
) -> Result<()> {
    let receipt = input.project_found_receipt;
    let projected = input.projected_custody;
    let found = Request::administrative(Action::Found, future.generation, future.market_id)
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)?;
    receipt
        .verify_found_request(hash(&found).to_bytes())
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)?;
    if receipt.market != future.market_id
        || receipt.generation != future.generation
        || receipt.realm != future.realm_id
        || receipt.product_record != future.product_record
        || receipt.product != future.product_id
        || receipt.source != future.resolution_policy
        || receipt.release_set != future.selected_release_set
        || receipt.collateral_mint.to_bytes() != projected.mint
        || receipt.token_program.to_bytes() != projected.token_program
        || receipt.collateral_release.to_bytes() != projected.collateral_release
        || receipt.rent_program.to_bytes() != projected.rent_program
        || receipt.principal_cap_sets != input.principal_cap_sets
    {
        return Err(SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_core_contract::ContentId;
    use dclutch_market::{Identity, ProjectFoundReceiptV2};
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, generated,
        occurrence_content_id, replay::SeriesStateV3, template_content_id,
    };
    use solana_program::hash::hashv;

    use super::*;

    const HASH_SEPARATOR: [u8; 1] = [0];

    struct Fixture {
        template: [u8; SERIES_TEMPLATE_BYTES_V3],
        occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3],
        ticket: [u8; SERIES_TICKET_BYTES_V3],
        siblings: [[u8; 32]; 2],
        series: SeriesStateV3,
        product: AuthenticatedProductProjectionV2,
        registry: AccountKeyV3,
        custody: SeriesCustodyPhysicalV3,
        projected: SeriesProjectedCustodyPhysicalV3,
        receipt: ProjectFoundReceiptV2,
    }

    impl Fixture {
        fn new() -> Self {
            let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
            let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
            let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
            let siblings = [[90; 32], [91; 32]];
            let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
            put(
                &mut template,
                generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
                &projection_root(occurrence_id, 1, &siblings),
            );
            let template_id = template_content_id(&template).expect("Template ID");
            put(
                &mut ticket,
                generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
                &template_id.to_bytes(),
            );
            put(
                &mut ticket,
                generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
                &occurrence_id.to_bytes(),
            );
            let series = SeriesStateV3::new(7)
                .prepare_ticket(0)
                .expect("prepare occurrence zero")
                .settle_current(1, 3)
                .expect("settle occurrence zero")
                .retire_ticket(2)
                .expect("retire occurrence zero");
            let admitted = admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
            let product = AuthenticatedProductProjectionV2::new(
                admitted.occurrence().product_record(),
                cid(61),
                cid(62),
            );
            let registry = key(59);
            let escrow = pre_founding_series_escrow(
                admitted,
                admit_ticket(&ticket).expect("Ticket"),
                product,
                registry,
            )
            .expect("escrow");
            let custody = SeriesCustodyPhysicalV3 {
                caller_program: [1; 32],
                parent_request_digest: [2; 32],
                payer: [3; 32],
                mint: [4; 32],
                token_program: [5; 32],
                founder_source: [6; 32],
                escrow_vault: [7; 32],
                hoard_vault: [8; 32],
                refund_destination: [9; 32],
                rent_credit: [10; 32],
                replay_rent_lamports: 11,
                vault_rent_lamports: 12,
            };
            let projected = SeriesProjectedCustodyPhysicalV3 {
                caller_program: custody.caller_program,
                core_program: [13; 32],
                rent_program: [14; 32],
                parent_capability_root: [15; 32],
                projection_receipt_digest: [16; 32],
                payer: custody.payer,
                rent_credit: custody.rent_credit,
                hoard_vault: custody.hoard_vault,
                escrow_vault: custody.escrow_vault,
                mint: custody.mint,
                token_program: custody.token_program,
                collateral_release: [17; 32],
                projected_state_rent_lamports: 18,
                hoard_vault_rent_lamports: 19,
                escrow_replay_rent_lamports: custody.replay_rent_lamports,
                escrow_vault_rent_lamports: custody.vault_rent_lamports,
            };
            let future = escrow.future_market().identity();
            let found = Request::administrative(Action::Found, future.generation, future.market_id)
                .encode()
                .expect("Found request");
            let receipt = ProjectFoundReceiptV2::new(
                future.market_id,
                future.generation,
                future.realm_id,
                ident(projected.mint),
                ident(projected.token_program),
                ident(projected.collateral_release),
                future.product_record,
                future.product_id,
                future.resolution_policy,
                future.selected_release_set,
                ident(projected.rent_program),
                23,
                hash(&found).to_bytes(),
            )
            .expect("ProjectFound receipt");
            Self {
                template,
                occurrence,
                ticket,
                siblings,
                series,
                product,
                registry,
                custody,
                projected,
                receipt,
            }
        }

        fn snapshot(&self, now_slot: u64) -> SeriesOccurrenceSnapshotV3<'_> {
            SeriesOccurrenceSnapshotV3 {
                template_bytes: &self.template,
                occurrence_bytes: &self.occurrence,
                ticket_bytes: &self.ticket,
                siblings: &self.siblings,
                series: self.series,
                ticket_state: None,
                now_slot,
            }
        }

        fn derive(&self, now_slot: u64) -> Result<SeriesPrepareDerivedRequestsV1> {
            let snapshot = self.snapshot(now_slot);
            let family = build_prepare_v3(snapshot).expect("canonical Prepare request");
            derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
                family_request: family.as_bytes(),
                snapshot,
                product: self.product,
                registry_program: self.registry,
                parent_root: self.projected.parent_capability_root,
                custody: self.custody,
                projected_custody: self.projected,
                project_found_receipt: self.receipt,
                principal_cap_sets: 23,
            })
        }
    }

    fn cid(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content identity")
    }

    fn key(value: u8) -> AccountKeyV3 {
        AccountKeyV3::new([value; 32]).expect("account identity")
    }

    fn ident(value: [u8; 32]) -> Identity {
        Identity::new(value).expect("Market identity")
    }

    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
        bytes
            .get_mut(offset..offset + N)
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn projection_root(
        occurrence_id: ContentId,
        mut index: u32,
        siblings: &[[u8; 32]],
    ) -> [u8; 32] {
        let mut node = occurrence_id.to_bytes();
        for sibling in siblings {
            node = if index & 1 == 0 {
                hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &HASH_SEPARATOR,
                    &node,
                    sibling,
                ])
                .to_bytes()
            } else {
                hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &HASH_SEPARATOR,
                    sibling,
                    &node,
                ])
                .to_bytes()
            };
            index >>= 1;
        }
        node
    }

    #[test]
    fn canonical_prepare_bank_is_complete_and_clock_guard_is_not_authority() {
        let fixture = Fixture::new();
        let early = fixture.derive(100).expect("early valid Prepare");
        let retry_boundary = fixture.derive(115).expect("last valid Prepare");
        assert_eq!(early, retry_boundary);

        let mut words = [0_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
        let family = build_prepare_v3(fixture.snapshot(100)).expect("canonical Prepare request");
        early
            .write_scalar_words(family.as_bytes(), &mut words)
            .expect("exact scalar bank");
        let encoded = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<std::vec::Vec<_>>();
        let expected = [
            early.projected_initialize().as_slice(),
            early.projected_open().as_slice(),
            early.replay_initialize().as_slice(),
            early.escrow_open().as_slice(),
            early.escrow_lock().as_slice(),
        ]
        .concat();
        assert_eq!(encoded, expected);
        assert_eq!(encoded.len(), SERIES_PREPARE_IR_REQUEST_BYTES_V3);
    }

    #[test]
    fn family_root_product_receipt_and_scalar_substitution_refuse_exactly() {
        let fixture = Fixture::new();
        let snapshot = fixture.snapshot(100);
        let family = build_prepare_v3(snapshot).expect("canonical Prepare request");
        let mut substituted_family = family.as_bytes().to_vec();
        *substituted_family.last_mut().expect("proof byte") ^= 1;
        let input = SeriesPrepareDerivedRequestInputV1 {
            family_request: &substituted_family,
            snapshot,
            product: fixture.product,
            registry_program: fixture.registry,
            parent_root: fixture.projected.parent_capability_root,
            custody: fixture.custody,
            projected_custody: fixture.projected,
            project_found_receipt: fixture.receipt,
            principal_cap_sets: 23,
        };
        assert_eq!(
            derive_series_prepare_requests_v1(input),
            Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest)
        );
        assert_eq!(
            derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
                family_request: family.as_bytes(),
                parent_root: [99; 32],
                ..input
            }),
            Err(SeriesPrepareDerivedRequestErrorV1::Physical)
        );
        assert_eq!(
            derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
                family_request: family.as_bytes(),
                product: AuthenticatedProductProjectionV2::new(cid(99), cid(61), cid(62)),
                ..input
            }),
            Err(SeriesPrepareDerivedRequestErrorV1::Content)
        );
        assert_eq!(
            derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
                family_request: family.as_bytes(),
                principal_cap_sets: 24,
                ..input
            }),
            Err(SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)
        );

        let bank = fixture.derive(100).expect("canonical bank");
        let mut wrong_width = [7_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1 - 1];
        assert_eq!(
            bank.write_scalar_words(family.as_bytes(), &mut wrong_width),
            Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)
        );
        assert!(wrong_width.iter().all(|word| *word == 7));

        let mut stale_words = [9_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
        assert_eq!(
            bank.write_scalar_words(&substituted_family, &mut stale_words),
            Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest)
        );
        assert!(stale_words.iter().all(|word| *word == 9));
    }
}
