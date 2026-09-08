//! Per-occurrence derivation of the canonical Series Prepare child bank.
//!
//! The immutable Prepare Effect carries scalar words derived by this module.
//! Callers supply authenticated records and physical observations, never child
//! request bytes or their digests. The physical runtime must still prove that
//! the supplied root header and Product/Source projection came from the current
//! transaction accounts. The later Projected Custody child independently
//! authenticates Core's actual receipt and return-data producer.

extern crate alloc;

use alloc::{boxed::Box, vec};

use dclutch_custody::CustodyRequestV1;
use dclutch_market::{
    Action, Identity, MarketIdentity, ProjectFoundError, ProjectFoundReceiptProjectionV2,
    ProjectFoundReceiptV2, Request, derive_project_found_receipt_v2,
};
use dclutch_trading::series::{
    AccountKeyV3, AdmittedOccurrenceV3, AuthenticatedProductProjectionV2,
    PrefoundingSeriesEscrowV3, admit_occurrence, admit_ticket,
    escrow::{PrepareSeriesEscrowPlanV3, prepare_series_escrow_v3},
    pre_founding_series_escrow,
    replay::SeriesStateV3,
};
use solana_program::hash::hash;

use super::{
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3,
        SERIES_PREPARE_IR_REQUEST_BYTES_V3, SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3,
        SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3, SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3,
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

/// Exact number of scalars carrying the native Prepare root successor.
pub const SERIES_PREPARE_DERIVED_REPLAY_SCALAR_COUNT_V1: usize = 3;

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
    parent_root: [u8; 32],
    successor: SeriesStateV3,
    requests: Box<[u8]>,
}

impl SeriesPrepareDerivedRequestsV1 {
    /// Borrow the projected-replay initialization request.
    pub fn projected_initialize(&self) -> &[u8] {
        self.request(
            SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        )
    }

    /// Borrow the projected-Hoard open request.
    pub fn projected_open(&self) -> &[u8] {
        self.request(
            SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        )
    }

    /// Borrow the normal Custody replay initialization request.
    pub fn replay_initialize(&self) -> &[u8] {
        self.request(
            SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3,
            SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        )
    }

    /// Borrow the normal SeriesEscrow Vault-open request.
    pub fn escrow_open(&self) -> &[u8] {
        self.request(
            SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3,
            SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        )
    }

    /// Borrow the founder-to-SeriesEscrow transfer request.
    pub fn escrow_lock(&self) -> &[u8] {
        self.request(
            SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        )
    }

    /// Write the complete request bank as ordered little-endian scalar words.
    ///
    /// `output` is exactly the appended request-word region, excluding the
    /// pre-existing Prepare scalars. Failure leaves the destination unchanged.
    pub fn write_scalar_words(
        &self,
        family_request: &[u8],
        authenticated_parent_root: [u8; 32],
        output: &mut [u64],
    ) -> Result<()> {
        self.require_binding(family_request, authenticated_parent_root)?;
        if output.len() != SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1 {
            return Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry);
        }
        for (word, chunk) in output.iter_mut().zip(self.requests.chunks_exact(8)) {
            *word = u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
        }
        Ok(())
    }

    /// Write the native Prepare root successor as prepared, outstanding, and revision.
    ///
    /// `output` is exactly the three appended replay scalars. Failure leaves
    /// the destination unchanged.
    pub fn write_replay_scalars(
        &self,
        family_request: &[u8],
        authenticated_parent_root: [u8; 32],
        output: &mut [u64],
    ) -> Result<()> {
        self.require_binding(family_request, authenticated_parent_root)?;
        if output.len() != SERIES_PREPARE_DERIVED_REPLAY_SCALAR_COUNT_V1 {
            return Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry);
        }
        let successor = [
            u64::from(self.successor.current_ticket_prepared()),
            u64::from(self.successor.outstanding_ticket_accounts()),
            self.successor.revision(),
        ];
        output.copy_from_slice(&successor);
        Ok(())
    }

    fn require_binding(
        &self,
        family_request: &[u8],
        authenticated_parent_root: [u8; 32],
    ) -> Result<()> {
        if hash(family_request).to_bytes() != self.family_request_digest {
            return Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest);
        }
        if authenticated_parent_root != self.parent_root {
            return Err(SeriesPrepareDerivedRequestErrorV1::Physical);
        }
        Ok(())
    }

    fn request(&self, offset: usize, width: usize) -> &[u8] {
        offset
            .checked_add(width)
            .and_then(|end| self.requests.get(offset..end))
            .unwrap_or(&[])
    }
}

