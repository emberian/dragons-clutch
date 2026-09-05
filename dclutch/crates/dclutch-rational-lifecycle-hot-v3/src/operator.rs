//! Unsigned, chain-derived Hot instruction construction for lifecycle actions.

use dclutch_market::capability_program::hot_v3::{
    HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HotBumpHintsV1,
};
#[cfg(test)]
use dclutch_market::capability_program::hot_v3::{
    HOT_FAMILY_REQUEST_OFFSET_V3, HotExecutionEnvelopeV3,
};
use dclutch_hot_bump_miner_v1::{HotBumpCorpusV1, mine_hot_bump_hints_v1};
#[cfg(test)]
use dclutch_claims::rational_lifecycle::{
    LifecycleRequestV2, hot_v3::RationalLifecycleHotRequestV3,
};
#[cfg(test)]
use solana_program::hash::hash;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use crate::{Error, Result};
#[cfg(test)]
use crate::{lifecycle_claims_account_count_v3, validate_action_geometry};

// IPv6 minimum-MTU Solana packet payload. Address lookup tables can compress
// account keys, but never instruction bytes themselves.
pub(crate) const MAX_SOLANA_PACKET_BYTES: usize = 1_232;

/// Checked release evidence for the immutable Trading Hot outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedRationalLifecycleHotOuterV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub artifact_release: [u8; 32],
    /// Digest of the checked multiprogram release manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized account projection needed to construct one Hot instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotStateV3<'a> {
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
    /// Immutable execution release set selected by Market.
    pub release_set: [u8; 32],
    /// Logical Core Market selected by the fixed frame.
    pub market: Pubkey,
    /// Immutable Market generation.
    pub generation: u64,
    /// Common finalized observation slot shared by every fetched input.
    pub finalized_slot: u64,
    /// Checked current Hot release; absent for unrecognized deployments.
    pub hot_outer: Option<CheckedRationalLifecycleHotOuterV3>,
}

/// Complete unsigned Trading instruction plus transaction-geometry facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotInstructionV3 {
    /// Exact user-triggered Trading instruction. Nothing here signs or submits it.
    pub instruction: Instruction,
    /// Wallet identities required by child semantics; lifecycle has none.
    pub required_wallet_signers: Vec<Pubkey>,
    /// Exact family request digest bound into the Claims child.
    pub family_digest: [u8; 32],
    /// Checked release-manifest identity used for the operator decision.
    pub checked_manifest_digest: [u8; 32],
    /// Finalized slot shared by every chain observation used to build it.
    pub finalized_slot: u64,
    /// The account geometry requires a v0 message with address lookup tables.
    pub requires_v0_address_lookup: bool,
}

