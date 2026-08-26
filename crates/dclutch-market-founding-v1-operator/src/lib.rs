//! Artifact-derived construction for atomic generic Market founding.
//!
//! This crate accepts the exact bytes of the Registry-selected founding
//! artifact plus its selected content identity.  It emits the two Core child
//! requests and their invocation-scoped Trading authorities.  Callers cannot
//! separately supply release, Market, founder, custody, width, rent, or
//! revision truth.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_capability_program_contract::CapabilityRootHeaderV1;
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    GENERIC_FOUNDING_REQUEST_BYTES_V1, GenericFoundingRequestV1, GenericFoundingStageV1, Identity,
    SeriesFoundingPermitSeedsV1, generic_founding_funding_list_id_v1,
};
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{hash::hash, pubkey::Pubkey};

/// Refusal from selected artifact authentication or deterministic construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericMarketFoundingOperatorErrorV1 {
    /// Raw artifact bytes did not match the selected content identity.
    ArtifactIdentity,
    /// The artifact was not one canonical Found-and-permit request.
    ArtifactEncoding,
    /// A derived digest, signer seed, or PDA coordinate refused.
    Derivation,
    /// The supplied capability selection coordinates refused.
    Selection,
}

/// Authenticated artifact whose private fields cannot be caller-authored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGenericMarketFoundingArtifactV1 {
    content_id: ContentId,
    request: GenericFoundingRequestV1,
    raw: [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1],
}

impl AuthenticatedGenericMarketFoundingArtifactV1 {
    /// Return the selected immutable artifact identity.
    pub const fn content_id(self) -> ContentId {
        self.content_id
    }

    /// Return the exact decoded Found-and-permit request.
    pub const fn request(self) -> GenericFoundingRequestV1 {
        self.request
    }

    /// Return the exact selected artifact bytes.
    pub const fn raw(self) -> [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1] {
        self.raw
    }
}

/// Deterministic two-stage Core plan derived from one selected artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericMarketFoundingPlanV1 {
    /// Exact selected Found-and-permit child bytes.
    pub found_request: [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1],
    /// Exact same coordinates with only the final-Open stage changed.
    pub open_request: [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1],
    /// SHA-256 of `found_request`.
    pub found_request_digest: ContentId,
    /// SHA-256 of `open_request`.
    pub open_request_digest: ContentId,
    /// Trading PDA signing only the exact Found-and-permit request.
    pub found_authority: Pubkey,
    /// Trading PDA signing only the exact final-Open request.
    pub open_authority: Pubkey,
    /// Sole Core-owned Claims founding permit PDA.
    pub permit: Pubkey,
    /// Exact selected artifact identity retained for Hot provenance.
    pub artifact_id: ContentId,
}

/// Deterministic Trading capability-root selection for one founding request.
///
/// The root address commits to the selected capability config identity, and
/// that identity is the digest of the request's root-free selection preimage.
/// Deriving in this order is the only acyclic construction: hashing the whole
/// request would require the request to carry the address of a PDA whose seeds
/// already contain that hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericFoundingRootSelectionV1 {
    /// Exact request the Trading outer and Core will authenticate, carrying
    /// the derived capability root.
    pub request: GenericFoundingRequestV1,
    /// Manifest-derived selection persisted in the Trading root header.
    pub selection: CapabilityExecutionSelectionV1,
    /// Immutable Trading root header committing to the selection.
    pub header: CapabilityRootHeaderV1,
    /// Sole Trading-owned capability-root PDA.
    pub capability_root: Pubkey,
    /// Selected capability config identity, equal to the preimage digest.
    pub config: ContentId,
}

/// Derive the acyclic capability-root selection for one founding request.
///
/// `template` supplies every artifact coordinate except the capability root;
/// its own `capability_root` field is a placeholder and is discarded. The
/// returned `request` is the finalized artifact.
pub fn construct_generic_founding_root_selection_v1(
    trading_program: Pubkey,
    template: GenericFoundingRequestV1,
    manifest: [u8; 32],
    entry_index: u16,
    kind: [u8; 32],
    capability_release: [u8; 32],
) -> Result<GenericFoundingRootSelectionV1, GenericMarketFoundingOperatorErrorV1> {
    let config = template
        .selection_config_id()
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    let selection = CapabilityExecutionSelectionV1::from_bytes(
        entry_index,
        manifest,
        kind,
        capability_release,
        config.to_bytes(),
    )
    .map_err(|_| GenericMarketFoundingOperatorErrorV1::Selection)?;
    let header = CapabilityRootHeaderV1::new(
        content(template.release_set().to_bytes())?,
        template.market().to_bytes(),
        template.generation(),
        selection,
    )
    .map_err(|_| GenericMarketFoundingOperatorErrorV1::Selection)?;
    let capability_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program).0;
    let root_identity = Identity::new(capability_root.to_bytes())
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::Derivation)?;
    let request = GenericFoundingRequestV1::new(
        GenericFoundingStageV1::FoundAndPermit,
        template.funding_count(),
        template.release_set(),
        template.market(),
        root_identity,
        template.context(),
        template.founder(),
        template.beneficiary(),
        template.funding_source(),
        template.hoard(),
        template.projected_replay(),
        template.funding_list_id(),
        template.generation(),
        template.quantity(),
        template.basis_scale(),
        template.expiry_slot(),
        template.market_rent(),
        template.permit_rent(),
        template.projected_resulting_revision(),
    )
    .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    if request
        .selection_config_id()
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?
        != config
    {
        return Err(GenericMarketFoundingOperatorErrorV1::Derivation);
    }
    Ok(GenericFoundingRootSelectionV1 {
        request,
        selection,
        header,
        capability_root,
        config: content(config.to_bytes())?,
    })
}

