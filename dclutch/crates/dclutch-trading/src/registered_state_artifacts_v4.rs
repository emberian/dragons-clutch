//! Data-defined maker and record first-use lifecycle for registered Direct V4.
//!
//! Registration uses two mandatory `AuthenticateOrCreate` plans.  The maker
//! replay root may be live or vacant, while the side-selected Transition
//! requires the registered record to be vacant.  Generic Trading owns PDA
//! derivation, current-Rent authentication, protected outputs, and creation;
//! this module contributes only the canonical Direct recipes and bindings.
//!
//! The V5 current-Rent quote table is ACTION-SELECTED, because the two sides
//! create different children. A registered Buy opens a Custody replay and a
//! Custody token Vault and must quote both; a registered Sell opens neither.
//! The quote table is the sole writer of those scalars: nothing else in the
//! family may target a lifecycle-protected destination.

use dclutch_vm::account_profile::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, IMMUTABLE_IDENTITY_BINDING_BYTES,
    PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV5,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
        LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3, LifecyclePlanInputV3,
        LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
        LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_custody::CUSTODY_REPLAY_BYTES_V1;

use crate::{
    execution_v3::DirectExecutionActionV3,
    generated_intent_v2 as intent,
    registered_creation_artifacts_v4::{
        REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4,
        REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4, REGISTERED_IDENTITY_MAKER_STATE_OWNER_V4,
        REGISTERED_IDENTITY_MAKER_STATE_V4, REGISTERED_IDENTITY_MARKET_V4,
        REGISTERED_IDENTITY_RECORD_BENEFICIARY_OBSERVATION_V4,
        REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4, REGISTERED_IDENTITY_RECORD_STATE_OWNER_V4,
        REGISTERED_IDENTITY_RECORD_STATE_V4, REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        REGISTERED_SCALAR_GENERATION_V4, REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4,
        REGISTERED_SCALAR_MAKER_BUMP_V4, REGISTERED_SCALAR_MAKER_CREATED_V4,
        REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4, REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4,
        REGISTERED_SCALAR_MAKER_PRINCIPAL_V4, REGISTERED_SCALAR_NONCE_V4,
        REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4, REGISTERED_SCALAR_RECORD_BUMP_V4,
        REGISTERED_SCALAR_RECORD_CREATED_V4, REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4,
        REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4, REGISTERED_SCALAR_RECORD_PRINCIPAL_V4,
        REGISTERED_SCALAR_REPLAY_RENT_V4, REGISTERED_SCALAR_VAULT_RENT_V4,
    },
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2,
        DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2,
    },
};

/// Maker replay state coordinate in the registered creation AccountProfile.
pub const DIRECT_REGISTERED_MAKER_ACCOUNT_V4: u16 = 5;
/// Sole registered-creation payer coordinate.
///
/// One rent payer funds both creations -- the maker signs one registration
/// request and prepays both accounts it opens -- and coordinate 9 is its
/// authenticated route alias. A plan names the REPRESENTATIVE, never the alias:
/// `require_permissions` reads the named rule directly, and the adapter records
/// a planned balance only at the representative, so naming 9 would both observe
/// a permission-free alias rule and re-read the payer's pre-debit balance.
pub const DIRECT_REGISTERED_PAYER_ACCOUNT_V4: u16 = 6;
/// Sole lifecycle-scoped RentCredit coordinate.
///
/// A `LifecycleRentCreditV2` is keyed by Market and generation alone, so one
/// credit serves the whole Market lifecycle and both creation plans name it.
pub const DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4: u16 = 7;
/// Registered record state coordinate.
pub const DIRECT_REGISTERED_RECORD_ACCOUNT_V4: u16 = 8;
/// Route alias of [`DIRECT_REGISTERED_PAYER_ACCOUNT_V4`], named by no plan.
pub const DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4: u16 = 9;
/// Executable Rent program that owns the lifecycle RentCredit.
///
/// The adapter authenticates the credit as a PDA of its own account owner and
/// requires that owner to be present in the frame as an executable readonly
/// account, so the Rent program is a coordinate of the profile. It is the slot
/// the second per-account credit vacated.
pub const DIRECT_REGISTERED_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V4: u16 = 10;

