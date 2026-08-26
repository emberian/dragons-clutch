//! Same-finalized chain corpus and unsigned-v0 construction.

#![allow(clippy::panic, clippy::unwrap_used)]

mod support;

use dclutch_fractional_claim_contract::{FractionalActionV1, NO_TERMINAL_OUTCOME_V1};
use dclutch_fractional_claim_kernel::FractionalPhaseV1;
use dclutch_fractional_claim_operator::{
    Error, FractionalActionObservationV1, FractionalClaimsAccountRuleV1, FractionalIntentV1,
    build_fractional_unsigned_v0_from_chain_v1, prepare_fractional_chain_artifacts_v1,
};
use dclutch_versioned_message_operator::PACKET_DATA_BYTES;
use sha2::{Digest, Sha256};
use solana_hash::Hash;
use solana_program::instruction::AccountMeta;

use support::FractionalChainFixtureV1;

fn compiler_only_claims_frame() -> [FractionalClaimsAccountRuleV1; 1] {
    // The canonical Claims owner has not yet published the physical FrameSpec.
    // Compile only the universally known executable role here; do not infer
    // child state widths, receipt accounts, or postconditions in this fixture.
    [FractionalClaimsAccountRuleV1 {
        signer: false,
        writable: false,
        executable: true,
        data_length: 0,
    }]
}

#[test]
fn finalized_product_market_records_drive_unsigned_wrap_without_projection_authority() {
    let fixture = FractionalChainFixtureV1::new(
        FractionalActionV1::Wrap,
        [62; 32],
        &compiler_only_claims_frame(),
    );
    let prepared = fixture.prepare();
    assert_eq!(prepared.observation(), fixture.observation);
    assert_eq!(prepared.terms().outcome_count(), 3);
    assert_eq!(
        prepared.request_context().product_record,
        <[u8; 32]>::from(Sha256::digest(&fixture.product.raw.data))
    );

    let destination = solana_program::pubkey::Pubkey::new_from_array([71; 32]);
    let observed = FractionalActionObservationV1 {
        observation: fixture.observation,
        revision: prepared.root().input().revision,
        phase: FractionalPhaseV1::Open,
        terminal_digest: [0; 32],
        terminal_outcome: NO_TERMINAL_OUTCOME_V1,
        reserves: &fixture.reserves,
        owner: fixture.owner,
        source_token_account: solana_program::pubkey::Pubkey::default(),
        destination_token_account: destination,
        actor_native_claims: 9,
        source_shards: 0,
        destination_shards: 3,
    };
    let accounts = [
        AccountMeta::new_readonly(fixture.owner, true),
        AccountMeta::new_readonly(fixture.claims_program.key, false),
        AccountMeta::new_readonly(fixture.custody_program.key, false),
        AccountMeta::new_readonly(fixture.token_program.key, false),
    ];
    let plan = build_fractional_unsigned_v0_from_chain_v1(
        prepared,
        FractionalIntentV1 {
            action: FractionalActionV1::Wrap,
            outcome: 0,
            quantity: 2,
        },
        observed,
        fixture.payer,
        Hash::new_from_array([81; 32]),
        &accounts,
        &[],
    )
    .expect("same-chain unsigned wrap");
    assert_eq!(plan.action.native_claims, 2);
    assert_eq!(plan.action.consumed_shards, 20);
    assert_eq!(plan.action.post_destination_shards, 23);
    assert_eq!(plan.message.loaded_addresses, 0);
    assert!(plan.message.wire_bytes <= PACKET_DATA_BYTES);
}

#[test]
fn stale_and_substituted_product_or_token_records_refuse_before_planning() {
    let fixture = FractionalChainFixtureV1::new(
        FractionalActionV1::Wrap,
        [62; 32],
        &compiler_only_claims_frame(),
    );

    let mut substituted = fixture.clone();
    *substituted
        .result_domain
        .raw
        .data
        .last_mut()
        .expect("domain tail") ^= 1;
    assert!(matches!(
        prepare_fractional_chain_artifacts_v1(substituted.snapshot(), substituted.checked),
        Err(Error::ChainArtifacts)
    ));

    let mut stale = fixture.clone();
    stale.token_behavior.raw.observation.slot -= 1;
    assert!(matches!(
        prepare_fractional_chain_artifacts_v1(stale.snapshot(), stale.checked),
        Err(Error::ChainArtifacts)
    ));

    let mut substituted_program = fixture.clone();
    substituted_program.token_program.key = solana_program::pubkey::Pubkey::new_unique();
    assert!(matches!(
        prepare_fractional_chain_artifacts_v1(
            substituted_program.snapshot(),
            substituted_program.checked,
        ),
        Err(Error::ChainArtifacts)
    ));
}

#[test]
fn same_finalized_runtime_graph_authenticates_at_258_outcomes() {
    let fixture = FractionalChainFixtureV1::new_with_outcomes(
        FractionalActionV1::Wrap,
        [62; 32],
        &compiler_only_claims_frame(),
        258,
    );
    let prepared = fixture.prepare();
    assert_eq!(prepared.terms().outcome_count(), 258);
    assert_eq!(fixture.reserves.len(), 258);
    assert_eq!(
        prepared
            .terms()
            .shard_mint(257)
            .expect("last Mint")
            .get(..4),
        Some(258_u32.to_le_bytes().as_slice())
    );
}
