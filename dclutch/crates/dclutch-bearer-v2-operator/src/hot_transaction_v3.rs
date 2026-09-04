//! Unsigned chain-derived Hot instruction construction for terminal redemption.

use dclutch_account_profile_contract::v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3,
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_hot_bump_miner_v1::{
    HotBumpCorpusV1, activated_custody_program_v1, mine_hot_bump_hints_v1,
};
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, RationalTerminalHotRequestV3, RepresentationActionV2,
    RepresentationCoordinateV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_operator::ConstructedInstructionV2;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use crate::hot_account_profile_v3::{
    InjectedCoordinateV3, LogicalCoordinateV3, logical_coordinate, logical_index,
};
use crate::open_capability_set_v3::require_open_program_selection_v3;
use crate::{
    ConstructedHotTerminalV3, Error, RationalOpenCapabilityProgramSetV3,
    RationalTerminalHotBundleV3, Result,
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3,
};

/// Checked-release evidence for the immutable Trading Hot outer.
///
/// The client does not recognize an official deployment from self-consistent
/// chain state alone. A release checker constructs this value only after a
/// user-supplied checked manifest joins the immutable ArtifactRelease and
/// current Loader observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedRationalHotOuterReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub artifact_release: [u8; 32],
    /// Digest of the checked multiprogram release manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized account projection needed to construct one Hot instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotStateV3<'a> {
    /// Exact physical Hot38 prefix in canonical ABI order.
    pub fixed_accounts: &'a [AccountMeta],
    /// Exact authenticated ExecutionStrategy transport suffix.
    pub strategy_accounts: &'a [AccountMeta],
    /// Current complete capability-root bytes used for optimistic concurrency.
    pub root_data: &'a [u8],
    /// Current complete Core Market state bytes for the frame's Market.
    ///
    /// Coordinate `HOT_MARKET_ACCOUNT_V3` of the same fixed frame, so a caller
    /// that assembled `fixed_accounts` has already fetched it. It is here for
    /// one reason: the Market's immutable identity is the preimage of its own
    /// PDA, and mining that bump is what stops the route searching for it. A
    /// caller that cannot supply the body may pass an empty slice and the
    /// route searches, exactly as it did before hints existed.
    pub market_data: &'a [u8],
    /// Current complete Market activation-cache bytes.
    ///
    /// Coordinate `HOT_ACTIVATION_CACHE_ACCOUNT_V3` of the same fixed frame.
    /// Custody is not in the frame; this account is what names the release
    /// set's Custody deployment, which is the program the transfer-authority
    /// hint is derived under. Empty degrades to a search, as above.
    pub activation_cache_data: &'a [u8],
    /// Immutable execution release set selected by Market.
    pub release_set: [u8; 32],
    /// Logical Core Market selected by the fixed frame.
    pub market: Pubkey,
    /// Immutable Market generation.
    pub generation: u64,
    /// Common finalized observation slot shared by every fetched input.
    pub finalized_slot: u64,
    /// Checked current Hot release; absent for unrecognized deployments.
    pub hot_outer: Option<CheckedRationalHotOuterReleaseV3>,
}

/// Complete unsigned Trading instruction and explicit wallet signer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotInstructionV3 {
    /// Exact user-triggered Trading instruction. Nothing here signs or submits it.
    pub instruction: Instruction,
    /// Wallet identities which must sign the eventual transaction.
    pub required_wallet_signers: Vec<Pubkey>,
    /// Exact family request digest bound into the Claims child.
    pub family_digest: [u8; 32],
    /// Checked release-manifest identity used for the operator decision.
    pub checked_manifest_digest: [u8; 32],
    /// Finalized slot shared by every chain observation used to build it.
    pub finalized_slot: u64,
}