const RECIPE_COUNT: usize = 2;
const MAKER_SEED_COUNT: usize = 5;
const RECORD_SEED_COUNT: usize = 6;
const SEED_COUNT: usize = MAKER_SEED_COUNT + RECORD_SEED_COUNT;
const PLAN_COUNT: usize = 2;
const BINDING_COUNT: usize = 4;
/// Maker replay and registered record: the two accounts BOTH sides create.
const COMMON_RENT_QUOTE_COUNT: usize = 2;
/// Plus the Custody replay and the Custody token Vault a Buy also opens.
const BUY_RENT_QUOTE_COUNT: usize = COMMON_RENT_QUOTE_COUNT + 2;
/// A Sell opens no Custody child, so its table is the common one exactly.
const SELL_RENT_QUOTE_COUNT: usize = COMMON_RENT_QUOTE_COUNT;
const MAX_RENT_QUOTE_COUNT: usize = BUY_RENT_QUOTE_COUNT;
const MAKER_SEED_COUNT_U8: u8 = 5;
const RECORD_SEED_COUNT_U8: u8 = 6;
const RECORD_SEED_START: u16 = 5;
const MAKER_BYTES_U32: u32 = 160;
const RECORD_BYTES_U32: u32 = 268;
const _: () = assert!(DIRECT_MAKER_REPLAY_BYTES_V1 == MAKER_BYTES_U32 as usize);
const _: () = assert!(DIRECT_REGISTERED_RECORD_BYTES_V2 == RECORD_BYTES_U32 as usize);

const fn lifecycle_bytes(rent_quotes: usize) -> usize {
    HEADER_BYTES
        + RECIPE_COUNT * RECIPE_BYTES
        + SEED_COUNT * SEED_BYTES
        + PLAN_COUNT * ACTION_PLAN_BYTES
        + PLAN_COUNT * PROTECTED_OUTPUT_BYTES
        + BINDING_COUNT * IMMUTABLE_IDENTITY_BINDING_BYTES
        + rent_quotes * CURRENT_RENT_QUOTE_BYTES_V5
}

/// Plans in the unified policy: maker and record, once per creation action.
const UNIFIED_PLAN_COUNT: usize = PLAN_COUNT * 2;
/// Bindings in the unified policy: the same four, once per creation action.
const UNIFIED_BINDING_COUNT: usize = BINDING_COUNT * 2;
/// Quotes in the unified policy: the Buy's four, two of them Buy-scoped.
const UNIFIED_RENT_QUOTE_COUNT: usize = BUY_RENT_QUOTE_COUNT;

const fn unified_lifecycle_bytes() -> usize {
    HEADER_BYTES
        + RECIPE_COUNT * RECIPE_BYTES
        + SEED_COUNT * SEED_BYTES
        + UNIFIED_PLAN_COUNT * ACTION_PLAN_BYTES
        + UNIFIED_PLAN_COUNT * PROTECTED_OUTPUT_BYTES
        + UNIFIED_BINDING_COUNT * IMMUTABLE_IDENTITY_BINDING_BYTES
        + UNIFIED_RENT_QUOTE_COUNT * CURRENT_RENT_QUOTE_BYTES_V5
}

/// Exact LifecycleV5 bytes for the ONE policy both creation actions share.
///
/// Wall B was: one manifest entry pins one `derivation_policy`, so a root admits
/// one lifecycle policy, and the two registered creation actions had policies of
/// different widths and therefore different digests. No Direct root could admit
/// both. This is the policy that ends that -- one digest, one entry, one root,
/// serving RegisterSell and RegisterBuy.
pub const DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5: usize = unified_lifecycle_bytes();

/// Exact LifecycleV5 bytes for registered Buy creation.
pub const DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5: usize = lifecycle_bytes(BUY_RENT_QUOTE_COUNT);
/// Exact LifecycleV5 bytes for registered Sell creation.
pub const DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5: usize = lifecycle_bytes(SELL_RENT_QUOTE_COUNT);

/// Exact LifecycleV5 bytes for one side-selected registered creation action.
#[must_use]
pub const fn direct_registered_creation_lifecycle_bytes_v5(
    action: DirectExecutionActionV3,
) -> Option<usize> {
    match action {
        DirectExecutionActionV3::RegisterBuy => Some(DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5),
        DirectExecutionActionV3::RegisterSell => Some(DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5),
        _ => None,
    }
}

/// Exact chain-observed widths of the children a registered creation opens.
///
/// A `LifecycleCurrentRentQuoteInputV5` quotes an EXACT width, and a Custody
/// token Vault does not have one this crate may state: a Token-2022 account
/// carrying extensions is not 165 bytes. The width is therefore an input,
/// exactly as the AccountProfile's logical data lengths are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisteredCreationChildRentWidthsV4 {
    /// Exact selected Token or Token-2022 vault-account bytes.
    pub custody_vault: u32,
}