/// Authenticate exact selected artifact bytes.
pub fn authenticate_generic_market_founding_artifact_v1(
    selected_content_id: ContentId,
    raw: &[u8],
) -> Result<AuthenticatedGenericMarketFoundingArtifactV1, GenericMarketFoundingOperatorErrorV1> {
    if hash(raw).to_bytes() != selected_content_id.to_bytes() {
        return Err(GenericMarketFoundingOperatorErrorV1::ArtifactIdentity);
    }
    let exact: [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1] = raw
        .try_into()
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    let request = GenericFoundingRequestV1::decode(&exact)
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    if request.stage() != GenericFoundingStageV1::FoundAndPermit {
        return Err(GenericMarketFoundingOperatorErrorV1::ArtifactEncoding);
    }
    Ok(AuthenticatedGenericMarketFoundingArtifactV1 {
        content_id: selected_content_id,
        request,
        raw: exact,
    })
}

/// Derive the exact two-stage child bytes, Trading signers, and Core permit.
///
/// The Trading and Core program keys must come from the same authenticated
/// Registry selection used by the caller to authenticate `selected_content`.
pub fn construct_generic_market_founding_plan_v1(
    selected: AuthenticatedGenericMarketFoundingArtifactV1,
    trading_program: Pubkey,
    core_program: Pubkey,
) -> Result<GenericMarketFoundingPlanV1, GenericMarketFoundingOperatorErrorV1> {
    let found = selected.request;
    let open = found
        .with_stage(GenericFoundingStageV1::Open)
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    let found_request = selected.raw;
    let open_request = open
        .encode()
        .map_err(|_| GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)?;
    let found_request_digest = content(hash(&found_request).to_bytes())?;
    let open_request_digest = content(hash(&open_request).to_bytes())?;
    let found_authority = authority(found, found_request_digest, trading_program)?;
    let open_authority = authority(open, open_request_digest, trading_program)?;
    if found_authority == open_authority {
        return Err(GenericMarketFoundingOperatorErrorV1::Derivation);
    }
    let permit_seeds =
        SeriesFoundingPermitSeedsV1::new(found.release_set(), found.market(), found.context());
    let permit = Pubkey::find_program_address(&permit_seeds.as_slices(), &core_program).0;
    if permit == found_authority || permit == open_authority {
        return Err(GenericMarketFoundingOperatorErrorV1::Derivation);
    }
    Ok(GenericMarketFoundingPlanV1 {
        found_request,
        open_request,
        found_request_digest,
        open_request_digest,
        found_authority,
        open_authority,
        permit,
        artifact_id: selected.content_id,
    })
}

/// Derive the artifact's ordered FundingState-list commitment from addresses.
pub fn derive_generic_funding_list_id_v1(
    funding_states: &[Pubkey],
) -> Result<ContentId, GenericMarketFoundingOperatorErrorV1> {
    let mut identities = Vec::with_capacity(funding_states.len());
    for key in funding_states {
        identities.push(
            Identity::new(key.to_bytes())
                .map_err(|_| GenericMarketFoundingOperatorErrorV1::Derivation)?,
        );
    }
    content(
        generic_founding_funding_list_id_v1(&identities)
            .map_err(|_| GenericMarketFoundingOperatorErrorV1::Derivation)?
            .to_bytes(),
    )
}

fn authority(
    request: GenericFoundingRequestV1,
    request_digest: ContentId,
    trading_program: Pubkey,
) -> Result<Pubkey, GenericMarketFoundingOperatorErrorV1> {
    let seeds = CallerAuthoritySeedsV1::new(
        content(request.release_set().to_bytes())?,
        request.market().to_bytes(),
        ExecutionRoleV1::Trading,
        request.context().to_bytes(),
        request_digest.to_bytes(),
    )
    .map_err(|_| GenericMarketFoundingOperatorErrorV1::Derivation)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &trading_program).0)
}

