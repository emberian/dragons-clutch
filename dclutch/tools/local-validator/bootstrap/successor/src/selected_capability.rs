//! Capability-neutral selection seam: one authoring path from a published
//! capability release to the Market's capability manifest.
//!
//! A founded Market binds ONE selected trade capability beside its three
//! same-release Resolution companions. This module owns the neutral half of
//! that binding: the manifest entry is DERIVED from the release's own selected
//! descriptor, program set, and config bytes — the publication is the single
//! author of every capability fact, and the founding restates none of them.
//! Direct is the first consumer; every additional family is a closure
//! compiler plus its publication, not new driver code.
//!
//! INVARIANT (the fixed point this seam refuses to express): the capability
//! manifest digest is a Market-PDA seed (`MarketIdentity::capability_manifest`,
//! see `derive_founding_targets_inner`), so EVERY entry-authored identity —
//! config, release/program-set (and therefore every descriptor and artifact
//! the set names), capacity, kind — must be derivable BEFORE the Market
//! address exists. An identity that consumes the Market PDA is
//! unsatisfiable-by-construction: manifest ⊃ entry identity =
//! SHA-256(bytes(Market PDA)) with Market PDA = f(SHA-256(manifest)) is a
//! SHA-256 fixed point no author can construct. Two shipped families carry
//! the trap through two different identities: Fractional through its CONFIG
//! (the config schema IS the exposure terms, and the terms bind the Core
//! Market — `fractional_atomic_v3.rs:224`, `request_v2.rs::bind_terms`;
//! pinned in `fractional_market.rs`), and Rational through its RELEASE_ID
//! (the compact effect's child template bakes per-Market custody-owner PDAs
//! into the descriptor bytes — pinned at c09cd7eb with a pre-registered
//! success criterion). General and Direct are acyclic in every entry
//! identity, which is what makes them foundable. A family compiler entering
//! this seam owes exactly that property, in every identity the entry names.

use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_market::capability_program::v4::CapabilityProgramV4;
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

/// One family's complete selected-capability closure, in the shape the
/// neutral seam consumes.
///
/// Every field is bytes the family's own release compiler emitted and
/// hostile-validated; the seam derives the manifest entry from them and
/// invents nothing. The family remains responsible for validating its own
/// artifact joins (descriptor ↔ program set ↔ config schema) before handing
/// bytes here — this seam owns only the facts every family shares.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedCapabilityClosureV1<'a> {
    /// Exact `CapabilityProgramSetV2` bytes. Their SHA-256 is the entry's
    /// release identity — derived here, never restated by a caller.
    pub(crate) program_set: &'a [u8],
    /// The selected V4 descriptor whose own coordinates author the entry:
    /// kind, capacity profile, root schema, derivation policy.
    pub(crate) selected_descriptor: &'a [u8],
    /// Exact config record body. Its SHA-256 is the entry's config identity.
    /// Must be derivable before the Market exists (module invariant).
    pub(crate) config: &'a [u8],
    /// Finite slot deadline for the entry's prepaid-lazy activation.
    pub(crate) activation_deadline_slot: u64,
    /// Exact Rent quote for the capability root the entry prepays.
    pub(crate) root_rent_minimum_lamports: u64,
    /// Family-derived native Creation principal delivered with the root.
    pub(crate) creation_principal_lamports: u64,
}