/// Build one complete unsigned Hot38 terminal redemption instruction.
///
/// The Trading caller PDA is a signer only in the downstream CPI, so its child
/// signer flag is deliberately removed from the outer transaction. The actor's
/// existing wallet signature is preserved and the common Claims executor
/// propagates it to the exact child frame.
pub fn build_rational_terminal_hot_instruction_v3(
    state: &RationalTerminalHotStateV3<'_>,
    terminal: &ConstructedHotTerminalV3,
    bundle: &RationalTerminalHotBundleV3,
    capability_set: &RationalOpenCapabilityProgramSetV3,
    authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
) -> Result<RationalTerminalHotInstructionV3> {
    RationalTerminalHotRequestV3::decode(&terminal.family_request).map_err(Error::HotContract)?;
    require_open_program_selection_v3(
        capability_set,
        authenticated_token_behavior,
        &terminal.family_request,
        &bundle.descriptor,
    )?;
    let child = RepresentationRequestV2::decode(&terminal.claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    let header = child.header();
    if header.action != RepresentationActionV2::RedeemTerminal
        || header.asset_count != 1
        || header.descriptor_id != authenticated_token_behavior.descriptor_id()
        || header.release_set != authenticated_token_behavior.selection().release_set()
        || header.release_set != state.release_set
    {
        return Err(Error::HotInstruction);
    }
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3(
        bundle,
        authenticated_token_behavior,
    )?;
    let built = build_profiled_hot_instruction_from_claims_child_v3(
        state,
        &terminal.family_request,
        terminal.family_digest,
        &terminal.claims_child,
        // A FOURTH AUTHOR for one width, and the last one still typed. The
        // terminal Claims frame is `RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3`,
        // which now derives from the request contract's frame spec; this said
        // `49` while the operator built 50 and while the Claims composition
        // would have refused anything but 50.
        usize::from(crate::RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3),
        &bundle.account_profile,
        header.outcome_count,
    )?;
    Ok(RationalTerminalHotInstructionV3 {
        instruction: built.instruction,
        required_wallet_signers: built.required_wallet_signers,
        family_digest: terminal.family_digest,
        checked_manifest_digest: built.checked_manifest_digest,
        finalized_slot: state.finalized_slot,
    })
}

pub(crate) struct BuiltHotInstructionV3 {
    pub(crate) instruction: Instruction,
    pub(crate) required_wallet_signers: Vec<Pubkey>,
    pub(crate) checked_manifest_digest: [u8; 32],
}

pub(crate) fn build_profiled_hot_instruction_from_claims_child_v3(
    state: &RationalTerminalHotStateV3<'_>,
    family_request: &[u8],
    family_digest: [u8; 32],
    claims_child: &ConstructedInstructionV2,
    expected_child_accounts: usize,
    account_profile_bytes: &[u8],
    tail_count: u32,
) -> Result<BuiltHotInstructionV3> {
    let profile =
        AccountProfileV2::decode(account_profile_bytes).map_err(Error::AccountProfileArtifact)?;
    build_hot_instruction_from_claims_child_inner_v3(
        state,
        family_request,
        family_digest,
        claims_child,
        expected_child_accounts,
        Some((profile, tail_count)),
    )
}

fn build_hot_instruction_from_claims_child_inner_v3(
    state: &RationalTerminalHotStateV3<'_>,
    family_request: &[u8],
    family_digest: [u8; 32],
    claims_child: &ConstructedInstructionV2,
    expected_child_accounts: usize,
    profile: Option<(AccountProfileV2<'_>, u32)>,
) -> Result<BuiltHotInstructionV3> {
    let checked = state.hot_outer.ok_or(Error::HotInstruction)?;
    validate_fixed_frame(state, checked)?;
    if state.finalized_slot == 0
        || state.release_set == [0; 32]
        || checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || family_digest == [0; 32]
        || hash(family_request).to_bytes() != family_digest
    {
        return Err(Error::HotInstruction);
    }

    let child = RepresentationRequestV2::decode(&claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    let header = child.header();
    if header.release_set != state.release_set
        || header.market != state.market.to_bytes()
        || header.generation != state.generation
        || header.parent_context != family_digest
        || claims_child.request_digest != hash(&claims_child.instruction.data).to_bytes()
        || claims_child.instruction.accounts.len() != expected_child_accounts
        || claims_child.instruction.program_id == Pubkey::default()
    {
        return Err(Error::HotInstruction);
    }
    let child_accounts = &claims_child.instruction.accounts;
    // Named, not numbered. The caller PDA and the wallet actor are the frame's
    // only two signers and the Claims program is where the child names itself;
    // all three used to be typed here as 0, 3 and 14.
    let caller = child_frame_index(RepresentationCoordinateV2::CallerAuthority)?;
    let actor = child_frame_index(RepresentationCoordinateV2::Actor)?;
    let claims_program = child_frame_index(RepresentationCoordinateV2::ClaimsProgram)?;
    if child_accounts
        .get(caller)
        .is_none_or(|account| !account.is_signer)
        || child_accounts
            .get(actor)
            .is_none_or(|account| !account.is_signer)
        || child_accounts
            .iter()
            .enumerate()
            .any(|(index, account)| index != caller && index != actor && account.is_signer)
        || child_accounts
            .get(claims_program)
            .is_none_or(|account| account.pubkey != claims_child.instruction.program_id)
    {
        return Err(Error::HotInstruction);
    }

    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len()).map_err(|_| Error::HotInstruction)?,
        state.release_set,
        state.market.to_bytes(),
        state.generation,
        hash(state.root_data).to_bytes(),
    )
    .map_err(Error::HotEnvelope)?
    .with_bump_hints(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: state.market,
        market_data: state.market_data,
        root_data: state.root_data,
        core_program: fixed_key(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?,
        trading_program: checked.trading_program,
        custody_program: activated_custody_program_v1(state.activation_cache_data),
        release_set: state.release_set,
    }));
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_request.len())
            .ok_or(Error::HotInstruction)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(family_request);

    let physical_child_accounts = match profile {
        Some((profile, tail_count)) => compact_profile13_child_accounts_v3(
            state,
            child_accounts,
            expected_child_accounts,
            profile,
            tail_count,
        )?,
        None => child_accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let mut outer = account.clone();
                if index == caller {
                    outer.is_signer = false;
                }
                outer
            })
            .collect(),
    };
    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| count.checked_add(physical_child_accounts.len()))
            .ok_or(Error::HotInstruction)?,
    );
    accounts.extend_from_slice(state.fixed_accounts);
    accounts.extend_from_slice(state.strategy_accounts);
    accounts.extend(physical_child_accounts);
    Ok(BuiltHotInstructionV3 {
        instruction: Instruction {
            program_id: checked.trading_program,
            accounts,
            data,
        },
        required_wallet_signers: vec![
            child_accounts
                .get(actor)
                .ok_or(Error::HotInstruction)?
                .pubkey,
        ],
        checked_manifest_digest: checked.checked_manifest_digest,
    })
}