fn content(bytes: [u8; 32]) -> Result<ContentId, GenericMarketFoundingOperatorErrorV1> {
    ContentId::new(bytes).map_err(|_| GenericMarketFoundingOperatorErrorV1::Derivation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market_core_codec::Identity;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn artifact() -> [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1] {
        GenericFoundingRequestV1::new(
            GenericFoundingStageV1::FoundAndPermit,
            3,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            11,
            12,
            13,
            14,
            15,
            16,
            2,
        )
        .expect("artifact")
        .encode()
        .expect("encode")
    }

    #[test]
    fn selected_artifact_is_the_only_source_of_both_stages() {
        let raw = artifact();
        let selected_id = ContentId::new(hash(&raw).to_bytes()).expect("content");
        let selected = authenticate_generic_market_founding_artifact_v1(selected_id, &raw)
            .expect("authenticate");
        let plan = construct_generic_market_founding_plan_v1(
            selected,
            Pubkey::new_from_array([21; 32]),
            Pubkey::new_from_array([22; 32]),
        )
        .expect("plan");
        assert_eq!(plan.artifact_id, selected_id);
        assert_ne!(plan.found_request, plan.open_request);
        assert_ne!(plan.found_authority, plan.open_authority);
        let open = GenericFoundingRequestV1::decode(&plan.open_request).expect("open");
        assert_eq!(open.stage(), GenericFoundingStageV1::Open);
        assert_eq!(
            open.with_stage(GenericFoundingStageV1::FoundAndPermit)
                .expect("found"),
            selected.request()
        );
    }

    #[test]
    fn capability_root_selection_is_acyclic_and_satisfies_core_authentication() {
        let trading = Pubkey::new_from_array([21; 32]);
        let template = GenericFoundingRequestV1::decode(&artifact()).expect("template");
        let selected = construct_generic_founding_root_selection_v1(
            trading, template, [0x71; 32], 3, [0x72; 32], [0x73; 32],
        )
        .expect("acyclic root selection");

        // Exactly the conjunction Core's authenticate_root evaluates.
        let header = selected.header;
        assert_eq!(
            Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0,
            selected.capability_root
        );
        assert_eq!(
            selected.request.capability_root().to_bytes(),
            selected.capability_root.to_bytes()
        );
        assert_eq!(
            header.release_set().to_bytes(),
            selected.request.release_set().to_bytes()
        );
        assert_eq!(header.market(), selected.request.market().to_bytes());
        assert_eq!(header.generation(), selected.request.generation());
        assert_eq!(
            header.selection().config().to_bytes(),
            hash(
                &selected
                    .request
                    .selection_preimage()
                    .expect("found preimage")
            )
            .to_bytes()
        );

        // The commit-last Open stage reuses the same root without a second
        // activation, because the preimage fixes the Found-and-permit stage.
        let open = selected
            .request
            .with_stage(GenericFoundingStageV1::Open)
            .expect("open stage");
        assert_eq!(
            header.selection().config().to_bytes(),
            hash(&open.selection_preimage().expect("open preimage")).to_bytes()
        );

        // Hashing the whole finalized request instead is the cycle this
        // construction exists to break: it can never equal the selected config.
        assert_ne!(
            header.selection().config().to_bytes(),
            hash(&selected.request.encode().expect("request bytes")).to_bytes()
        );

        // Any other substituted coordinate moves the root, so a hostile
        // artifact cannot reuse an activated capability root.
        let hostile = GenericFoundingRequestV1::new(
            GenericFoundingStageV1::FoundAndPermit,
            selected.request.funding_count(),
            selected.request.release_set(),
            selected.request.market(),
            selected.request.capability_root(),
            selected.request.context(),
            selected.request.founder(),
            selected.request.beneficiary(),
            selected.request.funding_source(),
            selected.request.hoard(),
            selected.request.projected_replay(),
            selected.request.funding_list_id(),
            selected.request.generation(),
            selected.request.quantity(),
            selected.request.basis_scale(),
            selected.request.expiry_slot(),
            selected.request.market_rent(),
            selected.request.permit_rent(),
            selected
                .request
                .projected_resulting_revision()
                .checked_add(1)
                .expect("revision"),
        )
        .expect("hostile request");
        assert_ne!(
            header.selection().config().to_bytes(),
            hash(&hostile.selection_preimage().expect("hostile preimage")).to_bytes()
        );
    }

    #[test]
    fn content_stage_and_substitution_refuse() {
        let raw = artifact();
        let wrong = ContentId::new([44; 32]).expect("content");
        assert_eq!(
            authenticate_generic_market_founding_artifact_v1(wrong, &raw),
            Err(GenericMarketFoundingOperatorErrorV1::ArtifactIdentity)
        );

        let open = GenericFoundingRequestV1::decode(&raw)
            .expect("request")
            .with_stage(GenericFoundingStageV1::Open)
            .expect("open")
            .encode()
            .expect("encode");
        let open_id = ContentId::new(hash(&open).to_bytes()).expect("content");
        assert_eq!(
            authenticate_generic_market_founding_artifact_v1(open_id, &open),
            Err(GenericMarketFoundingOperatorErrorV1::ArtifactEncoding)
        );
    }
}
