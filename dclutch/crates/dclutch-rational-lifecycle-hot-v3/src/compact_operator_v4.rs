//! Unsigned fixed-family operator for compact complete-support retirement.
//!
//! The operator transports only the exact 400-byte `DCRLHC04` family request.
//! It derives the ordered nonzero support from the authenticated descriptor,
//! verifies the supplied vacancy groups in that order, and relies on the
//! content-addressed effect artifact to synthesize the full Claims child.

use dclutch_account_profile_contract::v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HotExecutionEnvelopeV3,
};
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, RATIONAL_SHARD_MINT_SEED_V2, RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
};
use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LifecycleActionV2, LifecycleHeaderV2,
    compact_hot_v4::{
        RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4, RationalLifecycleCompactHotRequestV4,
    },
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    Error, RationalLifecycleCompactBundleV4, RationalLifecycleHotInstructionV3,
    RationalLifecycleHotStateV3, Result,
    operator::{MAX_SOLANA_PACKET_BYTES, validate_child_frame, validate_fixed_frame},
    validate_rational_lifecycle_compact_bundle_for_authenticated_selection_v4,
};

/// One supplied exact Claims vacancy account group in canonical physical order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleVacancyAccountsV4 {
    /// Claims-derived closeable shard Mint PDA.
    pub shard_mint: AccountMeta,
    /// Claims-derived Token-2022 Structured custody PDA.
    pub structured_custody: AccountMeta,
    /// Canonical LBV2 ProtocolPosition PDA for the derived custody owner.
    pub position: AccountMeta,
    /// Canonical ProtocolPosition admission PDA.
    pub admission: AccountMeta,
}

/// Same-finalized selected semantic authority for compact retirement.
#[derive(Clone, Copy, Debug)]
pub struct RationalLifecycleCompactSelectionV4<'descriptor, 'bundle> {
    /// Finalized immutable representation descriptor owning width `K` and support.
    pub descriptor: RepresentationDescriptorV2<'descriptor>,
    /// Exact CapabilityV4/LifecycleV5/Profile13 artifact bundle.
    pub bundle: &'bundle RationalLifecycleCompactBundleV4,
    /// Independently authenticated Realm/release Token behavior selection.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
}