const INJECTED_ACCOUNTS: usize = crate::RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3 as usize;

fn compact_profile13_child_accounts_v3(
    state: &RationalTerminalHotStateV3<'_>,
    child_accounts: &[AccountMeta],
    expected_child_accounts: usize,
    profile: AccountProfileV2<'_>,
    tail_count: u32,
) -> Result<Vec<AccountMeta>> {
    const INJECTED: usize = INJECTED_ACCOUNTS;
    let expected_logical = INJECTED
        .checked_add(expected_child_accounts)
        .ok_or(Error::HotInstruction)?;
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.dynamic_fixed_span_count() != 0
        || profile
            .logical_account_count_with_dynamic_spans(tail_count, &[])
            .map_err(Error::AccountProfileArtifact)?
            != expected_logical
        || child_accounts.len() != expected_child_accounts
    {
        return Err(Error::HotInstruction);
    }
    let physical = profile
        .physical_account_count_with_dynamic_spans(tail_count, &[])
        .map_err(Error::AccountProfileArtifact)?;
    if physical < INJECTED {
        return Err(Error::HotInstruction);
    }
    for coordinate in 0..INJECTED {
        if profile
            .representative_with_dynamic_spans(tail_count, &[], coordinate)
            .map_err(Error::AccountProfileArtifact)?
            != coordinate
        {
            return Err(Error::HotInstruction);
        }
        let expected = injected_meta_v3(state, coordinate)?;
        let (signer, writable) = physical_privileges_v3(profile, tail_count, coordinate)?;
        if expected.is_signer != signer || expected.is_writable != writable {
            return Err(Error::HotInstruction);
        }
    }

    let mut output = Vec::with_capacity(physical - INJECTED);
    for (child_index, account) in child_accounts.iter().enumerate() {
        let logical = INJECTED
            .checked_add(child_index)
            .ok_or(Error::HotInstruction)?;
        let route = profile
            .route_privileges_with_dynamic_spans(tail_count, &[], logical)
            .map_err(Error::AccountProfileArtifact)?;
        let caller = child_frame_index(RepresentationCoordinateV2::CallerAuthority)?;
        if account.is_writable != route.writable()
            || (child_index != caller && account.is_signer != route.signer())
            || (child_index == caller && !account.is_signer)
        {
            return Err(Error::HotInstruction);
        }
        let representative = profile
            .representative_with_dynamic_spans(tail_count, &[], logical)
            .map_err(Error::AccountProfileArtifact)?;
        let representative_meta = if representative < INJECTED {
            injected_meta_v3(state, representative)?
        } else {
            child_accounts
                .get(
                    representative
                        .checked_sub(INJECTED)
                        .ok_or(Error::HotInstruction)?,
                )
                .ok_or(Error::HotInstruction)?
        };
        if representative_meta.pubkey != account.pubkey {
            return Err(Error::HotInstruction);
        }
        if representative == logical {
            let (signer, writable) = physical_privileges_v3(profile, tail_count, representative)?;
            let mut outer = account.clone();
            outer.is_signer = signer;
            outer.is_writable = writable;
            output.push(outer);
        }
    }
    if output.len() != physical - INJECTED {
        return Err(Error::HotInstruction);
    }
    Ok(output)
}