/// Derive the one manifest entry a selected capability closure determines.
///
/// Every identity coordinate comes from the closure's own bytes: the kind,
/// capacity profile, root schema, and derivation policy from the selected
/// descriptor; the release identity from the program-set bytes; the config
/// identity from the config bytes. The two scalar facts (deadline, root rent)
/// are environment observations the caller made, both required positive.
pub(crate) fn selected_manifest_entry_v1(
    closure: SelectedCapabilityClosureV1<'_>,
) -> Result<CapabilityEntryV1> {
    if closure.activation_deadline_slot == 0 {
        return Err(Error::new(
            "selected-capability activation deadline slot must be positive",
        ));
    }
    if closure.root_rent_minimum_lamports == 0 {
        return Err(Error::new(
            "selected-capability root rent minimum must be positive",
        ));
    }
    let descriptor = CapabilityProgramV4::decode(closure.selected_descriptor)
        .map_err(|error| Error::new(format!("selected CapabilityProgramV4: {error:?}")))?;
    let program_set_id: [u8; 32] = Sha256::digest(closure.program_set).into();
    let config_id: [u8; 32] = Sha256::digest(closure.config).into();
    let none = CompartmentFundingV1::not_applicable();
    let creation = if closure.creation_principal_lamports == 0 {
        none
    } else {
        CompartmentFundingV1::native_lamports(closure.creation_principal_lamports)
            .map_err(|error| Error::new(format!("selected creation quote: {error:?}")))?
    };
    let amounts = FundingAmountsV1::new(
        // The funding ledger owns the complete exact Rent quote. Any lamports
        // already sitting on the vacant PDA are classified at activation by the
        // dust-safe root-creation semantic owner as displaced prepayment or
        // unsolicited surplus; they never reduce this immutable quote.
        CompartmentFundingV1::native_lamports(closure.root_rent_minimum_lamports)
            .map_err(|error| Error::new(format!("selected root rent quote: {error:?}")))?,
        creation,
        none,
        none,
        none,
        none,
        none,
    )
    .map_err(|error| Error::new(format!("selected funding amounts: {error:?}")))?;
    CapabilityEntryV1::new(
        capability_content(descriptor.kind().to_bytes())?,
        capability_content(program_set_id)?,
        capability_content(config_id)?,
        capability_content(descriptor.capacity_profile().to_bytes())?,
        capability_content(descriptor.root_schema().to_bytes())?,
        capability_content(descriptor.derivation_policy().to_bytes())?,
        ActivationPolicy::PrepaidLazy,
        closure.activation_deadline_slot,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None)
            .map_err(|error| Error::new(format!("selected funding quote: {error:?}")))?,
    )
    .map_err(|error| Error::new(format!("selected manifest entry: {error:?}")))
}

/// The dependency edges a selected trade entry carries: every OTHER manifest
/// index, ascending.
///
/// **DERIVED FROM THE CLOSE FRAME, NOT CHOSEN.** Retirement's stage four is
/// the production `F=2` Direct close: the frame carries two physical
/// `FundingLedgerV2` accounts — the Resolution-owned dependency ledger at
/// route coordinate 5, PRESERVED (`AccountMeta::new_readonly`), and the
/// Trading-owned selected ledger at 6, closed
/// (`dclutch-operator/src/terminal_retirement_v1.rs:120-123`, `:543-582`,
/// `:584`). `validate_funding_ledger_masks_v2`
/// (`dclutch-market/src/capability_manifest/funding.rs:1554-1587`) requires
/// those two masks to be a DISJOINT PARTITION of the header's required union,
/// and the union is `capability_dependency_closure_mask_v1(manifest,
/// selected_index)`. The Resolution ledger's mask is the three companions and
/// the Trading ledger's is the selected bit, so the union can only be every
/// entry — and a closure reaches every entry only if the selected entry names
/// the other three. There is no second admissible edge set: the closure
/// follows edges out of the selected entry alone.
///
/// The manifest digest is a Market-PDA seed, so this is not a configuration
/// that can be corrected later. A market founded with `dependency_count 0`
/// — every market cohort-15 and cohort-16 founded — reaches Terminal and can
/// never be Retired, because stage four's `CapabilityFundingHeaderV2::new`
/// refuses `physical_count 2 > logical_count 1` before any account is read.
pub(crate) fn selected_entry_dependencies_v1(
    entry_count: u16,
    selected_index: u16,
) -> Result<(u8, [u8; MAX_DEPENDENCIES_PER_CAPABILITY])> {
    if selected_index >= entry_count {
        return Err(Error::new(
            "the selected manifest index is outside its own manifest",
        ));
    }
    let count = usize::from(entry_count)
        .checked_sub(1)
        .ok_or_else(|| Error::new("a manifest with no entries selects nothing"))?;
    if count > MAX_DEPENDENCIES_PER_CAPABILITY {
        return Err(Error::new(
            "the selected entry depends on more entries than the dependency array admits",
        ));
    }
    let mut dependencies = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
    let mut position = 0_usize;
    for index in 0..entry_count {
        if index == selected_index {
            continue;
        }
        let slot = dependencies
            .get_mut(position)
            .ok_or_else(|| Error::new("dependency array overflow"))?;
        *slot = u8::try_from(index)
            .map_err(|_| Error::new("a manifest index outside the dependency array's domain"))?;
        position += 1;
    }
    debug_assert_eq!(position, count);
    let count =
        u8::try_from(count).map_err(|_| Error::new("dependency count outside its own domain"))?;
    Ok((count, dependencies))
}

