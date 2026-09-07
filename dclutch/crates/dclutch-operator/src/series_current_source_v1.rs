//! Production construction of a Series current-release source from a founder snapshot.
//!
//! A Series release is not a client-selected list of actions.  The Template,
//! realized occurrence and Ticket are admitted with their Merkle proof, the
//! replay tail selects Prepare, and the Series-to-Custody adapter derives the
//! three pre-founding Custody requests.  The remaining typed child requests
//! enter through the release owner's [`SeriesCurrentReleaseInputV5`]; its
//! canonical emitter authenticates their cross-action joins before returning
//! the only publishable five-action source bank.
//!
//! This module deliberately performs no RPC, account mutation, signing, or
//! submission.  A local-validator founder acquires finalized observations,
//! then supplies them here as one explicit input.  That keeps occurrence
//! leaves and proofs protocol evidence rather than a private fixture detail.

use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, TemplateV3, admit_occurrence, admit_ticket,
    escrow::prepare_series_escrow_v3, pre_founding_series_escrow, template_content_id,
};
use dclutch_trading_sbf::series::{
    activation_bundle_v1::{
        SeriesActivationBundleInputV1, build_series_activation_bundle_v1,
        build_series_activation_capable_program_set_v1,
    },
    custody_v3::{SeriesCustodyPhysicalV3, project_prepare_custody_v3},
    instruction::SeriesActionV3,
    projected_custody_v3::{
        SeriesProjectedCustodyPhysicalV3, project_prepare_initialize_v3,
        project_prepare_open_hoard_v3,
    },
    release_v5::{
        SeriesCurrentReleaseInputV5, SeriesOwnedReleaseSourceV5, SeriesReleaseV5,
        compile_series_release_v5, emit_current_series_release_source_v5,
    },
};
use solana_program::hash::hash;

use crate::series_lifecycle_v3::{
    SeriesCurrentOccurrenceV3, SeriesLifecycleSnapshotV3, SeriesNextActV3,
    inspect_series_lifecycle_v3,
};

/// Finalized founder evidence and physical observations for the first
/// Series `Found -> Prepare` vertical slice.
#[derive(Clone, Copy)]
pub struct SeriesFoundPrepareInputV1<'a> {
    /// Finalized immutable Template, root replay tail, current occurrence,
    /// immutable Ticket and its ordered Merkle proof.
    pub lifecycle: SeriesLifecycleSnapshotV3<'a>,
    /// Product Runtime V2 facts authenticated against the occurrence record.
    pub product: AuthenticatedProductProjectionV2,
    /// Current Registry program identity authenticated by the founder plan.
    pub registry_program: AccountKeyV3,
    /// Current Custody physical identities and Rent observations for Prepare.
    pub custody: SeriesCustodyPhysicalV3,
    /// Current projected-Custody physical identities and Rent observations for
    /// the same Prepare source.
    pub projected_custody: SeriesProjectedCustodyPhysicalV3,
    /// Typed child requests supplied by their semantic protocol owners.
    ///
    /// The canonical emitter owns validation of the projected-Custody, Core,
    /// Claims, Expire, Retire and Close joins; this constructor additionally
    /// proves that its three normal-Custody Prepare requests are the exact
    /// projection of the authenticated Series records and physical plan.
    pub release: SeriesCurrentReleaseInputV5<'a>,
}

/// Publishable five-action source bank and the exact first lifecycle request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFoundPrepareSourceV1 {
    source: SeriesOwnedReleaseSourceV5,
    prepare: Vec<u8>,
}

/// Activation-capable Series release material derived from the same founder
/// snapshot as the first Prepare request.
///
/// A host founder publishes the five action artifacts, the activation bundle,
/// and this six-entry ProgramSet before it can activate the Series composite
/// root. The initial root is therefore derived from the Template by the
/// canonical activation owner, not supplied by a launcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFoundPrepareActivationV1 {
    source: SeriesFoundPrepareSourceV1,
    release: SeriesReleaseV5,
    activation: dclutch_market::capability_activation::ActivationBundleV1,
    activation_program_set: Vec<u8>,
}

impl SeriesFoundPrepareActivationV1 {
    /// Borrow the validated five-action source bank.
    pub fn source(&self) -> &SeriesFoundPrepareSourceV1 {
        &self.source
    }

    /// Borrow the canonical five-action release compiler result.
    pub fn release(&self) -> &SeriesReleaseV5 {
        &self.release
    }

    /// Borrow the canonical activation descriptor/profile/effect bank.
    pub fn activation(&self) -> &dclutch_market::capability_activation::ActivationBundleV1 {
        &self.activation
    }

    /// Borrow the six-entry ProgramSet that admits both activation and the
    /// five lifecycle actions.
    pub fn activation_program_set(&self) -> &[u8] {
        self.activation_program_set.as_slice()
    }
}

impl SeriesFoundPrepareSourceV1 {
    /// Borrow the canonical five-action bank for compilation and selection.
    pub fn source(&self) -> &SeriesOwnedReleaseSourceV5 {
        &self.source
    }

    /// Return the canonical unsigned Prepare request selected from the same
    /// finalized snapshot that authenticated the source bank.
    pub fn prepare_request(&self) -> &[u8] {
        self.prepare.as_slice()
    }
}

/// Stable refusal from production Series current-source construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesCurrentSourceErrorV1 {
    /// Template, occurrence, Ticket, Merkle proof, or product join refused.
    Content,
    /// The root replay tail did not select a ready Prepare action.
    Lifecycle,
    /// Series-to-Custody projection refused the physical founder observations.
    Custody,
    /// A caller supplied a Prepare Custody request different from the
    /// canonical Series-to-Custody projection.
    PrepareCustody,
    /// The release identity or occurrence-count geometry differed from the
    /// admitted Template.
    ReleaseGeometry,
    /// The canonical five-action release emitter refused the typed child bank.
    Release,
    /// The canonical release compiler refused the emitted action bank.
    ReleaseCompilation,
    /// The canonical Series activation bundle refused the Template or action
    /// descriptor inherited from the compiled release.
    Activation,
}

