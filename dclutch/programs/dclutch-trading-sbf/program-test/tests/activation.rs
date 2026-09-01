//! ProgramTest evidence for the common data-defined Trading lifecycle outer.

use std::{path::PathBuf, vec::Vec};

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_capability_activation_codec::{
    ActivationBundleInputV1, ActivationTailFieldV1, build_activation_bundle_v1,
};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, CompartmentFundingV1, ContentId,
    FundingAmountsV1, FundingCompartment, FundingLedgerStatusV2, FundingLedgerV2, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
    funding_ledger_remaining_offset_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
    CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
    CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_PROFILE_V2, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRootAccountV1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    activation_registers_v2::{
        ACTIVATION_ACTION_SCALAR_V2, ACTIVATION_CONFIG_IDENTITY_V2,
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2, ACTIVATION_GENERATION_SCALAR_V2,
        ACTIVATION_MARKET_IDENTITY_V2, ACTIVATION_ROOT_ACCOUNT_V2, ACTIVATION_ROOT_IDENTITY_V2,
        ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
    },
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, SelectorWidthV2, encode_program_set_v2,
        encoded_program_set_bytes_v2,
    },
};
use dclutch_effect_kernel::v2::{
    SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
};
use dclutch_general_config_contract::{
    root::{
        GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_ACTIVE_HEADER_WORD_V2, GENERAL_ROOT_BYTES_V2,
        GENERAL_ROOT_CONFIG_ID_OFFSET_V2, GENERAL_ROOT_GENERATION_OFFSET_V2,
        GENERAL_ROOT_HEADER_WORD_OFFSET_V2, GENERAL_ROOT_INITIAL_REVISION_V2,
        GENERAL_ROOT_MAGIC_OFFSET_V2, GENERAL_ROOT_MAGIC_WORD_V2, GENERAL_ROOT_MARKET_OFFSET_V2,
        GENERAL_ROOT_REVISION_OFFSET_V2, GENERAL_ROOT_SCHEMA_ID_V2, GeneralRootV2,
        general_root_creation_tail_v2,
    },
    v3::{GeneralConfigV3, GeneralConfigV3Input},
};
use dclutch_market_core_codec::{
    CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, Identity, MarketCoreStateSeedsV2,
    MarketIdentity, Phase, Readiness, Role, StateBumpsV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_series_v3_kernel::{
    SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
    SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_DERIVATION_PREIMAGE_V3, TemplateV3, generated as series_generated,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3},
    template_content_id,
};
use dclutch_trading_sbf::TradingSbfError;
use dclutch_trading_sbf::dispatch::TRADING_CLOSE_RENT_CREDIT_IDENTITY_V2;
use dclutch_trading_sbf::series::{
    activation_bundle_v1::{
        SeriesActivationBundleInputV1, build_series_activation_bundle_v1,
        build_series_activation_capable_program_set_v1, series_activation_funding_plan_v1,
        series_activation_request_v1,
    },
    release_v5::{SeriesActionArtifactIdsV5, encode_series_action_descriptor_v5},
};
use dclutch_transition_vm::v2::encode::{
    RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
    encode_transition_program_v2_atomic, transition_program_v2_bytes,
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::instruction::InstructionError;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

/// `TradingSbfError::Root`, the refusal the composite-root plan carries.
const TRADING_ROOT_REFUSAL_CODE: u32 = TradingSbfError::Root as u32;
/// `TradingSbfError::Content`, the refusal record and selection joins carry.
const TRADING_CONTENT_REFUSAL_CODE: u32 = TradingSbfError::Content as u32;
/// `TradingSbfError::UnsupportedContent`, the refusal an unadmitted schema carries.
const TRADING_UNSUPPORTED_REFUSAL_CODE: u32 = TradingSbfError::UnsupportedContent as u32;
/// Selector the family activation request carries, and the set entry's own.
const FAMILY_ACTIVATION_SELECTOR: u32 = 1;
/// Selector the same ProgramSet assigns to its distinct close descriptor.
const FAMILY_CLOSE_SELECTOR: u32 = 2;

const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const WRONG_REGISTRY_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x75; 32]);
const REFUND_WALLET: Pubkey = Pubkey::new_from_array([0x76; 32]);
const GENERATION: u64 = 7;
const ROOT_INITIAL_DUST: u64 = 1;
/// Family root-tail width: one projected scalar then one projected identity.
///
/// The outer never decodes a family tail, so this fixture stands in for a real
/// family root the only way the seam can observe one -- as the exact width the
/// descriptor declares and the exact bytes the effect program's request buffer
/// projects.
const ROOT_TAIL_BYTES: usize = 40;
/// Tail offset of the projected generation scalar.
const TAIL_GENERATION_OFFSET: u32 = 0;
/// Tail offset of the projected Market identity.
const TAIL_MARKET_OFFSET: u32 = 8;
/// Scalar the profile projects the FundingLedger's remaining Rent quote into.
///
/// It is past the eight common slots the seam seeds, so nothing the outer wrote
/// is overwritten -- the register ABI's own boundary, not a coincidence.
const FUNDING_RENT_SCALAR_REGISTER: u16 = 6;
/// Scalar the profile projects the vacant root's prestate lamports into.
const ROOT_PRESTATE_SCALAR_REGISTER: u16 = 7;

const PROFILE_ACCOUNT_COUNT: u16 = 2;
const SCALAR_COUNT: u16 = 8;
const IDENTITY_COUNT: u16 = 12;
/// Close descriptor scalar holding the expected close action.
const CLOSE_ACTION_SCALAR: u16 = 8;
/// Close profile projection of the persisted root generation.
const CLOSE_ROOT_GENERATION_SCALAR: u16 = 9;
/// Close profile projection of the root's exact pre-close lamports.
const CLOSE_ROOT_LAMPORTS_SCALAR: u16 = 10;
const CLOSE_SCALAR_COUNT: u16 = 11;
const CLOSE_IDENTITY_COUNT: u16 = 13;
const CLOSE_PROFILE_ACCOUNT_COUNT: u16 = 3;
const CLOSE_REMAINING_NATIVE_PRINCIPAL: u64 = 17;
/// Manifest-declared `FundingCompartment::Creation` principal, delivered into
/// the root by activation rather than left parked in the ledger.
const CREATION_PRINCIPAL: u64 = 4_321;
/// Template-authenticated close principal for the Series campaign.
const SERIES_CLOSE_RENT: u64 = 5_000;

/// Scalar bank a General activation declares.
///
/// Eight common slots the seam seeds, one the profile projects the Rent quote
/// into, and three the transition loads with the constants an EffectProgram has
/// no way to produce -- it can only move a register.
const GENERAL_SCALAR_COUNT: u16 = 12;
/// Scalar the General profile projects the FundingLedger's Rent quote into.
const GENERAL_FUNDING_RENT_SCALAR: u16 = 8;
/// Scalar the General transition loads with `GENERAL_ROOT_MAGIC_WORD_V2`.
const GENERAL_MAGIC_SCALAR: u16 = 9;
/// Scalar the General transition loads with `GENERAL_ROOT_ACTIVE_HEADER_WORD_V2`.
const GENERAL_HEADER_WORD_SCALAR: u16 = 10;
/// Scalar the General transition loads with `GENERAL_ROOT_INITIAL_REVISION_V2`.
const GENERAL_REVISION_SCALAR: u16 = 11;

/// Which family's activation artifacts a campaign carries.
///
/// The seam is family-neutral and this is not a kind branch inside it: it is
/// the fixture choosing which family's real artifacts to publish, exactly as a
/// release author would.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Family {
    /// The neutral fixture: one projected scalar then one projected identity.
    Fixture,
    /// General: a real `GeneralRootV2` composed from its published coordinates.
    General,
    /// Series: a real `SeriesStateV3` composed from its own Template.
    ///
    /// Series never reaches the hand-built artifact builders below — it
    /// publishes its own profile, transition and effect through
    /// `series::activation_bundle_v1` — so where those builders match on the
    /// family, the Series arm exists only for exhaustiveness.
    Series,
}

impl Family {
    /// Exact family root-tail width the descriptor declares.
    const fn root_tail_bytes(self) -> usize {
        match self {
            Self::Fixture => ROOT_TAIL_BYTES,
            Self::General => GENERAL_ROOT_BYTES_V2,
            Self::Series => SERIES_STATE_BYTES_V3,
        }
    }

    /// Exact scalar bank the profile, transition and effect program share.
    const fn scalars(self) -> u16 {
        match self {
            Self::Fixture => SCALAR_COUNT,
            Self::General => GENERAL_SCALAR_COUNT,
            // Series publishes codec-built artifacts which carry their own bank.
            Self::Series => SCALAR_COUNT,
        }
    }
}