/// Build one complete unsigned Hot38 lifecycle instruction from an exact Claims child.
///
/// The child caller-authority PDA is a signer only during Trading's downstream
/// CPI and is therefore deliberately not a transaction signer. No wallet
/// identity is smuggled into the representation lifecycle authority path.
#[cfg(test)]
pub fn build_rational_lifecycle_hot_instruction_v3(
    state: &RationalLifecycleHotStateV3<'_>,
    claims_child: &Instruction,
) -> Result<RationalLifecycleHotInstructionV3> {
    let checked = state.hot_outer.ok_or(Error::Operator)?;
    validate_fixed_frame(state, checked)?;
    if state.finalized_slot == 0
        || state.release_set == [0; 32]
        || checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || claims_child.program_id == Pubkey::default()
    {
        return Err(Error::Operator);
    }

    let child = LifecycleRequestV2::decode(&claims_child.data).map_err(|_| Error::Operator)?;
    let header = child.header();
    validate_action_geometry(header.action, header.coordinate_count)?;
    if header.release_set != state.release_set
        || header.market != state.market.to_bytes()
        || header.generation != state.generation
        || claims_child.accounts.len()
            != usize::from(lifecycle_claims_account_count_v3(
                header.action,
                header.coordinate_count,
            )?)
    {
        return Err(Error::Operator);
    }
    validate_child_frame(claims_child, header.action)?;

    let mut family_bytes = vec![0_u8; claims_child.data.len()];
    let family = RationalLifecycleHotRequestV3::from_child_into(child, &mut family_bytes)
        .map_err(Error::Lifecycle)?;
    let family_digest = hash(family.as_bytes()).to_bytes();
    let mut exact_child = vec![0_u8; claims_child.data.len()];
    family
        .specialize_child_into(family_digest, &mut exact_child)
        .map_err(Error::Lifecycle)?;
    if exact_child != claims_child.data {
        return Err(Error::Operator);
    }

    let request_bytes = u32::try_from(family_bytes.len()).map_err(|_| Error::Operator)?;
    let envelope = HotExecutionEnvelopeV3::new(
        request_bytes,
        state.release_set,
        state.market.to_bytes(),
        state.generation,
        hash(state.root_data).to_bytes(),
    )
    .map_err(|_| Error::Operator)?
    .with_bump_hints(lifecycle_hot_bump_hints_v3(state, checked)?);
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_bytes.len())
            .ok_or(Error::Operator)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&family_bytes);
    if data.len() > MAX_SOLANA_PACKET_BYTES {
        return Err(Error::Operator);
    }

    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| count.checked_add(claims_child.accounts.len()))
            .ok_or(Error::Operator)?,
    );
    accounts.extend_from_slice(state.fixed_accounts);
    accounts.extend_from_slice(state.strategy_accounts);
    for (index, account) in claims_child.accounts.iter().enumerate() {
        let mut outer = account.clone();
        if index == 0 {
            outer.is_signer = false;
        }
        accounts.push(outer);
    }
    Ok(RationalLifecycleHotInstructionV3 {
        instruction: Instruction {
            program_id: checked.trading_program,
            accounts,
            data,
        },
        required_wallet_signers: Vec::new(),
        family_digest,
        checked_manifest_digest: checked.checked_manifest_digest,
        finalized_slot: state.finalized_slot,
        requires_v0_address_lookup: true,
    })
}

pub(crate) fn validate_child_frame(
    claims_child: &Instruction,
    action: dclutch_claims::rational_lifecycle::LifecycleActionV2,
) -> Result<()> {
    let accounts = &claims_child.accounts;
    if accounts
        .first()
        .is_none_or(|account| !account.is_signer || account.is_writable)
        || accounts
            .get(1)
            .is_none_or(|account| account.pubkey == Pubkey::default())
        || accounts
            .get(3)
            .is_none_or(|account| account.pubkey != claims_child.program_id)
        || accounts
            .iter()
            .enumerate()
            .any(|(index, account)| index != 0 && account.is_signer)
    {
        return Err(Error::Operator);
    }
    for (index, account) in accounts.iter().enumerate() {
        let writable = match index {
            12 => matches!(
                action,
                dclutch_claims::rational_lifecycle::LifecycleActionV2::ActivateReceipt
                    | dclutch_claims::rational_lifecycle::LifecycleActionV2::RetireReceipt
            ),
            14 => action.retires(),
            21..=24 => matches!(
                action,
                dclutch_claims::rational_lifecycle::LifecycleActionV2::ActivateCoordinate
                    | dclutch_claims::rational_lifecycle::LifecycleActionV2::RetireCoordinate
            ),
            _ => false,
        };
        if account.is_writable != writable {
            return Err(Error::Operator);
        }
    }
    Ok(())
}