/// Construct the complete current release source after a Series founder has
/// authenticated the root and its first occurrence proof.
///
/// The function has no action argument: the root tail must select `Prepare`.
/// It derives the normal-Custody initialization, vault-open, and lock requests
/// through the Series-owned escrow plan and the Custody adapter, then requires
/// the release input to carry those exact bytes before invoking the canonical
/// five-action emitter.
pub fn construct_series_found_prepare_source_v1(
    input: SeriesFoundPrepareInputV1<'_>,
) -> Result<SeriesFoundPrepareSourceV1, SeriesCurrentSourceErrorV1> {
    let current = input
        .lifecycle
        .current
        .ok_or(SeriesCurrentSourceErrorV1::Lifecycle)?;
    let template = TemplateV3::decode(input.lifecycle.template_bytes)
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    let template_id = template_content_id(input.lifecycle.template_bytes)
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    if input.release.template != template_id
        || input.release.template_occurrence_count != template.occurrence_count()
    {
        return Err(SeriesCurrentSourceErrorV1::ReleaseGeometry);
    }
    let report = inspect_series_lifecycle_v3(input.lifecycle)
        .map_err(|_| SeriesCurrentSourceErrorV1::Lifecycle)?;
    let prepare = match report.next() {
        SeriesNextActV3::Ready(plan) if plan.action() == SeriesActionV3::Prepare => plan,
        _ => return Err(SeriesCurrentSourceErrorV1::Lifecycle),
    };
    require_prepare_custody(
        input.lifecycle.template_bytes,
        current,
        input.product,
        input.registry_program,
        input.custody,
        input.projected_custody,
        input.release,
    )?;
    let source = emit_current_series_release_source_v5(input.release)
        .map_err(|_| SeriesCurrentSourceErrorV1::Release)?;
    Ok(SeriesFoundPrepareSourceV1 {
        source,
        prepare: prepare.request().as_bytes().to_vec(),
    })
}

/// Construct the publishable activation-capable release for a Series founder.
///
/// The five action descriptors are compiled only after
/// [`construct_series_found_prepare_source_v1`] has admitted the founder
/// snapshot. The activation descriptor inherits an action descriptor from
/// that release and re-derives the Template's initial root tail. It cannot be
/// joined to a substituted Template or to a caller-selected root state.
pub fn construct_series_found_prepare_activation_v1(
    input: SeriesFoundPrepareInputV1<'_>,
    funding_ledger_slot_count: u16,
) -> Result<SeriesFoundPrepareActivationV1, SeriesCurrentSourceErrorV1> {
    let template = input.lifecycle.template_bytes;
    let source = construct_series_found_prepare_source_v1(input)?;
    let release = compile_series_release_v5(source.source().as_source())
        .map_err(|_| SeriesCurrentSourceErrorV1::ReleaseCompilation)?;
    let activation = build_series_activation_bundle_v1(SeriesActivationBundleInputV1 {
        action_descriptor: &release.descriptors[0],
        template,
        funding_ledger_slot_count,
    })
    .map_err(|_| SeriesCurrentSourceErrorV1::Activation)?;
    let action_descriptor_ids = release
        .descriptors
        .map(|descriptor| hash(&descriptor).to_bytes());
    let activation_program_set = build_series_activation_capable_program_set_v1(
        &action_descriptor_ids,
        activation.descriptor_id,
    )
    .map_err(|_| SeriesCurrentSourceErrorV1::Activation)?;
    Ok(SeriesFoundPrepareActivationV1 {
        source,
        release,
        activation,
        activation_program_set,
    })
}

fn require_prepare_custody(
    template_bytes: &[u8],
    current: SeriesCurrentOccurrenceV3<'_>,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
    custody: SeriesCustodyPhysicalV3,
    projected_custody: SeriesProjectedCustodyPhysicalV3,
    release: SeriesCurrentReleaseInputV5<'_>,
) -> Result<(), SeriesCurrentSourceErrorV1> {
    let occurrence = admit_occurrence(template_bytes, current.occurrence_bytes, current.siblings)
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    let ticket =
        admit_ticket(current.ticket_bytes).map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    let escrow = pre_founding_series_escrow(occurrence, ticket, product, registry_program)
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    let expiry_slot = TemplateV3::decode(template_bytes)
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?
        .retry_through(escrow.occurrence())
        .map_err(|_| SeriesCurrentSourceErrorV1::Content)?;
    let projected_initialize =
        project_prepare_initialize_v3(escrow, expiry_slot, projected_custody)
            .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?
            .encode()
            .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    let projected_open = project_prepare_open_hoard_v3(escrow, expiry_slot, projected_custody)
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?
        .encode()
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    let derived = project_prepare_custody_v3(prepare_series_escrow_v3(escrow), custody)
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    let initialize = derived[0]
        .to_bytes()
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    let open = derived[1]
        .to_bytes()
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    let lock = derived[2]
        .to_bytes()
        .map_err(|_| SeriesCurrentSourceErrorV1::Custody)?;
    if release.prepare_requests.projected_initialize != &projected_initialize
        || release.prepare_requests.projected_open != &projected_open
        || release.prepare_requests.replay_initialize != &initialize
        || release.prepare_requests.escrow_open != &open
        || release.prepare_requests.escrow_lock != &lock
    {
        return Err(SeriesCurrentSourceErrorV1::PrepareCustody);
    }
    Ok(())
}
