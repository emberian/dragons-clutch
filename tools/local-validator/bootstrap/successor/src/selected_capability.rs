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
//! see `derive_founding_targets_inner`), so every fact in the manifest — the
//! config identity included — must be derivable BEFORE the Market address
//! exists. A capability whose config record binds the Market PDA is
//! unsatisfiable-by-construction: manifest ⊃ entry.config_id = SHA-256(config)
//! ⊃ config.market = PDA(seeds ⊇ SHA-256(manifest)) is a SHA-256 fixed point
//! no author can construct. Fractional's shipped contract has exactly this
//! shape (its config schema IS the exposure terms, and the terms bind the
//! Core Market: `fractional_atomic_v3.rs:224`, `request_v2.rs::bind_terms`),
//! which is why no Fractional capability can be selected by a founded Market
//! until its config is split into a market-free record. The executable pin of
//! that circle lives in this module's tests.

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::v4::CapabilityProgramV4;
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
    let amounts = FundingAmountsV1::new(
        // The funding ledger owns the complete exact Rent quote. Any lamports
        // already sitting on the vacant PDA are classified at activation by the
        // dust-safe root-creation semantic owner as displaced prepayment or
        // unsolicited surplus; they never reduce this immutable quote.
        CompartmentFundingV1::native_lamports(closure.root_rent_minimum_lamports)
            .map_err(|error| Error::new(format!("selected root rent quote: {error:?}")))?,
        none,
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

/// Merge one selected entry into the canonical three-entry Resolution base.
///
/// The base must be canonical, carry exactly three same-release companions,
/// and none of them may already claim the selected kind — one selected trade
/// capability per Market. Returns the canonical merged manifest bytes and the
/// selected entry's index in kind order.
pub(crate) fn merge_selected_manifest_v1(
    base_bytes: &[u8],
    selected: CapabilityEntryV1,
) -> Result<(Vec<u8>, u16)> {
    let base = CapabilityManifestV1::decode(base_bytes)
        .map_err(|error| Error::new(format!("Resolution capability manifest: {error:?}")))?;
    if base.entry_count() != 3 || base.as_bytes() != base_bytes {
        return Err(Error::new(
            "selected-capability compilation requires the canonical three-entry Resolution base",
        ));
    }
    let selected_kind = selected.kind_id().to_bytes();
    let first_release = base
        .entry(0)
        .map_err(|error| Error::new(format!("Resolution capability entry 0: {error:?}")))?
        .release_id();
    let mut entries = Vec::with_capacity(4);
    for index in 0..base.entry_count() {
        let entry = base
            .entry(index)
            .map_err(|error| Error::new(format!("Resolution capability entry {index}: {error:?}")))?;
        if entry.kind_id().to_bytes() == selected_kind || entry.release_id() != first_release {
            return Err(Error::new(
                "Resolution base must contain three same-release companions of other kinds \
                 than the selected capability",
            ));
        }
        entries.push(entry);
    }
    entries.push(selected);
    entries.sort_by_key(|entry| entry.kind_id().to_bytes());
    let selected_manifest_entry_index = entries
        .iter()
        .position(|entry| entry.kind_id().to_bytes() == selected_kind)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| Error::new("canonical manifest omitted its selected entry"))?;
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("selected-capable manifest: {error:?}")))?;
    Ok((manifest, selected_manifest_entry_index))
}

/// Validate one selected-capable manifest against the entry its closure
/// re-derives.
///
/// The manifest must be canonical with exactly four entries, the entry at the
/// recorded index must equal the re-derived closure entry byte for byte, and
/// the selected kind must appear exactly once.
pub(crate) fn validate_selected_manifest_v1(
    manifest_bytes: &[u8],
    expected: CapabilityEntryV1,
    selected_manifest_entry_index: u16,
) -> Result<()> {
    let manifest = CapabilityManifestV1::decode(manifest_bytes)
        .map_err(|error| Error::new(format!("selected-capable manifest: {error:?}")))?;
    if manifest.entry_count() != 4 || manifest.as_bytes() != manifest_bytes {
        return Err(Error::new(
            "selected-capable manifest must be canonical and contain exactly four entries",
        ));
    }
    let selected = manifest
        .entry(selected_manifest_entry_index)
        .map_err(|error| Error::new(format!("selected manifest entry: {error:?}")))?;
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
    Ok(())
}

fn capability_content(value: [u8; 32]) -> Result<CapabilityContentId> {
    CapabilityContentId::new(value)
        .map_err(|error| Error::new(format!("capability content: {error:?}")))
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
    fn merge_refuses_a_base_of_another_entry_count() {
        let wide = encode(&[
            entry([0x11; 32], COMPANION_RELEASE, [0x21; 32]),
            entry([0x12; 32], COMPANION_RELEASE, [0x22; 32]),
            entry([0x13; 32], COMPANION_RELEASE, [0x23; 32]),
            entry([0x14; 32], COMPANION_RELEASE, [0x24; 32]),
        ]);
        let selected = entry(SELECTED_KIND, [0x61; 32], [0x62; 32]);
        merge_selected_manifest_v1(&wide, selected)
            .expect_err("the canonical Resolution base carries exactly three entries");
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