/// Derive the exact five-route Prepare request bank from authenticated facts.
pub fn derive_series_prepare_requests_v1(
    input: SeriesPrepareDerivedRequestInputV1<'_>,
) -> Result<SeriesPrepareDerivedRequestsV1> {
    derive_series_prepare_requests_inner(&input)
}

#[inline(never)]
fn derive_series_prepare_requests_inner(
    input: &SeriesPrepareDerivedRequestInputV1<'_>,
) -> Result<SeriesPrepareDerivedRequestsV1> {
    require_canonical_family(input.snapshot, input.family_request)?;
    require_physical_agreement(input.parent_root, &input.custody, &input.projected_custody)?;
    let context = derive_prepare_context(input.snapshot, input.product, input.registry_program)?;
    let receipt_digest = derive_project_found_receipt_digest(
        context.escrow.future_market().identity(),
        input.projected_custody,
        input.principal_cap_sets,
    )?;

    let family_digest = hash(input.family_request).to_bytes();
    let mut custody = input.custody;
    custody.parent_request_digest = family_digest;
    let mut projected = input.projected_custody;
    projected.projection_receipt_digest = receipt_digest;
    let mut requests = vec![0_u8; SERIES_PREPARE_IR_REQUEST_BYTES_V3].into_boxed_slice();
    write_projected_initialize(*context.escrow, context.expiry, projected, &mut requests)?;
    write_projected_open(*context.escrow, context.expiry, projected, &mut requests)?;
    write_normal_prepare_requests(
        prepare_series_escrow_v3(*context.escrow),
        custody,
        &mut requests,
    )?;

    Ok(SeriesPrepareDerivedRequestsV1 {
        family_request_digest: family_digest,
        parent_root: input.parent_root,
        successor: context.successor,
        requests,
    })
}

struct PrepareDerivationContextV1 {
    escrow: Box<PrefoundingSeriesEscrowV3>,
    expiry: u64,
    successor: SeriesStateV3,
}

#[inline(never)]
fn require_canonical_family(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
    family_request: &[u8],
) -> Result<()> {
    let expected =
        build_prepare_v3(snapshot).map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    if expected.as_bytes() != family_request {
        return Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest);
    }
    Ok(())
}

#[inline(never)]
fn derive_prepare_context(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
) -> Result<PrepareDerivationContextV1> {
    let occurrence = derive_admitted_occurrence(snapshot)?;
    let expiry = occurrence
        .template()
        .retry_through(occurrence.occurrence().occurrence())
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    let escrow =
        derive_prefounding_escrow(occurrence, snapshot.ticket_bytes, product, registry_program)?;
    let successor = snapshot
        .series
        .prepare_ticket(snapshot.series.revision())
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    Ok(PrepareDerivationContextV1 {
        escrow,
        expiry,
        successor,
    })
}

#[inline(never)]
fn derive_admitted_occurrence(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
) -> Result<AdmittedOccurrenceV3> {
    admit_occurrence(
        snapshot.template_bytes,
        snapshot.occurrence_bytes,
        snapshot.siblings,
    )
    .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)
}