fn physical_privileges_v3(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    representative: usize,
) -> Result<(bool, bool)> {
    let logical = profile
        .logical_account_count_with_dynamic_spans(tail_count, &[])
        .map_err(Error::AccountProfileArtifact)?;
    let mut signer = false;
    let mut writable = false;
    for coordinate in 0..logical {
        if profile
            .representative_with_dynamic_spans(tail_count, &[], coordinate)
            .map_err(Error::AccountProfileArtifact)?
            == representative
        {
            let route = profile
                .route_privileges_with_dynamic_spans(tail_count, &[], coordinate)
                .map_err(Error::AccountProfileArtifact)?;
            signer |= route.signer();
            writable |= route.writable();
        }
    }
    Ok((signer, writable))
}

/// Where one Claims-child role sits inside the child frame.
fn child_frame_index(coordinate: RepresentationCoordinateV2) -> Result<usize> {
    logical_index(LogicalCoordinateV3::Child(coordinate))
        .and_then(|index| {
            index.checked_sub(crate::RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3 as usize)
        })
        .ok_or(Error::HotInstruction)
}

fn injected_meta_v3<'a>(
    state: &'a RationalTerminalHotStateV3<'_>,
    logical_coordinate_index: usize,
) -> Result<&'a AccountMeta> {
    let physical_coordinate = match logical_coordinate(logical_coordinate_index) {
        Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::Root)) => HOT_ROOT_ACCOUNT_V3,
        Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::TokenBehaviorSelection)) => {
            HOT_CONFIG_RAW_ACCOUNT_V3
        }
        Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::ProductRecord)) => {
            HOT_PRODUCT_RAW_ACCOUNT_V3
        }
        Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::PortfolioRecord)) => {
            HOT_PORTFOLIO_RAW_ACCOUNT_V3
        }
        Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::LinkedProductBasis)) => {
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3
        }
        Some(LogicalCoordinateV3::Child(_)) | None => return Err(Error::HotInstruction),
    };
    state
        .fixed_accounts
        .get(physical_coordinate)
        .ok_or(Error::HotInstruction)
}

/// One fixed-frame coordinate's key, named rather than indexed.
///
/// `validate_fixed_frame` has already required the frame's exact width, so this
/// cannot be absent on any path that reaches it; it returns a `Result` so that
/// a future caller placed BEFORE that check cannot silently read a default key.
fn fixed_key(state: &RationalTerminalHotStateV3<'_>, coordinate: usize) -> Result<Pubkey> {
    Ok(state
        .fixed_accounts
        .get(coordinate)
        .ok_or(Error::HotInstruction)?
        .pubkey)
}