/// The selected entry as it is FOUNDED: identical to the closure's own entry
/// in every identity, carrying the dependency edges its manifest position
/// determines.
///
/// One author for both sides of the seam — `merge_selected_manifest_v1`
/// encodes this entry and `validate_selected_manifest_v1` re-derives it — so
/// the manifest a founding publishes and the entry a founding authenticates
/// cannot disagree about the edges.
pub(crate) fn selected_entry_with_dependencies_v1(
    entry: CapabilityEntryV1,
    entry_count: u16,
    selected_index: u16,
) -> Result<CapabilityEntryV1> {
    let (dependency_count, dependencies) =
        selected_entry_dependencies_v1(entry_count, selected_index)?;
    CapabilityEntryV1::new(
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
        entry.capacity_profile_id(),
        entry.child_schema_id(),
        entry.child_derivation_id(),
        entry.activation_policy(),
        entry.activation_deadline_slot(),
        dependency_count,
        dependencies,
        entry.funding_quote(),
    )
    .map_err(|error| Error::new(format!("selected manifest entry dependencies: {error:?}")))
}

/// Merge one selected entry into a canonical Resolution-controller base.
///
/// The base's width is derived from the authenticated recovery policy: ordinary
/// markets have three Resolution rows while an Ensemble can fund every member
/// slot and therefore has more. The base must remain canonical, contain at
/// least the three semantic roles, carry one controller release, and contain
/// no selected trade kind. Returns the canonical merged bytes and selected
/// entry index.
pub(crate) fn merge_selected_manifest_v1(
    base_bytes: &[u8],
    selected: CapabilityEntryV1,
) -> Result<(Vec<u8>, u16)> {
    let (mut entries, selected_manifest_entry_index) =
        preselection_manifest_position_v1(base_bytes, selected.kind_id().to_bytes())?;
    // The canonical base is already sorted by kind. No release, config,
    // descriptor or funding byte can affect this insertion position.
    entries.insert(usize::from(selected_manifest_entry_index), selected);
    let entry_count = u16::try_from(entries.len())
        .map_err(|_| Error::new("selected-capable manifest entry count overflow"))?;
    let selected_slot = entries
        .get_mut(usize::from(selected_manifest_entry_index))
        .ok_or_else(|| Error::new("canonical manifest omitted its selected entry"))?;
    *selected_slot = selected_entry_with_dependencies_v1(
        *selected_slot,
        entry_count,
        selected_manifest_entry_index,
    )?;
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("selected-capable manifest: {error:?}")))?;
    Ok((manifest, selected_manifest_entry_index))
}

/// Derive a selected family's manifest coordinate before its closure exists.
///
/// Only the canonical Resolution base and the family's semantic kind determine
/// this coordinate. The final merge shares this exact ordering owner, so a
/// compiler can bind its index without inventing a provisional descriptor,
/// program set or certificate. This makes no account or reservation.
pub(crate) fn preselection_manifest_entry_index_v1(
    base_bytes: &[u8],
    selected_kind: [u8; 32],
) -> Result<u16> {
    preselection_manifest_position_v1(base_bytes, selected_kind).map(|(_, index)| index)
}

fn preselection_manifest_position_v1(
    base_bytes: &[u8],
    selected_kind: [u8; 32],
) -> Result<(Vec<CapabilityEntryV1>, u16)> {
    capability_content(selected_kind)?;
    let base = CapabilityManifestV1::decode(base_bytes)
        .map_err(|error| Error::new(format!("Resolution capability manifest: {error:?}")))?;
    if base.entry_count() < 3 || base.as_bytes() != base_bytes {
        return Err(Error::new(
            "selected-capability compilation requires a canonical Resolution base with at least three entries",
        ));
    }
    let first_release = base
        .entry(0)
        .map_err(|error| Error::new(format!("Resolution capability entry 0: {error:?}")))?
        .release_id();
    let mut entries = Vec::with_capacity(usize::from(base.entry_count()).saturating_add(1));
    for index in 0..base.entry_count() {
        let entry = base.entry(index).map_err(|error| {
            Error::new(format!("Resolution capability entry {index}: {error:?}"))
        })?;
        if entry.kind_id().to_bytes() == selected_kind || entry.release_id() != first_release {
            return Err(Error::new(
                "Resolution base must contain same-release controller companions of other kinds \
                 than the selected capability",
            ));
        }
        entries.push(entry);
    }
    let selected_index = u16::try_from(
        entries
            .iter()
            .take_while(|entry| entry.kind_id().to_bytes() < selected_kind)
            .count(),
    )
    .map_err(|_| Error::new("selected manifest index overflow"))?;
    let entry_count = base
        .entry_count()
        .checked_add(1)
        .ok_or_else(|| Error::new("selected-capable manifest entry count overflow"))?;
    // Admission of the prospective position includes the same dependency
    // capacity check that a completed selected entry must pass.
    selected_entry_dependencies_v1(entry_count, selected_index)?;
    Ok((entries, selected_index))
}

