//! The Structured consumer of the capability-neutral selection seam.
//!
//! Fourth family through the same neutral entry derivation, manifest merge and
//! validation that Direct, General and Rational use: a closure compiler plus the
//! family's publication, and no seam change. The release itself is
//! `dclutch_operator::structured_selected_release_v1`, whose publication is the
//! single author of every capability fact; this module only re-shapes its output
//! into the byte closure the seam consumes.
//!
//! # The ordering constraint, inherited whole from Rational
//!
//! Structured's config is a `TokenBehaviorSelectionV2` -- the same type
//! Rational's is -- so it carries exactly the same constraint and for exactly
//! the same reason: the Realm is `RealmV1` over the collateral Mint, so the
//! collateral Mint must be chosen before the closure is compiled. That is an
//! ORDERING, not a fixed point; `mint -> realm -> config -> manifest -> market`
//! runs strictly one way, and the Realm is itself a SEED of the Market PDA.
//!
//! Because it is inherited rather than re-derived, this module does not
//! re-author the Realm computation. It calls the seam's
//! [`market_realm_identity_v1`], which moved out of `rational_market` when this
//! second consumer proved it was never Rational's.
//!
//! # The one number this family chooses differently, and why it is not the
//! market's
//!
//! The representation width `K` here is NOT the market's outcome count. For
//! Rational the two coincide -- its coefficients span the graph's outcomes. For
//! Structured they are independent by construction, which the operator's own
//! `structured_actions_keep_descriptor_k_independent_from_product_n` pins: `K`
//! is the width of the composition the receipt is issued against, while the
//! Product result width `N` is the payoff geometry. The open RequestProfile V1
//! artifact bounds `K` at
//! `STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1` = 3, and the demo market
//! graph has four outcomes -- so a module that reached for the market's outcome
//! count here would compile a release that refuses at its first dispatch.

use dclutch_operator::structured_selected_release_v1::{
    STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1, StructuredSelectedReleaseInputV1,
    structured_selected_release_v1,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::selected_capability::market_realm_identity_v1;
use crate::{Error, Result};

/// One record the Registry must finalize for a selected Structured release.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct StructuredSelectedRecordV1 {
    /// Operator-facing name of the record's role.
    pub(crate) label: &'static str,
    /// Schema identity, read off the release's own artifacts.
    pub(crate) schema: [u8; 32],
    /// Exact semantic bytes.
    pub(crate) body: Vec<u8>,
}

/// One compiled Structured closure in the byte shape the neutral seam consumes.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct StructuredSelectedClosureBytesV1 {
    /// Exact five-entry `CapabilityProgramSetV2` bytes.
    pub(crate) program_set: Vec<u8>,
    /// The first bundle's descriptor; all five agree on every entry-authored
    /// coordinate, which the release admission enforces before returning.
    pub(crate) selected_descriptor: Vec<u8>,
    /// Exact `TokenBehaviorSelectionV2` config bytes -- market-free.
    pub(crate) config: Vec<u8>,
    /// Canonical Market-bindable publication bytes.
    pub(crate) publication: Vec<u8>,
    /// SHA-256 of the publication bytes.
    pub(crate) publication_id: [u8; 32],
    /// Every record the Registry must hold for this release.
    pub(crate) records: Vec<StructuredSelectedRecordV1>,
}

/// Compile one complete Structured selected-capability closure.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn structured_selected_closure_v1(
    input: StructuredSelectedReleaseInputV1<'_>,
) -> Result<StructuredSelectedClosureBytesV1> {
    let release = structured_selected_release_v1(input)
        .map_err(|error| Error::new(format!("Structured selected release: {error:?}")))?;
    let records = release
        .publication_records()
        .map_err(|error| Error::new(format!("Structured publication records: {error:?}")))?
        .into_iter()
        .map(|record| StructuredSelectedRecordV1 {
            label: record.label,
            schema: record.schema,
            body: record.body.to_vec(),
        })
        .collect();
    let selected_descriptor = release
        .selected
        .first()
        .ok_or_else(|| Error::new("Structured release carried no bundles"))?
        .descriptor
        .to_vec();
    let publication = release.publication.to_bytes().to_vec();
    Ok(StructuredSelectedClosureBytesV1 {
        program_set: release.program_set,
        selected_descriptor,
        config: release.config,
        publication_id: Sha256::digest(&publication).into(),
        publication,
        records,
    })
}

/// The complete capability-root width the closure's own descriptor names.
#[cfg_attr(not(test), allow(dead_code))]
fn structured_root_bytes_v1(closure: &StructuredSelectedClosureBytesV1) -> Result<usize> {
    let descriptor = dclutch_market::capability_program::v4::CapabilityProgramV4::decode(
        &closure.selected_descriptor,
    )
    .map_err(|error| Error::new(format!("Structured selected descriptor: {error:?}")))?;
    dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(
            usize::try_from(descriptor.root_state_bytes())
                .map_err(|_| Error::new("Structured root state width overflow"))?,
        )
        .ok_or_else(|| Error::new("Structured root width overflow"))
}