/// Mine the bumps every lifecycle Hot route would otherwise search for.
///
/// The DERIVATION is `dclutch_hot_bump_miner_v1`'s, shared with the Direct and
/// Dealer LP builders, the Rational public outer builders and the campaign's
/// bundle builder. What this function owns is the CORPUS: which coordinate of
/// the lifecycle Hot frame is the Market, which is the root, and which program
/// key each is derived under. All four lifecycle builders -- common, compact
/// V4, selected V5, selected V6 -- share one fixed frame and therefore one
/// corpus, which is why this is a crate function rather than four.
///
/// # Why the Custody transfer authority is deliberately left searching
///
/// Every lifecycle action executes a ONE-ROUTE `FixedRole::Claims`
/// EffectProgram (`effect::encode_lifecycle_effect_program_v3`, and
/// `compact_artifacts_v4` refuses any route whose role is not `Claims`), so no
/// lifecycle route ever reaches `execute_custody_route_v3` and there is no
/// Custody leg to spend `child_relay[1]` on. Supplying the Market activation
/// cache to mine a byte no reader reads would be a state field with no
/// consumer. `lifecycle` and `child_caller` stay zero for the reasons
/// `HotBumpHintsV1` gives: their seeds end in a digest over a request projected
/// ON chain, which no off-chain producer holds.
///
/// Total, like the miner: an unsupplied or undecodable body yields a zero and
/// the reader searches, which is the block's whole contract.
pub(crate) fn lifecycle_hot_bump_hints_v3(
    state: &RationalLifecycleHotStateV3<'_>,
    checked: CheckedRationalLifecycleHotOuterV3,
) -> Result<HotBumpHintsV1> {
    Ok(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: state.market,
        market_data: state.market_data,
        root_data: state.root_data,
        core_program: fixed_key(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?,
        trading_program: checked.trading_program,
        custody_program: None,
        release_set: state.release_set,
    }))
}

/// One fixed-frame coordinate's key, named rather than indexed.
///
/// `validate_fixed_frame` has already required the frame's exact width, so this
/// cannot be absent on any path that reaches it; it returns a `Result` so that
/// a future caller placed BEFORE that check cannot silently read a default key.
fn fixed_key(state: &RationalLifecycleHotStateV3<'_>, coordinate: usize) -> Result<Pubkey> {
    Ok(state
        .fixed_accounts
        .get(coordinate)
        .ok_or(Error::Operator)?
        .pubkey)
}

