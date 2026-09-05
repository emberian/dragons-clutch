//! The Rational consumer of the capability-neutral selection seam.
//!
//! Third family through the same neutral entry derivation, manifest merge and
//! validation that Direct and General use: a closure compiler plus the family's
//! publication, and no seam change. The release itself is
//! `dclutch_operator::rational_selected_release_v1`, whose publication is the
//! single author of every capability fact; this module only re-shapes its
//! output into the byte closure the seam consumes.
//!
//! # The one ordering constraint Rational has and General does not
//!
//! General's config is capacity, claim basis, program-set identity, generation
//! and policy windows -- nothing that any other founding input determines. A
//! General closure can therefore be compiled from the plan alone.
//!
//! Rational's config is a `TokenBehaviorSelectionV2`, whose two free fields are
//! the immutable Realm and the adapter release set. The Realm is NOT free: it
//! is `RealmV1` over the collateral Mint, so the collateral Mint must be chosen
//! before the closure is compiled.
//!
//! That is an ORDERING constraint, not a fixed point, and the distinction is
//! the whole of this lane's finding: the dependency runs
//! `mint -> realm -> config -> manifest -> market`, strictly one way. The Realm
//! is itself a SEED of the Market PDA (`MarketIdentity::realm_id`), so naming it
//! here is naming an input to the derivation rather than an output of it.
//! Contrast Fractional, whose config binds the Market address the manifest is
//! deriving, which is a SHA-256 fixed point no author can construct.
//!
//! It does mean [`demo_rational_market_input`] takes a collateral Mint, where
//! the General demo takes none. See the module test for what that costs today.

use dclutch_operator::rational_selected_release_v1::{
    RationalSelectedReleaseInputV1, rational_selected_release_v1,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::selected_capability::market_realm_identity_v1;
use crate::{Error, Result};

/// One record the Registry must finalize for a selected Rational release.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct RationalSelectedRecordV1 {
    /// Operator-facing name of the record's role.
    pub(crate) label: &'static str,
    /// Schema identity, read off the release's own artifacts.
    pub(crate) schema: [u8; 32],
    /// Exact semantic bytes.
    pub(crate) body: Vec<u8>,
}

/// One compiled Rational closure in the byte shape the neutral seam consumes.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct RationalSelectedClosureBytesV1 {
    /// Exact four-entry `CapabilityProgramSetV2` bytes.
    pub(crate) program_set: Vec<u8>,
    /// The first bundle's descriptor; all four agree on every entry-authored
    /// coordinate, which the release admission enforces before returning.
    pub(crate) selected_descriptor: Vec<u8>,
    /// Exact `TokenBehaviorSelectionV2` config bytes -- market-free.
    pub(crate) config: Vec<u8>,
    /// Canonical Market-bindable publication bytes.
    pub(crate) publication: Vec<u8>,
    /// SHA-256 of the publication bytes.
    pub(crate) publication_id: [u8; 32],
    /// Every record the Registry must hold for this release.
    pub(crate) records: Vec<RationalSelectedRecordV1>,
}