#[derive(Clone)]
struct Fixture {
    instruction: Instruction,
    root: Pubkey,
    funding: Pubkey,
    /// Raw record the selection's `capability_release` names.
    descriptor_raw: Pubkey,
    /// Raw record carrying the `CapabilityProgramV1` the seam actually runs.
    activation_descriptor_raw: Pubkey,
    hostile_record: Pubkey,
    market: Pubkey,
    root_rent: u64,
    funding_rent: u64,
    /// Which family's artifacts this fixture published.
    family: Family,
    /// Exact family root-tail width the descriptor declared.
    root_tail_bytes: usize,
    /// Content identity of the config record the selection names.
    config_id: ContentId,
    /// Exact capability-manifest bytes the seam authenticated.
    manifest: Vec<u8>,
    /// Content identity of those bytes.
    manifest_id: ContentId,
    /// Exact FundingLedgerV2 prestate the seam observed.
    funding_prestate: Vec<u8>,
    /// ProgramSet-selected close instruction for the lifecycle campaign.
    close_instruction: Option<Instruction>,
    /// Exact Market RentCredit receiving native principal, rent, and surplus.
    rent_credit: Pubkey,
    /// Native principal intentionally left in the ledger after activation.
    close_principal: u64,
    /// Manifest-declared Creation principal the activation must deliver.
    creation_principal: u64,
    /// Exact config record the selection named; for Series this is its Template.
    config: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Campaign {
    Success,
    LateEffectRefusal,
    /// Declares the tail width and projects nothing into it.
    UnwrittenTail,
    /// Projects the whole tail into a request buffer wider than the tail.
    MismatchedTailWidth,
    /// `capability_release` names a `CapabilityProgramSetV2`, not a descriptor.
    ProgramSetRelease,
    /// The selected set entry names a descriptor schema this seam cannot run.
    ProgramSetWrongSchema,
    /// No set entry admits the selector the family activation request carries.
    ProgramSetMissingSelector,
    /// General's own three activation artifacts, creating a real `GeneralRootV2`.
    General,
    /// General's artifacts with the two constant words never projected.
    GeneralUnwrittenMagic,
    /// ProgramSet with separate activation and native close descriptors.
    NativeClose,
    /// A manifest declaring a nonzero `Creation` compartment, with artifacts
    /// built by the real family-neutral codec that deliver it into the root.
    CreationPrincipal,
    /// Series' own activation bundle, against its own Template as the config
    /// record, published through the six-entry activation-capable release set
    /// its compiler emits. The prepaid close principal rides the Creation
    /// compartment, and the root must open decodable as `SeriesStateV3`.
    Series,
    /// The same manifest, with rent-only artifacts that never move it. The
    /// compartment is released by `activate_in_place` and `release_in_place`
    /// refuses it forever, so an activation that does not deliver it strands
    /// it: the seam must refuse rather than create that root.
    CreationPrincipalNotDelivered,
}

impl Campaign {
    /// Whether `capability_release` names a set rather than a flat descriptor.
    const fn program_set(self) -> bool {
        matches!(
            self,
            Self::ProgramSetRelease
                | Self::ProgramSetWrongSchema
                | Self::ProgramSetMissingSelector
                | Self::General
                | Self::GeneralUnwrittenMagic
                | Self::NativeClose
                | Self::Series
        )
    }

    /// Manifest-declared Creation compartment for this campaign.
    const fn creation_principal(self) -> u64 {
        match self {
            Self::CreationPrincipal | Self::CreationPrincipalNotDelivered => CREATION_PRINCIPAL,
            // Series' principal is not a fixture number: it is whatever its
            // Template says, and the same value the root must persist.
            Self::Series => SERIES_CLOSE_RENT,
            _ => 0,
        }
    }

    /// Which family's artifacts this campaign publishes.
    const fn family(self) -> Family {
        match self {
            Self::General | Self::GeneralUnwrittenMagic => Family::General,
            Self::Series => Family::Series,
            _ => Family::Fixture,
        }
    }
}

/// One-entry `CapabilityProgramSetV2` naming the activation descriptor.
///
/// The selector is read from byte 0 of the family activation request, which is
/// the same one-byte action the flat campaigns already send. A real family puts
/// its activation action wherever its own request grammar puts an action.
fn program_set(
    descriptor_id: ContentId,
    close_descriptor_id: Option<ContentId>,
    campaign: Campaign,
) -> Vec<u8> {
    let schema = match campaign {
        Campaign::ProgramSetWrongSchema => id(0x77),
        _ => ContentId::new(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1).expect("descriptor schema"),
    };
    let selector = match campaign {
        Campaign::ProgramSetMissingSelector => FAMILY_ACTIVATION_SELECTOR + 1,
        _ => FAMILY_ACTIVATION_SELECTOR,
    };
    let entry = CapabilityProgramSetEntryV2::new(
        selector,
        CapabilityDescriptorReferenceV2::new(schema, descriptor_id),
    );
    let close = close_descriptor_id.map(|value| {
        CapabilityProgramSetEntryV2::new(
            FAMILY_CLOSE_SELECTOR,
            CapabilityDescriptorReferenceV2::new(
                ContentId::new(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1).expect("descriptor schema"),
                value,
            ),
        )
    });
    let entries = close.map_or_else(|| vec![entry], |value| vec![entry, value]);
    let mut output = vec![0_u8; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
    encode_program_set_v2(0, SelectorWidthV2::U8, &entries, &mut output).expect("set bytes");
    output
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero content")
}

/// One immutable `GeneralConfigV3` bound to the release it is activated under.
///
/// `require_entry` joins the manifest entry to exactly two of these fields --
/// `program_set_id` and `capacity_profile_id` -- so the config cannot be a
/// placeholder if the General activation oracle is to accept the same manifest the
/// seam read. The remaining capacities are plausible and unread by activation.
fn general_config(capacity_profile_id: [u8; 32], program_set_id: [u8; 32]) -> GeneralConfigV3 {
    GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id,
        claim_basis_id: [0x51; 32],
        program_set_id,
        generation: GENERATION,
        price_scale: 1,
        collection_slots: 1,
        selection_slots: 1,
        settlement_slots: 1,
        max_orders_per_candidate: 8,
        max_pages_per_candidate: 8,
        continuation_reward_lamports: 1,
        selection_policy_id: [0x52; 32],
        quote_surplus_beneficiary: [0x53; 32],
    })
    .expect("General config")
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero identity")
}

fn program_identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program")
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let end = offset.checked_add(source.len()).expect("fixture width");
    output
        .get_mut(offset..end)
        .expect("fixture destination")
        .copy_from_slice(source);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

/// Encode one artifact through its owning crate's public encoder.
///
/// Every artifact below is built this way. The three generations' offsets and
/// opcodes are private to their crates, so before they had encoders this file
/// wrote `b"DCTV"`, `b"DCE2"` and the AccountProfile header at literal offsets
/// and passed bare opcode integers with comments -- a second ABI authority
/// living in a test. Nothing here writes an artifact byte any more.
fn encoded<E, T>(width: usize, encode: E) -> Vec<u8>
where
    E: Fn(&mut [u8], &mut [u8]) -> Result<(), T>,
    T: core::fmt::Debug,
{
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode(&mut scratch, &mut output).expect("artifact encodes");
    output
}

/// The two account rules every activation in this file declares.
///
/// The seam pins their order itself: the composite root is
/// `ACTIVATION_ROOT_ACCOUNT_V2` and the FundingLedgers follow from
/// `ACTIVATION_FIRST_FUNDING_ACCOUNT_V2`, in role-request order.
fn activation_rules() -> [AccountRuleInputV1; 2] {
    [
        // The composite root: vacant, credited by the funding transfer.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: 0,
        },
        // The FundingLedger: debited, and rewritten by the outer's own commit.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(funding_ledger_bytes_v2(1).expect("funding width"))
                .expect("funding width"),
        },
    ]
}

/// Encode one `AccountProfileV1` over the family's declared register bank.
fn account_profile(family: Family) -> Vec<u8> {
    let rules = activation_rules();
    let mut operations = vec![
        AccountOperationInputV1::RequireKey {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
    ];
    // An EffectProgram has no arithmetic over account data -- it can only move a
    // register's worth of lamports -- so a funded activation MUST project the
    // live FundingLedger's remaining Rent quote into a scalar here.
    operations.push(AccountOperationInputV1::ProjectDataU64 {
        account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        data_offset: u32::try_from(
            funding_ledger_remaining_offset_v2(0, FundingCompartment::Rent)
                .expect("rent quote offset"),
        )
        .expect("rent quote offset"),
        destination: match family {
            Family::Fixture | Family::Series => FUNDING_RENT_SCALAR_REGISTER,
            Family::General => GENERAL_FUNDING_RENT_SCALAR,
        },
    });
    if family == Family::Fixture {
        // Observe vacant-root dust for the late effect check.
        operations.push(AccountOperationInputV1::ProjectLamports {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            destination: ROOT_PRESTATE_SCALAR_REGISTER,
        });
    }
    let width = account_profile_v1_bytes(rules.len(), operations.len()).expect("profile width");
    encoded(width, |scratch, output| {
        encode_account_profile_v1_atomic(
            &rules,
            &operations,
            RegisterGeometryV1 {
                scalars: family.scalars(),
                identities: IDENTITY_COUNT,
            },
            scratch,
            output,
        )
    })
}

/// Close profile over the existing root, selected ledger, and exact RentCredit.
fn close_account_profile(family: Family) -> Vec<u8> {
    let rules = [
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + family.root_tail_bytes())
                .expect("root width"),
        },
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(funding_ledger_bytes_v2(1).expect("funding width"))
                .expect("funding width"),
        },
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(
                dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2,
            )
            .expect("credit width"),
        },
    ];
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            data_offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1).expect("root tail offset"),
            destination: CLOSE_ROOT_GENERATION_SCALAR,
        },
        AccountOperationInputV1::ProjectLamports {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            destination: CLOSE_ROOT_LAMPORTS_SCALAR,
        },
        AccountOperationInputV1::RequireKey {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2 + 1,
            expected: TRADING_CLOSE_RENT_CREDIT_IDENTITY_V2,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len()).expect("profile width");
    encoded(width, |scratch, output| {
        encode_account_profile_v1_atomic(
            &rules,
            &operations,
            RegisterGeometryV1 {
                scalars: CLOSE_SCALAR_COUNT,
                identities: CLOSE_IDENTITY_COUNT,
            },
            scratch,
            output,
        )
    })
}

/// Encode one transition `ProgramV2` over the family's declared register bank.
fn transition_program(family: Family) -> Vec<u8> {
    let instructions = match family {
        // loadConst scalar[0] = activation action. Other projected registers survive.
        Family::Fixture | Family::Series => vec![TransitionInstructionV2::load_const(
            ACTIVATION_ACTION_SCALAR_V2,
            CoreEffectActionV1::ActivateCapability as u64,
        )],
        // The three words a `GeneralRootV2` tail needs that are neither a
        // register the seam seeds nor a value any account carries: the magic,
        // the active header word, and the initial revision. Market, config and
        // generation are already common registers, so they are not loaded here.
        Family::General => vec![
            TransitionInstructionV2::load_const(GENERAL_MAGIC_SCALAR, GENERAL_ROOT_MAGIC_WORD_V2),
            TransitionInstructionV2::load_const(
                GENERAL_HEADER_WORD_SCALAR,
                GENERAL_ROOT_ACTIVE_HEADER_WORD_V2,
            ),
            TransitionInstructionV2::load_const(
                GENERAL_REVISION_SCALAR,
                GENERAL_ROOT_INITIAL_REVISION_V2,
            ),
        ],
    };
    let width = transition_program_v2_bytes(instructions.len()).expect("transition width");
    encoded(width, |scratch, output| {
        encode_transition_program_v2_atomic(
            TransitionRegisterGeometryV2 {
                scalars: family.scalars(),
                identities: IDENTITY_COUNT,
            },
            &instructions,
            scratch,
            output,
        )
    })
}