/// Stable registered lifecycle artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredStateArtifactErrorV4 {
    /// An action, account, register, or byte coordinate was invalid.
    Coordinate,
    /// The declared child widths did not match the selected action.
    ChildWidths,
    /// The semantic-owner lifecycle encoder or hostile decoder refused.
    Lifecycle(dclutch_vm::account_profile::lifecycle_v3::Error),
}

/// Emit the exact maker/record lifecycle for RegisterSell or RegisterBuy.
///
/// `child_widths` is required exactly when the action opens a Custody child --
/// RegisterBuy -- and must be absent otherwise. Passing `None` for a Buy used
/// to be the only possibility, and it left `REGISTERED_SCALAR_REPLAY_RENT_V4`
/// and `REGISTERED_SCALAR_VAULT_RENT_V4` -- the two registers the Buy Effect
/// writes into the Custody `InitializeReplay` and `OpenVault` requests'
/// `rent_lamports` field -- with no writer at all. `CustodyRequestV1::validate`
/// refuses `rent_lamports == 0` for both operations, so the first Custody route
/// of every registered Buy would have refused before touching an account.
pub fn encode_direct_registered_creation_lifecycle_v5_atomic(
    action: DirectExecutionActionV3,
    child_widths: Option<DirectRegisteredCreationChildRentWidthsV4>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredStateArtifactErrorV4> {
    require_creation_action(action)?;
    let expected = direct_registered_creation_lifecycle_bytes_v5(action)
        .ok_or(DirectRegisteredStateArtifactErrorV4::Coordinate)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DirectRegisteredStateArtifactErrorV4::Coordinate);
    }
    let recipes = registered_creation_recipes_v4();
    let seeds = registered_creation_seeds_v4()?;
    // Both plans fund through the one lifecycle credit. There is exactly one
    // `LifecycleRentCreditV2` per Market lifecycle, so `plan` no longer takes a
    // rent-credit coordinate at all: naming it per plan is what let the profile
    // declare two credits that were never two accounts on any chain.
    let plans = registered_creation_plans_v4(action)?;
    let protected = registered_creation_protected_v4()?;
    let bindings = registered_creation_bindings_v4(0)?;
    let quotes = rent_quotes(action, child_widths)?;
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &bindings,
        quotes
            .get(..rent_quote_count(action)?)
            .ok_or(DirectRegisteredStateArtifactErrorV4::Coordinate)?,
        scratch,
        output,
    )
    .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    Ok(())
}

const fn rent_quote_count(
    action: DirectExecutionActionV3,
) -> Result<usize, DirectRegisteredStateArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::RegisterBuy => Ok(BUY_RENT_QUOTE_COUNT),
        DirectExecutionActionV3::RegisterSell => Ok(SELL_RENT_QUOTE_COUNT),
        _ => Err(DirectRegisteredStateArtifactErrorV4::Coordinate),
    }
}

/// The action-selected current-Rent quote table.
///
/// The decoder requires STRICTLY ASCENDING scalar destinations, so the table is
/// ordered by register index and not by the order the accounts are created:
/// the two Custody children a Buy opens hold registers 50 and 51, and the maker
/// replay and registered record hold 52 and 53. A Sell opens no Custody child,
/// so its table is registers 52 and 53 alone and the Buy's is all four.
///
/// The Custody replay width this crate does know exactly; the token Vault's
/// belongs to the selected token program and arrives as an observation.
///
/// Every destination here is a lifecycle-PROTECTED scalar -- no AccountProfile
/// operation, RequestProfile projection, or Transition instruction in this
/// family targets 50, 51, 52 or 53 -- so this table is the sole writer of all
/// four.
fn rent_quotes(
    action: DirectExecutionActionV3,
    child_widths: Option<DirectRegisteredCreationChildRentWidthsV4>,
) -> Result<
    [LifecycleCurrentRentQuoteInputV5; MAX_RENT_QUOTE_COUNT],
    DirectRegisteredStateArtifactErrorV4,