/// Compile one complete Rational selected-capability closure.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn rational_selected_closure_v1(
    input: RationalSelectedReleaseInputV1<'_>,
) -> Result<RationalSelectedClosureBytesV1> {
    let release = rational_selected_release_v1(input)
        .map_err(|error| Error::new(format!("Rational selected release: {error:?}")))?;
    let records = release
        .publication_records()
        .map_err(|error| Error::new(format!("Rational publication records: {error:?}")))?
        .into_iter()
        .map(|record| RationalSelectedRecordV1 {
            label: record.label,
            schema: record.schema,
            body: record.body.to_vec(),
        })
        .collect();
    let selected_descriptor = release
        .fixed
        .first()
        .ok_or_else(|| Error::new("Rational release carried no bundles"))?
        .descriptor
        .to_vec();
    let publication = release.publication.to_bytes().to_vec();
    Ok(RationalSelectedClosureBytesV1 {
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
fn rational_root_bytes_v1(closure: &RationalSelectedClosureBytesV1) -> Result<usize> {
    let descriptor = dclutch_market::capability_program::v4::CapabilityProgramV4::decode(
        &closure.selected_descriptor,
    )
    .map_err(|error| Error::new(format!("Rational selected descriptor: {error:?}")))?;
    dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(
            usize::try_from(descriptor.root_state_bytes())
                .map_err(|_| Error::new("Rational root state width overflow"))?,
        )
        .ok_or_else(|| Error::new("Rational root width overflow"))
}

/// Serialize one Rational closure into the family-neutral driver payload.
#[cfg_attr(not(test), allow(dead_code))]
fn rational_selected_payload_v1(
    closure: &RationalSelectedClosureBytesV1,
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
) -> crate::model::SelectedCapabilityV1 {
    crate::model::SelectedCapabilityV1 {
        family: "rational".into(),
        program_set_hex: crate::plan::hex(&closure.program_set),
        selected_descriptor_hex: crate::plan::hex(&closure.selected_descriptor),
        config_hex: crate::plan::hex(&closure.config),
        publication_hex: crate::plan::hex(&closure.publication),
        records: closure
            .records
            .iter()
            .enumerate()
            .map(|(index, record)| crate::model::SelectedCapabilityRecordV1 {
                label: format!(
                    "rational_{index:02}_{}_record",
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

/// The demo Rational-selected market: the same lab market graph as
/// `demo_market_input`, with the Rational capability selected through the
/// neutral seam.
///
/// Derived facts come from the base graph itself -- capacity profile from the
/// carried source-capacity body, representation width from the cuts -- and the
/// Realm from the collateral Mint this market will be founded over. LAB FACTS,
/// labeled: the release set and root schema are domain-separated projections of
/// the plan's own release-set identity, because no Rational adapter deployment
/// exists locally to observe, and the coefficients are a lab representation.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn demo_rational_market_input(
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

    let capacity_profile: [u8; 32] = Sha256::digest(crate::runtime::decode_hex(
        &input.source_capacity_profile_hex,
    )?)
    .into();
    let lab = |label: &str| -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch:lab:rational-selection:v1");
        hasher.update([0]);
        hasher.update(plan.release_set_id.as_bytes());
        hasher.update([0]);
        hasher.update(label.as_bytes());
        hasher.finalize().into()
    };
    // One nonzero coefficient per outcome: the lab representation retires every
    // coordinate, which is the widest complete-retirement the graph admits.
    let outcome_count = input
        .cuts
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::new("Rational market outcome width overflow"))?;
    let coefficients = vec![1_u64; outcome_count];
    let product_basis = crate::runtime::decode_hex(&input.linked_basis_hex)?;

    let closure = rational_selected_closure_v1(RationalSelectedReleaseInputV1 {
        realm: market_realm_identity_v1(collateral_mint)?,
        release_set: lab("release-set"),
        capacity_profile,
        root_schema: lab("root-schema"),
        root_state_bytes: 64,
        coefficients: &coefficients,
        product_basis: &product_basis,
    })?;
    let root_bytes = rational_root_bytes_v1(&closure)?;
    let payload = rational_selected_payload_v1(
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

    fn release_input<'a>(
        realm: [u8; 32],
        coefficients: &'a [u64],
        basis: &'a [u8],
    ) -> RationalSelectedReleaseInputV1<'a> {
        RationalSelectedReleaseInputV1 {
            realm,
            release_set: [0x15; 32],
            capacity_profile: [0x43; 32],
            root_schema: [0x42; 32],
            root_state_bytes: 64,
            coefficients,
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
    fn the_seam_entry_agrees_with_the_rational_publication() {
        use crate::selected_capability::{SelectedCapabilityClosureV1, selected_manifest_entry_v1};
        use dclutch_operator::rational_selected_release_v1::rational_selected_release_v1;

        let basis = basis();
        let coefficients = [1_u64, 1, 1];
        let release =
            rational_selected_release_v1(release_input([0x18; 32], &coefficients, &basis))
                .expect("release");
        let closure =
            rational_selected_closure_v1(release_input([0x18; 32], &coefficients, &basis))
                .expect("closure");
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

    /// A Rational selection precedes the Market it will bind.
    ///
    /// The seam's invariant, at driver level. The closure takes a Realm --
    /// which is a Market-PDA SEED, not a Market output -- and no Market
    /// address, so the manifest entry is byte-stable and fully determined
    /// before any Market exists.
    #[test]
    fn a_rational_selection_precedes_the_market_it_will_bind() {
        use crate::selected_capability::{SelectedCapabilityClosureV1, selected_manifest_entry_v1};

        let basis = basis();
        let coefficients = [1_u64, 1, 1];
        let entry_for = || {
            let closure =
                rational_selected_closure_v1(release_input([0x18; 32], &coefficients, &basis))
                    .expect("closure");
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
    /// This is the ordering constraint made executable: the Mint really does
    /// reach the manifest entry, so it must be chosen before the closure is
    /// compiled. It reaches it through the CONFIG only -- the ProgramSet is
    /// untouched -- because the Realm is the config's field and nothing else's.
    #[test]
    fn the_collateral_mint_reaches_the_config_and_only_the_config() {
        let basis = basis();
        let coefficients = [1_u64, 1, 1];
        let first_realm = market_realm_identity_v1(Pubkey::new_from_array([7; 32])).expect("first");
        let second_realm =
            market_realm_identity_v1(Pubkey::new_from_array([9; 32])).expect("second");
        assert_ne!(first_realm, second_realm);

        let first = rational_selected_closure_v1(release_input(first_realm, &coefficients, &basis))
            .expect("first closure");
        let second =
            rational_selected_closure_v1(release_input(second_realm, &coefficients, &basis))
                .expect("second closure");
        assert_ne!(first.config, second.config);
        assert_eq!(first.program_set, second.program_set);
        assert_ne!(first.publication_id, second.publication_id);
    }
}