fn close_transition_program() -> Vec<u8> {
    let instructions = [
        TransitionInstructionV2::load_const(
            CLOSE_ACTION_SCALAR,
            CoreEffectActionV1::CloseCapability as u64,
        ),
        TransitionInstructionV2::scalar_eq(ACTIVATION_ACTION_SCALAR_V2, CLOSE_ACTION_SCALAR),
        TransitionInstructionV2::scalar_eq(
            ACTIVATION_GENERATION_SCALAR_V2,
            CLOSE_ROOT_GENERATION_SCALAR,
        ),
    ];
    let width = transition_program_v2_bytes(instructions.len()).expect("transition width");
    encoded(width, |scratch, output| {
        encode_transition_program_v2_atomic(
            TransitionRegisterGeometryV2 {
                scalars: CLOSE_SCALAR_COUNT,
                identities: CLOSE_IDENTITY_COUNT,
            },
            &instructions,
            scratch,
            output,
        )
    })
}

fn close_effect_program() -> Vec<u8> {
    let instructions = [EffectInstructionV2::require_lamports_eq(
        ACTIVATION_ROOT_ACCOUNT_V2,
        CLOSE_ROOT_LAMPORTS_SCALAR,
    )];
    let width = effect_program_v2_bytes(instructions.len()).expect("effect width");
    encoded(width, |scratch, output| {
        encode_effect_program_v2_atomic(
            EffectGeometryV2 {
                accounts: CLOSE_PROFILE_ACCOUNT_COUNT,
                scalars: CLOSE_SCALAR_COUNT,
                identities: CLOSE_IDENTITY_COUNT,
                request_bytes: 0,
            },
            &instructions,
            scratch,
            output,
        )
    })
}