fn validate_fixed_frame(
    state: &RationalTerminalHotStateV3<'_>,
    checked: CheckedRationalHotOuterReleaseV3,
) -> Result<()> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state
            .fixed_accounts
            .get(HOT_MARKET_ACCOUNT_V3)
            .is_none_or(|account| account.pubkey != state.market)
        || state
            .fixed_accounts
            .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .is_none_or(|account| account.pubkey != checked.trading_program)
        || state
            .fixed_accounts
            .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .is_none_or(|account| account.pubkey != sysvar::rent::ID)
        || state
            .fixed_accounts
            .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .is_none_or(|account| account.pubkey != sysvar::instructions::ID)
    {
        return Err(Error::HotInstruction);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.is_signer || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3) {
            return Err(Error::HotInstruction);
        }
        if state
            .fixed_accounts
            .iter()
            .take(index)
            .any(|prior| prior.pubkey == account.pubkey)
        {
            return Err(Error::HotInstruction);
        }
    }
    for coordinate in [
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
    ] {
        if state
            .fixed_accounts
            .get(coordinate)
            .is_none_or(|account| account.pubkey == Pubkey::default())
        {
            return Err(Error::HotInstruction);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_rational_representation_v2_contract::{
        ABSENT_REVISION, ASSET_BYTES_V3, AssetV2, CallerRoleV2,
        RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RationalTerminalHotRequestV3,
        RepresentationActionV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
    };
    use dclutch_rational_representation_v2_operator::ConstructedInstructionV2;
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
    use solana_sdk_ids::system_program;

    use crate::test_open_fixture_v3::{authenticated_token_behavior_v3, open_artifact_fixture_v3};

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn terminal() -> ConstructedHotTerminalV3 {
        let mut asset = [0_u8; ASSET_BYTES_V3];
        AssetV2 {
            shard_mint: key(20).to_bytes(),
            actor_shard_account: key(21).to_bytes(),
            structured_custody_account: key(22).to_bytes(),
            claims_custody_owner: key(23).to_bytes(),
            coefficient: 10,
            expected_shard_supply: 100,
            expected_actor_shards: 30,
            expected_structured_shards: 0,
        }
        .encode_into(&mut asset)
        .expect("asset");
        let header = RepresentationRequestHeaderV2 {
            action: RepresentationActionV2::RedeemTerminal,
            caller_role: CallerRoleV2::Trading,
            release_set: key(1).to_bytes(),
            market: key(2).to_bytes(),
            graph_id: key(3).to_bytes(),
            descriptor_id: key(4).to_bytes(),
            parent_context: key(5).to_bytes(),
            actor: key(6).to_bytes(),
            receipt_mint: key(7).to_bytes(),
            receipt_account: [0; 32],
            representation_authority: key(8).to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            realm: key(9).to_bytes(),
            collateral_recipient: key(10).to_bytes(),
            expected_representation_revision: 4,
            expected_claims_market_revision: 11,
            expected_actor_position_revision: ABSENT_REVISION,
            expected_custody_position_revision: 12,
            expected_custody_replay_revision: 13,
            generation: 14,
            quantity: 2,
            denominator: 10,
            expected_receipt_supply: 0,
            outcome_count: 3,
            selected_outcome: 2,
            asset_count: 1,
        };
        let child = RepresentationRequestV2::new(header, &asset).expect("child");
        let mut child_data = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        child.encode_into(&mut child_data).expect("encode child");
        let mut family_request = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        let family = RationalTerminalHotRequestV3::from_child_into(child, &mut family_request)
            .expect("family");
        let family_digest = hash(family.as_bytes()).to_bytes();
        let mut exact_child = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        family
            .specialize_child_into(family_digest, &mut exact_child)
            .expect("exact child");
        let claims_program = key(70);
        // A FIFTH AUTHOR for the same width AND a sixth for the frame's order:
        // this fixture typed the child's writable set and its three same-frame
        // aliases as bare indices, which is why it stayed green while the
        // profile it feeds was one account short. Both now come off the profile
        // module's declaration, by role.
        let child_width = usize::from(crate::RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3);
        let child_rule = |index: usize| {
            crate::hot_account_profile_v3::declared(
                logical_coordinate(index + INJECTED_ACCOUNTS).expect("child coordinate"),
            )
        };
        let mut metas = (0..child_width)
            .map(|index| {
                let rule = child_rule(index);
                let mut meta = AccountMeta::new_readonly(
                    key(100_u8.wrapping_add(u8::try_from(index).expect("child key"))),
                    false,
                );
                meta.is_writable = rule.writable;
                meta.is_signer = rule.signer;
                meta
            })
            .collect::<Vec<_>>();
        let at = |coordinate| child_frame_index(coordinate).expect("child role");
        metas
            .get_mut(at(RepresentationCoordinateV2::CallerAuthority))
            .expect("caller meta")
            .is_signer = true;
        *metas
            .get_mut(at(RepresentationCoordinateV2::Actor))
            .expect("actor meta") = AccountMeta::new_readonly(key(6), true);
        *metas
            .get_mut(at(RepresentationCoordinateV2::ClaimsProgram))
            .expect("Claims meta") = AccountMeta::new_readonly(claims_program, false);
        // Every alias whose source is another CHILD coordinate; the three that
        // point at injected Hot accounts are bound by `bind_injected_aliases`.
        for index in 0..child_width {
            let Some(LogicalCoordinateV3::Child(source)) = child_rule(index).alias else {
                continue;
            };
            let mut account = metas.get(at(source)).expect("alias source").clone();
            account.is_signer = false;
            account.is_writable = false;
            *metas.get_mut(index).expect("alias target") = account;
        }
        ConstructedHotTerminalV3 {
            family_request,
            family_digest,
            claims_child: ConstructedInstructionV2 {
                instruction: Instruction {
                    program_id: claims_program,
                    accounts: metas,
                    data: exact_child.to_vec(),
                },
                request_digest: hash(&exact_child).to_bytes(),
                representation_authority: key(8),
                representation_replay: key(80),
                claims_aggregate: key(81),
                assets: Vec::new(),
                terminal: None,
            },
        }
    }

    fn state<'a>(fixed: &'a [AccountMeta], root: &'a [u8]) -> RationalTerminalHotStateV3<'a> {
        RationalTerminalHotStateV3 {
            fixed_accounts: fixed,
            strategy_accounts: &[],
            root_data: root,
            // No corpus, so every hint degrades to zero and the route
            // searches. These cases pin the frame geometry, not the hint
            // block; the derivation has its own tests in
            // `dclutch-hot-bump-miner-v1`, and its byte identity with the
            // campaign builder is asserted on real ELFs by
            // `rational_representation_v2_program_test`.
            market_data: &[],
            activation_cache_data: &[],
            release_set: key(1).to_bytes(),
            market: key(2),
            generation: 14,
            finalized_slot: 99,
            hot_outer: Some(CheckedRationalHotOuterReleaseV3 {
                trading_program: key(60),
                artifact_release: key(61).to_bytes(),
                checked_manifest_digest: key(62).to_bytes(),
            }),
        }
    }

    fn fixed() -> Vec<AccountMeta> {
        let mut fixed = (0_u8..u8::try_from(HOT_FIXED_ACCOUNT_COUNT_V3).expect("Hot width"))
            .map(|index| AccountMeta::new_readonly(key(150_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        *fixed.get_mut(HOT_MARKET_ACCOUNT_V3).expect("Market meta") =
            AccountMeta::new_readonly(key(2), false);
        fixed
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root meta")
            .is_writable = true;
        *fixed
            .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .expect("Trading meta") = AccountMeta::new_readonly(key(60), false);
        *fixed
            .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .expect("Rent meta") = AccountMeta::new_readonly(sysvar::rent::ID, false);
        *fixed
            .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("Instructions meta") =
            AccountMeta::new_readonly(sysvar::instructions::ID, false);
        // Avoid accidental collision with the system IDs used above.
        assert_ne!(
            fixed
                .get(HOT_LINKED_BASIS_RAW_ACCOUNT_V3)
                .expect("basis meta")
                .pubkey,
            system_program::ID
        );
        fixed
    }

    fn bind_injected_aliases(terminal: &mut ConstructedHotTerminalV3, fixed: &[AccountMeta]) {
        for (child, injected) in [
            (
                RepresentationCoordinateV2::LinkedBasisRecord,
                HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
            ),
            (
                RepresentationCoordinateV2::ProductRecord,
                HOT_PRODUCT_RAW_ACCOUNT_V3,
            ),
            (
                RepresentationCoordinateV2::PortfolioRecord,
                HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            ),
        ] {
            let child = child_frame_index(child).expect("child role");
            let mut account = fixed.get(injected).expect("injected account").clone();
            account.is_signer = false;
            account.is_writable = false;
            *terminal
                .claims_child
                .instruction
                .accounts
                .get_mut(child)
                .expect("injected child alias") = account;
        }
    }

    #[test]
    fn builds_unsigned_hot38_and_preserves_only_wallet_signer() {
        let mut terminal = terminal();
        let fixed = fixed();
        bind_injected_aliases(&mut terminal, &fixed);
        let state = state(&fixed, &[7; 64]);
        let artifacts = open_artifact_fixture_v3(key(9).to_bytes(), key(1).to_bytes(), 258);
        let report = build_rational_terminal_hot_instruction_v3(
            &state,
            &terminal,
            &artifacts.redeem,
            &artifacts.set,
            artifacts.token_behavior,
        )
        .expect("hot");
        assert_eq!(report.instruction.program_id, key(60));
        // Physical width is the child frame minus its aliases, both counted off
        // the declaration rather than typed.
        let aliases = (0..usize::from(crate::RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3))
            .filter(|index| {
                crate::hot_account_profile_v3::declared(
                    logical_coordinate(index + INJECTED_ACCOUNTS).expect("child coordinate"),
                )
                .alias
                .is_some()
            })
            .count();
        assert_eq!(
            report.instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3
                + usize::from(crate::RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3)
                - aliases,
        );
        assert!(
            !report
                .instruction
                .accounts
                .get(
                    HOT_FIXED_ACCOUNT_COUNT_V3
                        + child_frame_index(RepresentationCoordinateV2::CallerAuthority)
                            .expect("caller role")
                )
                .expect("outer caller")
                .is_signer
        );
        assert!(
            report
                .instruction
                .accounts
                .get(
                    HOT_FIXED_ACCOUNT_COUNT_V3
                        + child_frame_index(RepresentationCoordinateV2::Actor).expect("actor role")
                )
                .expect("outer actor")
                .is_signer
        );
        assert_eq!(report.required_wallet_signers, vec![key(6)]);
        let (envelope, family) =
            HotExecutionEnvelopeV3::split_instruction(&report.instruction.data).expect("envelope");
        assert_eq!(envelope.market(), key(2).to_bytes());
        assert_eq!(family, terminal.family_request);
    }

    #[test]
    fn refuses_unchecked_release_and_noncanonical_actor_signer() {
        let mut terminal = terminal();
        let fixed = fixed();
        bind_injected_aliases(&mut terminal, &fixed);
        let mut state = state(&fixed, &[7; 64]);
        let artifacts = open_artifact_fixture_v3(key(9).to_bytes(), key(1).to_bytes(), 258);
        state.hot_outer = None;
        assert_eq!(
            build_rational_terminal_hot_instruction_v3(
                &state,
                &terminal,
                &artifacts.redeem,
                &artifacts.set,
                artifacts.token_behavior,
            ),
            Err(Error::HotInstruction)
        );
        state.hot_outer = Some(CheckedRationalHotOuterReleaseV3 {
            trading_program: key(60),
            artifact_release: key(61).to_bytes(),
            checked_manifest_digest: key(62).to_bytes(),
        });
        terminal
            .claims_child
            .instruction
            .accounts
            .get_mut(3)
            .expect("actor meta")
            .is_signer = false;
        assert_eq!(
            build_rational_terminal_hot_instruction_v3(
                &state,
                &terminal,
                &artifacts.redeem,
                &artifacts.set,
                artifacts.token_behavior,
            ),
            Err(Error::HotInstruction)
        );

        let hostile_behavior = authenticated_token_behavior_v3(
            key(4).to_bytes(),
            key(99).to_bytes(),
            key(1).to_bytes(),
            3,
        );
        assert_eq!(
            build_rational_terminal_hot_instruction_v3(
                &state,
                &terminal,
                &artifacts.redeem,
                &artifacts.set,
                hostile_behavior,
            ),
            Err(Error::ContentIdentity)
        );
    }
}
