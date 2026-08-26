#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-independent construction of the exact shared
//! [`RepresentationRequestV2`] for Bearer basis-vector actions.
//!
//! Static asset identities use the generic Rational V2 Claims PDA domains.
//! The actor holder may use its associated token account or another exact
//! transferable Token account; the Claims adapter remains responsible for
//! authenticating that account's Mint and owner from chain state.

use dclutch_bearer_v2_contract::{BearerAssetIdentityV2, BearerDescriptorV2};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use solana_program::pubkey::Pubkey;
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;

/// Holder token-account selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HolderAccountV2 {
    /// Derive the canonical associated token account for actor, Mint, and Token program.
    Associated,
    /// Use another transferable Token account. Claims must authenticate its Mint and owner.
    Exact([u8; 32]),
}

/// Observed Token balances used by the exact shared request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerBalancesV2 {
    /// Token-owned shard Mint supply.
    pub shard_supply: u64,
    /// Selected actor Token-account shard balance.
    pub actor_shards: u64,
    /// Representation-authority custody ATA shard balance.
    pub structured_shards: u64,
    /// Token-owned Structured receipt Mint supply.
    pub receipt_supply: u64,
}

/// Shared active-action observations for Denominate and Reconstitute.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveBearerInputV2 {
    /// Registry-authenticated upstream caller role.
    pub caller_role: CallerRoleV2,
    /// Claims program which owns generic Rational V2 PDAs.
    pub claims_program: [u8; 32],
    /// Complete upstream replay/digest context.
    pub parent_context: [u8; 32],
    /// Transferable holder and Claims Position owner.
    pub actor: [u8; 32],
    /// Holder Token-account selection.
    pub holder_account: HolderAccountV2,
    /// Expected shared representation replay revision.
    pub representation_revision: u64,
    /// Expected Claims aggregate revision.
    pub claims_market_revision: u64,
    /// Expected actor Claims Position revision.
    pub actor_position_revision: u64,
    /// Expected canonical custody Claims Position revision.
    pub custody_position_revision: u64,
    /// Market generation.
    pub generation: u64,
    /// Native Claims atoms to denominate or reconstitute.
    pub quantity: u64,
    /// Exact observed Token balances and supplies.
    pub balances: BearerBalancesV2,
}

/// Terminal-action observations for RedeemTerminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalBearerInputV2 {
    /// Registry-authenticated upstream caller role.
    pub caller_role: CallerRoleV2,
    /// Claims program which owns generic Rational V2 PDAs.
    pub claims_program: [u8; 32],
    /// Complete upstream replay/digest context.
    pub parent_context: [u8; 32],
    /// Transferable holder redeeming shard atoms.
    pub actor: [u8; 32],
    /// Holder Token-account selection.
    pub holder_account: HolderAccountV2,
    /// Immutable Realm identity.
    pub realm: [u8; 32],
    /// Actor-owned collateral recipient.
    pub collateral_recipient: [u8; 32],
    /// Expected shared representation replay revision.
    pub representation_revision: u64,
    /// Expected Claims aggregate revision.
    pub claims_market_revision: u64,
    /// Expected canonical custody Claims Position revision.
    pub custody_position_revision: u64,
    /// Expected Custody replay revision.
    pub custody_replay_revision: u64,
    /// Market generation.
    pub generation: u64,
    /// Native Claims atoms paid by terminal redemption.
    pub quantity: u64,
    /// Exact observed Token balances and supplies.
    pub balances: BearerBalancesV2,
}

/// Canonically constructed request plus its physical derived identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructedBearerRequestV2<'a> {
    /// The exact shared Rational Representation V2 request.
    pub request: RepresentationRequestV2<'a>,
    /// Generic Rational V2 physical asset identities.
    pub asset_identity: BearerAssetIdentityV2,
}

/// Stable operator construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Claims program, actor, context, Realm, recipient, or exact holder was zero.
    ZeroIdentity,
    /// Descriptor authority was not the canonical generic Rational V2 PDA.
    RepresentationAuthorityMismatch,
    /// Caller-owned asset scratch did not have the shared one-row width.
    InvalidScratch,
    /// Shared Rational Representation V2 request construction refused.
    Representation(dclutch_rational_representation_v2_contract::Error),
}

/// Result alias for operator construction.
pub type Result<T> = core::result::Result<T, Error>;