/// Build one complete unsigned compact RetireReceipt Hot instruction.
///
/// `claims_common_accounts` is the exact 20-account DCRRLC02 common frame.
/// `vacancy_accounts` contains only physical accounts, never duplicate outcome,
/// coefficient, owner, amount, rent, or revision DTOs. Claims remains the final
/// PDA/owner/vacancy authority for Position and admission accounts.
pub fn build_rational_lifecycle_compact_hot_instruction_v4(
    state: &RationalLifecycleHotStateV3<'_>,
    header: LifecycleHeaderV2,
    selection: RationalLifecycleCompactSelectionV4<'_, '_>,
    claims_program: Pubkey,
    claims_common_accounts: &[AccountMeta],
    vacancy_accounts: &[RationalLifecycleVacancyAccountsV4],
) -> Result<RationalLifecycleHotInstructionV3> {
    let RationalLifecycleCompactSelectionV4 {
        descriptor,
        bundle,
        authenticated_token_behavior,
    } = selection;
    let checked = state.hot_outer.ok_or(Error::Operator)?;
    validate_fixed_frame(state, checked)?;
    validate_rational_lifecycle_compact_bundle_for_authenticated_selection_v4(
        bundle,
        authenticated_token_behavior,
    )?;
    if state.finalized_slot == 0
        || claims_program == Pubkey::default()
        || claims_common_accounts.len() != LIFECYCLE_COMMON_ACCOUNT_COUNT_V2
        || header.action != LifecycleActionV2::RetireReceipt
        || header.coordinate_count != 0
        || header.release_set != state.release_set
        || header.market != state.market.to_bytes()
        || header.generation != state.generation
        || header.descriptor_id != descriptor.descriptor_id()
        || header.graph_id != descriptor.graph_id()
        || header.market != descriptor.market_id()
        || header.release_set != descriptor.release_set_id()
        || header.representation_authority != descriptor.representation_authority()
        || header.receipt_mint != descriptor.receipt_mint()
        || header.token_program != descriptor.token_program()
        || header.outcome_count != descriptor.outcome_count()
        || descriptor.descriptor_id() != authenticated_token_behavior.descriptor_id()
        || descriptor.release_set_id() != authenticated_token_behavior.selection().release_set()
        || descriptor.token_program() != authenticated_token_behavior.selection().token_program()
    {
        return Err(Error::Operator);
    }

    let support = support_outcomes(descriptor)?;
    if support.len() != vacancy_accounts.len() {
        return Err(Error::Operator);
    }
    if bundle.support_count != u32::try_from(support.len()).map_err(|_| Error::Operator)? {
        return Err(Error::Operator);
    }
    let mut claims_accounts = Vec::with_capacity(
        claims_common_accounts
            .len()
            .checked_add(
                vacancy_accounts
                    .len()
                    .checked_mul(4)
                    .ok_or(Error::Operator)?,
            )
            .ok_or(Error::Operator)?,
    );
    claims_accounts.extend_from_slice(claims_common_accounts);
    for ((outcome, _), group) in support.iter().copied().zip(vacancy_accounts) {
        validate_vacancy_group(claims_program, descriptor.descriptor_id(), outcome, group)?;
        claims_accounts.extend([
            group.shard_mint.clone(),
            group.structured_custody.clone(),
            group.position.clone(),
            group.admission.clone(),
        ]);
    }
    let empty_child = Instruction {
        program_id: claims_program,
        accounts: claims_accounts.clone(),
        data: Vec::new(),
    };
    validate_child_frame(&empty_child, LifecycleActionV2::RetireReceipt)?;

    let mut family_bytes = [0_u8; RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4];
    let family = RationalLifecycleCompactHotRequestV4::from_header_into(header, &mut family_bytes)
        .map_err(Error::Lifecycle)?;
    let family_digest = hash(family.as_bytes()).to_bytes();
    let mut specialized = [0_u8; RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4];
    family
        .specialize_child_header_into(
            family_digest,
            u32::try_from(support.len()).map_err(|_| Error::Operator)?,
            &mut specialized,
        )
        .map_err(Error::Lifecycle)?;

    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family.as_bytes().len()).map_err(|_| Error::Operator)?,
        state.release_set,
        state.market.to_bytes(),
        state.generation,
        hash(state.root_data).to_bytes(),
    )
    .map_err(|_| Error::Operator)?;
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family.as_bytes().len())
            .ok_or(Error::Operator)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(family.as_bytes());
    if data.len() > MAX_SOLANA_PACKET_BYTES {
        return Err(Error::Operator);
    }

    let profile =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfile)?;
    let physical_claims_accounts =
        compact_profile13_claims_accounts_v4(state, &claims_accounts, profile)?;
    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| count.checked_add(physical_claims_accounts.len()))
            .ok_or(Error::Operator)?,
    );
    accounts.extend_from_slice(state.fixed_accounts);
    accounts.extend_from_slice(state.strategy_accounts);
    accounts.extend(physical_claims_accounts);
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