> {
    let maker = LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: MAKER_BYTES_U32,
        scalar_destination: scalar(REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4)?,
        action: None,
    };
    let record = LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: RECORD_BYTES_U32,
        scalar_destination: scalar(REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4)?,
        action: None,
    };
    match (action, child_widths) {
        (DirectExecutionActionV3::RegisterBuy, Some(widths)) if widths.custody_vault != 0 => Ok([
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: u32::try_from(CUSTODY_REPLAY_BYTES_V1)
                    .map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)?,
                scalar_destination: scalar(REGISTERED_SCALAR_REPLAY_RENT_V4)?,
                action: None,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: widths.custody_vault,
                scalar_destination: scalar(REGISTERED_SCALAR_VAULT_RENT_V4)?,
                action: None,
            },
            maker,
            record,
        ]),
        (DirectExecutionActionV3::RegisterSell, None) => Ok([maker, record, maker, record]),
        _ => Err(DirectRegisteredStateArtifactErrorV4::ChildWidths),
    }
}

const fn recipe(
    state: u16,
    seed_start: u16,
    seed_count: u8,
    data_base: u32,
) -> LifecycleRecipeInputV3 {
    LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(state),
        seed_start,
        seed_count,
        bump_offset: seed_count - 1,
        data_base,
        data_stride: 0,
    }
}

fn plan(
    action: DirectExecutionActionV3,
    recipe: u16,
    payer: u16,
    principal_observation: usize,
    beneficiary_observation: usize,
) -> Result<LifecyclePlanInputV3, DirectRegisteredStateArtifactErrorV4> {
    Ok(LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe,
        payer: Some(LifecycleAccountCoordinateV3::fixed(payer)),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar(
            principal_observation,
        )?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity(
            beneficiary_observation,
        )?)),
        refund_source: LifecycleRefundSourceInputV3::Credit,
        guard: LifecycleGuardInputV3::Always,
    })
}

fn binding(
    plan: u16,
    data_offset: usize,
    canonical: usize,
) -> Result<LifecycleImmutableIdentityBindingInputV4, DirectRegisteredStateArtifactErrorV4> {
    Ok(LifecycleImmutableIdentityBindingInputV4 {
        plan,
        data_offset: u32::try_from(data_offset)
            .map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)?,
        canonical: LifecycleRegisterCoordinateV3::common(identity(canonical)?),
    })
}

const fn require_creation_action(
    action: DirectExecutionActionV3,
) -> Result<(), DirectRegisteredStateArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy => Ok(()),
        _ => Err(DirectRegisteredStateArtifactErrorV4::Coordinate),
    }
}

fn scalar(value: usize) -> Result<u16, DirectRegisteredStateArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)
}