/// Derive the generic Rational V2 identities for one Bearer coordinate.
///
/// The representation authority, shard Mint, and Claims custody owner are
/// Claims PDAs. Structured custody is its canonical ATA. An arbitrary exact
/// holder account remains transferable and is authenticated onchain by Mint and
/// owner rather than being made part of descriptor identity.
pub fn derive_asset_identities(
    bearer: BearerDescriptorV2<'_>,
    claims_program: [u8; 32],
    actor: [u8; 32],
    holder: HolderAccountV2,
) -> Result<BearerAssetIdentityV2> {
    require_nonzero(claims_program)?;
    require_nonzero(actor)?;
    let descriptor = bearer.representation();
    let claims_program = Pubkey::new_from_array(claims_program);
    let descriptor_id = descriptor.descriptor_id();
    let outcome = bearer.selected_outcome().to_le_bytes();
    let authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &claims_program,
    )
    .0;
    if authority.to_bytes() != descriptor.representation_authority() {
        return Err(Error::RepresentationAuthorityMismatch);
    }
    let shard_mint = Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor_id, &outcome],
        &claims_program,
    )
    .0;
    let claims_custody_owner = Pubkey::find_program_address(
        &[
            RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
            &descriptor_id,
            &outcome,
        ],
        &claims_program,
    )
    .0;
    let token_program = Pubkey::new_from_array(descriptor.token_program());
    let actor = Pubkey::new_from_array(actor);
    let actor_shard_account = match holder {
        HolderAccountV2::Associated => {
            get_associated_token_address_with_program_id(&actor, &shard_mint, &token_program)
        }
        HolderAccountV2::Exact(value) => {
            require_nonzero(value)?;
            Pubkey::new_from_array(value)
        }
    };
    let structured_custody_account =
        get_associated_token_address_with_program_id(&authority, &shard_mint, &token_program);
    Ok(BearerAssetIdentityV2 {
        shard_mint: shard_mint.to_bytes(),
        actor_shard_account: actor_shard_account.to_bytes(),
        structured_custody_account: structured_custody_account.to_bytes(),
        claims_custody_owner: claims_custody_owner.to_bytes(),
    })
}

/// Construct the exact shared Denominate request.
pub fn construct_denominate<'a>(
    bearer: BearerDescriptorV2<'_>,
    input: ActiveBearerInputV2,
    asset_scratch: &'a mut [u8],
) -> Result<ConstructedBearerRequestV2<'a>> {
    construct_active(
        RepresentationActionV2::Denominate,
        bearer,
        input,
        asset_scratch,
    )
}

/// Construct the exact shared Reconstitute request.
pub fn construct_reconstitute<'a>(
    bearer: BearerDescriptorV2<'_>,
    input: ActiveBearerInputV2,
    asset_scratch: &'a mut [u8],
) -> Result<ConstructedBearerRequestV2<'a>> {
    construct_active(
        RepresentationActionV2::Reconstitute,
        bearer,
        input,
        asset_scratch,
    )
}

/// Construct the exact shared RedeemTerminal request.
pub fn construct_redeem_terminal<'a>(
    bearer: BearerDescriptorV2<'_>,
    input: TerminalBearerInputV2,
    asset_scratch: &'a mut [u8],
) -> Result<ConstructedBearerRequestV2<'a>> {
    require_nonzero(input.realm)?;
    require_nonzero(input.collateral_recipient)?;
    let identity = derive_asset_identities(
        bearer,
        input.claims_program,
        input.actor,
        input.holder_account,
    )?;
    construct(
        bearer,
        RepresentationRequestHeaderV2 {
            action: RepresentationActionV2::RedeemTerminal,
            caller_role: input.caller_role,
            release_set: bearer.representation().release_set_id(),
            market: bearer.representation().market_id(),
            graph_id: bearer.representation().graph_id(),
            descriptor_id: bearer.representation().descriptor_id(),
            parent_context: input.parent_context,
            actor: input.actor,
            receipt_mint: bearer.representation().receipt_mint(),
            receipt_account: [0; 32],
            representation_authority: bearer.representation().representation_authority(),
            token_program: bearer.representation().token_program(),
            realm: input.realm,
            collateral_recipient: input.collateral_recipient,
            expected_representation_revision: input.representation_revision,
            expected_claims_market_revision: input.claims_market_revision,
            expected_actor_position_revision: ABSENT_REVISION,
            expected_custody_position_revision: input.custody_position_revision,
            expected_custody_replay_revision: input.custody_replay_revision,
            generation: input.generation,
            quantity: input.quantity,
            denominator: bearer.denominator(),
            expected_receipt_supply: input.balances.receipt_supply,
            outcome_count: bearer.representation().outcome_count(),
            selected_outcome: bearer.selected_outcome(),
            asset_count: 1,
        },
        identity,
        input.balances,
        asset_scratch,
    )
}