fn compact_profile13_claims_accounts_v4(
    state: &RationalLifecycleHotStateV3<'_>,
    claims_accounts: &[AccountMeta],
    profile: AccountProfileV2<'_>,
) -> Result<Vec<AccountMeta>> {
    const INJECTED: usize = 5;
    const TAIL_COUNT: u32 = 0;
    let expected_logical = INJECTED
        .checked_add(claims_accounts.len())
        .ok_or(Error::Operator)?;
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.dynamic_fixed_span_count() != 0
        || profile
            .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
            .map_err(Error::AccountProfile)?
            != expected_logical
    {
        return Err(Error::Operator);
    }
    let physical = profile
        .physical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(Error::AccountProfile)?;
    if physical < INJECTED {
        return Err(Error::Operator);
    }
    for coordinate in 0..INJECTED {
        if profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(Error::AccountProfile)?
            != coordinate
        {
            return Err(Error::Operator);
        }
        let expected = injected_meta_v4(state, coordinate)?;
        let (signer, writable) = physical_privileges_v4(profile, coordinate)?;
        if expected.is_signer != signer || expected.is_writable != writable {
            return Err(Error::Operator);
        }
    }

    let mut output = Vec::with_capacity(physical - INJECTED);
    for (child_index, account) in claims_accounts.iter().enumerate() {
        let logical = INJECTED.checked_add(child_index).ok_or(Error::Operator)?;
        let route = profile
            .route_privileges_with_dynamic_spans(TAIL_COUNT, &[], logical)
            .map_err(Error::AccountProfile)?;
        if account.is_writable != route.writable()
            || (child_index != 0 && account.is_signer != route.signer())
            || (child_index == 0 && !account.is_signer)
        {
            return Err(Error::Operator);
        }
        let representative = profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], logical)
            .map_err(Error::AccountProfile)?;
        let representative_meta = if representative < INJECTED {
            injected_meta_v4(state, representative)?
        } else {
            claims_accounts
                .get(
                    representative
                        .checked_sub(INJECTED)
                        .ok_or(Error::Operator)?,
                )
                .ok_or(Error::Operator)?
        };
        if representative_meta.pubkey != account.pubkey {
            return Err(Error::Operator);
        }
        if representative == logical {
            let (signer, writable) = physical_privileges_v4(profile, representative)?;
            let mut outer = account.clone();
            outer.is_signer = signer;
            outer.is_writable = writable;
            output.push(outer);
        }
    }
    if output.len() != physical - INJECTED {
        return Err(Error::Operator);
    }
    Ok(output)
}

fn physical_privileges_v4(
    profile: AccountProfileV2<'_>,
    representative: usize,
) -> Result<(bool, bool)> {
    const TAIL_COUNT: u32 = 0;
    let logical = profile
        .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(Error::AccountProfile)?;
    let mut signer = false;
    let mut writable = false;
    for coordinate in 0..logical {
        if profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(Error::AccountProfile)?
            == representative
        {
            let route = profile
                .route_privileges_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
                .map_err(Error::AccountProfile)?;
            signer |= route.signer();
            writable |= route.writable();
        }
    }
    Ok((signer, writable))
}

fn injected_meta_v4<'a>(
    state: &'a RationalLifecycleHotStateV3<'_>,
    logical_coordinate: usize,
) -> Result<&'a AccountMeta> {
    let physical_coordinate = match logical_coordinate {
        0 => HOT_ROOT_ACCOUNT_V3,
        1 => HOT_CONFIG_RAW_ACCOUNT_V3,
        2 => HOT_PRODUCT_RAW_ACCOUNT_V3,
        3 => HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        4 => HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        _ => return Err(Error::Operator),
    };
    state
        .fixed_accounts
        .get(physical_coordinate)
        .ok_or(Error::Operator)
}

fn support_outcomes(descriptor: RepresentationDescriptorV2<'_>) -> Result<Vec<(u32, u64)>> {
    let mut support = Vec::new();
    for outcome in 0..descriptor.outcome_count() {
        let coefficient = descriptor
            .coefficient(outcome)
            .map_err(|_| Error::Operator)?;
        if coefficient != 0 {
            support.push((outcome, coefficient));
        }
    }
    if support.is_empty() {
        return Err(Error::Operator);
    }
    Ok(support)
}