fn identity(value: usize) -> Result<u16, DirectRegisteredStateArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use dclutch_vm::account_profile::lifecycle_v3::LifecycleOperationV3;
    use sha2::{Digest, Sha256};

    /// The observed Token-2022 vault width the Buy quotes against in tests.
    const OBSERVED_VAULT_BYTES: u32 = 165;

    fn buy() -> [u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5] {
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        let mut output = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(
            DirectExecutionActionV3::RegisterBuy,
            Some(DirectRegisteredCreationChildRentWidthsV4 {
                custody_vault: OBSERVED_VAULT_BYTES,
            }),
            &mut scratch,
            &mut output,
        )
        .expect("registered Buy lifecycle");
        output
    }

    fn sell() -> [u8; DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5] {
        let mut scratch = [0_u8; DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5];
        let mut output = [0_u8; DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(
            DirectExecutionActionV3::RegisterSell,
            None,
            &mut scratch,
            &mut output,
        )
        .expect("registered Sell lifecycle");
        output
    }

    fn policy(bytes: &[u8]) -> StateLifecyclePolicyV5<'_> {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        StateLifecyclePolicyV5::decode_selected(digest, digest, bytes).expect("lifecycle decode")
    }

    #[test]
    fn both_sides_select_two_protected_first_use_plans_and_exact_rent_quotes() {
        for (action, bytes, first_lifecycle_quote) in [
            (
                DirectExecutionActionV3::RegisterSell,
                sell().to_vec(),
                0_u16,
            ),
            (DirectExecutionActionV3::RegisterBuy, buy().to_vec(), 2),
        ] {
            let policy = policy(&bytes);
            assert_eq!(policy.action_plan_count(action as u32).expect("count"), 2);
            for ordinal in 0..2 {
                let selected = policy.action_plan(action as u32, ordinal).expect("plan");
                assert_eq!(
                    selected.operation(),
                    LifecycleOperationV3::AuthenticateOrCreate
                );
                assert!(selected.protected_outputs().expect("protected").is_some());
            }
            let maker = policy
                .current_rent_quote(first_lifecycle_quote)
                .expect("maker rent");
            let record = policy
                .current_rent_quote(first_lifecycle_quote + 1)
                .expect("record rent");
            assert_eq!(maker.exact_data_len(), MAKER_BYTES_U32);
            assert_eq!(record.exact_data_len(), RECORD_BYTES_U32);
            assert_eq!(
                maker.scalar_destination().index(),
                u16::try_from(REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4).expect("maker register")
            );
            assert_eq!(
                record.scalar_destination().index(),
                u16::try_from(REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4).expect("record register")
            );
        }
    }

    /// The registered Buy Effect writes registers 50 and 51 into the Custody
    /// `InitializeReplay` and `OpenVault` requests' `rent_lamports`, and
    /// `CustodyRequestV1::validate` refuses `rent_lamports == 0` for both. The
    /// quote table is the only artifact in the family entitled to write a
    /// lifecycle-protected scalar, so if it does not carry these two entries
    /// nothing does and every registered Buy refuses at its first CPI.
    #[test]
    fn the_buy_quotes_both_custody_children_and_the_sell_quotes_neither() {
        let buy_bytes = buy();
        let buy_policy = policy(&buy_bytes);
        assert_eq!(
            buy_policy.current_rent_quote_count(),
            u16::try_from(BUY_RENT_QUOTE_COUNT).expect("Buy quote count")
        );
        let replay = buy_policy
            .current_rent_quote(0)
            .expect("Custody replay rent");
        let vault = buy_policy
            .current_rent_quote(1)
            .expect("Custody Vault rent");
        assert_eq!(
            replay.exact_data_len(),
            u32::try_from(CUSTODY_REPLAY_BYTES_V1).expect("replay width")
        );
        assert_eq!(vault.exact_data_len(), OBSERVED_VAULT_BYTES);
        assert_eq!(
            replay.scalar_destination().index(),
            u16::try_from(REGISTERED_SCALAR_REPLAY_RENT_V4).expect("replay register")
        );
        assert_eq!(
            vault.scalar_destination().index(),
            u16::try_from(REGISTERED_SCALAR_VAULT_RENT_V4).expect("Vault register")
        );

        let sell_bytes = sell();
        assert_eq!(
            policy(&sell_bytes).current_rent_quote_count(),
            u16::try_from(SELL_RENT_QUOTE_COUNT).expect("Sell quote count")
        );
    }

    /// A Token-2022 vault carrying extensions is not 165 bytes, so the quote
    /// has to move with the observation rather than restate a constant.
    #[test]
    fn the_observed_vault_width_moves_the_emitted_quote() {
        let baseline = buy();
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        let mut extended = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(
            DirectExecutionActionV3::RegisterBuy,
            Some(DirectRegisteredCreationChildRentWidthsV4 { custody_vault: 182 }),
            &mut scratch,
            &mut extended,
        )
        .expect("Token-2022 vault");
        assert_ne!(baseline, extended);
        assert_eq!(
            policy(&extended)
                .current_rent_quote(1)
                .expect("Vault rent")
                .exact_data_len(),
            182
        );
    }

    #[test]
    fn unsupported_action_child_widths_or_wrong_width_preserves_output() {
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        let mut output = [0x55_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5];
        let before = output;
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::FillRegisteredOrdinary,
                None,
                &mut scratch,
                &mut output,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::Coordinate)
        );
        assert_eq!(output, before);

        // A Buy with no observed Custody widths is the exact shape that shipped
        // an unwritten `rent_lamports`; it is now a refusal, not a default.
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::RegisterBuy,
                None,
                &mut scratch,
                &mut output,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::ChildWidths)
        );
        assert_eq!(output, before);

        // A zero vault width is the same defect wearing an observation.
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::RegisterBuy,
                Some(DirectRegisteredCreationChildRentWidthsV4 { custody_vault: 0 }),
                &mut scratch,
                &mut output,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::ChildWidths)
        );
        assert_eq!(output, before);

        // A Sell carries no Custody child, so declaring one is a refusal too.
        let mut sell_scratch = [0_u8; DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5];
        let mut sell_output = [0x55_u8; DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5];
        let sell_before = sell_output;
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::RegisterSell,
                Some(DirectRegisteredCreationChildRentWidthsV4 {
                    custody_vault: OBSERVED_VAULT_BYTES,
                }),
                &mut sell_scratch,
                &mut sell_output,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::ChildWidths)
        );
        assert_eq!(sell_output, sell_before);

        let mut short = [0x55_u8; DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5 - 1];
        let short_before = short;
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::RegisterBuy,
                Some(DirectRegisteredCreationChildRentWidthsV4 {
                    custody_vault: OBSERVED_VAULT_BYTES,
                }),
                &mut scratch,
                &mut short,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::Coordinate)
        );
        assert_eq!(short, short_before);
    }
}