pub(crate) fn validate_fixed_frame(
    state: &RationalLifecycleHotStateV3<'_>,
    checked: CheckedRationalLifecycleHotOuterV3,
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
        return Err(Error::Operator);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.is_signer || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3) {
            return Err(Error::Operator);
        }
        if state
            .fixed_accounts
            .iter()
            .take(index)
            .any(|prior| prior.pubkey == account.pubkey)
        {
            return Err(Error::Operator);
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
            return Err(Error::Operator);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::rational_lifecycle::{
        LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_BYTES_V2,
        LIFECYCLE_HEADER_BYTES_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LifecycleActionV2,
        LifecycleCoordinateV2, LifecycleHeaderV2,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
    use solana_sdk_ids::{system_program, sysvar};

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn exact_child_with_coordinate_count(
        action: LifecycleActionV2,
        coordinate_count: u32,
    ) -> Instruction {
        let mut coordinate_bytes = Vec::new();
        for index in 0..coordinate_count {
            let tag = u8::try_from(index).expect("small row");
            let mut row = [0_u8; LIFECYCLE_COORDINATE_BYTES_V2];
            LifecycleCoordinateV2 {
                outcome: index.checked_add(1).expect("outcome"),
                coefficient: 1,
                shard_mint: key(31_u8.checked_add(tag).expect("shard key")).to_bytes(),
                structured_custody_account: key(41_u8.checked_add(tag).expect("custody key"))
                    .to_bytes(),
                claims_custody_owner: key(51_u8.checked_add(tag).expect("owner key")).to_bytes(),
                claims_custody_position: key(61_u8.checked_add(tag).expect("Position key"))
                    .to_bytes(),
                position_admission: key(71_u8.checked_add(tag).expect("admission key")).to_bytes(),
                observed_shard_lamports: 10,
                observed_structured_lamports: 11,
                observed_position_lamports: 12,
                observed_admission_lamports: 13,
                shard_rent_principal: 10,
                structured_rent_principal: 11,
                position_rent_principal: 12,
                admission_rent_principal: 13,
                expected_shard_supply: 0,
                expected_structured_amount: 0,
                expected_position_revision: 0,
            }
            .encode_into(&mut row)
            .expect("row");
            coordinate_bytes.extend_from_slice(&row);
        }
        let header = LifecycleHeaderV2 {
            action,
            release_set: key(1).to_bytes(),
            market: key(2).to_bytes(),
            graph_id: key(3).to_bytes(),
            descriptor_id: key(4).to_bytes(),
            parent_context: key(5).to_bytes(),
            representation_authority: key(6).to_bytes(),
            receipt_mint: key(7).to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: key(8).to_bytes(),
            rent_program: key(9).to_bytes(),
            generation: 14,
            expected_claims_market_revision: 3,
            observed_receipt_lamports: 10,
            receipt_rent_principal: 10,
            expected_receipt_supply: 0,
            outcome_count: 258,
            coordinate_count,
            rent_credit_before: 100,
            rent_credit_after: if action.retires() { 110 } else { 100 },
        };
        let request = LifecycleRequestV2::new(header, &coordinate_bytes).expect("request");
        let mut family = vec![
            0_u8;
            LIFECYCLE_HEADER_BYTES_V2
                + usize::try_from(coordinate_count).expect("count")
                    * LIFECYCLE_COORDINATE_BYTES_V2
        ];
        let family =
            RationalLifecycleHotRequestV3::from_child_into(request, &mut family).expect("family");
        let digest = hash(family.as_bytes()).to_bytes();
        let mut child_data = vec![0_u8; family.as_bytes().len()];
        family
            .specialize_child_into(digest, &mut child_data)
            .expect("child");

        let count = if action == LifecycleActionV2::RetireReceipt {
            LIFECYCLE_COMMON_ACCOUNT_COUNT_V2
                + usize::try_from(coordinate_count).expect("count")
                    * LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2
        } else {
            usize::from(lifecycle_claims_account_count_v3(action, coordinate_count).expect("frame"))
        };
        let claims = key(70);
        let mut accounts = (0..count)
            .map(|index| {
                AccountMeta::new_readonly(
                    Pubkey::new_from_array([u8::try_from(index).expect("small") + 100; 32]),
                    false,
                )
            })
            .collect::<Vec<_>>();
        *accounts.get_mut(0).expect("caller") = AccountMeta::new_readonly(key(71), true);
        *accounts.get_mut(1).expect("Trading") = AccountMeta::new_readonly(key(60), false);
        *accounts.get_mut(3).expect("Claims") = AccountMeta::new_readonly(claims, false);
        if matches!(
            action,
            LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
        ) {
            accounts.get_mut(12).expect("receipt").is_writable = true;
        }
        if action.retires() {
            accounts.get_mut(14).expect("RentCredit").is_writable = true;
        }
        if matches!(
            action,
            LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
        ) {
            for account in accounts.get_mut(21..=24).expect("coordinate resources") {
                account.is_writable = true;
            }
        }
        Instruction {
            program_id: claims,
            accounts,
            data: child_data,
        }
    }

    fn exact_child(action: LifecycleActionV2) -> Instruction {
        let coordinate_count = u32::from(!matches!(action, LifecycleActionV2::ActivateReceipt));
        exact_child_with_coordinate_count(action, coordinate_count)
    }

    fn fixed() -> Vec<AccountMeta> {
        let mut fixed = (0_u8..u8::try_from(HOT_FIXED_ACCOUNT_COUNT_V3).expect("fixed hot prefix"))
            .map(|index| AccountMeta::new_readonly(key(150_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        *fixed.get_mut(HOT_MARKET_ACCOUNT_V3).expect("Market") =
            AccountMeta::new_readonly(key(2), false);
        fixed
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .is_writable = true;
        *fixed
            .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .expect("Trading") = AccountMeta::new_readonly(key(60), false);
        *fixed.get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3).expect("Rent") =
            AccountMeta::new_readonly(sysvar::rent::ID, false);
        *fixed
            .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("Instructions") = AccountMeta::new_readonly(sysvar::instructions::ID, false);
        assert_ne!(
            fixed
                .get(HOT_LINKED_BASIS_RAW_ACCOUNT_V3)
                .expect("basis")
                .pubkey,
            system_program::ID
        );
        fixed
    }

    fn state<'a>(fixed: &'a [AccountMeta]) -> RationalLifecycleHotStateV3<'a> {
        RationalLifecycleHotStateV3 {
            fixed_accounts: fixed,
            strategy_accounts: &[],
            root_data: &[7; 64],
            // No Market body, so the `market` hint degrades to zero and the
            // route searches. These cases pin the frame geometry, not the hint
            // block; the derivation has its own tests in
            // `dclutch-hot-bump-miner-v1`.
            market_data: &[],
            release_set: key(1).to_bytes(),
            market: key(2),
            generation: 14,
            finalized_slot: 99,
            hot_outer: Some(CheckedRationalLifecycleHotOuterV3 {
                trading_program: key(60),
                artifact_release: key(61).to_bytes(),
                checked_manifest_digest: key(62).to_bytes(),
            }),
        }
    }

    #[test]
    fn selected_actions_build_unsigned_v0_alt_hot_instructions() {
        let fixed = fixed();
        for action in [
            LifecycleActionV2::ActivateReceipt,
            LifecycleActionV2::ActivateCoordinate,
            LifecycleActionV2::RetireCoordinate,
        ] {
            let child = exact_child(action);
            let result = build_rational_lifecycle_hot_instruction_v3(&state(&fixed), &child)
                .expect("Hot instruction");
            assert!(result.required_wallet_signers.is_empty());
            assert!(result.requires_v0_address_lookup);
            assert_eq!(result.instruction.program_id, key(60));
            assert!(
                !result
                    .instruction
                    .accounts
                    .get(HOT_FIXED_ACCOUNT_COUNT_V3)
                    .expect("caller")
                    .is_signer
            );
            let (envelope, family) =
                HotExecutionEnvelopeV3::split_instruction(&result.instruction.data)
                    .expect("envelope");
            assert_eq!(envelope.market(), key(2).to_bytes());
            assert_eq!(hash(family).to_bytes(), result.family_digest);
        }
    }

    #[test]
    fn parent_or_privilege_substitution_refuses_atomically() {
        let fixed = fixed();
        let mut child = exact_child(LifecycleActionV2::ActivateCoordinate);
        child
            .data
            .get_mut(144)
            .expect("parent byte")
            .clone_from(&99);
        assert_eq!(
            build_rational_lifecycle_hot_instruction_v3(&state(&fixed), &child),
            Err(Error::Operator)
        );

        child = exact_child(LifecycleActionV2::ActivateCoordinate);
        child.accounts.get_mut(21).expect("Position").is_writable = false;
        assert_eq!(
            build_rational_lifecycle_hot_instruction_v3(&state(&fixed), &child),
            Err(Error::Operator)
        );
    }

    #[test]
    fn caller_carried_retirement_support_is_unreachable() {
        let fixed = fixed();
        for count in [1, 2, 3] {
            let child = exact_child_with_coordinate_count(LifecycleActionV2::RetireReceipt, count);
            assert_eq!(
                build_rational_lifecycle_hot_instruction_v3(&state(&fixed), &child),
                Err(Error::ActionGeometry)
            );
        }
    }

    /// The lifecycle corpus reads THIS frame's Market, root and Core program.
    ///
    /// All four lifecycle builders -- common, compact V4, selected V5 and
    /// selected V6 -- mine through `lifecycle_hot_bump_hints_v3`, so this is
    /// one control for four producers. It is here because every other fixture
    /// in this crate leaves `market_data` empty and `root_data` a constant
    /// fill, which makes both decodes fail and every slot degrade to zero: a
    /// corpus reading the wrong coordinate would emit the same all-zero block
    /// as one reading the right one, and "nothing fired" would be
    /// indistinguishable from "my instrument was disconnected".
    ///
    /// Two authors. This side builds a Core Market state and a capability-root
    /// header and keeps the bumps its own `find_program_address` calls
    /// returned; `lifecycle_hot_bump_hints_v3` decodes those same bodies and
    /// re-derives.
    #[test]
    fn the_lifecycle_corpus_mines_this_frames_market_and_root() {
        use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
        use dclutch_core_contract::ContentId;
        use dclutch_market::{
            CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
            StateBumpsV1,
        };
        use dclutch_registry::release_set::CapabilityExecutionSelectionV1;

        let identity = |byte: u8| Identity::new([byte; 32]).expect("distinct nonzero fill");
        let market_state = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: identity(0x21),
                realm_id: identity(0x22),
                product_record: identity(0x23),
                product_id: identity(0x24),
                resolution_policy: identity(0x25),
                capability_manifest: identity(0x26),
                selected_release_set: identity(1),
                registry_program: identity(0x27),
                generation: 14,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: u64::MAX,
            rent_beneficiary: identity(0x28),
            terminal_receipt: None,
            // UNRECORDED on purpose: a Market that records its own bump makes
            // the hint inert, because the reader takes the record and never the
            // wire. This is the shape the slot still buys something on.
            bumps: StateBumpsV1::UNRECORDED,
        }
        .encode()
        .expect("canonical open Core Market state");
        let root_header = CapabilityRootHeaderV1::new(
            ContentId::new(key(1).to_bytes()).expect("release set"),
            key(2).to_bytes(),
            14,
            CapabilityExecutionSelectionV1::new(
                3,
                ContentId::new([0x26; 32]).expect("manifest"),
                ContentId::new([0x29; 32]).expect("kind"),
                ContentId::new([0x2a; 32]).expect("capability release"),
                ContentId::new([0x2b; 32]).expect("config"),
            )
            .expect("execution selection")
            .with_capability_release_record_bumps(0xfd, 0xfc),
            SelectedRecordBumpsV1::new(0xff, 0xfe, 0xfb, 0xfa),
        )
        .expect("capability root header")
        .to_bytes();

        let fixed = fixed();
        let checked = CheckedRationalLifecycleHotOuterV3 {
            trading_program: key(60),
            artifact_release: key(61).to_bytes(),
            checked_manifest_digest: key(62).to_bytes(),
        };
        let state = RationalLifecycleHotStateV3 {
            root_data: &root_header,
            market_data: &market_state,
            ..state(&fixed)
        };
        let mined = lifecycle_hot_bump_hints_v3(&state, checked).expect("staged corpus mines");

        let expected_market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(
                CoreState::decode(&market_state)
                    .expect("staged Market decodes")
                    .identity,
            )
            .as_slices(),
            &fixed_key(&state, HOT_CORE_PROGRAM_ACCOUNT_V3).expect("Core program coordinate"),
        )
        .1;
        let expected_root = Pubkey::find_program_address(
            &CapabilityRootHeaderV1::decode(&root_header)
                .expect("staged root header decodes")
                .seeds()
                .as_slices(),
            &checked.trading_program,
        )
        .1;
        assert_eq!(
            mined,
            HotBumpHintsV1 {
                market: expected_market,
                root: expected_root,
                ..HotBumpHintsV1::ABSENT
            }
        );
        // The control on the control: two searches comparing equal would prove
        // nothing at all.
        assert_ne!(mined, HotBumpHintsV1::ABSENT);
        assert_ne!(mined.market, 0);
        assert_ne!(mined.root, 0);
        // And the Custody transfer authority stays absent, because a lifecycle
        // action is a ONE-ROUTE Claims EffectProgram with no Custody leg to
        // spend it on. A producer that started filling it would go red here.
        assert_eq!(mined.child_relay, [0, 0]);
    }
}