/// Serialize one Structured closure into the family-neutral driver payload.
#[cfg_attr(not(test), allow(dead_code))]
fn structured_selected_payload_v1(
    closure: &StructuredSelectedClosureBytesV1,
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
) -> crate::model::SelectedCapabilityV1 {
    crate::model::SelectedCapabilityV1 {
        family: "structured".into(),
        program_set_hex: crate::plan::hex(&closure.program_set),
        selected_descriptor_hex: crate::plan::hex(&closure.selected_descriptor),
        config_hex: crate::plan::hex(&closure.config),
        publication_hex: crate::plan::hex(&closure.publication),
        records: closure
            .records
            .iter()
            .enumerate()
            .map(|(index, record)| crate::model::SelectedCapabilityRecordV1 {
                // Positional prefix + `_record` suffix: unique, deterministic,
                // and covered by the founding checkpoint's record-graph census.
                label: format!(
                    "structured_{index:02}_{}_record",
                    record.label.replace('-', "_")
                ),
                schema_hex: crate::plan::hex(&record.schema),
                body_hex: crate::plan::hex(&record.body),
            })
            .collect(),
        activation_deadline_slot,
        root_rent_minimum_lamports,
        selected_manifest_entry_index: 0,
    }
}

/// The demo Structured-selected market: the same lab market graph as
/// `demo_market_input`, with the Structured capability selected through the
/// neutral seam.
///
/// Derived facts come from the release compiler itself -- kind and capacity
/// profile are the family's Lean-generated constants and are not parameters at
/// all -- and the Realm from the collateral Mint this market will be founded
/// over. LAB FACTS, labeled: the release set and root schema are
/// domain-separated projections of the plan's own release-set identity, because
/// no Structured adapter deployment exists locally to observe.
///
/// The per-coordinate item width is also a lab fact and the honest one to
/// scrutinise. It is the width of the one non-opaque per-coordinate account the
/// structured account profile observes; 64 is what every fixture in the tree
/// uses. It used to be noted here that this coincided with the Lean-generated
/// `dclutch-claims-representation-codec` `STATE_BYTES`, recorded as a
/// coincidence rather than wired up as a derivation because that crate had zero
/// consumers -- claiming a provenance on a matching width would be exactly the
/// kind of unverified constant this lane exists to remove. That crate was
/// deleted on 2026-09-01 with the Materialize route (N-11's reject decision),
/// so the coincidence has no other end and 64 stands on the fixtures alone.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn demo_structured_market_input(
    plan_path: &std::path::Path,
    rpc_url: &str,
    registry: Pubkey,
    collateral_mint: Pubkey,
    shape: &crate::market::LocalMarketShapeV1,
) -> Result<crate::model::MarketRunInput> {
    let (plan, observation) =
        crate::direct_market::observe_local_market_policy_v1(plan_path, rpc_url, registry)?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(&plan)?;
    let mut input =
        crate::market::demo_market_input_base_shaped(registry, resolution_release, shape)?;

    let lab = |label: &str| -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch:lab:structured-selection:v1");
        hasher.update([0]);
        hasher.update(plan.release_set_id.as_bytes());
        hasher.update([0]);
        hasher.update(label.as_bytes());
        hasher.finalize().into()
    };
    let product_basis = crate::runtime::decode_hex(&input.linked_basis_hex)?;

    let closure = structured_selected_closure_v1(StructuredSelectedReleaseInputV1 {
        realm: market_realm_identity_v1(collateral_mint)?,
        release_set: lab("release-set"),
        root_schema: lab("root-schema"),
        root_state_bytes: 8,
        // NOT the market's outcome count -- see this module's header. The
        // composition width the receipt is issued against, at the widest the
        // open RequestProfile V1 artifact can dispatch.
        representation_outcome_count: STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1,
        item_state_bytes: 64,
        product_basis: &product_basis,
    })?;
    let root_bytes = structured_root_bytes_v1(&closure)?;
    let payload = structured_selected_payload_v1(
        &closure,
        observation.activation_deadline_slot_v1()?,
        observation.root_rent_minimum_for_width_v1(root_bytes)?,
    );
    crate::selected_capability::attach_selected_capability_v1(&mut input, payload)?;
    crate::market::validate_market_input(&input)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_input(realm: [u8; 32], basis: &[u8]) -> StructuredSelectedReleaseInputV1<'_> {
        StructuredSelectedReleaseInputV1 {
            realm,
            release_set: [0x15; 32],
            root_schema: [0x42; 32],
            root_state_bytes: 8,
            representation_outcome_count: STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1,
            item_state_bytes: 64,
            product_basis: basis,
        }
    }

    fn basis() -> Vec<u8> {
        use dclutch_product::payoff::runtime_v3::{
            BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
        };
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: [1; 32],
                result_domain_id: [2; 32],
                coordinate_domain_id: [3; 32],
                result_unit_id: [4; 32],
                evaluator_release_id: [5; 32],
                basis_width: 258,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut output,
        )
        .expect("ProductBasisV3");
        output.to_vec()
    }

    /// The seam derives the same entry the publication declares.
    ///
    /// Two independent derivations of one fact: the seam reads kind, release,
    /// config and capacity off the closure's own BYTES, and the publication
    /// states them. Neither restates the other.
    #[test]
    fn the_seam_entry_agrees_with_the_structured_publication() {
        use crate::selected_capability::{SelectedCapabilityClosureV1, selected_manifest_entry_v1};
        use dclutch_operator::structured_selected_release_v1::structured_selected_release_v1;

        let basis = basis();
        let release =
            structured_selected_release_v1(release_input([0x18; 32], &basis)).expect("release");
        let closure =
            structured_selected_closure_v1(release_input([0x18; 32], &basis)).expect("closure");
        let entry = selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
            program_set: &closure.program_set,
            selected_descriptor: &closure.selected_descriptor,
            config: &closure.config,
            activation_deadline_slot: 1_000,
            root_rent_minimum_lamports: 1_000_000,
        })
        .expect("entry");

        assert_eq!(entry.kind_id().to_bytes(), release.publication.kind_id);
        assert_eq!(
            entry.release_id().to_bytes(),
            release.publication.program_set_id
        );
        assert_eq!(entry.config_id().to_bytes(), release.publication.config_id);
        assert_eq!(
            entry.capacity_profile_id().to_bytes(),
            release.publication.capacity_profile
        );
    }

    /// *** A STRUCTURED SELECTION PRECEDES THE MARKET IT WILL BIND. ***
    ///
    /// The seam's invariant at driver level, and the fourth family to state it.
    /// The closure takes a Realm -- a Market-PDA SEED, not a Market output --
    /// and no Market address, so the manifest entry is byte-stable and fully
    /// determined before any Market exists.
    #[test]
    fn a_structured_selection_precedes_the_market_it_will_bind() {
        use crate::selected_capability::{SelectedCapabilityClosureV1, selected_manifest_entry_v1};

        let basis = basis();
        let entry_for = || {
            let closure =
                structured_selected_closure_v1(release_input([0x18; 32], &basis)).expect("closure");
            let entry = selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
                program_set: &closure.program_set,
                selected_descriptor: &closure.selected_descriptor,
                config: &closure.config,
                activation_deadline_slot: 1_000,
                root_rent_minimum_lamports: 1_000_000,
            })
            .expect("entry");
            (closure.publication_id, entry)
        };
        let (first_publication, first) = entry_for();
        let (second_publication, second) = entry_for();
        assert_eq!(first_publication, second_publication);
        assert_eq!(first, second);
    }

    /// A different collateral Mint is a different Realm is a different config.
    ///
    /// The ordering constraint made executable for the family that INHERITED it:
    /// the Mint really does reach the manifest entry, so it must be chosen
    /// before the closure is compiled. It reaches it through the CONFIG only --
    /// the ProgramSet is untouched -- because the Realm is the config's field and
    /// nothing else's. Same experiment as Rational's, on a family whose
    /// ProgramSet is a completely different shape, which is what makes the
    /// second run evidence rather than repetition.
    #[test]
    fn the_collateral_mint_reaches_the_config_and_only_the_config() {
        let basis = basis();
        let first_realm = market_realm_identity_v1(Pubkey::new_from_array([7; 32])).expect("first");
        let second_realm =
            market_realm_identity_v1(Pubkey::new_from_array([9; 32])).expect("second");
        assert_ne!(first_realm, second_realm);

        let first = structured_selected_closure_v1(release_input(first_realm, &basis))
            .expect("first closure");
        let second = structured_selected_closure_v1(release_input(second_realm, &basis))
            .expect("second closure");
        assert_ne!(first.config, second.config);
        assert_eq!(first.program_set, second.program_set);
        assert_ne!(first.publication_id, second.publication_id);
    }

    /// The closure's records satisfy the seam's label contract.
    ///
    /// Unique, `_record`-suffixed, nonempty bodies, and none colliding with the
    /// four terminal-composition labels the founding driver appends for every
    /// family -- a collision there is a hard error at publish time, far from
    /// here.
    #[test]
    fn the_published_labels_satisfy_the_seams_contract() {
        let basis = basis();
        let closure =
            structured_selected_closure_v1(release_input([0x18; 32], &basis)).expect("closure");
        let payload = structured_selected_payload_v1(&closure, 1_000, 1_000_000);
        assert_eq!(payload.records.len(), 2 + 7 * 5);

        let mut seen = std::collections::BTreeSet::new();
        for record in &payload.records {
            assert!(record.label.ends_with("_record"), "{}", record.label);
            assert!(
                seen.insert(record.label.clone()),
                "{} repeats",
                record.label
            );
            assert!(!record.body_hex.is_empty());
        }
        for reserved in [
            "terminal_composition_descriptor_record",
            "terminal_composition_graph_record",
            "terminal_composition_translation_record",
            "terminal_composition_exposure_record",
        ] {
            assert!(
                !seen.contains(reserved),
                "the driver appends {reserved} for every family; a closure emitting it collides"
            );
        }
    }
}