/// Validate one selected-capable manifest against the entry its closure
/// re-derives.
///
/// The manifest must be canonical with one selected entry beside an
/// authenticated Resolution-controller base. The entry at the recorded index
/// must equal the re-derived closure byte for byte, and the selected kind must
/// appear exactly once.
pub(crate) fn validate_selected_manifest_v1(
    manifest_bytes: &[u8],
    expected: CapabilityEntryV1,
    selected_manifest_entry_index: u16,
) -> Result<()> {
    let manifest = CapabilityManifestV1::decode(manifest_bytes)
        .map_err(|error| Error::new(format!("selected-capable manifest: {error:?}")))?;
    if manifest.entry_count() < 4 || manifest.as_bytes() != manifest_bytes {
        return Err(Error::new(
            "selected-capable manifest must be canonical and contain a Resolution base plus one selected entry",
        ));
    }
    let selected = manifest
        .entry(selected_manifest_entry_index)
        .map_err(|error| Error::new(format!("selected manifest entry: {error:?}")))?;
    // The typed closure authors every identity; its manifest POSITION authors
    // the edges, and this re-derives them from the position rather than
    // accepting whatever the published bytes carry.
    let expected = selected_entry_with_dependencies_v1(
        expected,
        manifest.entry_count(),
        selected_manifest_entry_index,
    )?;
    if selected != expected {
        return Err(Error::new(
            "selected manifest entry did not equal the typed capability closure",
        ));
    }
    let selected_kind = expected.kind_id();
    let mut selected_count = 0_u16;
    for index in 0..manifest.entry_count() {
        if manifest
            .entry(index)
            .map_err(|error| Error::new(format!("capability entry {index}: {error:?}")))?
            .kind_id()
            == selected_kind
        {
            selected_count = selected_count
                .checked_add(1)
                .ok_or_else(|| Error::new("selected manifest count overflow"))?;
        }
    }
    if selected_count != 1 {
        return Err(Error::new(
            "selected-capable manifest did not contain exactly one entry of the selected kind",
        ));
    }
    let controller_release = manifest
        .entry(if selected_manifest_entry_index == 0 {
            1
        } else {
            0
        })
        .map_err(|error| Error::new(format!("Resolution controller companion: {error:?}")))?
        .release_id();
    if expected.release_id() == controller_release {
        return Err(Error::new(
            "selected capability may not claim the Resolution controller release",
        ));
    }
    for index in 0..manifest.entry_count() {
        if index == selected_manifest_entry_index {
            continue;
        }
        let companion = manifest.entry(index).map_err(|error| {
            Error::new(format!(
                "Resolution controller companion {index}: {error:?}"
            ))
        })?;
        if companion.release_id() != controller_release || companion.kind_id() == selected_kind {
            return Err(Error::new(
                "selected-capable manifest contains a non-controller companion",
            ));
        }
    }
    Ok(())
}

fn capability_content(value: [u8; 32]) -> Result<CapabilityContentId> {
    CapabilityContentId::new(value)
        .map_err(|error| Error::new(format!("capability content: {error:?}")))
}

/// Derive the manifest entry one serialized closure payload determines.
pub(crate) fn payload_manifest_entry_v1(
    payload: &crate::model::SelectedCapabilityV1,
) -> Result<CapabilityEntryV1> {
    let program_set = crate::runtime::decode_hex(&payload.program_set_hex)?;
    let selected_descriptor = crate::runtime::decode_hex(&payload.selected_descriptor_hex)?;
    let config = crate::runtime::decode_hex(&payload.config_hex)?;
    selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
        program_set: &program_set,
        selected_descriptor: &selected_descriptor,
        config: &config,
        activation_deadline_slot: payload.activation_deadline_slot,
        root_rent_minimum_lamports: payload.root_rent_minimum_lamports,
        creation_principal_lamports: payload.creation_principal_lamports,
    })
}