/// Encode the ONE lifecycle policy both registered creation actions share.
///
/// This is the Direct half of crossing wall B. `c8396b0b` gave
/// `LifecycleCurrentRentQuoteV5` an action tag at zero digest cost; this emits
/// the policy that uses it, so a Sell and a Buy stop needing two policies and
/// therefore two manifest entries and therefore two roots.
///
/// The union is safe because every part of it is already action-selected by the
/// contract, not by this crate:
///
/// - PLANS were always tagged (`LifecyclePlanInputV3::action`). The four here are
///   maker and record once per action, and `action_plan_count` returns two for
///   each, exactly what each side saw when it had its own policy.
/// - PROTECTED OUTPUTS are 1:1 with plans, so the same two sets appear twice.
///   That is admissible and not an accident:
///   `validate_protected_output_uniqueness` SKIPS pairs whose plans carry
///   different actions, which is the decoder saying multi-action policies were
///   always the intent.
/// - QUOTES are scoped. The Custody replay and vault a Buy opens are tagged
///   `RegisterBuy`; the maker replay and registered record both sides create are
///   unscoped. A Sell therefore projects registers 52 and 53 and never 50 or 51,
///   which is what it projected before, and cannot be made to quote rent for a
///   Custody child it does not open.
/// - SEEDS, RECIPES and BINDINGS are identical between the sides already; the
///   bindings simply repeat per action because they name a plan index.
///
/// `child_widths` is REQUIRED here, unlike the per-action encoder: this policy
/// always carries the Buy's two quotes, so the vault width is always needed.
/// A Sell built from it still never reads them.
pub fn encode_direct_registered_creation_unified_lifecycle_v5_atomic(
    child_widths: DirectRegisteredCreationChildRentWidthsV4,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredStateArtifactErrorV4> {
    if child_widths.custody_vault == 0 {
        return Err(DirectRegisteredStateArtifactErrorV4::ChildWidths);
    }
    if scratch.len() != DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5
        || output.len() != DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5
    {
        return Err(DirectRegisteredStateArtifactErrorV4::Coordinate);
    }
    let recipes = registered_creation_recipes_v4();
    let seeds = registered_creation_seeds_v4()?;
    // No allocation: this crate is `no_alloc`, so the union is built as fixed
    // arrays whose widths are the `UNIFIED_*` constants the header width is
    // computed from. If a side ever gained a plan these would not compile,
    // which is the point.
    let [sell_maker, sell_record] =
        registered_creation_plans_v4(DirectExecutionActionV3::RegisterSell)?;
    let [buy_maker, buy_record] =
        registered_creation_plans_v4(DirectExecutionActionV3::RegisterBuy)?;
    let plans: [LifecyclePlanInputV3; UNIFIED_PLAN_COUNT] =
        [sell_maker, sell_record, buy_maker, buy_record];
    let [protected_maker, protected_record] = registered_creation_protected_v4()?;
    let protected: [Option<LifecycleProtectedOutputsInputV3>; UNIFIED_PLAN_COUNT] = [
        protected_maker,
        protected_record,
        protected_maker,
        protected_record,
    ];
    let [sell_b0, sell_b1, sell_b2, sell_b3] = registered_creation_bindings_v4(0)?;
    let buy_base =
        u16::try_from(PLAN_COUNT).map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)?;
    let [buy_b0, buy_b1, buy_b2, buy_b3] = registered_creation_bindings_v4(buy_base)?;
    let bindings: [LifecycleImmutableIdentityBindingInputV4; UNIFIED_BINDING_COUNT] = [
        sell_b0, sell_b1, sell_b2, sell_b3, buy_b0, buy_b1, buy_b2, buy_b3,
    ];
    let quotes = [
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: u32::try_from(CUSTODY_REPLAY_BYTES_V1)
                .map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)?,
            scalar_destination: scalar(REGISTERED_SCALAR_REPLAY_RENT_V4)?,
            action: Some(DirectExecutionActionV3::RegisterBuy as u32),
        },
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: child_widths.custody_vault,
            scalar_destination: scalar(REGISTERED_SCALAR_VAULT_RENT_V4)?,
            action: Some(DirectExecutionActionV3::RegisterBuy as u32),
        },
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: MAKER_BYTES_U32,
            scalar_destination: scalar(REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4)?,
            action: None,
        },
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: RECORD_BYTES_U32,
            scalar_destination: scalar(REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4)?,
            action: None,
        },
    ];
    encode_lifecycle_policy_v5_atomic(
        &recipes, &seeds, &plans, &protected, &bindings, &quotes, scratch, output,
    )
    .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The parts BOTH encoders share, with one author each.