fn validate_vacancy_group(
    claims_program: Pubkey,
    descriptor_id: [u8; 32],
    outcome: u32,
    group: &RationalLifecycleVacancyAccountsV4,
) -> Result<()> {
    let outcome = outcome.to_le_bytes();
    let expected_shard = Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor_id, &outcome],
        &claims_program,
    )
    .0;
    let expected_structured = Pubkey::find_program_address(
        &[
            RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
            &descriptor_id,
            &outcome,
        ],
        &claims_program,
    )
    .0;
    if group.shard_mint.pubkey != expected_shard
        || group.structured_custody.pubkey != expected_structured
        || [
            &group.shard_mint,
            &group.structured_custody,
            &group.position,
            &group.admission,
        ]
        .into_iter()
        .any(|account| {
            account.is_signer || account.is_writable || account.pubkey == Pubkey::default()
        })
    {
        return Err(Error::Operator);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::{
        HEADER_BYTES as LIFECYCLE_HEADER_BYTES, encode::encode_lifecycle_policy_v5_atomic,
    };
    use dclutch_capability_program_contract::hot_v3::{
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };
    use dclutch_rational_representation_v2_contract::{
        TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
    };
    use dclutch_rational_representation_v2_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
        DESCRIPTOR_SCHEMA_VERSION_V3, DescriptorAdmissionV2,
    };
    use dclutch_token_svm::{
        TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_BYTES_V2,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
    };
    use solana_sdk_ids::{system_program, sysvar};

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn put(bytes: &mut [u8], offset: usize, value: &[u8]) {
        bytes
            .get_mut(offset..offset + value.len())
            .expect("fixture range")
            .copy_from_slice(value);
    }

    fn descriptor_bytes() -> Vec<u8> {
        let mut output = vec![0_u8; DESCRIPTOR_HEADER_BYTES + 5 * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut output, 8, &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes());
        for (offset, value) in [
            (16, key(11).to_bytes()),
            (48, key(12).to_bytes()),
            (80, key(13).to_bytes()),
            (112, key(14).to_bytes()),
            (144, key(15).to_bytes()),
            (176, key(16).to_bytes()),
            (208, TOKEN_2022_PROGRAM_ID),
        ] {
            put(&mut output, offset, &value);
        }
        put(&mut output, 240, &5_u32.to_le_bytes());
        put(&mut output, 248, &10_u64.to_le_bytes());
        for (index, coefficient) in [0_u64, 7, 5, 0, 9].iter().enumerate() {
            put(
                &mut output,
                DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
                &coefficient.to_le_bytes(),
            );
        }
        output
    }

    fn descriptor(bytes: &[u8]) -> RepresentationDescriptorV2<'_> {
        RepresentationDescriptorV2::decode(
            bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: key(21).to_bytes(),
                finalized_descriptor_id: key(21).to_bytes(),
                recomputed_descriptor_digest: key(21).to_bytes(),
                finalized_descriptor_digest: key(21).to_bytes(),
                record_authenticated: true,
                derived_representation_authority: key(22).to_bytes(),
                authority_derivation_authenticated: true,
            },
        )
        .expect("descriptor")
    }

    fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: key(1).to_bytes(),
                result_domain_id: key(2).to_bytes(),
                coordinate_domain_id: key(3).to_bytes(),
                result_unit_id: key(4).to_bytes(),
                evaluator_release_id: key(5).to_bytes(),
                basis_width: 258,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    fn token_behavior(
        descriptor: RepresentationDescriptorV2<'_>,
        realm: [u8; 32],
    ) -> AuthenticatedTokenBehaviorV2 {
        let selection = TokenBehaviorSelectionV2::new(realm, descriptor.release_set_id())
            .expect("Token selection")
            .to_bytes();
        let digest = hash(&selection).to_bytes();
        authenticate_token_behavior_v2(
            descriptor,
            realm,
            &selection,
            TokenBehaviorRecordAdmissionV2 {
                selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                selected_content_digest: digest,
                finalized_content_digest: digest,
                recomputed_content_digest: digest,
                record_authenticated: true,
                market_realm_authenticated: true,
            },
        )
        .expect("Token behavior")
    }

    fn lifecycle_policy() -> Vec<u8> {
        let mut scratch = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        let mut output = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
            .expect("empty lifecycle V5");
        output
    }

    fn compact_bundle<'a>(
        descriptor: RepresentationDescriptorV2<'a>,
        claims: Pubkey,
        logical_accounts: usize,
    ) -> (
        RationalLifecycleCompactBundleV4,
        AuthenticatedTokenBehaviorV2,
    ) {
        let basis = basis();
        let mut lengths = vec![0_u32; logical_accounts];
        *lengths.get_mut(1).expect("Token selection") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        *lengths.get_mut(4).expect("basis") = u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(14).expect("descriptor") = u32::try_from(
            DESCRIPTOR_HEADER_BYTES
                + usize::try_from(descriptor.outcome_count()).expect("K")
                    * DESCRIPTOR_COEFFICIENT_BYTES,
        )
        .expect("descriptor width");
        let authenticated = token_behavior(descriptor, key(51).to_bytes());
        let lifecycle = lifecycle_policy();
        let bundle = crate::build_rational_lifecycle_compact_bundle_v4(
            crate::RationalLifecycleCompactBundleInputV4 {
                artifacts: crate::RationalLifecycleCompactArtifactInputV4 {
                    logical_data_lengths: &lengths,
                    product_basis: &basis,
                    descriptor,
                    claims_program: claims,
                },
                kind: key(41).to_bytes(),
                authenticated_token_behavior: authenticated,
                root_schema: key(42).to_bytes(),
                lifecycle_policy: &lifecycle,
                capacity_profile: key(44).to_bytes(),
                root_state_bytes: 64,
            },
        )
        .expect("compact bundle");
        (bundle, authenticated)
    }

    fn header() -> LifecycleHeaderV2 {
        LifecycleHeaderV2 {
            action: LifecycleActionV2::RetireReceipt,
            release_set: key(15).to_bytes(),
            market: key(14).to_bytes(),
            graph_id: key(11).to_bytes(),
            descriptor_id: key(21).to_bytes(),
            parent_context: key(23).to_bytes(),
            representation_authority: key(22).to_bytes(),
            receipt_mint: key(16).to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: key(24).to_bytes(),
            rent_program: key(25).to_bytes(),
            generation: 14,
            expected_claims_market_revision: 3,
            observed_receipt_lamports: 10,
            receipt_rent_principal: 10,
            expected_receipt_supply: 0,
            outcome_count: 5,
            coordinate_count: 0,
            rent_credit_before: 100,
            rent_credit_after: 110,
        }
    }

    fn fixed() -> Vec<AccountMeta> {
        let mut fixed = (0_u8..u8::try_from(
            dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3,
        )
        .expect("fixed hot prefix"))
            .map(|index| AccountMeta::new_readonly(key(100_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        *fixed.get_mut(HOT_MARKET_ACCOUNT_V3).expect("Market") =
            AccountMeta::new_readonly(key(14), false);
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
        assert_ne!(
            fixed
                .get(HOT_LINKED_BASIS_STAGING_ACCOUNT_V3)
                .expect("basis staging")
                .pubkey,
            system_program::ID
        );
        fixed
    }

    fn state(fixed: &[AccountMeta]) -> RationalLifecycleHotStateV3<'_> {
        RationalLifecycleHotStateV3 {
            fixed_accounts: fixed,
            strategy_accounts: &[],
            root_data: &[7; 64],
            release_set: key(15).to_bytes(),
            market: key(14),
            generation: 14,
            finalized_slot: 99,
            hot_outer: Some(crate::CheckedRationalLifecycleHotOuterV3 {
                trading_program: key(60),
                artifact_release: key(61).to_bytes(),
                checked_manifest_digest: key(62).to_bytes(),
            }),
        }
    }

    fn common(claims: Pubkey) -> Vec<AccountMeta> {
        let mut output = (0_u8..20)
            .map(|index| AccountMeta::new_readonly(key(150_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        output.get_mut(0).expect("caller").is_signer = true;
        *output.get_mut(3).expect("Claims") = AccountMeta::new_readonly(claims, false);
        output.get_mut(12).expect("receipt").is_writable = true;
        output.get_mut(14).expect("RentCredit").is_writable = true;
        output
    }

    fn vacancies(claims: Pubkey) -> Vec<RationalLifecycleVacancyAccountsV4> {
        [1_u32, 2, 4]
            .into_iter()
            .enumerate()
            .map(|(index, outcome)| {
                let outcome = outcome.to_le_bytes();
                RationalLifecycleVacancyAccountsV4 {
                    shard_mint: AccountMeta::new_readonly(
                        Pubkey::find_program_address(
                            &[RATIONAL_SHARD_MINT_SEED_V2, key(21).as_ref(), &outcome],
                            &claims,
                        )
                        .0,
                        false,
                    ),
                    structured_custody: AccountMeta::new_readonly(
                        Pubkey::find_program_address(
                            &[
                                RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                                key(21).as_ref(),
                                &outcome,
                            ],
                            &claims,
                        )
                        .0,
                        false,
                    ),
                    position: AccountMeta::new_readonly(
                        key(200_u8
                            .checked_add(u8::try_from(index).expect("row"))
                            .expect("key")),
                        false,
                    ),
                    admission: AccountMeta::new_readonly(
                        key(210_u8
                            .checked_add(u8::try_from(index).expect("row"))
                            .expect("key")),
                        false,
                    ),
                }
            })
            .collect()
    }

    #[test]
    fn k3_uses_fixed_family_bytes_and_exact_ordered_account_groups() {
        let fixed = fixed();
        let descriptor_bytes = descriptor_bytes();
        let claims = key(31);
        let common = common(claims);
        let vacancies = vacancies(claims);
        let (bundle, authenticated) = compact_bundle(
            descriptor(&descriptor_bytes),
            claims,
            5 + common.len() + vacancies.len() * 4,
        );
        let instruction = build_rational_lifecycle_compact_hot_instruction_v4(
            &state(&fixed),
            header(),
            RationalLifecycleCompactSelectionV4 {
                descriptor: descriptor(&descriptor_bytes),
                bundle: &bundle,
                authenticated_token_behavior: authenticated,
            },
            claims,
            &common,
            &vacancies,
        )
        .expect("compact K3 instruction");
        assert_eq!(instruction.instruction.data.len(), 528);
        // 70 before the validated-artifact seal joined the fixed hot prefix.
        assert_eq!(instruction.instruction.accounts.len(), 71);
        assert!(instruction.requires_v0_address_lookup);
        assert!(instruction.required_wallet_signers.is_empty());
        let (_, family) = HotExecutionEnvelopeV3::split_instruction(&instruction.instruction.data)
            .expect("family");
        assert_eq!(family.len(), 400);
        assert_eq!(hash(family).to_bytes(), instruction.family_digest);

        let mut reordered = vacancies.clone();
        reordered.swap(0, 1);
        assert_eq!(
            build_rational_lifecycle_compact_hot_instruction_v4(
                &state(&fixed),
                header(),
                RationalLifecycleCompactSelectionV4 {
                    descriptor: descriptor(&descriptor_bytes),
                    bundle: &bundle,
                    authenticated_token_behavior: authenticated,
                },
                claims,
                &common,
                &reordered,
            ),
            Err(Error::Operator)
        );
        assert_eq!(
            build_rational_lifecycle_compact_hot_instruction_v4(
                &state(&fixed),
                header(),
                RationalLifecycleCompactSelectionV4 {
                    descriptor: descriptor(&descriptor_bytes),
                    bundle: &bundle,
                    authenticated_token_behavior: authenticated,
                },
                claims,
                &common,
                vacancies.get(..2).expect("omitted tail"),
            ),
            Err(Error::Operator)
        );

        assert_eq!(
            build_rational_lifecycle_compact_hot_instruction_v4(
                &state(&fixed),
                header(),
                RationalLifecycleCompactSelectionV4 {
                    descriptor: descriptor(&descriptor_bytes),
                    bundle: &bundle,
                    authenticated_token_behavior: token_behavior(
                        descriptor(&descriptor_bytes),
                        key(52).to_bytes(),
                    ),
                },
                claims,
                &common,
                &vacancies,
            ),
            Err(Error::ContentIdentity)
        );
    }
}