/// Request writes that compose the family root tail, in tail order.
///
/// For General every offset and both constant scalars come from
/// `dclutch-general-config-contract`'s published creation coordinates, and the
/// Market, config and generation come from `activation_registers_v2`. Nothing
/// about `GeneralRootV2`'s layout is restated here.
fn tail_writes(campaign: Campaign) -> Vec<EffectInstructionV2> {
    // `GeneralUnwrittenMagic` projects everything a General tail needs EXCEPT the
    // two words that make it decodable. The tail is not all zero, so the seam's
    // one tail check cannot see it: the seam owns no family decoder by design.
    let constant_words = !matches!(campaign, Campaign::GeneralUnwrittenMagic);
    match campaign.family() {
        Family::Fixture | Family::Series => vec![
            EffectInstructionV2::write_request_u64(
                TAIL_GENERATION_OFFSET,
                ACTIVATION_GENERATION_SCALAR_V2,
            ),
            EffectInstructionV2::write_request_identity(
                TAIL_MARKET_OFFSET,
                ACTIVATION_MARKET_IDENTITY_V2,
            ),
        ],
        Family::General => [
            constant_words.then(|| {
                EffectInstructionV2::write_request_u64(
                    tail_offset(GENERAL_ROOT_MAGIC_OFFSET_V2),
                    GENERAL_MAGIC_SCALAR,
                )
            }),
            constant_words.then(|| {
                EffectInstructionV2::write_request_u64(
                    tail_offset(GENERAL_ROOT_HEADER_WORD_OFFSET_V2),
                    GENERAL_HEADER_WORD_SCALAR,
                )
            }),
            Some(EffectInstructionV2::write_request_identity(
                tail_offset(GENERAL_ROOT_MARKET_OFFSET_V2),
                ACTIVATION_MARKET_IDENTITY_V2,
            )),
            Some(EffectInstructionV2::write_request_identity(
                tail_offset(GENERAL_ROOT_CONFIG_ID_OFFSET_V2),
                ACTIVATION_CONFIG_IDENTITY_V2,
            )),
            Some(EffectInstructionV2::write_request_u64(
                tail_offset(GENERAL_ROOT_GENERATION_OFFSET_V2),
                ACTIVATION_GENERATION_SCALAR_V2,
            )),
            Some(EffectInstructionV2::write_request_u64(
                tail_offset(GENERAL_ROOT_REVISION_OFFSET_V2),
                GENERAL_REVISION_SCALAR,
            )),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

fn tail_offset(offset: usize) -> u32 {
    u32::try_from(offset).expect("tail offset")
}

fn effect_program(campaign: Campaign) -> Vec<u8> {
    let family = campaign.family();
    // Instruction 0 is always the funding transfer. The request writes that
    // compose the family root tail follow it, except in `UnwrittenTail`. The late
    // requirement, when present, is last so it runs after the transfer.
    let mut instructions = vec![EffectInstructionV2::transfer_lamports(
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_ROOT_ACCOUNT_V2,
        match family {
            Family::Fixture | Family::Series => FUNDING_RENT_SCALAR_REGISTER,
            Family::General => GENERAL_FUNDING_RENT_SCALAR,
        },
    )];
    if !matches!(campaign, Campaign::UnwrittenTail) {
        instructions.extend(tail_writes(campaign));
    }
    if matches!(campaign, Campaign::LateEffectRefusal) {
        // After the transfer, root lamports cannot still equal prestate scalar[7].
        instructions.push(EffectInstructionV2::require_lamports_eq(
            ACTIVATION_ROOT_ACCOUNT_V2,
            ROOT_PRESTATE_SCALAR_REGISTER,
        ));
    }
    let request_bytes = match campaign {
        Campaign::MismatchedTailWidth => family.root_tail_bytes() + 8,
        _ => family.root_tail_bytes(),
    };
    let width = effect_program_v2_bytes(instructions.len()).expect("effect width");
    encoded(width, |scratch, output| {
        encode_effect_program_v2_atomic(
            EffectGeometryV2 {
                accounts: PROFILE_ACCOUNT_COUNT,
                scalars: family.scalars(),
                identities: IDENTITY_COUNT,
                request_bytes: u16::try_from(request_bytes).expect("request width"),
            },
            &instructions,
            scratch,
            output,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn descriptor(
    family: Family,
    profile_id: [u8; 32],
    effect_id: [u8; 32],
    kind: ContentId,
    capacity: ContentId,
    root_schema: ContentId,
    derivation: ContentId,
    config_schema: ContentId,
) -> Vec<u8> {
    let transition = transition_program(family);
    descriptor_with_transition(
        family,
        profile_id,
        effect_id,
        kind,
        capacity,
        root_schema,
        derivation,
        config_schema,
        &transition,
    )
}

#[allow(clippy::too_many_arguments)]
fn descriptor_with_transition(
    family: Family,
    profile_id: [u8; 32],
    effect_id: [u8; 32],
    kind: ContentId,
    capacity: ContentId,
    root_schema: ContentId,
    derivation: ContentId,
    config_schema: ContentId,
    transition: &[u8],
) -> Vec<u8> {
    let mut output = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
    put(&mut output, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
    put_u16(&mut output, 8, 1);
    put_u16(
        &mut output,
        CAPABILITY_PROGRAM_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_PROFILE_V2,
    );
    for (offset, value) in [
        (CAPABILITY_PROGRAM_KIND_OFFSET, kind.to_bytes()),
        (
            CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
            config_schema.to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
            id(0x23).to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
            root_schema.to_bytes(),
        ),
        (CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, profile_id),
        (
            CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
            derivation.to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
            capacity.to_bytes(),
        ),
        (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, effect_id),
    ] {
        put(&mut output, offset, &value);
    }
    put_u32(
        &mut output,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        u32::try_from(family.root_tail_bytes()).expect("tail width"),
    );
    put(&mut output, CAPABILITY_PROGRAM_HEADER_BYTES_V1, transition);
    CapabilityProgramV1::decode(&output).expect("descriptor");
    output
}

/// The ProgramData address Loader V3 derives for `program`.
fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// The deployment slot every release in this fixture is pinned to.
///
/// One value, used by the release and by the ProgramData account staged for it,
/// because a slot pin is exactly an equality between those two and a fixture
/// that let them drift would stage a superseded deployment by accident.
const FIXTURE_DEPLOYMENT_SLOT: u64 = 0;

/// Loader V3's Program account body: the variant tag, then the ProgramData link.
fn loader_program_bytes(program: Pubkey) -> Vec<u8> {
    let mut output = vec![0_u8; 36];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..36)
        .expect("link")
        .copy_from_slice(programdata_address(program).as_ref());
    output
}

/// Loader V3's ProgramData body: the 45-byte metadata span, then the ELF.
///
/// `Immutable` with no upgrade authority, matching what [`release`] binds, so
/// `slot_pinned_release_elf_digest_v1` takes the activation-bound digest and
/// never hashes this tail. The tail still has to BE there -- the runtime
/// executes the program out of it.
fn loader_programdata_bytes(elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&FIXTURE_DEPLOYMENT_SLOT.to_le_bytes());
    output.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    output
}

/// One test program's ELF, from the directory that built it.
fn test_program_elf(name: &str) -> Vec<u8> {
    let directory = PathBuf::from(std::env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    std::fs::read(directory.join(format!("{name}.so"))).expect("required test-program ELF")
}

/// Stage one role as a real Loader V3 upgradeable deployment.
///
/// # Why this fixture stopped staging a fake one
///
/// It used to name a loader of `[0x91; 32]`, a ProgramData address of
/// `[seed + 1; 32]`, and it staged that ProgramData as a system-program-owned
/// account holding one byte. None of that is a deployment, and the fixture got
/// away with it because the seam under test asked a MOCK Registry
/// (`test-programs/registry`) to reauthenticate the roles, and that mock reads
/// the cache, compares one program id and returns a receipt -- it authenticates
/// no Loader account, no ProgramData link, no deployment slot and no ELF digest.
///
/// Decision 0017's option B removed the CPI, so `outer.rs` now runs the same
/// deployment authentication the real Registry runs, and the fake substrate
/// refuses. That is the fixture being wrong, not the change: `process_activation`
/// had never once been executed against a deployment anything authenticated.
fn add_upgradeable_role(test: &mut ProgramTest, name: &'static str, program: Pubkey) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let elf = test_program_elf(name);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        Rent::default().minimum_balance(45 + elf.len()),
        loader_programdata_bytes(&elf),
    );
    test.add_account(
        program,
        Account {
            lamports: Rent::default().minimum_balance(36),
            data: loader_program_bytes(program),
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, seed: u8) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        id(seed.wrapping_add(2)),
        [seed.wrapping_add(3); 32],
        FIXTURE_DEPLOYMENT_SLOT,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        value.program().to_bytes(),
        value.loader_program().to_bytes(),
        true,
        value.programdata(),
        value.loader_program().to_bytes(),
        false,
        value.programdata(),
        value.loader_program().to_bytes(),
        value.deployment_slot(),
        value.elf_digest(),
        value.upgrade_authority(),
    )
    .expect("observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

fn activation_cache() -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x31);
    let trading = release(TRADING_PROGRAM_ID, 0x41);
    let set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(core),
        binding(trading),
        binding(core),
        binding(core),
    )
    .expect("release set");
    let set_id = hash(&set.to_bytes()).to_bytes();
    let content = ContentId::new(set_id).expect("release set content");
    let mut output = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut output, content).expect("initialize cache");
    for (role, value) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, core),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, core),
    ] {
        activate_execution_role_into_v1(&mut output, content, &set, role, &activation_input(value))
            .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&output).expect("complete cache");
    (set_id, output)
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_record(test: &mut ProgramTest, schema: [u8; 32], bytes: Vec<u8>) -> (Pubkey, Pubkey) {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        test,
        raw,
        REGISTRY_PROGRAM_ID,
        Rent::default().minimum_balance(bytes.len()),
        bytes,
    );
    add_account(test, staging, system_program::ID, 1, Vec::new());
    (raw, staging)
}

fn build_fixture(campaign: Campaign) -> (ProgramTest, Fixture) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    // Trading and Core are the two roles `process_activation` authenticates out
    // of the activation cache, so they are the two that must be REAL Loader V3
    // deployments rather than plain executables. Everything else in this frame
    // is a mock this seam only calls, never authenticates.
    add_upgradeable_role(
        &mut test,
        "dclutch_trading_outer_test_program",
        TRADING_PROGRAM_ID,
    );
    add_upgradeable_role(
        &mut test,
        "dclutch_trading_core_caller_test_program",
        CORE_PROGRAM_ID,
    );
    test.add_program(
        "dclutch_trading_registry_test_program",
        REGISTRY_PROGRAM_ID,
        None,
    );
    test.add_program(
        "dclutch_trading_registry_test_program",
        WRONG_REGISTRY_ID,
        None,
    );
    test.add_program(
        "dclutch_trading_registry_test_program",
        RENT_PROGRAM_ID,
        None,
    );

    let rent = Rent::default();
    let family = campaign.family();
    let root_rent = rent.minimum_balance(232 + family.root_tail_bytes());
    let funding_rent = rent.minimum_balance(funding_ledger_bytes_v2(1).expect("funding width"));
    let profile = account_profile(family);
    let effect = effect_program(campaign);
    // General's kind and root schema are its own published protocol facts, not
    // fixture bytes: `require_entry` in `root_v3.rs` refuses a manifest entry
    // that names anything else, so a General campaign that invented them could
    // not be checked against General's own activation function.
    let mut series_template = series_generated::SERIES_EXAMPLE_TEMPLATE_V3;
    put(
        &mut series_template,
        series_generated::SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3,
        &SERIES_CLOSE_RENT.to_le_bytes(),
    );
    let series_template_id = template_content_id(&series_template).expect("Template ID");
    let (kind, root_schema) = match family {
        Family::Fixture => (id(0x11), id(0x13)),
        Family::Series => (
            ContentId::new(hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes())
                .expect("Series kind"),
            ContentId::new(hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes())
                .expect("Series root schema"),
        ),
        Family::General => (
            ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("General kind"),
            ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2).expect("General root schema"),
        ),
    };
    // Series stores its exact Template identity in `capacity_profile`; that is
    // what `validate_selection` joins against the manifest entry, and what the
    // activation bundle's completeness gate requires.
    let capacity = match family {
        Family::Series => series_template_id,
        _ => id(0x12),
    };
    let derivation = match family {
        Family::Series => ContentId::new(hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes())
            .expect("Series derivation"),
        _ => id(0x14),
    };
    let config_schema = match family {
        Family::Series => {
            ContentId::new(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3).expect("Template schema")
        }
        _ => id(0x15),
    };
    let descriptor = descriptor(
        family,
        hash(&profile).to_bytes(),
        hash(&effect).to_bytes(),
        kind,
        capacity,
        root_schema,
        derivation,
        config_schema,
    );
    // The creation-principal campaign publishes the REAL artifacts the
    // family-neutral codec builds, so what the chain executes is production
    // code rather than this fixture's imitation of it. The Fixture tail is
    // exactly a generation word then a Market identity, which is two seam
    // fields over an all-zero constant tail -- so the SAME expected-tail
    // assertion the hand-built campaigns use still applies, and any divergence
    // between the two constructions shows up as a failed root decode.
    let series_action_descriptors: Vec<Vec<u8>> = (0..5_u8)
        .map(|index| {
            encode_series_action_descriptor_v5(
                series_template_id,
                SeriesActionArtifactIdsV5 {
                    account_profile: [index.wrapping_add(0x20); 32],
                    request_profile: [index.wrapping_add(0x30); 32],
                    lifecycle: [index.wrapping_add(0x40); 32],
                    strategy: [index.wrapping_add(0x50); 32],
                    transition: [index.wrapping_add(0x60); 32],
                    effect: [index.wrapping_add(0x70); 32],
                },
            )
            .expect("Series action descriptor")
            .to_vec()
        })
        .collect();
    let (profile, effect, descriptor) = if matches!(campaign, Campaign::Series) {
        // Series' own published triple, bound to a real Series action descriptor
        // so the bundle's completeness gate joins against the Template this
        // release actually carries.
        let bundle = build_series_activation_bundle_v1(SeriesActivationBundleInputV1 {
            action_descriptor: series_action_descriptors
                .first()
                .expect("Prepare descriptor"),
            template: &series_template,
            funding_ledger_slot_count: 1,
        })
        .expect("Series activation bundle");
        (bundle.account_profile, bundle.effect, bundle.descriptor)
    } else if matches!(
        campaign,
        Campaign::CreationPrincipal | Campaign::CreationPrincipalNotDelivered
    ) {
        let mut constant_tail = [0_u8; ROOT_TAIL_BYTES];
        if !matches!(campaign, Campaign::CreationPrincipal) {
            put(
                &mut constant_tail,
                0,
                &0x4443_4c54_4649_5831_u64.to_le_bytes(),
            );
        }
        let bundle = build_activation_bundle_v1(ActivationBundleInputV1 {
            kind,
            config_schema,
            request_schema: id(0x16),
            root_schema,
            derivation_policy: derivation,
            capacity_profile: capacity,
            root_state_bytes: u32::try_from(ROOT_TAIL_BYTES).expect("tail width"),
            // The delivering campaign's tail is exactly the Fixture tail, so the
            // shared expected-tail assertion still applies to it. The hostile
            // carries one constant word instead of the generation seam field:
            // a tail composed ENTIRELY of seam fields has no constant writes,
            // and the codec will not emit an instructionless transition. The
            // seam never compares tail CONTENT -- it owns no family decoder --
            // so this difference cannot be what refuses the hostile.
            constant_root_tail: &constant_tail,
            seam_fields: if matches!(campaign, Campaign::CreationPrincipal) {
                &[
                    ActivationTailFieldV1::SeamScalar {
                        offset: 0,
                        register: ACTIVATION_GENERATION_SCALAR_V2,
                    },
                    ActivationTailFieldV1::SeamIdentity {
                        offset: 8,
                        register: ACTIVATION_MARKET_IDENTITY_V2,
                    },
                ][..]
            } else {
                &[ActivationTailFieldV1::SeamIdentity {
                    offset: 8,
                    register: ACTIVATION_MARKET_IDENTITY_V2,
                }][..]
            },
            funding_ledger_slot_count: 1,
            // The ONLY difference between the two creation campaigns.
            delivers_creation_principal: matches!(campaign, Campaign::CreationPrincipal),
        })
        .expect("real activation bundle");
        (bundle.account_profile, bundle.effect, bundle.descriptor)
    } else {
        (profile, effect, descriptor)
    };
    let descriptor_id = ContentId::new(hash(&descriptor).to_bytes()).expect("descriptor ID");
    let close_profile =
        matches!(campaign, Campaign::NativeClose).then(|| close_account_profile(family));
    let close_effect = matches!(campaign, Campaign::NativeClose).then(close_effect_program);
    let close_descriptor =
        close_profile
            .as_ref()
            .zip(close_effect.as_ref())
            .map(|(profile, effect)| {
                let transition = close_transition_program();
                descriptor_with_transition(
                    family,
                    hash(profile).to_bytes(),
                    hash(effect).to_bytes(),
                    kind,
                    capacity,
                    root_schema,
                    derivation,
                    config_schema,
                    &transition,
                )
            });
    let close_descriptor_id = close_descriptor
        .as_ref()
        .map(|bytes| ContentId::new(hash(bytes).to_bytes()).expect("close descriptor ID"));
    // For a set release the selection names the SET, and the descriptor is one of
    // its entries; for a flat release the two identities are the same record.
    let program_set_bytes = if matches!(campaign, Campaign::Series) {
        let action_ids: Vec<[u8; 32]> = series_action_descriptors
            .iter()
            .map(|bytes| hash(bytes).to_bytes())
            .collect();
        Some(
            build_series_activation_capable_program_set_v1(&action_ids, descriptor_id.to_bytes())
                .expect("activation-capable Series release set"),
        )
    } else {
        campaign
            .program_set()
            .then(|| program_set(descriptor_id, close_descriptor_id, campaign))
    };
    let release_id = match &program_set_bytes {
        Some(bytes) => ContentId::new(hash(bytes).to_bytes()).expect("release ID"),
        None => descriptor_id,
    };
    // The config record is real for General, because the root's tail carries its
    // identity and the General planner decodes it. It names the release
    // it is activated under, which is why it is built after the set identity.
    let config = match family {
        Family::Fixture => vec![0x61; 32],
        // Not fixture bytes: the config record a Series capability selects is
        // its own finalized Template, and every byte of the root tail below is
        // derived from it.
        Family::Series => series_template.to_vec(),
        Family::General => {
            let config = general_config(capacity.to_bytes(), release_id.to_bytes());
            config.to_bytes().to_vec()
        }
    };
    let config_id = ContentId::new(hash(&config).to_bytes()).expect("config ID");
    let close_principal = if matches!(campaign, Campaign::NativeClose) {
        CLOSE_REMAINING_NATIVE_PRINCIPAL
    } else {
        0
    };
    let creation_principal = campaign.creation_principal();
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(root_rent - ROOT_INITIAL_DUST)
            .expect("root rent quote"),
        if creation_principal == 0 {
            CompartmentFundingV1::not_applicable()
        } else {
            CompartmentFundingV1::native_lamports(creation_principal).expect("creation principal")
        },
        if close_principal == 0 {
            CompartmentFundingV1::not_applicable()
        } else {
            CompartmentFundingV1::native_lamports(close_principal).expect("close principal")
        },
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding amounts");
    let entry = CapabilityEntryV1::new(
        kind,
        release_id,
        config_id,
        capacity,
        root_schema,
        derivation,
        if matches!(campaign, Campaign::NativeClose) {
            ActivationPolicy::PrepaidLazy
        } else {
            ActivationPolicy::RequiredAtFounding
        },
        if matches!(campaign, Campaign::NativeClose) {
            u64::MAX
        } else {
            0
        },
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("quote"),
    )
    .expect("entry");
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
    let manifest_id = ContentId::new(hash(&manifest).to_bytes()).expect("manifest ID");
    let selection =
        CapabilityExecutionSelectionV1::new(0, manifest_id, kind, release_id, config_id)
            .expect("selection");

    let (release_set, cache_bytes) = activation_cache();
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(cache_bytes.len()),
        cache_bytes,
    );
    // The Loader's own addresses, already staged by `add_upgradeable_role`.
    let core_programdata = programdata_address(CORE_PROGRAM_ID);
    let trading_programdata = programdata_address(TRADING_PROGRAM_ID);

    let mut state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity([0x21; 32]),
            realm_id: identity([0x22; 32]),
            product_record: identity([0x23; 32]),
            product_id: identity([0x24; 32]),
            resolution_policy: identity([0x25; 32]),
            capability_manifest: identity(manifest_id.to_bytes()),
            selected_release_set: identity(release_set),
            registry_program: identity(REGISTRY_PROGRAM_ID.to_bytes()),
            generation: GENERATION,
        },
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        // Replaced with the exact lifecycle RentCredit after the Market PDA is
        // derived from the immutable identity below.
        rent_beneficiary: identity([0x26; 32]),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    state.identity.market_id = identity(market.to_bytes());
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_state = LifecycleRentCreditV2::new(
        RefundAuthority::new(REFUND_WALLET.to_bytes()).expect("refund wallet"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        GENERATION,
        rent_credit_bump,
    )
    .expect("RentCredit");
    state.rent_beneficiary = identity(rent_credit.to_bytes());
    let state_bytes = state.encode().expect("Core state");
    add_account(
        &mut test,
        market,
        CORE_PROGRAM_ID,
        rent.minimum_balance(state_bytes.len()),
        state_bytes.to_vec(),
    );
    add_account(
        &mut test,
        rent_credit,
        RENT_PROGRAM_ID,
        rent.minimum_balance(rent_credit_state.to_bytes().len()),
        rent_credit_state.to_bytes().to_vec(),
    );

    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        GENERATION,
        selection,
        // The activation under test is the authority that fills these in; this
        // header exists only to derive the vacant root's address, and the root
        // PDA seeds do not include them.
        SelectedRecordBumpsV1::default(),
    )
    .expect("root header");
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    add_account(
        &mut test,
        root,
        system_program::ID,
        ROOT_INITIAL_DUST,
        Vec::new(),
    );
    let decoded_manifest = CapabilityManifestV1::decode(&manifest).expect("manifest");
    let mut funding_state = vec![0_u8; funding_ledger_bytes_v2(1).expect("funding width")];
    FundingLedgerV2::initialize(&mut funding_state, manifest_id, decoded_manifest, 0b1)
        .expect("funding ledger");
    let funding_ledger = FundingLedgerV2::decode(&funding_state).expect("funding ledger");
    let funding_derivation = CapabilityFundingLedgerDerivationV2::new(
        TRADING_PROGRAM_ID.to_bytes(),
        market.to_bytes(),
        GENERATION,
        manifest_id,
        funding_ledger,
    )
    .expect("funding derivation");
    let funding =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &TRADING_PROGRAM_ID).0;
    add_account(
        &mut test,
        funding,
        TRADING_PROGRAM_ID,
        funding_rent + root_rent - ROOT_INITIAL_DUST + close_principal + creation_principal,
        funding_state.clone(),
    );

    let descriptor_record = add_record(
        &mut test,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        descriptor,
    );
    let release_record = match program_set_bytes {
        Some(bytes) => add_record(
            &mut test,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            bytes,
        ),
        None => descriptor_record,
    };
    let config_record = add_record(&mut test, config_schema.to_bytes(), config.clone());
    let profile_record = add_record(&mut test, ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, profile);
    let effect_record = add_record(&mut test, EFFECT_PROGRAM_SCHEMA, effect);
    let close_records = close_descriptor.zip(close_profile).zip(close_effect).map(
        |((descriptor, profile), effect)| {
            (
                add_record(
                    &mut test,
                    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
                    descriptor,
                ),
                add_record(&mut test, ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, profile),
                add_record(&mut test, EFFECT_PROGRAM_SCHEMA, effect),
            )
        },
    );
    let hostile_record = Pubkey::new_from_array([0xa1; 32]);
    add_account(
        &mut test,
        hostile_record,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(32),
        vec![0xa5; 32],
    );
    // The manifest account is the FINALIZED MANIFEST RECORD, so its address is
    // derived, never chosen. It used to be the literal [0xa2; 32], which every
    // hot action would have refused: `hot_v3` locates this record at exactly the
    // coordinate below, and W2q made `process_activation` require it too rather
    // than admitting any account whose bytes happen to hash to the selected
    // manifest identity.
    let manifest_raw = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &hash(&manifest).to_bytes(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        manifest_raw,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(manifest.len()),
        manifest.clone(),
    );

    let mut role_request = selection.to_bytes().to_vec();
    role_request.extend_from_slice(
        &dclutch_market_core_codec::CapabilityFundingHeaderV2::new(1, 1, 0b1)
            .expect("funding header")
            .encode(),
    );
    match campaign {
        // Selector 255 at offset twelve: the coordinate no Series action request
        // can produce, carried by the only request that selects the activation
        // descriptor.
        Campaign::Series => role_request
            .extend_from_slice(&series_activation_request_v1().expect("Series activation request")),
        _ => role_request.push(1),
    }
    let role_digest = hash(&role_request).to_bytes();
    let context = [0x81; 32];
    let authority_seeds = dclutch_release_set_contract::CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        role_digest,
    )
    .expect("caller authority seeds");
    let caller_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &CORE_PROGRAM_ID).0;
    add_account(
        &mut test,
        caller_authority,
        system_program::ID,
        1,
        Vec::new(),
    );
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        identity(CORE_PROGRAM_ID.to_bytes()),
        identity(caller_authority.to_bytes()),
        identity(release_set),
        identity(market.to_bytes()),
        identity(context),
        identity(hash(&state_bytes).to_bytes()),
        identity(role_digest),
        GENERATION,
        0,
        0,
        u32::try_from(role_request.len()).expect("request width"),
    )
    .expect("envelope");
    let mut instruction_data = envelope.encode().expect("envelope bytes").to_vec();
    instruction_data.extend_from_slice(&role_request);
    let mut accounts = vec![
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new(root, false),
        AccountMeta::new(funding, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(market, false),
        AccountMeta::new_readonly(release_record.0, false),
        AccountMeta::new_readonly(release_record.1, false),
        AccountMeta::new_readonly(config_record.0, false),
        AccountMeta::new_readonly(config_record.1, false),
        AccountMeta::new_readonly(profile_record.0, false),
        AccountMeta::new_readonly(profile_record.1, false),
        AccountMeta::new_readonly(effect_record.0, false),
        AccountMeta::new_readonly(effect_record.1, false),
        AccountMeta::new_readonly(activation_cache, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(core_programdata, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(trading_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    if campaign.program_set() {
        // Family accounts 16 and 17: the descriptor the set entry names. A flat
        // release carries neither, and its frame is byte-identical to before.
        accounts.push(AccountMeta::new_readonly(descriptor_record.0, false));
        accounts.push(AccountMeta::new_readonly(descriptor_record.1, false));
    }
    let close_instruction = close_records.as_ref().map(
        |(close_descriptor_record, close_profile_record, close_effect_record)| {
            let mut close_role_request = selection.to_bytes().to_vec();
            close_role_request.extend_from_slice(
                &dclutch_market_core_codec::CapabilityFundingHeaderV2::new(1, 1, 0b1)
                    .expect("funding header")
                    .encode(),
            );
            close_role_request.push(u8::try_from(FAMILY_CLOSE_SELECTOR).expect("selector"));
            let close_role_digest = hash(&close_role_request).to_bytes();
            let close_context = [0x82; 32];
            let close_authority_seeds =
                dclutch_release_set_contract::CallerAuthoritySeedsV1::from_bytes(
                    release_set,
                    market.to_bytes(),
                    ExecutionRoleV1::Core,
                    close_context,
                    close_role_digest,
                )
                .expect("close caller authority seeds");
            let close_authority =
                Pubkey::find_program_address(&close_authority_seeds.as_slices(), &CORE_PROGRAM_ID)
                    .0;
            add_account(
                &mut test,
                close_authority,
                system_program::ID,
                1,
                Vec::new(),
            );
            let close_envelope = CoreEffectEnvelopeV1::new(
                CoreEffectActionV1::CloseCapability,
                Role::Trading,
                identity(CORE_PROGRAM_ID.to_bytes()),
                identity(close_authority.to_bytes()),
                identity(release_set),
                identity(market.to_bytes()),
                identity(close_context),
                identity(hash(&state_bytes).to_bytes()),
                identity(close_role_digest),
                GENERATION,
                0,
                0,
                u32::try_from(close_role_request.len()).expect("request width"),
            )
            .expect("close envelope");
            let mut close_data = close_envelope.encode().expect("envelope bytes").to_vec();
            close_data.extend_from_slice(&close_role_request);
            Instruction {
                program_id: CORE_PROGRAM_ID,
                accounts: vec![
                    AccountMeta::new_readonly(close_authority, false),
                    AccountMeta::new(root, false),
                    AccountMeta::new(funding, false),
                    AccountMeta::new_readonly(manifest_raw, false),
                    AccountMeta::new_readonly(market, false),
                    AccountMeta::new_readonly(release_record.0, false),
                    AccountMeta::new_readonly(release_record.1, false),
                    AccountMeta::new_readonly(config_record.0, false),
                    AccountMeta::new_readonly(config_record.1, false),
                    AccountMeta::new_readonly(close_profile_record.0, false),
                    AccountMeta::new_readonly(close_profile_record.1, false),
                    AccountMeta::new_readonly(close_effect_record.0, false),
                    AccountMeta::new_readonly(close_effect_record.1, false),
                    AccountMeta::new_readonly(activation_cache, false),
                    AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                    AccountMeta::new_readonly(core_programdata, false),
                    AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
                    AccountMeta::new_readonly(trading_programdata, false),
                    AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
                    AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
                    AccountMeta::new_readonly(system_program::ID, false),
                    AccountMeta::new_readonly(close_descriptor_record.0, false),
                    AccountMeta::new_readonly(close_descriptor_record.1, false),
                    AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
                    AccountMeta::new(rent_credit, false),
                ],
                data: close_data,
            }
        },
    );
    (
        test,
        Fixture {
            instruction: Instruction {
                program_id: CORE_PROGRAM_ID,
                accounts,
                data: instruction_data,
            },
            root,
            funding,
            descriptor_raw: release_record.0,
            activation_descriptor_raw: descriptor_record.0,
            hostile_record,
            market,
            root_rent,
            funding_rent,
            family,
            root_tail_bytes: family.root_tail_bytes(),
            config_id,
            manifest,
            manifest_id,
            funding_prestate: funding_state,
            close_instruction,
            rent_credit,
            close_principal,
            creation_principal,
            config,
        },
    )
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(400_000),
            instruction,
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
        .expect("account exists")
}

async fn maybe_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
}

async fn assert_rollback(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
) -> BanksClientError {
    let root_before = account(context, fixture.root).await;
    let funding_before = account(context, fixture.funding).await;
    let error = submit(context, instruction)
        .await
        .expect_err("activation refuses");
    assert_eq!(account(context, fixture.root).await, root_before);
    assert_eq!(account(context, fixture.funding).await, funding_before);
    error
}

/// The custom program code the refusal carried, so a test can name it.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

/// Submit one campaign and require the exact created root and funding poststate.
///
/// Returns the created family tail and complete FundingLedgerV2 poststate.
async fn assert_activation_succeeds(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> (Vec<u8>, Vec<u8>) {
    submit(context, fixture.instruction.clone())
        .await
        .expect("activation succeeds");
    let root = account(context, fixture.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    // The root's opening balance, read back off the chain: its exact Rent
    // reserve PLUS whatever Creation principal the manifest declared.
    assert_eq!(
        root.lamports,
        fixture.root_rent + fixture.creation_principal
    );
    let descriptor_account = account(context, fixture.activation_descriptor_raw).await;
    let descriptor = CapabilityProgramV1::decode(&descriptor_account.data).expect("descriptor");
    let decoded = CapabilityRootAccountV1::decode(&root.data, descriptor).expect("root account");
    assert_eq!(decoded.header().market(), fixture.market.to_bytes());
    assert_eq!(decoded.state().len(), fixture.root_tail_bytes);
    // The family tail is the effect program's projected request buffer, exactly.
    // Before this was so, the seam wrote `vec![0; root_state_bytes]` and no family
    // root -- General's or Direct's -- could be decoded out of what it created.
    let expected_tail = match fixture.family {
        Family::Fixture => {
            let mut tail = vec![0_u8; ROOT_TAIL_BYTES];
            put(&mut tail, 0, &GENERATION.to_le_bytes());
            put(&mut tail, 8, &fixture.market.to_bytes());
            tail
        }
        // General's own oracle, from the type that owns the layout. Whatever the
        // artifacts projected has to equal this or the capability the seam
        // created is one no General action can authenticate.
        // Series' own creation oracle. Whatever the published artifacts
        // projected has to equal this, or the root the seam created is one no
        // Series action can authenticate and terminal Close can never open.
        Family::Series => SeriesStateV3::new(SERIES_CLOSE_RENT)
            .encode(
                TemplateV3::decode(&fixture.config)
                    .expect("Series Template")
                    .occurrence_count(),
            )
            .expect("Series initial state")
            .to_vec(),
        Family::General => general_root_creation_tail_v2(
            fixture.market.to_bytes(),
            fixture.config_id.to_bytes(),
            GENERATION,
        )
        .expect("General creation tail")
        .to_vec(),
    };
    assert_eq!(decoded.state(), expected_tail.as_slice());
    let funding = account(context, fixture.funding).await;
    assert_eq!(
        funding.lamports,
        fixture.funding_rent + fixture.close_principal
    );
    let manifest = CapabilityManifestV1::decode(&fixture.manifest).expect("manifest");
    let authenticated = FundingLedgerV2::decode(&funding.data)
        .expect("funding poststate")
        .authenticate(fixture.manifest_id, manifest)
        .expect("authenticated funding poststate");
    let slot = authenticated.slot(0).expect("selected slot");
    assert_eq!(slot.status(), FundingLedgerStatusV2::Active);
    assert!(slot.activation_slot() > 0);
    assert_eq!(slot.remaining().rent().amount(), 0);
    // Both native compartments are released by `activate_in_place`; the seam is
    // what makes the second one arrive somewhere instead of vanishing.
    assert_eq!(slot.remaining().creation().amount(), 0);
    (decoded.state().to_vec(), funding.data)
}

#[tokio::test]
async fn common_outer_activates_root_and_funding_commit_last() {
    let (test, fixture) = build_fixture(Campaign::Success);
    let mut context = test.start_with_context().await;
    assert_activation_succeeds(&mut context, &fixture).await;
}

#[tokio::test]
async fn native_close_refunds_principal_rent_and_surplus_then_replay_refuses() {
    const ROOT_SURPLUS: u64 = 11;
    const LEDGER_SURPLUS: u64 = 13;

    let (test, fixture) = build_fixture(Campaign::NativeClose);
    let mut context = test.start_with_context().await;
    assert_eq!(
        fixture
            .close_instruction
            .as_ref()
            .expect("close instruction")
            .accounts
            .len(),
        25
    );
    assert_activation_succeeds(&mut context, &fixture).await;
    let payer = context.payer.pubkey();
    submit(&mut context, transfer(&payer, &fixture.root, ROOT_SURPLUS))
        .await
        .expect("root donation");
    submit(
        &mut context,
        transfer(&payer, &fixture.funding, LEDGER_SURPLUS),
    )
    .await
    .expect("ledger donation");

    let root_before = account(&mut context, fixture.root).await;
    let funding_before = account(&mut context, fixture.funding).await;
    let credit_before = account(&mut context, fixture.rent_credit).await;
    assert_eq!(root_before.lamports, fixture.root_rent + ROOT_SURPLUS);
    assert_eq!(
        funding_before.lamports,
        fixture.funding_rent + fixture.close_principal + LEDGER_SURPLUS
    );

    // A writable account with the wrong key/owner/width cannot substitute for
    // the Market's exact RentCredit, and refusal is commit-last.
    let mut substituted = fixture
        .close_instruction
        .clone()
        .expect("close instruction");
    *substituted.accounts.last_mut().expect("RentCredit meta") =
        AccountMeta::new(fixture.hostile_record, false);
    submit(&mut context, substituted)
        .await
        .expect_err("beneficiary substitution refuses");
    assert_eq!(account(&mut context, fixture.root).await, root_before);
    assert_eq!(account(&mut context, fixture.funding).await, funding_before);
    assert_eq!(
        account(&mut context, fixture.rent_credit).await,
        credit_before
    );

    let close = fixture
        .close_instruction
        .clone()
        .expect("close instruction");
    submit(&mut context, close.clone())
        .await
        .expect("native close succeeds");
    for closed in [fixture.root, fixture.funding] {
        if let Some(account) = maybe_account(&mut context, closed).await {
            assert_eq!(account.owner, system_program::ID);
            assert_eq!(account.lamports, 0);
            assert!(account.data.is_empty());
        }
    }
    let expected_credit = credit_before
        .lamports
        .checked_add(fixture.root_rent)
        .and_then(|value| value.checked_add(ROOT_SURPLUS))
        .and_then(|value| value.checked_add(fixture.funding_rent))
        .and_then(|value| value.checked_add(fixture.close_principal))
        .and_then(|value| value.checked_add(LEDGER_SURPLUS))
        .expect("classified refund sum");
    assert_eq!(
        account(&mut context, fixture.rent_credit).await.lamports,
        expected_credit
    );

    submit(&mut context, close)
        .await
        .expect_err("closed root/ledger replay refuses");
    assert_eq!(
        account(&mut context, fixture.rent_credit).await.lamports,
        expected_credit
    );
}

/// General's three activation artifacts create a real `GeneralRootV2`.
///
/// This is the first composite root in the tree whose family tail a family can
/// actually decode. The three artifacts are authored against published
/// coordinates only -- `activation_registers_v2` for the seam's registers,
/// `GENERAL_ROOT_*_OFFSET_V2` plus the two constant words for the tail, and
/// `funding_ledger_remaining_offset_v2` for the Rent quote -- so no
/// General layout is restated in an artifact author.
///
/// The three claims, in strengthening order: the projected tail is
/// `general_root_creation_tail_v2`; `GeneralRootV2::decode` accepts it and it
/// equals `GeneralRootV2::active`; and General's own
/// `FundingLedgerV2::activate_in_place`, run over the same manifest, funding and
/// slot the chain used, agrees with the interpreted artifacts byte for byte on
/// the selected-row FundingLedger poststate. That last one is what
/// makes this an activation of General rather than a coincidence of widths: two
/// independent authorities -- three data artifacts run by a family-neutral seam,
/// and a Rust function that knows what a General root is -- produce the same
/// hundred and twenty-eight bytes.
/// Series activates: its own bundle, its own Template, a decodable root.
///
/// This is the difference between "the seam arithmetic executes" and "Series
/// activates". Everything here is Series' own published truth rather than a
/// fixture's imitation of it:
///
/// - the config record is a real `TemplateV3`, the kernel's own example record
///   with this campaign's close principal written into it;
/// - the descriptor, profile and effect are the triple
///   `build_series_activation_bundle_v1` publishes, bound to a real Series V4
///   action descriptor through `capacity_profile`, which is the completeness
///   gate that refuses a substituted Template;
/// - the release is the six-entry set `build_series_activation_capable_program_set_v1`
///   emits, and the request that reaches the V1 coordinate is the canonical
///   Series activation request carrying selector 255 at offset twelve — the one
///   coordinate `SeriesActionV3::decode` can never produce;
/// - the prepaid close principal rides `FundingCompartment::Creation` and is
///   whatever the Template says, not a fixture constant.
///
/// The root that comes back is decoded with `SeriesStateV3::decode` under the
/// Template's own occurrence count and compared to `SeriesStateV3::new`. Before
/// this lane, the Series ProgramSet carried five V4 action descriptors and no
/// activation coordinate at all, so `authenticate_set_descriptor` refused every
/// entry it could select and no Series Market could ever create its root.
#[tokio::test]
async fn series_activates_its_own_root_from_its_own_template_on_chain() {
    let (test, fixture) = build_fixture(Campaign::Series);
    let mut context = test.start_with_context().await;
    let template = TemplateV3::decode(&fixture.config).expect("Series Template");
    assert_eq!(template.close_rent(), SERIES_CLOSE_RENT);
    assert_eq!(fixture.creation_principal, SERIES_CLOSE_RENT);

    // The founding parks exactly the two compartments the Series funding plan
    // names, and the plan is derived from the kernel rather than restated.
    let plan = series_activation_funding_plan_v1(template, fixture.root_rent)
        .expect("Series funding plan");
    assert_eq!(plan.creation_compartment(), SERIES_CLOSE_RENT);
    assert_eq!(
        plan.parked_quote().expect("parked"),
        fixture.root_rent + SERIES_CLOSE_RENT
    );

    let (tail, _) = assert_activation_succeeds(&mut context, &fixture).await;

    // The root the chain created is one Series' own decoder accepts, in its
    // initial state, holding the principal terminal Close will classify.
    let state = SeriesStateV3::decode(&tail, template.occurrence_count())
        .expect("the activated root decodes as SeriesStateV3");
    assert_eq!(state, SeriesStateV3::new(SERIES_CLOSE_RENT));
    assert_eq!(state.close_rent_remaining(), SERIES_CLOSE_RENT);
    assert_eq!(state.revision(), 0);
    assert_eq!(state.outstanding_ticket_accounts(), 0);

    // And the balance the terminal contract will require, read off the bank.
    let root = account(&mut context, fixture.root).await;
    assert_eq!(root.lamports, fixture.root_rent + SERIES_CLOSE_RENT);
}

/// A manifest-declared Creation principal reaches the root, on chain.
///
/// This is the executing witness for the seam rule. `outer.rs` requires the
/// activated root to hold `debit.rent_lamports() + debit.creation_lamports()`,
/// both halves read off the manifest entry this Market's own selection names,
/// with the rent half independently pinned to the live Rent sysvar. Nothing
/// here reads a family config record, and nothing asserts the rule from the
/// fixture's own arithmetic: the balance is fetched back off the bank.
///
/// Before the seam captured the debit, this campaign could not have existed.
/// `activate_in_place` releases Rent AND Creation in one statement while the
/// activation moved only the rent, so the ledger poststate check refused; and
/// `release_in_place` refuses both compartments forever after, so the principal
/// had no later route either. It was a declared, authenticated, wire-visible
/// compartment with no transport and no reader.
#[tokio::test]
async fn a_declared_creation_principal_is_delivered_into_the_root_on_chain() {
    let (test, fixture) = build_fixture(Campaign::CreationPrincipal);
    let mut context = test.start_with_context().await;
    assert_eq!(fixture.creation_principal, CREATION_PRINCIPAL);

    // The ledger really is carrying the principal before activation runs.
    let funding_before = account(&mut context, fixture.funding).await;
    assert_eq!(
        funding_before.lamports,
        fixture.funding_rent + fixture.root_rent - ROOT_INITIAL_DUST + CREATION_PRINCIPAL
    );

    // Root balance, tail, and ledger poststate are all checked against the bank
    // inside this helper, including root == rent + creation.
    let (tail, _) = assert_activation_succeeds(&mut context, &fixture).await;

    // The delivered principal funds the root; it composes no byte of the tail.
    let mut expected = vec![0_u8; ROOT_TAIL_BYTES];
    put(&mut expected, 0, &GENERATION.to_le_bytes());
    put(&mut expected, 8, &fixture.market.to_bytes());
    assert_eq!(tail, expected);

    // And the lamports are conserved across the two accounts the seam touched.
    let root_after = account(&mut context, fixture.root).await;
    let funding_after = account(&mut context, fixture.funding).await;
    assert_eq!(
        root_after.lamports + funding_after.lamports,
        funding_before.lamports + ROOT_INITIAL_DUST
    );
    assert_eq!(root_after.lamports, fixture.root_rent + CREATION_PRINCIPAL);
}

/// A principal the manifest declares and the artifacts never move refuses.
///
/// The fail-closed half. These are the ordinary rent-only artifacts against a
/// manifest that declares a Creation compartment, which is exactly the shape
/// every release in the tree had before this seam was finished. It must not
/// create a root, because the compartment it stranded can never be released.
#[tokio::test]
async fn a_declared_principal_the_artifacts_never_deliver_refuses_atomically() {
    let (test, fixture) = build_fixture(Campaign::CreationPrincipalNotDelivered);
    let mut context = test.start_with_context().await;
    let error = submit(&mut context, fixture.instruction.clone())
        .await
        .expect_err("an undelivered creation principal must refuse");
    // The exact conjunct that owns it, taken from the chain rather than chosen.
    //
    // It is the LEDGER conservation check, not the root-balance rule. Both would
    // refuse, and the ledger one comes first: `activate_in_place` has already
    // zeroed the Creation compartment's remaining, so the outer requires the row
    // to hold `ledger_rent + remaining_native_lamports_total()` while the
    // undelivered principal is still sitting in it. The root rule never gets to
    // speak. That ordering is worth pinning -- it is why an undelivered
    // principal cannot strand silently even for a family that gets its root
    // arithmetic right.
    assert_eq!(refusal_code(&error), Some(TRADING_CONTENT_REFUSAL_CODE));
    // The root was never created: it is still the vacant, System-owned dust
    // account the fixture staged, not a Trading-owned capability root.
    let root_after = maybe_account(&mut context, fixture.root)
        .await
        .expect("the staged vacant root survives");
    assert_ne!(root_after.owner, TRADING_PROGRAM_ID);
    assert_eq!(root_after.lamports, ROOT_INITIAL_DUST);
    assert!(root_after.data.is_empty());
    let funding_after = account(&mut context, fixture.funding).await;
    assert_eq!(
        funding_after.lamports,
        fixture.funding_rent + fixture.root_rent - ROOT_INITIAL_DUST + CREATION_PRINCIPAL
    );
    assert_eq!(funding_after.data, fixture.funding_prestate);
}

#[tokio::test]
async fn general_activation_artifacts_create_a_real_general_root() {
    let (test, fixture) = build_fixture(Campaign::General);
    let mut context = test.start_with_context().await;
    let (tail, funding_after) = assert_activation_succeeds(&mut context, &fixture).await;

    let market = fixture.market.to_bytes();
    let config_id = fixture.config_id.to_bytes();
    let root = GeneralRootV2::decode(&tail).expect("General root decodes");
    assert_eq!(
        root,
        GeneralRootV2::active(market, config_id, GENERATION).expect("active root")
    );

    let manifest = CapabilityManifestV1::decode(&fixture.manifest).expect("manifest");
    let authenticated = FundingLedgerV2::decode(&funding_after)
        .expect("funding poststate")
        .authenticate(fixture.manifest_id, manifest)
        .expect("authenticated funding poststate");
    let activation_slot = authenticated
        .slot(0)
        .expect("selected slot")
        .activation_slot();
    let mut expected_funding = fixture.funding_prestate.clone();
    FundingLedgerV2::activate_in_place(
        &mut expected_funding,
        fixture.manifest_id,
        manifest,
        0,
        activation_slot,
    )
    .expect("selected funding activation");
    assert_eq!(expected_funding, funding_after);
}

/// A `CapabilityProgramSetV2` at `capability_release` activates the same root.
///
/// This is the generation `hot_v3` authenticates. Before it, the seam decoded
/// the record at `selection.capability_release()` as a `CapabilityProgramV1`
/// and nothing else, so a capability whose release is a selector table -- which
/// is every V3/V4 family -- had no route that could create its root at all. The
/// selection is a seed of the root PDA, so one selection could not satisfy both
/// generations and the newer one simply had no door.
///
/// Nothing here is a kind branch: the release generation is read off the raw
/// record's own PDA, and the descriptor the set names must still satisfy the
/// same manifest-entry join the flat generation does.
#[tokio::test]
async fn a_program_set_release_activates_through_its_selected_descriptor() {
    let (test, fixture) = build_fixture(Campaign::ProgramSetRelease);
    let mut context = test.start_with_context().await;
    assert_ne!(fixture.descriptor_raw, fixture.activation_descriptor_raw);
    assert_eq!(fixture.instruction.accounts.len(), 23);
    assert_activation_succeeds(&mut context, &fixture).await;
}

/// Where the family-neutral boundary actually is, executed.
///
/// The seam owns no family decoder and must not acquire one, so its ONLY check
/// on the projected tail is that it is not entirely zero. An artifact that
/// projects the Market, the config identity, the generation and the revision but
/// never the two constant words therefore ACTIVATES: the tail is nonzero, every
/// lamport conserves, and the seam has no way to know that what it just wrote is
/// not a `GeneralRootV2`.
///
/// That is not a hole in the seam, and this test is what makes the reason
/// checkable rather than argued. The tail is a projection of one finalized
/// content-addressed artifact that the manifest entry and the descriptor both
/// bind; a family that publishes an effect program with the wrong writes has
/// published a wrong RELEASE, which is caught where releases are admitted, not
/// at runtime by a seam that would need a decoder per family to catch it.
///
/// What the seam DOES guarantee is stated by its sibling
/// `a_tail_that_is_unwritten_or_the_wrong_width_refuses`: nothing at all, and
/// the wrong width, both refuse. Between them the boundary is exact.
#[tokio::test]
async fn a_general_tail_missing_its_constant_words_activates_and_is_not_a_general_root() {
    let (test, fixture) = build_fixture(Campaign::GeneralUnwrittenMagic);
    let mut context = test.start_with_context().await;
    submit(&mut context, fixture.instruction.clone())
        .await
        .expect("the seam cannot see a partially projected family tail");

    let root = account(&mut context, fixture.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    let descriptor_account = account(&mut context, fixture.activation_descriptor_raw).await;
    let descriptor = CapabilityProgramV1::decode(&descriptor_account.data).expect("descriptor");
    let decoded = CapabilityRootAccountV1::decode(&root.data, descriptor).expect("root account");
    let tail = decoded.state();
    assert_eq!(tail.len(), GENERAL_ROOT_BYTES_V2);
    assert!(tail.iter().any(|byte| *byte != 0), "the seam's one check");

    let canonical = general_root_creation_tail_v2(
        fixture.market.to_bytes(),
        fixture.config_id.to_bytes(),
        GENERATION,
    )
    .expect("General creation tail");
    assert_ne!(tail, canonical.as_slice());
    assert!(
        GeneralRootV2::decode(tail).is_err(),
        "a capability no General action can authenticate was still created"
    );
}

/// Reversion control for the set path, at both of its own joins.
///
/// `ProgramSetWrongSchema` is the case that matters most: a set entry naming a
/// descriptor schema this seam cannot run is refused at the entry, before any
/// account is read, so a hot-action `CapabilityProgramV4` can never arrive here
/// as an activation descriptor. `ProgramSetMissingSelector` is the request-side
/// half -- a family request selecting no entry refuses instead of defaulting.
#[tokio::test]
async fn a_set_entry_this_seam_cannot_run_or_cannot_select_refuses() {
    for (campaign, expected) in [
        (
            Campaign::ProgramSetWrongSchema,
            TRADING_UNSUPPORTED_REFUSAL_CODE,
        ),
        (
            Campaign::ProgramSetMissingSelector,
            TRADING_CONTENT_REFUSAL_CODE,
        ),
    ] {
        let (test, fixture) = build_fixture(campaign);
        let mut context = test.start_with_context().await;
        let error = assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
        assert_eq!(refusal_code(&error).expect("custom refusal code"), expected);
    }
}

#[tokio::test]
async fn substituted_registry_record_and_root_refuse_atomically() {
    for substitution in 0..3 {
        let (test, fixture) = build_fixture(Campaign::Success);
        let mut context = test.start_with_context().await;
        let mut instruction = fixture.instruction.clone();
        match substitution {
            0 => {
                instruction
                    .accounts
                    .get_mut(18)
                    .expect("Registry meta")
                    .pubkey = WRONG_REGISTRY_ID
            }
            1 => {
                instruction
                    .accounts
                    .get_mut(5)
                    .expect("descriptor record meta")
                    .pubkey = fixture.hostile_record
            }
            _ => {
                instruction.accounts.get_mut(1).expect("root meta").pubkey = fixture.hostile_record
            }
        }
        assert_rollback(&mut context, &fixture, instruction).await;
    }
}

#[tokio::test]
async fn late_effect_refusal_rolls_back_the_projected_transfer() {
    let (test, fixture) = build_fixture(Campaign::LateEffectRefusal);
    let mut context = test.start_with_context().await;
    assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
}

/// Reversion control for the tail channel, both directions.
///
/// `UnwrittenTail` is the exact prior behaviour of this seam -- a declared tail
/// width with nothing projected into it -- and it now refuses instead of
/// creating a root no family can decode. `MismatchedTailWidth` projects the
/// whole tail into a request buffer eight bytes wider than the descriptor's
/// `root_state_bytes`, which the outer refuses rather than truncating.
#[tokio::test]
async fn a_tail_that_is_unwritten_or_the_wrong_width_refuses() {
    for campaign in [Campaign::UnwrittenTail, Campaign::MismatchedTailWidth] {
        let (test, fixture) = build_fixture(campaign);
        let mut context = test.start_with_context().await;
        let error = assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
        assert_eq!(
            refusal_code(&error).expect("custom refusal code"),
            TRADING_ROOT_REFUSAL_CODE
        );
        let root = account(&mut context, fixture.root).await;
        assert_eq!(root.owner, system_program::ID);
        assert!(root.data.is_empty());
    }
}

/// The three artifacts are byte-identical to their canonical V2 reference.
///
/// Before `73f7ec7`/`f98d439`/`d18c32d` gave the three generations public
/// encoders, this file wrote all three wire formats itself. The funding profile
/// intentionally moved at the V2 clean break: its account width is one 120-byte
/// ledger and its Rent amount is at offset 64. These are also the record digests
/// the descriptor names and the PDA seeds every raw record sits at, so the
/// migrated bytes remain pinned here.
#[test]
fn the_public_encoders_reproduce_the_canonical_v2_artifact_bytes() {
    const PROFILE: [u8; 128] = [
        0x44, 0x43, 0x4c, 0x54, 0x41, 0x50, 0x30, 0x31, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x04,
        0x00, 0x08, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02, 0x05, 0x01, 0x00, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x01, 0x00, 0x06, 0x00, 0x00, 0x00, 0x40,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const TRANSITION: [u8; 40] = [
        0x44, 0x43, 0x54, 0x56, 0x02, 0x00, 0x01, 0x00, 0x08, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const EFFECT: [u8; 64] = [
        0x44, 0x43, 0x45, 0x32, 0x02, 0x00, 0x03, 0x00, 0x02, 0x00, 0x08, 0x00, 0x0c, 0x00, 0x28,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x08, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        account_profile(Family::Fixture).as_slice(),
        PROFILE.as_slice()
    );
    assert_eq!(
        transition_program(Family::Fixture).as_slice(),
        TRANSITION.as_slice()
    );
    assert_eq!(
        effect_program(Campaign::Success).as_slice(),
        EFFECT.as_slice()
    );
}

/// The shippable General activation artifacts ARE the ones this file proves.
///
/// `general_activation_artifacts_create_a_real_general_root` runs the real
/// Trading ELF over the fixture's hand-built General triple and decodes the
/// account it creates as a real `GeneralRootV2`. That is the strongest evidence
/// in the tree that a data-defined activation works -- and until now it was
/// evidence about fixture bytes, because the only General activation artifacts
/// that existed were the ones a few hundred lines up this file.
///
/// `dclutch-general-adapter-contract::activation_bundle_v1` now builds them for
/// a release to publish. This test is the join: the profile, the transition and
/// the effect are BYTE-IDENTICAL, so the ELF result above is a result about the
/// shippable records, not about their look-alikes. The descriptor differs at
/// exactly one 32-byte field -- the request schema, which the fixture invented
/// as `id(0x23)` before General published one -- and this asserts that it
/// differs THERE AND NOWHERE ELSE, because "nearly the same descriptor" is not a
/// claim anyone should have to take on trust.
///
/// The activation seam never reads a descriptor's request schema
/// (`CapabilityProgramV1::validate_selection` joins kind, capacity, root schema
/// and derivation policy against the manifest entry, and no more), so the
/// difference cannot change what the ELF did.
#[test]
fn the_shippable_general_bundle_is_the_triple_this_file_runs_on_the_real_elf() {
    use dclutch_capability_program_contract::v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    };
    use dclutch_general_adapter_contract::activation_bundle_v1::{
        GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1, GeneralActivationBundleInputV1,
        build_general_activation_bundle_v1,
    };

    // The same manifest-selected coordinates `build_fixture` gives its General
    // campaign, carried on a General action descriptor the way a release does.
    let action = CapabilityProgramV4::new(
        ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("General kind"),
        id(0x15),
        id(0x23),
        ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2).expect("General root schema"),
        id(0x14),
        id(0x12),
        CapabilityArtifactsV4 {
            account_profile: ArtifactReferenceV4::new(id(0x61), id(0x71)),
            request_profile: ArtifactReferenceV4::new(id(0x62), id(0x72)),
            lifecycle: ArtifactReferenceV4::new(
                ContentId::new(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5).expect("lifecycle schema"),
                id(0x73),
            ),
            strategy: ArtifactReferenceV4::new(id(0x64), id(0x74)),
            transition: ArtifactReferenceV4::new(id(0x65), id(0x75)),
            effect: ArtifactReferenceV4::new(id(0x66), id(0x76)),
        },
        u32::try_from(GENERAL_ROOT_BYTES_V2).expect("General root width"),
    )
    .expect("General action descriptor")
    .encode()
    .to_vec();
    let bundle = build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
        action_descriptor: &action,
        funding_ledger_slot_count: 1,
    })
    .expect("shippable General activation bundle");

    assert_eq!(bundle.account_profile, account_profile(Family::General));
    assert_eq!(bundle.transition, transition_program(Family::General));
    assert_eq!(bundle.effect, effect_program(Campaign::General));

    let fixture_descriptor = descriptor(
        Family::General,
        hash(&bundle.account_profile).to_bytes(),
        hash(&bundle.effect).to_bytes(),
        ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("General kind"),
        id(0x12),
        ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2).expect("General root schema"),
        id(0x14),
        id(0x15),
    );
    assert_eq!(bundle.descriptor.len(), fixture_descriptor.len());
    let request_schema =
        CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET..CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET + 32;
    for (offset, (mine, fixture)) in bundle
        .descriptor
        .iter()
        .zip(fixture_descriptor.iter())
        .enumerate()
    {
        if request_schema.contains(&offset) {
            continue;
        }
        assert_eq!(mine, fixture, "descriptor byte {offset}");
    }
    assert_eq!(
        bundle
            .descriptor
            .get(request_schema.clone())
            .expect("request schema field"),
        GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1.as_slice()
    );
    assert_eq!(
        fixture_descriptor
            .get(request_schema)
            .expect("fixture request schema field"),
        id(0x23).to_bytes().as_slice()
    );
}