fn construct_active<'a>(
    action: RepresentationActionV2,
    bearer: BearerDescriptorV2<'_>,
    input: ActiveBearerInputV2,
    asset_scratch: &'a mut [u8],
) -> Result<ConstructedBearerRequestV2<'a>> {
    require_nonzero(input.parent_context)?;
    let identity = derive_asset_identities(
        bearer,
        input.claims_program,
        input.actor,
        input.holder_account,
    )?;
    construct(
        bearer,
        RepresentationRequestHeaderV2 {
            action,
            caller_role: input.caller_role,
            release_set: bearer.representation().release_set_id(),
            market: bearer.representation().market_id(),
            graph_id: bearer.representation().graph_id(),
            descriptor_id: bearer.representation().descriptor_id(),
            parent_context: input.parent_context,
            actor: input.actor,
            receipt_mint: bearer.representation().receipt_mint(),
            receipt_account: [0; 32],
            representation_authority: bearer.representation().representation_authority(),
            token_program: bearer.representation().token_program(),
            realm: [0; 32],
            collateral_recipient: [0; 32],
            expected_representation_revision: input.representation_revision,
            expected_claims_market_revision: input.claims_market_revision,
            expected_actor_position_revision: input.actor_position_revision,
            expected_custody_position_revision: input.custody_position_revision,
            expected_custody_replay_revision: ABSENT_REVISION,
            generation: input.generation,
            quantity: input.quantity,
            denominator: bearer.denominator(),
            expected_receipt_supply: input.balances.receipt_supply,
            outcome_count: bearer.representation().outcome_count(),
            selected_outcome: bearer.selected_outcome(),
            asset_count: 1,
        },
        identity,
        input.balances,
        asset_scratch,
    )
}