//
// The per-action policies and the unified one differ in exactly three places --
// how many plans, how many bindings, and whether the quotes are scoped. Every
// other component is identical, so it is defined once here rather than copied.
// A predicate or a table with two authors drifts; this crate has already paid
// for that lesson twice this session.
// ---------------------------------------------------------------------------

fn registered_creation_recipes_v4() -> [LifecycleRecipeInputV3; RECIPE_COUNT] {
    [
        recipe(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            0,
            MAKER_SEED_COUNT_U8,
            MAKER_BYTES_U32,
        ),
        recipe(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            RECORD_SEED_START,
            RECORD_SEED_COUNT_U8,
            RECORD_BYTES_U32,
        ),
    ]
}

fn registered_creation_seeds_v4()
-> Result<[LifecycleSeedInputV3<'static>; SEED_COUNT], DirectRegisteredStateArtifactErrorV4> {
    Ok([
        LifecycleSeedInputV3::Literal(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2),
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_NONCE_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CanonicalBump,
    ])
}

fn registered_creation_plans_v4(
    action: DirectExecutionActionV3,
) -> Result<[LifecyclePlanInputV3; PLAN_COUNT], DirectRegisteredStateArtifactErrorV4> {
    Ok([
        plan(
            action,
            0,
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4,
            REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4,
        )?,
        plan(
            action,
            1,
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
            REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4,
            REGISTERED_IDENTITY_RECORD_BENEFICIARY_OBSERVATION_V4,
        )?,
    ])
}

fn registered_creation_protected_v4() -> Result<
    [Option<LifecycleProtectedOutputsInputV3>; PLAN_COUNT],
    DirectRegisteredStateArtifactErrorV4,
> {
    Ok([
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(REGISTERED_SCALAR_MAKER_CREATED_V4)?,
            bump_observation: scalar(REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4)?,
            bump: scalar(REGISTERED_SCALAR_MAKER_BUMP_V4)?,
            historical_rent_principal: scalar(REGISTERED_SCALAR_MAKER_PRINCIPAL_V4)?,
            beneficiary: identity(REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4)?,
            state: identity(REGISTERED_IDENTITY_MAKER_STATE_V4)?,
            owner: identity(REGISTERED_IDENTITY_MAKER_STATE_OWNER_V4)?,
        }),
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(REGISTERED_SCALAR_RECORD_CREATED_V4)?,
            bump_observation: scalar(REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4)?,
            bump: scalar(REGISTERED_SCALAR_RECORD_BUMP_V4)?,
            historical_rent_principal: scalar(REGISTERED_SCALAR_RECORD_PRINCIPAL_V4)?,
            beneficiary: identity(REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4)?,
            state: identity(REGISTERED_IDENTITY_RECORD_STATE_V4)?,
            owner: identity(REGISTERED_IDENTITY_RECORD_STATE_OWNER_V4)?,
        }),
    ])
}

/// Plan index, within one action's plan pair, of the maker-replay plan.
///
/// It was written as `base + 0` to say exactly this, which is a comment the
/// compiler deletes and a lint then reports as an operation with no effect.
/// Naming it keeps the statement and loses the arithmetic.
const REPLAY_PLAN_OFFSET_V4: u16 = 0;
/// Plan index, within one action's plan pair, of the registered-record plan.
const RECORD_PLAN_OFFSET_V4: u16 = 1;

/// The four bindings, rebased onto one action's plan pair.
///
/// A binding names a PLAN INDEX, so the unified policy repeats them per action
/// with `base` stepping over that action's plans.
fn registered_creation_bindings_v4(
    base: u16,
) -> Result<
    [LifecycleImmutableIdentityBindingInputV4; BINDING_COUNT],
    DirectRegisteredStateArtifactErrorV4,