#[inline(never)]
/// Admit one Ticket against an already admitted occurrence and derive its escrow projection.
pub(crate) fn derive_prefounding_escrow(
    occurrence: AdmittedOccurrenceV3,
    ticket_bytes: &[u8],
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
) -> Result<Box<PrefoundingSeriesEscrowV3>> {
    let ticket =
        admit_ticket(ticket_bytes).map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    let escrow = pre_founding_series_escrow(occurrence, ticket, product, registry_program)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Content)?;
    Ok(Box::new(escrow))
}

#[inline(never)]
fn derive_project_found_receipt_digest(
    future: MarketIdentity,
    projected: SeriesProjectedCustodyPhysicalV3,
    principal_cap_sets: u64,
) -> Result<[u8; 32]> {
    let receipt = derive_series_project_found_receipt_v1(future, projected, principal_cap_sets)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)?;
    let bytes = receipt
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)?;
    Ok(hash(&bytes).to_bytes())
}

#[inline(never)]
fn write_projected_initialize(
    escrow: dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<()> {
    let request = project_prepare_initialize_v3(escrow, expiry, physical)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    copy_request(
        output,
        SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3,
        &request,
    )
}

#[inline(never)]
fn write_projected_open(
    escrow: dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<()> {
    let request = project_prepare_open_hoard_v3(escrow, expiry, physical)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?
        .encode()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    copy_request(output, SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3, &request)
}

#[inline(never)]
fn write_normal_prepare_requests(
    plan: PrepareSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<()> {
    let [replay_initialize, escrow_open, escrow_lock] = project_prepare_custody_v3(plan, physical)
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    write_normal_request(
        replay_initialize,
        SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3,
        output,
    )?;
    write_normal_request(escrow_open, SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3, output)?;
    write_normal_request(
        escrow_lock,
        SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        output,
    )
}

#[inline(never)]
fn write_normal_request(request: CustodyRequestV1, offset: usize, output: &mut [u8]) -> Result<()> {
    let encoded = request
        .to_bytes()
        .map_err(|_| SeriesPrepareDerivedRequestErrorV1::Custody)?;
    copy_request(output, offset, &encoded)
}

fn copy_request(output: &mut [u8], offset: usize, request: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(request.len())
        .ok_or(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)?;
    output
        .get_mut(offset..end)
        .ok_or(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)?
        .copy_from_slice(request);
    Ok(())
}

#[inline(never)]
fn require_physical_agreement(
    parent_root: [u8; 32],
    custody: &SeriesCustodyPhysicalV3,
    projected: &SeriesProjectedCustodyPhysicalV3,
) -> Result<()> {
    if parent_root == [0; 32]
        || projected.parent_capability_root != parent_root
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

/// Derive the ProjectFound receipt Core must return for one admitted Series
/// future Market and authenticated physical/Source projection.
///
/// This is an expected receipt, constructed before the child invocation. The
/// Projected Custody child still authenticates the actual Core return-data
/// producer and exact returned bytes during that invocation.
pub fn derive_series_project_found_receipt_v1(
    future: MarketIdentity,
    projected: SeriesProjectedCustodyPhysicalV3,
    principal_cap_sets: u64,
) -> core::result::Result<ProjectFoundReceiptV2, ProjectFoundError> {
    let found = Request::administrative(Action::Found, future.generation, future.market_id);
    derive_project_found_receipt_v2(
        found,
        ProjectFoundReceiptProjectionV2 {
            realm: future.realm_id,
            collateral_mint: Identity::new(projected.mint)
                .map_err(|_| ProjectFoundError::ZeroIdentity)?,
            token_program: Identity::new(projected.token_program)
                .map_err(|_| ProjectFoundError::ZeroIdentity)?,
            collateral_release: Identity::new(projected.collateral_release)
                .map_err(|_| ProjectFoundError::ZeroIdentity)?,
            product_record: future.product_record,
            product: future.product_id,
            source: future.resolution_policy,
            release_set: future.selected_release_set,
            rent_program: Identity::new(projected.rent_program)
                .map_err(|_| ProjectFoundError::ZeroIdentity)?,
            principal_cap_sets,
        },
    )
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
        future: MarketIdentity,
        custody: SeriesCustodyPhysicalV3,
        projected: SeriesProjectedCustodyPhysicalV3,
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
            Self {
                template,
                occurrence,
                ticket,
                siblings,
                series,
                product,
                registry,
                future,
                custody,
                projected,
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

        let found = Request::administrative(
            Action::Found,
            fixture.future.generation,
            fixture.future.market_id,
        )
        .encode()
        .expect("Found request");
        let receipt = derive_series_project_found_receipt_v1(fixture.future, fixture.projected, 23)
            .expect("expected ProjectFound receipt");
        let oracle = ProjectFoundReceiptV2::new(
            fixture.future.market_id,
            fixture.future.generation,
            fixture.future.realm_id,
            ident(fixture.projected.mint),
            ident(fixture.projected.token_program),
            ident(fixture.projected.collateral_release),
            fixture.future.product_record,
            fixture.future.product_id,
            fixture.future.resolution_policy,
            fixture.future.selected_release_set,
            ident(fixture.projected.rent_program),
            23,
            hash(&found).to_bytes(),
        )
        .expect("Core receipt oracle");
        assert_eq!(receipt, oracle);

        let mut words = [0_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
        let family = build_prepare_v3(fixture.snapshot(100)).expect("canonical Prepare request");
        early
            .write_scalar_words(
                family.as_bytes(),
                fixture.projected.parent_capability_root,
                &mut words,
            )
            .expect("exact scalar bank");
        let encoded = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<std::vec::Vec<_>>();
        let expected = [
            early.projected_initialize(),
            early.projected_open(),
            early.replay_initialize(),
            early.escrow_open(),
            early.escrow_lock(),
        ]
        .concat();
        assert_eq!(encoded, expected);
        assert_eq!(encoded.len(), SERIES_PREPARE_IR_REQUEST_BYTES_V3);

        let mut replay = [0_u64; SERIES_PREPARE_DERIVED_REPLAY_SCALAR_COUNT_V1];
        early
            .write_replay_scalars(
                family.as_bytes(),
                fixture.projected.parent_capability_root,
                &mut replay,
            )
            .expect("native replay successor");
        assert_eq!(replay, [1, 1, fixture.series.revision() + 1]);
    }

    #[test]
    fn family_root_product_projection_and_scalar_substitution_refuse_exactly() {
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
                principal_cap_sets: 0,
                ..input
            }),
            Err(SeriesPrepareDerivedRequestErrorV1::ProjectFoundReceipt)
        );

        let bank = fixture.derive(100).expect("canonical bank");
        let mut wrong_width = [7_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1 - 1];
        assert_eq!(
            bank.write_scalar_words(
                family.as_bytes(),
                fixture.projected.parent_capability_root,
                &mut wrong_width,
            ),
            Err(SeriesPrepareDerivedRequestErrorV1::ScalarGeometry)
        );
        assert!(wrong_width.iter().all(|word| *word == 7));

        let mut stale_words = [9_u64; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
        assert_eq!(
            bank.write_scalar_words(
                &substituted_family,
                fixture.projected.parent_capability_root,
                &mut stale_words,
            ),
            Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest)
        );
        assert!(stale_words.iter().all(|word| *word == 9));

        assert_eq!(
            bank.write_scalar_words(family.as_bytes(), [98; 32], &mut stale_words),
            Err(SeriesPrepareDerivedRequestErrorV1::Physical)
        );
        assert!(stale_words.iter().all(|word| *word == 9));

        let mut stale_replay = [11_u64; SERIES_PREPARE_DERIVED_REPLAY_SCALAR_COUNT_V1];
        assert_eq!(
            bank.write_replay_scalars(family.as_bytes(), [98; 32], &mut stale_replay),
            Err(SeriesPrepareDerivedRequestErrorV1::Physical)
        );
        assert!(stale_replay.iter().all(|word| *word == 11));
    }
}