fn construct<'a>(
    bearer: BearerDescriptorV2<'_>,
    header: RepresentationRequestHeaderV2,
    identity: BearerAssetIdentityV2,
    balances: BearerBalancesV2,
    asset_scratch: &'a mut [u8],
) -> Result<ConstructedBearerRequestV2<'a>> {
    if asset_scratch.len() != ASSET_BYTES_V2 {
        return Err(Error::InvalidScratch);
    }
    AssetV2 {
        shard_mint: identity.shard_mint,
        actor_shard_account: identity.actor_shard_account,
        structured_custody_account: identity.structured_custody_account,
        claims_custody_owner: identity.claims_custody_owner,
        coefficient: bearer.denominator(),
        expected_shard_supply: balances.shard_supply,
        expected_actor_shards: balances.actor_shards,
        expected_structured_shards: balances.structured_shards,
    }
    .encode_into(asset_scratch)
    .map_err(Error::Representation)?;
    let request =
        RepresentationRequestV2::new(header, asset_scratch).map_err(Error::Representation)?;
    Ok(ConstructedBearerRequestV2 {
        request,
        asset_identity: identity,
    })
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_bearer_v2_contract::{BearerBindingV2, BearerDescriptorV2};
    use dclutch_rational_representation_v2_contract::REQUEST_HEADER_BYTES_V2;
    use dclutch_rational_representation_v2_kernel::{
        ContentAdmissionV2, DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES,
        DESCRIPTOR_MAGIC_V2, DescriptorAdmissionV2, GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2,
        GRAPH_NODE_BYTES, RepresentationDescriptorV2, RepresentationGraphV2, SCHEMA_VERSION_V2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    const WIDTH: u32 = 3;
    const SELECTED: u32 = 1;
    const DENOMINATOR: u64 = 10;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture offset")
            .copy_from_slice(value);
    }

    fn put_u32(output: &mut [u8], offset: usize, value: u32) {
        put(output, offset, &value.to_le_bytes());
    }

    fn put_u64(output: &mut [u8], offset: usize, value: u64) {
        put(output, offset, &value.to_le_bytes());
    }

    fn graph_fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES + WIDTH as usize * 8];
        put(&mut bytes, 0, &GRAPH_MAGIC_V2);
        put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut bytes, 16, &id(20));
        put(&mut bytes, 48, &id(14));
        put_u32(&mut bytes, 80, WIDTH);
        put_u32(&mut bytes, 84, 1);
        put_u32(&mut bytes, 88, 0);
        put_u64(&mut bytes, 96, 100);
        put(&mut bytes, GRAPH_HEADER_BYTES, &id(14));
        *bytes.get_mut(GRAPH_HEADER_BYTES + 44).expect("kind") = 0;
        put_u64(&mut bytes, GRAPH_HEADER_BYTES + 48, u64::from(SELECTED));
        let exposure = GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES;
        put_u64(&mut bytes, exposure, 0);
        put_u64(&mut bytes, exposure + 8, 100);
        put_u64(&mut bytes, exposure + 16, 0);
        bytes
    }

    fn descriptor_fixture(authority: [u8; 32]) -> Vec<u8> {
        let mut bytes =
            vec![0_u8; DESCRIPTOR_HEADER_BYTES + WIDTH as usize * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V2);
        put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut bytes, 16, &id(20));
        put(&mut bytes, 48, &id(21));
        put(&mut bytes, 80, &id(14));
        put(&mut bytes, 112, &id(2));
        put(&mut bytes, 144, &id(3));
        put(&mut bytes, 176, &id(4));
        put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
        put(&mut bytes, 240, &authority);
        put_u32(&mut bytes, 272, WIDTH);
        put_u64(&mut bytes, 280, DENOMINATOR);
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + DESCRIPTOR_COEFFICIENT_BYTES,
            DENOMINATOR,
        );
        bytes
    }

    fn bearer<'a>(descriptor_bytes: &'a [u8], graph_bytes: &'a [u8]) -> BearerDescriptorV2<'a> {
        let descriptor = RepresentationDescriptorV2::decode(
            descriptor_bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: id(1),
                finalized_descriptor_id: id(1),
                recomputed_descriptor_digest: id(1),
                finalized_descriptor_digest: id(1),
                record_authenticated: true,
            },
        )
        .expect("descriptor");
        let graph = RepresentationGraphV2::decode(
            graph_bytes,
            ContentAdmissionV2 {
                selected_graph_id: id(20),
                finalized_graph_id: id(20),
                recomputed_graph_digest: id(21),
                finalized_graph_digest: id(21),
                record_authenticated: true,
            },
        )
        .expect("graph");
        BearerDescriptorV2::authenticate(
            descriptor,
            graph,
            BearerBindingV2 {
                descriptor_id: id(1),
                graph_id: id(20),
                graph_digest: id(21),
                root_id: id(14),
                market: id(2),
                release_set: id(3),
                receipt_mint: id(4),
                token_program: TOKEN_2022_PROGRAM_ID,
                representation_authority: descriptor.representation_authority(),
                outcome_count: WIDTH,
                denominator: DENOMINATOR,
                selected_outcome: SELECTED,
            },
        )
        .expect("basis")
    }

    fn fixture() -> (Vec<u8>, Vec<u8>, [u8; 32]) {
        let claims_program = id(60);
        let authority = Pubkey::find_program_address(
            &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &id(1)],
            &Pubkey::new_from_array(claims_program),
        )
        .0
        .to_bytes();
        (
            descriptor_fixture(authority),
            graph_fixture(),
            claims_program,
        )
    }

    fn balances() -> BearerBalancesV2 {
        BearerBalancesV2 {
            shard_supply: 30,
            actor_shards: 30,
            structured_shards: 0,
            receipt_supply: 0,
        }
    }

    fn active(claims_program: [u8; 32], actor: [u8; 32]) -> ActiveBearerInputV2 {
        ActiveBearerInputV2 {
            caller_role: CallerRoleV2::Trading,
            claims_program,
            parent_context: id(6),
            actor,
            holder_account: HolderAccountV2::Associated,
            representation_revision: 9,
            claims_market_revision: 10,
            actor_position_revision: 11,
            custody_position_revision: 12,
            generation: 13,
            quantity: 2,
            balances: balances(),
        }
    }

    #[test]
    fn constructs_all_actions_as_the_shared_request() {
        let (descriptor_bytes, graph_bytes, claims_program) = fixture();
        let bearer = bearer(&descriptor_bytes, &graph_bytes);
        let mut denominate_row = [0_u8; ASSET_BYTES_V2];
        let denominate =
            construct_denominate(bearer, active(claims_program, id(40)), &mut denominate_row)
                .expect("denominate");
        assert_eq!(
            denominate.request.header().action,
            RepresentationActionV2::Denominate
        );
        assert_eq!(denominate.request.header().release_set, id(3));
        assert_eq!(denominate.request.header().graph_id, id(20));
        assert_eq!(denominate.request.header().selected_outcome, SELECTED);
        assert_eq!(
            denominate.request.asset(0).expect("asset").coefficient,
            DENOMINATOR
        );
        let mut encoded = vec![0_u8; REQUEST_HEADER_BYTES_V2 + ASSET_BYTES_V2];
        denominate
            .request
            .encode_into(&mut encoded)
            .expect("shared encoding");
        assert_eq!(
            RepresentationRequestV2::decode(&encoded),
            Ok(denominate.request)
        );

        let mut reconstitute_row = [0_u8; ASSET_BYTES_V2];
        let reconstitute = construct_reconstitute(
            bearer,
            active(claims_program, id(40)),
            &mut reconstitute_row,
        )
        .expect("reconstitute");
        assert_eq!(
            reconstitute.request.header().action,
            RepresentationActionV2::Reconstitute
        );

        let mut terminal_row = [0_u8; ASSET_BYTES_V2];
        let terminal = construct_redeem_terminal(
            bearer,
            TerminalBearerInputV2 {
                caller_role: CallerRoleV2::Trading,
                claims_program,
                parent_context: id(6),
                actor: id(40),
                holder_account: HolderAccountV2::Associated,
                realm: id(7),
                collateral_recipient: id(8),
                representation_revision: 9,
                claims_market_revision: 10,
                custody_position_revision: 12,
                custody_replay_revision: 13,
                generation: 14,
                quantity: 3,
                balances: balances(),
            },
            &mut terminal_row,
        )
        .expect("terminal");
        assert_eq!(
            terminal.request.header().action,
            RepresentationActionV2::RedeemTerminal
        );
        assert_eq!(
            terminal.request.header().expected_actor_position_revision,
            ABSENT_REVISION
        );
        assert_eq!(terminal.request.header().realm, id(7));
    }

    #[test]
    fn holder_transfer_does_not_rebind_static_asset_identity() {
        let (descriptor_bytes, graph_bytes, claims_program) = fixture();
        let bearer = bearer(&descriptor_bytes, &graph_bytes);
        let first =
            derive_asset_identities(bearer, claims_program, id(40), HolderAccountV2::Associated)
                .expect("first holder");
        let second =
            derive_asset_identities(bearer, claims_program, id(41), HolderAccountV2::Associated)
                .expect("second holder");
        assert_eq!(first.shard_mint, second.shard_mint);
        assert_eq!(first.claims_custody_owner, second.claims_custody_owner);
        assert_eq!(
            first.structured_custody_account,
            second.structured_custody_account
        );
        assert_ne!(first.actor_shard_account, second.actor_shard_account);

        let exact = derive_asset_identities(
            bearer,
            claims_program,
            id(41),
            HolderAccountV2::Exact(id(42)),
        )
        .expect("transferable exact holder account");
        assert_eq!(exact.actor_shard_account, id(42));
        assert_eq!(exact.shard_mint, first.shard_mint);
    }

    #[test]
    fn substituted_authority_and_invalid_scratch_refuse() {
        let (_, graph_bytes, claims_program) = fixture();
        let hostile_descriptor = descriptor_fixture(id(99));
        let hostile_bearer = bearer(&hostile_descriptor, &graph_bytes);
        assert_eq!(
            derive_asset_identities(
                hostile_bearer,
                claims_program,
                id(40),
                HolderAccountV2::Associated,
            ),
            Err(Error::RepresentationAuthorityMismatch)
        );

        let (descriptor_bytes, graph_bytes, claims_program) = fixture();
        let bearer = bearer(&descriptor_bytes, &graph_bytes);
        let mut short = [0_u8; ASSET_BYTES_V2 - 1];
        assert_eq!(
            construct_denominate(bearer, active(claims_program, id(40)), &mut short),
            Err(Error::InvalidScratch)
        );
    }
}