/// Attach one compiled family closure to a market input.
///
/// Merges the entry the payload's own bytes derive into the input's canonical
/// policy-derived Resolution-controller base, records the selected index, and
/// stores the payload. The input must not already carry a capability closure of either
/// shape — one selected trade capability per Market.
pub(crate) fn attach_selected_capability_v1(
    input: &mut crate::model::MarketRunInput,
    payload: crate::model::SelectedCapabilityV1,
) -> Result<()> {
    if input.direct_capability.is_some() || input.selected_capability.is_some() {
        return Err(Error::new(
            "a market input carries exactly one selected-capability closure, and this one \
             already has one",
        ));
    }
    let entry = payload_manifest_entry_v1(&payload)?;
    let base = crate::runtime::decode_hex(&input.capability_manifest_hex)?;
    let (manifest, selected_manifest_entry_index) = merge_selected_manifest_v1(&base, entry)?;
    let mut payload = payload;
    payload.selected_manifest_entry_index = selected_manifest_entry_index;
    input.capability_manifest_hex = crate::plan::hex(&manifest);
    input.selected_capability = Some(payload);
    validate_selected_capability_input_v1(input)
}

/// Validate one market input carrying a family-neutral selected closure.
///
/// The entry is re-derived from the payload's own bytes and the manifest must
/// carry exactly it at the recorded index; the publication must be nonempty;
/// every record label must be unique and end in `_record` so the founding
/// checkpoint's record-graph census re-authenticates the whole closure.
pub(crate) fn validate_selected_capability_input_v1(
    input: &crate::model::MarketRunInput,
) -> Result<()> {
    let payload = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("market input omitted its selected-capability closure"))?;
    if input.direct_capability.is_some() {
        return Err(Error::new(
            "a market input carries exactly one selected-capability closure; this one carries \
             both the Direct closure and a family-neutral closure",
        ));
    }
    if payload.family.is_empty() {
        return Err(Error::new("selected capability must name its family"));
    }
    if crate::runtime::decode_hex(&payload.publication_hex)?.is_empty() {
        return Err(Error::new(
            "selected capability omitted its canonical publication bytes",
        ));
    }
    let mut labels = std::collections::BTreeSet::new();
    for record in &payload.records {
        if !record.label.ends_with("_record") {
            return Err(Error::new(format!(
                "selected-capability record label {} must end in _record so the founding \
                 checkpoint's record-graph census covers it",
                record.label
            )));
        }
        if !labels.insert(record.label.as_str()) {
            return Err(Error::new(format!(
                "selected-capability record label {} repeats",
                record.label
            )));
        }
        let _ = crate::runtime::decode_hex(&record.schema_hex)?;
        if crate::runtime::decode_hex(&record.body_hex)?.is_empty() {
            return Err(Error::new(format!(
                "selected-capability record {} carried no body",
                record.label
            )));
        }
    }
    let entry = payload_manifest_entry_v1(payload)?;
    let manifest = crate::runtime::decode_hex(&input.capability_manifest_hex)?;
    validate_selected_manifest_v1(&manifest, entry, payload.selected_manifest_entry_index)
}

/// The selected entry's index for either closure shape.
pub(crate) fn selected_manifest_entry_index_v1(
    input: &crate::model::MarketRunInput,
) -> Result<u16> {
    match (&input.direct_capability, &input.selected_capability) {
        (Some(direct), None) => Ok(direct.selected_manifest_entry_index),
        (None, Some(selected)) => Ok(selected.selected_manifest_entry_index),
        _ => Err(Error::new(
            "market input must carry exactly one selected-capability closure",
        )),
    }
}

/// The selected entry's kind for either closure shape, read from the
/// validated manifest at the recorded index.
pub(crate) fn selected_capability_kind_v1(
    input: &crate::model::MarketRunInput,
) -> Result<[u8; 32]> {
    let index = selected_manifest_entry_index_v1(input)?;
    let manifest_bytes = crate::runtime::decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(|error| Error::new(format!("selected-capable manifest: {error:?}")))?;
    Ok(manifest
        .entry(index)
        .map_err(|error| Error::new(format!("selected manifest entry: {error:?}")))?
        .kind_id()
        .to_bytes())
}