> {
    let replay_plan = base
        .checked_add(REPLAY_PLAN_OFFSET_V4)
        .ok_or(DirectRegisteredStateArtifactErrorV4::Coordinate)?;
    let record_plan = base
        .checked_add(RECORD_PLAN_OFFSET_V4)
        .ok_or(DirectRegisteredStateArtifactErrorV4::Coordinate)?;
    Ok([
        binding(
            replay_plan,
            DirectMakerReplayLayoutV1::MARKET,
            REGISTERED_IDENTITY_MARKET_V4,
        )?,
        binding(
            replay_plan,
            DirectMakerReplayLayoutV1::MAKER,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        )?,
        binding(
            record_plan,
            DirectRegisteredRecordLayoutV2::MAKER,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        )?,
        binding(
            record_plan,
            DirectRegisteredRecordLayoutV2::INTENT + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            REGISTERED_IDENTITY_MARKET_V4,
        )?,
    ])
}

#[cfg(test)]
mod unified_lifecycle_tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;
    use dclutch_vm::account_profile::lifecycle_v3::StateLifecyclePolicyV5;

    const OBSERVED_VAULT_BYTES: u32 = 165;

    fn unified() -> [u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5] {
        let mut scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        let mut output = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_unified_lifecycle_v5_atomic(
            DirectRegisteredCreationChildRentWidthsV4 {
                custody_vault: OBSERVED_VAULT_BYTES,
            },
            &mut scratch,
            &mut output,
        )
        .expect("unified registered creation lifecycle");
        output
    }

    /// **Wall B's premise is false now: ONE policy serves both creation actions.**
    ///
    /// The wall was that a Sell's policy and a Buy's policy had different widths
    /// and therefore different digests, so one manifest entry -- which pins one
    /// `derivation_policy` -- could not admit both. This decodes one policy and
    /// asks it, per action, for exactly what each side used to get from a policy
    /// of its own.
    #[test]
    fn one_policy_serves_both_registered_creation_actions() {
        let bytes = unified();
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
            .expect("unified policy decodes");

        for action in [
            DirectExecutionActionV3::RegisterSell,
            DirectExecutionActionV3::RegisterBuy,
        ] {
            assert_eq!(
                policy.action_plan_count(action as u32),
                Ok(u16::try_from(PLAN_COUNT).expect("plan count")),
                "{action:?} must see exactly the maker and record plans it had alone",
            );
        }

        // The whole point of the action tag: the two sides quote DIFFERENT rent,
        // out of one policy. A Sell must not be able to quote rent for a Custody
        // child it never opens.
        assert_eq!(policy.current_rent_quote_count(), 4);
        let sell_quotes = quote_destinations(&bytes, DirectExecutionActionV3::RegisterSell);
        let buy_quotes = quote_destinations(&bytes, DirectExecutionActionV3::RegisterBuy);
        assert_eq!(
            sell_quotes.len(),
            SELL_RENT_QUOTE_COUNT,
            "a Sell projects only the two accounts both sides create",
        );
        assert_eq!(buy_quotes.len(), BUY_RENT_QUOTE_COUNT);
        for destination in &sell_quotes {
            assert!(
                buy_quotes.contains(destination),
                "every Sell quote is also a Buy quote; the Buy adds two",
            );
        }
        assert!(
            !sell_quotes.contains(&scalar(REGISTERED_SCALAR_REPLAY_RENT_V4).expect("replay"))
                && !sell_quotes.contains(&scalar(REGISTERED_SCALAR_VAULT_RENT_V4).expect("vault")),
            "a Sell must never quote the Custody replay or vault",
        );
    }

    fn quote_destinations(bytes: &[u8], action: DirectExecutionActionV3) -> Vec<u16> {
        let policy =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], bytes).expect("policy");
        let mut found = Vec::new();
        let mut ordinal = 0_u16;
        while ordinal < policy.current_rent_quote_count() {
            let quote = policy.current_rent_quote(ordinal).expect("quote");
            if quote.applies_to(action as u32) {
                found.push(quote.scalar_destination().index());
            }
            ordinal += 1;
        }
        found
    }

    /// The two per-action policies this replaces had different widths, which was
    /// the wall; this one has a single width and therefore a single digest.
    #[test]
    fn the_unified_policy_is_one_digest_where_there_were_two_widths() {
        assert_ne!(
            DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5, DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5,
            "the wall this crosses was two widths; if they ever match, say why",
        );
        assert!(unified().len() == DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5);
    }
}