/// The immutable Realm identity a Market over this collateral Mint will carry.
///
/// Recomputed here exactly as `compile_market_bodies` builds it, because a
/// family whose config binds a Realm must bind the same one the founded Market
/// will. The canonical collateral adapter, Token program and authority policies
/// are the market compiler's, not choices made here.
///
/// THIS LIVES IN THE SEAM RATHER THAN IN A FAMILY because a second family
/// needing it is what proves it was never the first family's. It arrived with
/// Rational, whose `TokenBehaviorSelectionV2` config is a Realm and a release
/// set; Structured inherits that exact config type and therefore that exact
/// ordering constraint.
///
/// The constraint is an ORDERING, not a fixed point, and the distinction is the
/// whole reason this seam can express it: the dependency runs
/// `mint -> realm -> config -> manifest -> market`, strictly one way. The Realm
/// is itself a SEED of the Market PDA, so naming it is naming an INPUT to the
/// derivation rather than an output of it.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn market_realm_identity_v1(
    collateral_mint: solana_sdk::pubkey::Pubkey,
) -> Result<[u8; 32]> {
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
    use dclutch_market::realm::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input,
    };

    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id:
            crate::collateral_release::founded_collateral_adapter_release_id_v1(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map_err(|error| Error::new(format!("canonical collateral Realm: {error:?}")))?
    .to_bytes();
    Ok(Sha256::digest(realm).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SELECTED_KIND: [u8; 32] = [0x51; 32];
    const COMPANION_RELEASE: [u8; 32] = [0x52; 32];

    fn entry(kind: [u8; 32], release: [u8; 32], config: [u8; 32]) -> CapabilityEntryV1 {
        let none = CompartmentFundingV1::not_applicable();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1_000_000).expect("native funding"),
            none,
            none,
            none,
            none,
            none,
            none,
        )
        .expect("funding amounts");
        CapabilityEntryV1::new(
            capability_content(kind).expect("kind"),
            capability_content(release).expect("release"),
            capability_content(config).expect("config"),
            capability_content([0x54; 32]).expect("capacity"),
            capability_content([0x55; 32]).expect("root schema"),
            capability_content([0x56; 32]).expect("derivation"),
            ActivationPolicy::PrepaidLazy,
            u64::MAX,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("funding quote"),
        )
        .expect("capability entry")
    }

    fn encode(entries: &[CapabilityEntryV1]) -> Vec<u8> {
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(entries, &mut bytes).expect("canonical manifest");
        bytes
    }

    fn base() -> Vec<u8> {
        encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
        ])
    }

    #[test]
    fn merge_places_the_selected_entry_in_kind_order_and_validates() {
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        let (manifest, index) = merge_selected_manifest_v1(&base(), selected).expect("merge");
        // 0x51 sorts after 0x11/0x12/0x13: the selected entry is last.
        assert_eq!(index, 3);
        validate_selected_manifest_v1(&manifest, selected, index).expect("validate");
    }

    #[test]
    fn preselection_index_agrees_with_every_insertion_position_without_release_bytes() {
        let mut between_first_and_second = [0x11; 32];
        between_first_and_second[31] = 0x12;
        let mut between_second_and_third = [0x12; 32];
        between_second_and_third[31] = 0x13;
        for (expected, kind) in [
            [0x01; 32],
            between_first_and_second,
            between_second_and_third,
            SELECTED_KIND,
        ]
        .into_iter()
        .enumerate()
        {
            let before = preselection_manifest_entry_index_v1(&base(), kind)
                .expect("kind alone selects a canonical position");
            assert_eq!(usize::from(before), expected);
            for (release, config) in [([0x61; 32], [0x62; 32]), ([0x71; 32], [0x72; 32])] {
                let selected = entry(kind, release, config);
                let (manifest, after) = merge_selected_manifest_v1(&base(), selected)
                    .expect("complete selected closure merges at the predicted position");
                assert_eq!(before, after);
                validate_selected_manifest_v1(&manifest, selected, after)
                    .expect("merged closure preserves its canonical dependencies");
            }
        }
    }

    #[test]
    fn preselection_index_refuses_a_duplicate_kind_before_compiling_a_closure() {
        let refusal = preselection_manifest_entry_index_v1(&base(), [0x12; 32])
            .expect_err("a duplicate kind has no prospective selected position");
        assert_eq!(
            refusal.to_string(),
            "Resolution base must contain same-release controller companions of other kinds than the selected capability"
        );
    }

    /// STAGE FOUR'S CLOSURE COVERS WHAT THE CLOSE FRAME PRESERVES.
    ///
    /// The production `F=2` Direct close carries two physical funding ledgers:
    /// the Resolution-owned dependency ledger at route coordinate 5, preserved,
    /// and the Trading-owned selected ledger at 6, closed
    /// (`dclutch-operator/src/terminal_retirement_v1.rs:120-123`, `:543-582`).
    /// Their masks must be a disjoint partition of the funding header's
    /// required union, and the union is the selected entry's dependency
    /// closure. This asserts the founded manifest makes that true, through the
    /// same three functions the deployed programs run.
    #[test]
    fn the_founded_closure_covers_the_close_frame_preserved_set() {
        use dclutch_market::CapabilityFundingHeaderV2;
        use dclutch_market::capability_manifest::{
            capability_dependency_closure_mask_v1, validate_funding_ledger_masks_v2,
        };

        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        let (bytes, index) = merge_selected_manifest_v1(&base(), selected).expect("merge");
        let manifest = CapabilityManifestV1::decode(&bytes).expect("manifest");
        assert_eq!(manifest.entry_count(), 4);
        // `build_direct_native_close_v1` requires the selected entry at index 3.
        assert_eq!(index, 3);

        let entry_at = manifest.entry(index).expect("selected entry");
        assert_eq!(entry_at.dependency_count(), 3);
        assert_eq!(
            [
                entry_at.dependency(0).expect("edge 0"),
                entry_at.dependency(1).expect("edge 1"),
                entry_at.dependency(2).expect("edge 2"),
            ],
            [0, 1, 2]
        );
        assert!(
            entry_at.dependency(3).is_err(),
            "the array tail is inactive"
        );

        let closure = capability_dependency_closure_mask_v1(manifest, index).expect("closure");
        assert_eq!(closure, 0b1111);
        assert_eq!(
            closure,
            crate::market::manifest_required_union_v1(manifest.entry_count()).expect("union")
        );

        // The close frame's own two masks, in the order the frame carries them.
        const RESOLUTION_DEPENDENCY_MASK: u16 = 0b0111;
        const TRADING_SELECTED_MASK: u16 = 0b1000;
        validate_funding_ledger_masks_v2(
            manifest.entry_count(),
            closure,
            &[RESOLUTION_DEPENDENCY_MASK, TRADING_SELECTED_MASK],
        )
        .expect("the close frame's ledgers partition the closure exactly");
        assert_eq!(RESOLUTION_DEPENDENCY_MASK | TRADING_SELECTED_MASK, closure);
        assert_eq!(RESOLUTION_DEPENDENCY_MASK & TRADING_SELECTED_MASK, 0);
        CapabilityFundingHeaderV2::new(
            2,
            u8::try_from(closure.count_ones()).expect("logical count"),
            closure,
        )
        .expect("stage four's funding header");

        // The activation frame is the same two ledgers, so the same header is
        // what this driver now builds at ActivateCapability.
        validate_selected_manifest_v1(&bytes, selected, index).expect("validate");
    }

    /// THE HOSTILE: a zero-edge manifest is what cohort-15 and cohort-16
    /// founded, and it still refuses, by discriminant.
    ///
    /// Stage four builds `CapabilityFundingHeaderV2::new(physical_count 2,
    /// logical_count 1, mask 0b1000)` and the constructor refuses
    /// `physical_count > logical_count` before any account is read. That is the
    /// wall: a market founded without the edges reaches Terminal and can never
    /// be Retired, and no later transaction can repair it, because the manifest
    /// digest is a Market-PDA seed.
    #[test]
    fn a_zero_edge_manifest_still_refuses_stage_four_by_name() {
        use dclutch_market::capability_manifest::{
            capability_dependency_closure_mask_v1, validate_funding_ledger_masks_v2,
        };
        use dclutch_market::{CapabilityFundingHeaderV2, Error as FundingHeaderError};

        // Exactly what the founding compiler emitted before this change: the
        // four canonical entries, every one of them with no edges.
        let legacy = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
            entry(SELECTED_KIND, [0x61; 32], [0x62; 32]),
        ]);
        let manifest = CapabilityManifestV1::decode(&legacy).expect("manifest");
        assert_eq!(manifest.entry_count(), 4);
        assert_eq!(manifest.entry(3).expect("selected").dependency_count(), 0);

        let closure = capability_dependency_closure_mask_v1(manifest, 3).expect("closure");
        assert_eq!(
            closure, 0b1000,
            "a zero-edge entry closes over itself alone"
        );

        assert_eq!(
            CapabilityFundingHeaderV2::new(
                2,
                u8::try_from(closure.count_ones()).expect("logical count"),
                closure,
            )
            .expect_err("physical_count 2 > logical_count 1"),
            FundingHeaderError::InvalidLength
        );
        // And even at physical_count 1 the frame's own ledgers cannot cover it:
        // the Resolution ledger's three bits are outside the union entirely.
        assert!(
            validate_funding_ledger_masks_v2(manifest.entry_count(), closure, &[0b0111, 0b1000])
                .is_err()
        );
    }

    #[test]
    fn merge_refuses_a_base_that_already_claims_the_selected_kind() {
        let poisoned = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry(SELECTED_KIND, COMPANION_RELEASE, [0x23; 32]),
        ]);
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        let refusal = merge_selected_manifest_v1(&poisoned, selected)
            .expect_err("one selected trade capability per Market");
        assert!(refusal.to_string().contains("other kinds"), "{refusal}");
    }

    #[test]
    fn merge_refuses_mixed_release_companions() {
        let mixed = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], [0x99; 32], [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
        ]);
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        merge_selected_manifest_v1(&mixed, selected)
            .expect_err("companions must share one Resolution release");
    }

    #[test]
    fn merge_preserves_a_wide_same_release_resolution_base() {
        let wide = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
            entry([0x14; 32], COMPANION_RELEASE, [0x24; 32]),
        ]);
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        let (manifest, index) = merge_selected_manifest_v1(&wide, selected)
            .expect("a policy-derived four-row Resolution base merges");
        let decoded = CapabilityManifestV1::decode(&manifest).expect("merged manifest");
        assert_eq!(decoded.entry_count(), 5);
        validate_selected_manifest_v1(&manifest, selected, index)
            .expect("the one selected capability remains authenticated");
    }

    #[test]
    fn validate_refuses_a_substituted_entry_and_a_wrong_index() {
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        let (manifest, index) = merge_selected_manifest_v1(&base(), selected).expect("merge");

        let substituted = entry(SELECTED_KIND, [0x61; 32], [0x63; 32]);
        validate_selected_manifest_v1(&manifest, substituted, index)
            .expect_err("a selected entry differing in one identity refuses");

        validate_selected_manifest_v1(&manifest, selected, 0)
            .expect_err("an index naming a companion refuses");
    }

    /// The cycle-3 multi-capability ruling, read off the codec and welded
    /// deliberately: the manifest CODEC permits one entry each of several
    /// distinct kinds (a five-entry manifest with two trade kinds encodes
    /// canonically — coexisting capabilities are not a wire impossibility),
    /// and the selection SEAM welds founding to exactly one selected trade
    /// capability: validation pins every nonselected entry to the one
    /// Resolution controller release, while the merge grows the authenticated
    /// policy-derived controller base by one. The
    /// founding funding census welds the same fact independently
    /// (`founding_masks_weld_the_manifest_to_one_selected_capability`,
    /// `market.rs`): every entry must be funded by exactly one of the two
    /// controllers, and the Trading controller funds exactly the one
    /// selected entry. Widening to several selected capabilities is a
    /// deliberate future decision — per-entry roots and funding already
    /// exist on chain — not an accident this seam can back into.
    #[test]
    fn several_distinct_kinds_encode_but_the_selection_seam_welds_to_one() {
        let second_trade_kind = [0x61; 32];
        let five = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
            entry(SELECTED_KIND, [0x91; 32], [0x62; 32]),
            entry(second_trade_kind, [0x92; 32], [0x63; 32]),
        ]);
        // (a) the codec ADMITS the shape: five distinct kinds encode.
        CapabilityManifestV1::decode(&five).expect("distinct kinds coexist at the codec");

        // (b) the seam refuses it as a founding manifest…
        let selected = entry(SELECTED_KIND, [0x91; 32], [0x62; 32]);
        validate_selected_manifest_v1(&five, selected, 3)
            .expect_err("a second non-controller entry is not a Resolution companion");
        // …and as a merge base, so a second trade capability cannot enter
        // through either door.
        let another = entry([0x71; 32], [0x93; 32], [0x64; 32]);
        merge_selected_manifest_v1(&five, another)
            .expect_err("the merge requires one controller release across its base");
    }

    #[test]
    fn the_manifest_codec_itself_refuses_two_entries_of_one_kind() {
        // The day's one-selected-capability-per-Market answer, read off the
        // codec rather than asserted in prose: canonical manifest encoding
        // demands strictly ascending kinds, so a second entry of the selected
        // kind is unencodable and validate's exactly-one check is
        // defense-in-depth over bytes no canonical author can produce.
        let entries = [
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry(SELECTED_KIND, [0x61; 32], [0x62; 32]),
            entry(SELECTED_KIND, [0x61; 32], [0x62; 32]),
        ];
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes)
            .expect_err("a duplicated kind is unencodable");
    }
}
