//! Data-defined Trading-owned PDA derivation and state lifecycle planning.
//!
//! The authenticated policy describes bounded Solana seed slices, but this
//! pure kernel neither hashes nor derives an address.  A separately named
//! adapter materializes the seeds, derives the PDA under the authenticated
//! Trading program, and returns that derived key to [`plan_lifecycle`].

use core::convert::{TryFrom, TryInto};

use super::{
    AccountObservationV1, EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA,
    v2::{
        AccountPrestateV2, AccountProfileV2, AccountRuleV2, ProjectionRegisterKindV2,
        ProjectionRegisterSpaceV2, ProjectionTargetV2,
    },
};

/// Safe, allocation-free typed StateLifecyclePolicy V3 artifact encoder.
pub mod encode;

/// Canonical V3 lifecycle-policy magic.
pub const MAGIC: [u8; 8] = *b"DCLTDP03";
/// Finalized-record schema label for lifecycle policies.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/state-lifecycle-policy-v3";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0xad, 0xfe, 0x22, 0x40, 0x22, 0xdf, 0xb6, 0xff, 0xb2, 0x14, 0xd7, 0xd4, 0x24, 0x83, 0xf9, 0x64,
    0xc9, 0xe0, 0x8b, 0x7f, 0xb1, 0xa2, 0x80, 0x1e, 0x2e, 0x8c, 0x73, 0x8a, 0x34, 0xad, 0x03, 0x0a,
];
/// Successor schema label with protected outputs and immutable identity bindings.
pub const SUCCESSOR_SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/state-lifecycle-policy-v4";
/// SHA-256 of [`SUCCESSOR_SCHEMA_RELEASE_PREIMAGE`].
pub const SUCCESSOR_SCHEMA_RELEASE_ID: [u8; 32] = [
    0x3a, 0x15, 0x1e, 0xd5, 0x08, 0x0b, 0x68, 0xd7, 0xc9, 0xe3, 0xbd, 0x2c, 0x5d, 0xf5, 0x17, 0x20,
    0xe4, 0x31, 0x48, 0x4d, 0x92, 0xee, 0x64, 0xdc, 0xfb, 0x87, 0x2a, 0x1e, 0x6f, 0x09, 0x38, 0x6a,
];
/// V5 schema label with adapter-authenticated current-Rent quote projection.
pub const CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5: &[u8] =
    b"dclutch/schema/state-lifecycle-policy-v5-current-rent-quotes-v1";
/// SHA-256 of [`CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5`].
pub const CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5: [u8; 32] = [
    0x10, 0xfb, 0xed, 0x6c, 0x13, 0x26, 0x12, 0x7c, 0xf7, 0xe5, 0x47, 0x83, 0xb1, 0xa5, 0x97, 0xd7,
    0x7c, 0xa3, 0xe7, 0x6b, 0x53, 0xde, 0x97, 0xc0, 0x8f, 0x27, 0x3f, 0x5e, 0x67, 0xe3, 0x98, 0x3b,
];
/// Canonical schema version.
pub const VERSION: u16 = 3;
/// Canonical physical artifact profile.
pub const ARTIFACT_PROFILE: u16 = 1;
/// Successor artifact profile with lifecycle-owned protected outputs.
pub const PROTECTED_OUTPUT_ARTIFACT_PROFILE: u16 = 2;
/// Successor artifact profile with immutable identity-field bindings.
pub const SUCCESSOR_ARTIFACT_PROFILE: u16 = 3;
/// V5 artifact profile with bounded protected current-Rent quote declarations.
pub const CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5: u16 = 4;
/// Exact header width.
pub const HEADER_BYTES: usize = 40;
/// Exact derivation-recipe width.
pub const RECIPE_BYTES: usize = 16;
/// Exact seed-operation width.
pub const SEED_BYTES: usize = 40;
/// Exact action-plan width.
pub const ACTION_PLAN_BYTES: usize = 40;
/// Exact protected-output record width for artifact profile 2.
pub const PROTECTED_OUTPUT_BYTES: usize = 16;
/// Exact immutable identity-binding width for the successor profile.
pub const IMMUTABLE_IDENTITY_BINDING_BYTES: usize = 16;
/// Exact width of one V5 `(exact_data_len, scalar_destination)` declaration.
pub const CURRENT_RENT_QUOTE_BYTES_V5: usize = 16;
/// Maximum quote declarations in this executable lifecycle capacity profile.
pub const MAX_CURRENT_RENT_QUOTES_V5: u16 = 16;
/// Solana's chain-derived maximum number of seeds per PDA.
pub const MAX_SEEDS: u8 = 16;
/// Solana's chain-derived maximum width of one seed.
pub const MAX_SEED_BYTES: u8 = 32;

const SCOPE_FIXED: u8 = 0;
const SCOPE_ITEM: u8 = 1;

const SEED_LITERAL: u8 = 0;
const SEED_COMMON_IDENTITY: u8 = 1;
const SEED_ITEM_IDENTITY: u8 = 2;
const SEED_COMMON_SCALAR_LE: u8 = 3;
const SEED_ITEM_SCALAR_LE: u8 = 4;
const SEED_ITEM_INDEX_LE: u8 = 5;
const SEED_CANONICAL_BUMP: u8 = 6;

const PLAN_AUTHENTICATE: u8 = 0;
const PLAN_CREATE: u8 = 1;
const PLAN_CLOSE: u8 = 2;
const PLAN_AUTHENTICATE_OR_CREATE: u8 = 3;

const SOURCE_COMMON: u8 = 0;
const SOURCE_ITEM: u8 = 1;

const GUARD_ALWAYS: u8 = 0;
const GUARD_SCALAR_EQ: u8 = 1;

const PROTECTED_OUTPUT_NONE: u8 = 0;
const PROTECTED_OUTPUT_AUTHENTICATE_OR_CREATE: u8 = 1;

/// Stable hostile-decode, substitution, or arithmetic refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes or runtime banks did not have their exact checked width.
    InvalidLength,
    /// Magic did not identify a V3 lifecycle policy.
    InvalidMagic,
    /// Schema or artifact profile was unsupported.
    UnsupportedProfile,
    /// Selected/authenticated content identities were zero or unequal.
    ContentIdentity,
    /// Reserved or inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A policy contained no recipe, seed, or plan.
    EmptyPolicy,
    /// A scope, seed source, register source, or operation was unknown.
    UnknownTag,
    /// A recipe, seed range, coordinate, or action order was noncanonical.
    InvalidCoordinate,
    /// A literal or scalar seed exceeded Solana's exact bounded encoding.
    InvalidSeed,
    /// AccountProfile geometry did not cover every referenced coordinate.
    ProfileMismatch,
    /// Runtime item index or flat account/register bank differed.
    RuntimeWidth,
    /// A caller tried to execute a plan whose exact post-transition guard was false.
    PlanDisabled,
    /// The adapter-returned PDA or an authenticated account identity differed.
    IdentityMismatch,
    /// Owner, data width, vacancy, or privilege did not realize the plan.
    InvalidState,
    /// Payer or permanent RentCredit account was absent, aliased, or invalid.
    InvalidFunding,
    /// Historical rent principal or immutable beneficiary was invalid.
    InvalidRent,
    /// A current-Rent quote declaration or authenticated adapter input was invalid.
    InvalidRentQuote,
    /// Checked lamport or affine-width arithmetic refused.
    Arithmetic,
}

/// Result alias for lifecycle-policy operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Fixed-prefix or runtime-item AccountProfile coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateScopeV3 {
    /// One coordinate in the fixed prefix.
    Fixed,
    /// One coordinate within the current runtime item.
    Item,
}

/// Scalar or identity lifecycle register kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRegisterKindV3 {
    /// Scalar register bank.
    Scalar,
    /// Identity register bank.
    Identity,
}

/// One typed lifecycle observation or protected-output register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRegisterTargetV3 {
    kind: LifecycleRegisterKindV3,
    scope: CoordinateScopeV3,
    index: u16,
}

impl LifecycleRegisterTargetV3 {
    /// Scalar or identity register bank.
    pub const fn kind(self) -> LifecycleRegisterKindV3 {
        self.kind
    }

    /// Common prefix or current runtime-item register space.
    pub const fn scope(self) -> CoordinateScopeV3 {
        self.scope
    }

    /// Local common or item-relative register coordinate.
    pub const fn index(self) -> u16 {
        self.index
    }
}

/// Lifecycle-owned outputs for one AuthenticateOrCreate plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleProtectedOutputsV3 {
    scope: CoordinateScopeV3,
    created: u16,
    bump_observation: u16,
    bump: u16,
    historical_rent_principal: u16,
    beneficiary: u16,
    state: u16,
    owner: u16,
}

/// One lifecycle-owned immutable state-field binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleImmutableIdentityBindingV4 {
    data_offset: u32,
    canonical: LifecycleRegisterTargetV3,
}

/// One protected current-Rent quote declaration from a finalized V5 policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCurrentRentQuoteV5 {
    exact_data_len: u32,
    scalar_destination: u16,
}

impl LifecycleCurrentRentQuoteV5 {
    /// Exact state width supplied to the current Rent calculation.
    pub const fn exact_data_len(self) -> u32 {
        self.exact_data_len
    }

    /// Lifecycle-protected common scalar receiving the authenticated minimum.
    pub const fn scalar_destination(self) -> LifecycleRegisterTargetV3 {
        LifecycleRegisterTargetV3 {
            kind: LifecycleRegisterKindV3::Scalar,
            scope: CoordinateScopeV3::Fixed,
            index: self.scalar_destination,
        }
    }
}

impl LifecycleImmutableIdentityBindingV4 {
    /// Exact 32-byte identity offset in the live state body.
    pub const fn data_offset(self) -> u32 {
        self.data_offset
    }

    /// Canonical common or item identity register.
    pub const fn canonical(self) -> LifecycleRegisterTargetV3 {
        self.canonical
    }
}

impl LifecycleProtectedOutputsV3 {
    /// Lifecycle-derived branch result: zero for Authenticate, one for Create.
    pub const fn created(self) -> LifecycleRegisterTargetV3 {
        self.scalar(self.created)
    }

    /// AccountProfile-owned persisted bump observation.
    pub const fn bump_observation(self) -> LifecycleRegisterTargetV3 {
        self.scalar(self.bump_observation)
    }

    /// Lifecycle-derived canonical PDA bump.
    pub const fn bump(self) -> LifecycleRegisterTargetV3 {
        self.scalar(self.bump)
    }

    /// Lifecycle-derived historical rent principal.
    pub const fn historical_rent_principal(self) -> LifecycleRegisterTargetV3 {
        self.scalar(self.historical_rent_principal)
    }

    /// Lifecycle-derived immutable RentCredit beneficiary.
    pub const fn beneficiary(self) -> LifecycleRegisterTargetV3 {
        self.identity(self.beneficiary)
    }

    /// Lifecycle-derived exact state PDA.
    pub const fn state(self) -> LifecycleRegisterTargetV3 {
        self.identity(self.state)
    }

    /// Lifecycle-derived current Trading owner.
    pub const fn owner(self) -> LifecycleRegisterTargetV3 {
        self.identity(self.owner)
    }

    const fn scalar(self, index: u16) -> LifecycleRegisterTargetV3 {
        LifecycleRegisterTargetV3 {
            kind: LifecycleRegisterKindV3::Scalar,
            scope: self.scope,
            index,
        }
    }

    const fn identity(self, index: u16) -> LifecycleRegisterTargetV3 {
        LifecycleRegisterTargetV3 {
            kind: LifecycleRegisterKindV3::Identity,
            scope: self.scope,
            index,
        }
    }
}

impl CoordinateScopeV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            SCOPE_FIXED => Ok(Self::Fixed),
            SCOPE_ITEM => Ok(Self::Item),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// One exact account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCoordinateV3 {
    scope: CoordinateScopeV3,
    index: u16,
}

impl AccountCoordinateV3 {
    /// Coordinate scope.
    pub const fn scope(self) -> CoordinateScopeV3 {
        self.scope
    }

    /// Local fixed-prefix or item index.
    pub const fn index(self) -> u16 {
        self.index
    }
}

/// Lifecycle operation selected by a family action without naming the family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleOperationV3 {
    /// Authenticate an existing exact Trading-owned state PDA.
    Authenticate,
    /// Allocate and assign a System-owned empty dust-tolerant PDA.
    Create,
    /// Zero and close an exact Trading-owned state PDA to permanent RentCredit.
    Close,
    /// Authenticate an exact live PDA or create its exact vacant PDA prestate.
    AuthenticateOrCreate,
}

impl LifecycleOperationV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            PLAN_AUTHENTICATE => Ok(Self::Authenticate),
            PLAN_CREATE => Ok(Self::Create),
            PLAN_CLOSE => Ok(Self::Close),
            PLAN_AUTHENTICATE_OR_CREATE => Ok(Self::AuthenticateOrCreate),
            _ => Err(Error::UnknownTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecipeV3 {
    account: AccountCoordinateV3,
    seed_start: u16,
    seed_count: u8,
    bump_offset: u8,
    data_base: u32,
    data_stride: u32,
}

/// One hostile-decoded action-selected lifecycle plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActionPlanV3 {
    action: u32,
    operation: LifecycleOperationV3,
    recipe: u16,
    payer: Option<AccountCoordinateV3>,
    rent_credit: Option<AccountCoordinateV3>,
    principal: Option<RegisterSourceV3>,
    beneficiary: Option<RegisterSourceV3>,
    guard: PlanGuardV3,
}

impl ActionPlanV3 {
    /// Family-defined action selector.
    pub const fn action(self) -> u32 {
        self.action
    }

    /// Generic state operation.
    pub const fn operation(self) -> LifecycleOperationV3 {
        self.operation
    }
}

/// One plan selected together with the exact policy that owns its recipes.
///
/// Private fields prevent combining an action record from one authenticated
/// policy with another policy's seed table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedLifecycleV3<'a> {
    policy: StateLifecyclePolicyV3<'a>,
    plan: ActionPlanV3,
    plan_index: u16,
    join: Option<ValidatedProfileJoinV3<'a>>,
}

impl<'a> SelectedLifecycleV3<'a> {
    /// Carry evidence that this policy's join to one AccountProfile is proved.
    ///
    /// Six methods on this type and its policy each re-derived that join on
    /// every call, and a lifecycle batch calls them once per seed, per
    /// invocation, per plan, twice over. Attaching the evidence once makes the
    /// join a fact of the selection rather than a per-call derivation; a method
    /// still derives it for any AccountProfile the evidence does not name.
    #[must_use]
    pub const fn with_validated_join(self, join: ValidatedProfileJoinV3<'a>) -> Self {
        Self {
            join: Some(join),
            ..self
        }
    }

    fn require_join(self, profile: AccountProfileV2<'_>) -> Result<()> {
        self.policy.require_join(self.join, profile)
    }
}

impl SelectedLifecycleV3<'_> {
    /// Family-defined action selector.
    pub const fn action(self) -> u32 {
        self.plan.action()
    }

    /// Generic state operation.
    pub const fn operation(self) -> LifecycleOperationV3 {
        self.plan.operation()
    }

    /// Lifecycle-owned protected output declaration, when selected.
    pub fn protected_outputs(self) -> Result<Option<LifecycleProtectedOutputsV3>> {
        self.policy.protected_outputs(self.plan_index)
    }

    /// Number of AccountProfile-owned protected observations for this plan.
    pub fn protected_observation_count(self) -> Result<u8> {
        Ok(if self.protected_outputs()?.is_some() {
            3
        } else {
            0
        })
    }

    /// Select one AccountProfile-owned observation target.
    ///
    /// Canonical order is bump, historical rent principal, beneficiary.
    pub fn protected_observation_target(self, ordinal: u8) -> Result<LifecycleRegisterTargetV3> {
        let outputs = self.protected_outputs()?.ok_or(Error::InvalidCoordinate)?;
        match ordinal {
            0 => Ok(outputs.bump_observation()),
            1 => Ok(lifecycle_target(
                LifecycleRegisterKindV3::Scalar,
                self.plan.principal.ok_or(Error::InvalidRent)?,
            )),
            2 => Ok(lifecycle_target(
                LifecycleRegisterKindV3::Identity,
                self.plan.beneficiary.ok_or(Error::InvalidRent)?,
            )),
            _ => Err(Error::InvalidCoordinate),
        }
    }

    /// Number of lifecycle-owned protected destinations for this plan.
    pub fn protected_output_count(self) -> Result<u8> {
        Ok(if self.protected_outputs()?.is_some() {
            6
        } else {
            0
        })
    }

    /// Select one lifecycle-owned protected destination.
    ///
    /// Canonical order is created, bump, principal, beneficiary, state, owner.
    pub fn protected_output_target(self, ordinal: u8) -> Result<LifecycleRegisterTargetV3> {
        let outputs = self.protected_outputs()?.ok_or(Error::InvalidCoordinate)?;
        match ordinal {
            0 => Ok(outputs.created()),
            1 => Ok(outputs.bump()),
            2 => Ok(outputs.historical_rent_principal()),
            3 => Ok(outputs.beneficiary()),
            4 => Ok(outputs.state()),
            5 => Ok(outputs.owner()),
            _ => Err(Error::InvalidCoordinate),
        }
    }

    /// Number of immutable identity-field bindings for this plan.
    pub fn immutable_identity_binding_count(self) -> Result<u16> {
        self.policy
            .immutable_identity_binding_count(self.plan_index)
    }

    /// Select one immutable identity-field binding for this plan.
    pub fn immutable_identity_binding(
        self,
        ordinal: u16,
    ) -> Result<LifecycleImmutableIdentityBindingV4> {
        self.policy
            .immutable_identity_binding_for_plan(self.plan_index, ordinal)
    }

    /// Whether this plan runs once or once for every authenticated runtime item.
    pub fn invocation_scope(self) -> Result<CoordinateScopeV3> {
        Ok(self.policy.recipe(self.plan.recipe)?.account.scope)
    }

    /// Exact number of mandatory invocation candidates for this plan.
    ///
    /// Fixed plans run once. Item plans run exactly `tail_count` times; a
    /// guard may then disable an individual invocation.
    pub fn invocation_count(self, tail_count: u32) -> Result<u32> {
        match self.invocation_scope()? {
            CoordinateScopeV3::Fixed => Ok(1),
            CoordinateScopeV3::Item => Ok(tail_count),
        }
    }

    /// Convert one canonical invocation ordinal into its optional item index.
    pub fn invocation_item(self, tail_count: u32, ordinal: u32) -> Result<Option<u32>> {
        match self.invocation_scope()? {
            CoordinateScopeV3::Fixed if ordinal == 0 => Ok(None),
            CoordinateScopeV3::Item if ordinal < tail_count => Ok(Some(ordinal)),
            _ => Err(Error::InvalidCoordinate),
        }
    }

    /// Exact checked state-data width selected for this runtime tail.
    ///
    /// Create adapters use this before planning to query the current Rent
    /// minimum for the same width. Authenticate and Close use it to size the
    /// exact existing state observation.
    pub fn target_data_bytes(self, tail_count: u32) -> Result<u32> {
        let recipe = self.policy.recipe(self.plan.recipe)?;
        recipe
            .data_stride
            .checked_mul(tail_count)
            .and_then(|tail| recipe.data_base.checked_add(tail))
            .ok_or(Error::Arithmetic)
    }

    /// Project the exact expanded AccountProfile indices used by one invocation.
    pub fn project_account_indices(
        self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        item_index: Option<u32>,
    ) -> Result<LifecycleAccountIndicesV3> {
        self.require_join(profile)?;
        let recipe = self.policy.recipe(self.plan.recipe)?;
        validate_item(recipe.account.scope, tail_count, item_index)?;
        Ok(LifecycleAccountIndicesV3 {
            state: expanded_account_index(profile, tail_count, item_index, recipe.account)?,
            payer: self
                .plan
                .payer
                .map(|coordinate| {
                    expanded_account_index(profile, tail_count, item_index, coordinate)
                })
                .transpose()?,
            rent_credit: self
                .plan
                .rent_credit
                .map(|coordinate| {
                    expanded_account_index(profile, tail_count, item_index, coordinate)
                })
                .transpose()?,
        })
    }

    /// Exact seed count for this selected derivation recipe.
    pub fn seed_count(self) -> Result<u8> {
        Ok(self.policy.recipe(self.plan.recipe)?.seed_count)
    }

    /// Whether the trusted PDA adapter must derive the sole final bump seed.
    pub fn uses_canonical_bump(self) -> Result<bool> {
        let recipe = self.policy.recipe(self.plan.recipe)?;
        Ok(self
            .policy
            .seed(
                recipe
                    .seed_start
                    .checked_add(u16::from(recipe.bump_offset))
                    .ok_or(Error::InvalidSeed)?,
            )?
            .source
            == SeedSourceV3::CanonicalBump)
    }

    /// Materialize one exact bounded seed for the separately named PDA adapter.
    pub fn materialize_seed(
        self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
        seed_ordinal: u8,
    ) -> Result<SeedValueV3> {
        self.policy.materialize_seed_for(
            profile,
            self.plan,
            tail_count,
            item_index,
            registers,
            seed_ordinal,
        )
    }

    /// Materialize one seed or identify the sole adapter-derived bump slot.
    pub fn materialize_seed_input(
        self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
        seed_ordinal: u8,
    ) -> Result<LifecycleSeedInputValueV3> {
        self.policy.materialize_seed_input_for(
            profile,
            self.join,
            self.plan,
            tail_count,
            item_index,
            registers,
            seed_ordinal,
        )
    }

    /// Register source used by one PDA seed, when the seed is register-backed.
    ///
    /// Hot execution uses this static inspection to prevent Transition from
    /// changing any seed input between protected preplanning and post-transition
    /// lifecycle revalidation.
    pub fn seed_register_target(
        self,
        seed_ordinal: u8,
    ) -> Result<Option<LifecycleRegisterTargetV3>> {
        let recipe = self.policy.recipe(self.plan.recipe)?;
        if seed_ordinal >= recipe.seed_count {
            return Err(Error::InvalidCoordinate);
        }
        let seed = self.policy.seed(
            recipe
                .seed_start
                .checked_add(u16::from(seed_ordinal))
                .ok_or(Error::Arithmetic)?,
        )?;
        Ok(match seed.source {
            SeedSourceV3::CommonIdentity => Some(LifecycleRegisterTargetV3 {
                kind: LifecycleRegisterKindV3::Identity,
                scope: CoordinateScopeV3::Fixed,
                index: seed.index,
            }),
            SeedSourceV3::ItemIdentity => Some(LifecycleRegisterTargetV3 {
                kind: LifecycleRegisterKindV3::Identity,
                scope: CoordinateScopeV3::Item,
                index: seed.index,
            }),
            SeedSourceV3::CommonScalar => Some(LifecycleRegisterTargetV3 {
                kind: LifecycleRegisterKindV3::Scalar,
                scope: CoordinateScopeV3::Fixed,
                index: seed.index,
            }),
            SeedSourceV3::ItemScalar => Some(LifecycleRegisterTargetV3 {
                kind: LifecycleRegisterKindV3::Scalar,
                scope: CoordinateScopeV3::Item,
                index: seed.index,
            }),
            SeedSourceV3::Literal | SeedSourceV3::ItemIndex | SeedSourceV3::CanonicalBump => None,
        })
    }

    /// Evaluate the exact guard against post-transition candidate registers.
    ///
    /// Every enabled plan for an action is mandatory. A false guard is the
    /// only data-defined way to skip a result-conditional operation.
    pub fn is_enabled(
        self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
    ) -> Result<bool> {
        self.require_join(profile)?;
        validate_runtime_width(profile, tail_count, registers)?;
        match self.plan.guard {
            PlanGuardV3::Always => Ok(true),
            PlanGuardV3::ScalarEq { source, expected } => Ok(scalar_register(
                profile, tail_count, item_index, registers, source,
            )? == expected),
        }
    }
}

/// Expanded physical account indices for one selected lifecycle invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleAccountIndicesV3 {
    state: usize,
    payer: Option<usize>,
    rent_credit: Option<usize>,
}

impl LifecycleAccountIndicesV3 {
    /// Exact Trading state/vacancy account index.
    pub const fn state(self) -> usize {
        self.state
    }

    /// Exact payer index for Create; absent for Authenticate/Close.
    pub const fn payer(self) -> Option<usize> {
        self.payer
    }

    /// Exact permanent RentCredit index for Create/Close.
    pub const fn rent_credit(self) -> Option<usize> {
        self.rent_credit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegisterSourceV3 {
    item: bool,
    index: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlanGuardV3 {
    Always,
    ScalarEq {
        source: RegisterSourceV3,
        expected: u64,
    },
}

/// Borrowed exact lifecycle policy selected by `CapabilityProgramV3.derivation_policy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateLifecyclePolicyV3<'a> {
    artifact_profile: u16,
    recipes: u16,
    seeds: u16,
    plans: u16,
    protected_outputs: u16,
    immutable_identity_bindings: u16,
    current_rent_quotes: u16,
    bytes: &'a [u8],
}

/// Successor-only lifecycle policy selected under the V4 Registry schema.
///
/// This wrapper makes the legacy profiles unreachable to the Hot successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateLifecyclePolicyV4<'a>(StateLifecyclePolicyV3<'a>);

impl<'a> StateLifecyclePolicyV4<'a> {
    /// Decode exact successor bytes after content authentication.
    pub fn decode_selected(
        selected_id: [u8; 32],
        authenticated_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        let policy = StateLifecyclePolicyV3::decode_selected(selected_id, authenticated_id, bytes)?;
        if policy.artifact_profile != SUCCESSOR_ARTIFACT_PROFILE {
            return Err(Error::UnsupportedProfile);
        }
        Ok(Self(policy))
    }

    /// Exact canonical successor bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.0.bytes()
    }

    /// Number of plans selected by one action.
    pub fn action_plan_count(self, action: u32) -> Result<u16> {
        self.0.action_plan_count(action)
    }

    /// Select one canonical action plan.
    pub fn action_plan(self, action: u32, ordinal: u16) -> Result<SelectedLifecycleV3<'a>> {
        self.0.action_plan(action, ordinal)
    }

    /// Join the successor policy to the authenticated AccountProfile.
    pub fn validate_account_profile(self, profile: AccountProfileV2<'_>) -> Result<()> {
        self.0.validate_account_profile(profile)
    }
}

/// Successor-only lifecycle policy with protected current-Rent quote projection.
///
/// V4 remains decodable for migration, but this wrapper admits only artifact
/// profile 4 selected under [`CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateLifecyclePolicyV5<'a>(StateLifecyclePolicyV3<'a>);

impl<'a> StateLifecyclePolicyV5<'a> {
    /// Decode exact V5 bytes after finalized content selection/authentication.
    pub fn decode_selected(
        selected_id: [u8; 32],
        authenticated_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        let policy = StateLifecyclePolicyV3::decode_selected(selected_id, authenticated_id, bytes)?;
        if policy.artifact_profile != CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5 {
            return Err(Error::UnsupportedProfile);
        }
        Ok(Self(policy))
    }

    /// Exact canonical V5 bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.0.bytes()
    }

    /// Validate this policy's join to one exact AccountProfile and record it.
    ///
    /// The returned evidence lets a batch of plans over the same two artifacts
    /// skip re-deriving a join that cannot have changed. It proves exactly what
    /// `validate_account_profile` proves, for exactly these bytes.
    pub fn validate_account_profile_join<'b>(
        self,
        profile: AccountProfileV2<'b>,
    ) -> Result<ValidatedProfileJoinV3<'b>>
    where
        'a: 'b,
    {
        self.0.validate_account_profile_join(profile)
    }

    /// Whether every lifecycle and quote table is canonically empty.
    pub const fn is_empty(self) -> bool {
        self.0.recipes == 0
            && self.0.seeds == 0
            && self.0.plans == 0
            && self.0.protected_outputs == 0
            && self.0.immutable_identity_bindings == 0
            && self.0.current_rent_quotes == 0
    }

    /// Number of plans selected by one action.
    pub fn action_plan_count(self, action: u32) -> Result<u16> {
        self.0.action_plan_count(action)
    }

    /// Select one canonical action plan.
    pub fn action_plan(self, action: u32, ordinal: u16) -> Result<SelectedLifecycleV3<'a>> {
        self.0.action_plan(action, ordinal)
    }

    /// Exact bounded current-Rent quote declaration count.
    pub const fn current_rent_quote_count(self) -> u16 {
        self.0.current_rent_quotes
    }

    /// Decode one canonical current-Rent quote declaration.
    pub fn current_rent_quote(self, ordinal: u16) -> Result<LifecycleCurrentRentQuoteV5> {
        self.0.current_rent_quote(ordinal)
    }

    /// Join every lifecycle and quote coordinate to the authenticated AccountProfile.
    pub fn validate_account_profile(self, profile: AccountProfileV2<'_>) -> Result<()> {
        self.0.validate_account_profile(profile)
    }

    /// Seed adapter-authenticated current Rent minima into protected scalars atomically.
    ///
    /// `input_scalars`, scratch, and output are the exact complete runtime scalar
    /// bank for `tail_count`. Quote destinations must be zero before projection;
    /// AccountProfile writes and lifecycle protected outputs are statically
    /// forbidden from targeting them. The Hot adapter must additionally forbid
    /// RequestProfile and Transition writes before and after execution.
    #[allow(clippy::too_many_arguments)]
    pub fn project_authenticated_current_rent_quotes_atomic(
        self,
        profile: AccountProfileV2<'_>,
        join: Option<ValidatedProfileJoinV3<'_>>,
        tail_count: u32,
        input_scalars: &[u64],
        quotes: &[AuthenticatedRentQuoteV5],
        buffers: LifecycleRentQuoteBuffersV5<'_>,
    ) -> Result<()> {
        self.0.require_join(join, profile)?;
        let expected_width = affine_width(
            profile.common_scalar_count(),
            profile.item_scalar_stride(),
            tail_count,
        )?;
        if quotes.len() != usize::from(self.0.current_rent_quotes) {
            return Err(Error::InvalidRentQuote);
        }
        if input_scalars.len() != expected_width
            || buffers.scalar_scratch.len() != expected_width
            || buffers.output_scalars.len() != expected_width
        {
            return Err(Error::RuntimeWidth);
        }
        buffers.scalar_scratch.copy_from_slice(input_scalars);
        let mut ordinal = 0_u16;
        while ordinal < self.0.current_rent_quotes {
            let declaration = self.0.current_rent_quote(ordinal)?;
            let quote = quotes
                .get(usize::from(ordinal))
                .copied()
                .ok_or(Error::InvalidRentQuote)?;
            validate_authenticated_rent_quote(declaration, quote)?;
            let destination = usize::from(declaration.scalar_destination);
            let target = buffers
                .scalar_scratch
                .get_mut(destination)
                .ok_or(Error::RuntimeWidth)?;
            if *target != 0 {
                return Err(Error::ProfileMismatch);
            }
            *target = quote.current_minimum;
            ordinal = ordinal.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        buffers
            .output_scalars
            .copy_from_slice(buffers.scalar_scratch);
        Ok(())
    }

    /// Revalidate protected current-Rent quote scalars after Request/Transition execution.
    ///
    /// This closes the protected-output boundary without trusting request or
    /// configuration bytes: every destination must still equal the same
    /// adapter-authenticated current minimum used during projection.
    pub fn validate_projected_current_rent_quotes(
        self,
        profile: AccountProfileV2<'_>,
        join: Option<ValidatedProfileJoinV3<'_>>,
        tail_count: u32,
        projected_scalars: &[u64],
        quotes: &[AuthenticatedRentQuoteV5],
    ) -> Result<()> {
        self.0.require_join(join, profile)?;
        if quotes.len() != usize::from(self.0.current_rent_quotes)
            || projected_scalars.len()
                != affine_width(
                    profile.common_scalar_count(),
                    profile.item_scalar_stride(),
                    tail_count,
                )?
        {
            return Err(Error::InvalidRentQuote);
        }
        let mut ordinal = 0_u16;
        while ordinal < self.0.current_rent_quotes {
            let declaration = self.0.current_rent_quote(ordinal)?;
            let quote = quotes
                .get(usize::from(ordinal))
                .copied()
                .ok_or(Error::InvalidRentQuote)?;
            validate_authenticated_rent_quote(declaration, quote)?;
            if projected_scalars
                .get(usize::from(declaration.scalar_destination))
                .copied()
                != Some(quote.current_minimum)
            {
                return Err(Error::InvalidRentQuote);
            }
            ordinal = ordinal.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
    }
}

fn validate_authenticated_rent_quote(
    declaration: LifecycleCurrentRentQuoteV5,
    quote: AuthenticatedRentQuoteV5,
) -> Result<()> {
    if quote.exact_data_len != declaration.exact_data_len
        || quote.scalar_destination != declaration.scalar_destination
        || quote.current_minimum == 0
    {
        Err(Error::InvalidRentQuote)
    } else {
        Ok(())
    }
}

impl<'a> StateLifecyclePolicyV3<'a> {
    /// Decode canonical bytes after exact content authentication.
    pub fn decode_selected(
        selected_id: [u8; 32],
        authenticated_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_id == [0; 32] || authenticated_id == [0; 32] || selected_id != authenticated_id
        {
            return Err(Error::ContentIdentity);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode one complete exact artifact.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != VERSION {
            return Err(Error::UnsupportedProfile);
        }
        let artifact_profile = read_u16(bytes, 10)?;
        let (protected_outputs, immutable_identity_bindings, current_rent_quotes) =
            match artifact_profile {
                ARTIFACT_PROFILE => {
                    require_zero(bytes, 18, 22)?;
                    (0, 0, 0)
                }
                PROTECTED_OUTPUT_ARTIFACT_PROFILE => {
                    let count = read_u16(bytes, 18)?;
                    require_zero(bytes, 20, 20)?;
                    (count, 0, 0)
                }
                SUCCESSOR_ARTIFACT_PROFILE => {
                    let protected = read_u16(bytes, 18)?;
                    let bindings = read_u16(bytes, 20)?;
                    require_zero(bytes, 22, 18)?;
                    (protected, bindings, 0)
                }
                CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5 => {
                    let protected = read_u16(bytes, 18)?;
                    let bindings = read_u16(bytes, 20)?;
                    let quotes = read_u16(bytes, 22)?;
                    require_zero(bytes, 24, 16)?;
                    (protected, bindings, quotes)
                }
                _ => return Err(Error::UnsupportedProfile),
            };
        let value = Self {
            artifact_profile,
            recipes: read_u16(bytes, 12)?,
            seeds: read_u16(bytes, 14)?,
            plans: read_u16(bytes, 16)?,
            protected_outputs,
            immutable_identity_bindings,
            current_rent_quotes,
            bytes,
        };
        let empty = value.recipes == 0
            && value.seeds == 0
            && value.plans == 0
            && value.protected_outputs == 0
            && value.immutable_identity_bindings == 0
            && value.current_rent_quotes == 0;
        if (value.recipes == 0 || value.seeds == 0 || value.plans == 0)
            && !(value.artifact_profile == CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5 && empty)
        {
            return Err(Error::EmptyPolicy);
        }
        if matches!(
            value.artifact_profile,
            PROTECTED_OUTPUT_ARTIFACT_PROFILE
                | SUCCESSOR_ARTIFACT_PROFILE
                | CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5
        ) && value.protected_outputs != value.plans
        {
            return Err(Error::InvalidLength);
        }
        if value.current_rent_quotes > MAX_CURRENT_RENT_QUOTES_V5 {
            return Err(Error::InvalidRentQuote);
        }
        let expected = usize::from(value.recipes)
            .checked_mul(RECIPE_BYTES)
            .and_then(|width| {
                usize::from(value.seeds)
                    .checked_mul(SEED_BYTES)
                    .and_then(|seeds| width.checked_add(seeds))
            })
            .and_then(|width| {
                usize::from(value.plans)
                    .checked_mul(ACTION_PLAN_BYTES)
                    .and_then(|plans| width.checked_add(plans))
            })
            .and_then(|width| {
                usize::from(value.protected_outputs)
                    .checked_mul(PROTECTED_OUTPUT_BYTES)
                    .and_then(|protected| width.checked_add(protected))
            })
            .and_then(|width| {
                usize::from(value.immutable_identity_bindings)
                    .checked_mul(IMMUTABLE_IDENTITY_BINDING_BYTES)
                    .and_then(|bindings| width.checked_add(bindings))
            })
            .and_then(|width| {
                usize::from(value.current_rent_quotes)
                    .checked_mul(CURRENT_RENT_QUOTE_BYTES_V5)
                    .and_then(|quotes| width.checked_add(quotes))
            })
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        value.validate()?;
        Ok(value)
    }

    /// Exact canonical bytes whose SHA-256 is selected by the descriptor.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Number of plans selected by one action.
    pub fn action_plan_count(self, action: u32) -> Result<u16> {
        let mut found = 0_u16;
        let mut index = 0_u16;
        while index < self.plans {
            if self.plan(index)?.action == action {
                found = found.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(found)
    }

    /// Select the `ordinal`th canonical plan for one action.
    pub fn action_plan(self, action: u32, ordinal: u16) -> Result<SelectedLifecycleV3<'a>> {
        let mut seen = 0_u16;
        let mut index = 0_u16;
        while index < self.plans {
            let plan = self.plan(index)?;
            if plan.action == action {
                if seen == ordinal {
                    return Ok(SelectedLifecycleV3 {
                        policy: self,
                        plan,
                        plan_index: index,
                        join: None,
                    });
                }
                seen = seen.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Err(Error::InvalidCoordinate)
    }

    /// Validate this policy's join to one exact AccountProfile and record it.
    ///
    /// The returned evidence lets a batch of plans over the same two artifacts
    /// skip re-deriving a join that cannot have changed. It proves exactly what
    /// `validate_account_profile` proves, for exactly these bytes.
    pub fn validate_account_profile_join<'b>(
        self,
        profile: AccountProfileV2<'b>,
    ) -> Result<ValidatedProfileJoinV3<'b>>
    where
        'a: 'b,
    {
        self.validate_account_profile(profile)?;
        Ok(ValidatedProfileJoinV3 {
            policy: self.bytes(),
            profile: profile.bytes(),
        })
    }

    /// Establish this policy's join to `profile`, deriving it only if the
    /// supplied evidence does not already name exactly these two artifacts.
    fn require_join(
        self,
        join: Option<ValidatedProfileJoinV3<'_>>,
        profile: AccountProfileV2<'_>,
    ) -> Result<()> {
        if join.is_some_and(|join| join.covers(self, profile)) {
            Ok(())
        } else {
            self.validate_account_profile(profile)
        }
    }

    /// Require every policy coordinate to fit the sole AccountProfile geometry.
    pub fn validate_account_profile(self, profile: AccountProfileV2<'_>) -> Result<()> {
        let mut recipe_index = 0_u16;
        while recipe_index < self.recipes {
            let recipe = self.recipe(recipe_index)?;
            validate_account_coordinate(profile, recipe.account)?;
            let mut seed = 0_u8;
            while seed < recipe.seed_count {
                self.validate_seed_against_profile(
                    self.seed(
                        recipe
                            .seed_start
                            .checked_add(u16::from(seed))
                            .ok_or(Error::Arithmetic)?,
                    )?,
                    profile,
                )?;
                seed = seed.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            recipe_index = recipe_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut plan_index = 0_u16;
        while plan_index < self.plans {
            let plan = self.plan(plan_index)?;
            let recipe = self.recipe(plan.recipe)?;
            if let Some(payer) = plan.payer {
                validate_account_coordinate(profile, payer)?;
                validate_invocation_reference(recipe.account.scope, payer.scope)?;
            }
            if let Some(credit) = plan.rent_credit {
                validate_account_coordinate(profile, credit)?;
                validate_invocation_reference(recipe.account.scope, credit.scope)?;
            }
            if let Some(principal) = plan.principal {
                validate_scalar_source(profile, principal)?;
                validate_invocation_source(recipe.account.scope, principal)?;
            }
            if let Some(beneficiary) = plan.beneficiary {
                validate_identity_source(profile, beneficiary)?;
                validate_invocation_source(recipe.account.scope, beneficiary)?;
            }
            if let PlanGuardV3::ScalarEq { source, .. } = plan.guard {
                validate_scalar_source(profile, source)?;
                validate_invocation_source(recipe.account.scope, source)?;
            }
            self.validate_protected_outputs_against_profile(plan_index, plan, recipe, profile)?;
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        self.validate_protected_output_uniqueness()?;
        let mut binding_index = 0_u16;
        while binding_index < self.immutable_identity_bindings {
            let (plan_index, binding) = self.immutable_identity_binding(binding_index)?;
            let plan = self.plan(plan_index)?;
            let recipe = self.recipe(plan.recipe)?;
            validate_lifecycle_target(profile, binding.canonical)?;
            validate_invocation_source(recipe.account.scope, register_source(binding.canonical))?;
            binding_index = binding_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        self.validate_current_rent_quotes_against_profile(profile)?;
        self.validate_lifecycle_prestates(profile)
    }

    fn validate_current_rent_quotes_against_profile(
        self,
        profile: AccountProfileV2<'_>,
    ) -> Result<()> {
        let mut quote_index = 0_u16;
        while quote_index < self.current_rent_quotes {
            let quote = self.current_rent_quote(quote_index)?;
            let target = quote.scalar_destination();
            validate_lifecycle_target(profile, target)?;
            if profile
                .writes_register(profile_target(target))
                .map_err(|_| Error::ProfileMismatch)?
            {
                return Err(Error::ProfileMismatch);
            }
            let mut plan_index = 0_u16;
            while plan_index < self.plans {
                if let Some(outputs) = self.protected_outputs(plan_index)? {
                    let scalar_outputs = [
                        outputs.created(),
                        outputs.bump(),
                        outputs.historical_rent_principal(),
                    ];
                    if scalar_outputs.contains(&target) {
                        return Err(Error::ProfileMismatch);
                    }
                }
                plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            quote_index = quote_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
    }

    fn validate_protected_output_uniqueness(self) -> Result<()> {
        let mut left = 0_u16;
        while left < self.plans {
            let left_plan = self.plan(left)?;
            if let Some(left_outputs) = self.protected_outputs(left)? {
                let mut right = left.checked_add(1).ok_or(Error::Arithmetic)?;
                while right < self.plans {
                    let right_plan = self.plan(right)?;
                    if right_plan.action != left_plan.action {
                        right = right.checked_add(1).ok_or(Error::Arithmetic)?;
                        continue;
                    }
                    if let Some(right_outputs) = self.protected_outputs(right)? {
                        for left_target in protected_output_targets(left_outputs) {
                            if protected_output_targets(right_outputs).contains(&left_target) {
                                return Err(Error::ProfileMismatch);
                            }
                        }
                    }
                    right = right.checked_add(1).ok_or(Error::Arithmetic)?;
                }
            }
            left = left.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
    }

    fn validate_protected_outputs_against_profile(
        self,
        plan_index: u16,
        plan: ActionPlanV3,
        recipe: RecipeV3,
        profile: AccountProfileV2<'_>,
    ) -> Result<()> {
        let Some(outputs) = self.protected_outputs(plan_index)? else {
            return Ok(());
        };
        if plan.operation != LifecycleOperationV3::AuthenticateOrCreate
            || outputs.scope != recipe.account.scope
        {
            return Err(Error::ProfileMismatch);
        }
        let bump_observation = outputs.bump_observation();
        let principal_observation = lifecycle_target(
            LifecycleRegisterKindV3::Scalar,
            plan.principal.ok_or(Error::InvalidRent)?,
        );
        let beneficiary_observation = lifecycle_target(
            LifecycleRegisterKindV3::Identity,
            plan.beneficiary.ok_or(Error::InvalidRent)?,
        );
        for observation in [
            bump_observation,
            principal_observation,
            beneficiary_observation,
        ] {
            validate_lifecycle_target(profile, observation)?;
            if !profile
                .writes_register(profile_target(observation))
                .map_err(|_| Error::ProfileMismatch)?
            {
                return Err(Error::ProfileMismatch);
            }
        }
        let scalar_outputs = [
            outputs.created(),
            outputs.bump(),
            outputs.historical_rent_principal(),
        ];
        let identity_outputs = [outputs.beneficiary(), outputs.state(), outputs.owner()];
        for output in scalar_outputs.into_iter().chain(identity_outputs) {
            validate_lifecycle_target(profile, output)?;
            if profile
                .writes_register(profile_target(output))
                .map_err(|_| Error::ProfileMismatch)?
            {
                return Err(Error::ProfileMismatch);
            }
        }
        if bump_observation == principal_observation
            || scalar_outputs[0] == scalar_outputs[1]
            || scalar_outputs[0] == scalar_outputs[2]
            || scalar_outputs[1] == scalar_outputs[2]
            || scalar_outputs.contains(&bump_observation)
            || scalar_outputs.contains(&principal_observation)
            || identity_outputs[0] == identity_outputs[1]
            || identity_outputs[0] == identity_outputs[2]
            || identity_outputs[1] == identity_outputs[2]
            || identity_outputs.contains(&beneficiary_observation)
        {
            return Err(Error::ProfileMismatch);
        }
        Ok(())
    }

    fn validate_lifecycle_prestates(self, profile: AccountProfileV2<'_>) -> Result<()> {
        let mut fixed = 0_u16;
        while fixed < profile.fixed_account_count() {
            self.validate_lifecycle_prestate_coordinate(
                profile,
                AccountCoordinateV3 {
                    scope: CoordinateScopeV3::Fixed,
                    index: fixed,
                },
            )?;
            fixed = fixed.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut item = 0_u16;
        while item < profile.item_account_stride() {
            self.validate_lifecycle_prestate_coordinate(
                profile,
                AccountCoordinateV3 {
                    scope: CoordinateScopeV3::Item,
                    index: item,
                },
            )?;
            item = item.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut plan_index = 0_u16;
        while plan_index < self.plans {
            let plan = self.plan(plan_index)?;
            if plan.operation == LifecycleOperationV3::AuthenticateOrCreate {
                let recipe = self.recipe(plan.recipe)?;
                if rule_for_coordinate(profile, recipe.account)?.prestate()
                    != AccountPrestateV2::LifecycleBound
                    || plan.guard != PlanGuardV3::Always
                {
                    return Err(Error::ProfileMismatch);
                }
            }
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
    }

    fn validate_lifecycle_prestate_coordinate(
        self,
        profile: AccountProfileV2<'_>,
        coordinate: AccountCoordinateV3,
    ) -> Result<()> {
        let rule = rule_for_coordinate(profile, coordinate)?;
        if rule.prestate() != AccountPrestateV2::LifecycleBound {
            return Ok(());
        }
        let mut matching_recipe = None;
        let mut recipe_index = 0_u16;
        while recipe_index < self.recipes {
            let recipe = self.recipe(recipe_index)?;
            if recipe.account == coordinate {
                if matching_recipe.is_some()
                    || recipe.data_base != rule.data_length()
                    || recipe.data_stride != rule.data_item_stride()
                {
                    return Err(Error::ProfileMismatch);
                }
                matching_recipe = Some(recipe_index);
            }
            recipe_index = recipe_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let recipe_index = matching_recipe.ok_or(Error::ProfileMismatch)?;
        let mut found_alternative = false;
        let mut plan_index = 0_u16;
        while plan_index < self.plans {
            let plan = self.plan(plan_index)?;
            if plan.recipe == recipe_index
                && plan.operation == LifecycleOperationV3::AuthenticateOrCreate
            {
                if plan.guard != PlanGuardV3::Always {
                    return Err(Error::ProfileMismatch);
                }
                found_alternative = true;
            }
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        if found_alternative {
            Ok(())
        } else {
            Err(Error::ProfileMismatch)
        }
    }

    /// Materialize one exact bounded seed for the separately named PDA adapter.
    fn materialize_seed_for(
        self,
        profile: AccountProfileV2<'_>,
        plan: ActionPlanV3,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
        seed_ordinal: u8,
    ) -> Result<SeedValueV3> {
        self.validate_account_profile(profile)?;
        validate_runtime_width(profile, tail_count, registers)?;
        let recipe = self.recipe(plan.recipe)?;
        validate_item(recipe.account.scope, tail_count, item_index)?;
        if seed_ordinal >= recipe.seed_count {
            return Err(Error::InvalidCoordinate);
        }
        let seed = self.seed(
            recipe
                .seed_start
                .checked_add(u16::from(seed_ordinal))
                .ok_or(Error::Arithmetic)?,
        )?;
        seed.materialize(profile, tail_count, item_index, registers)
    }

    #[allow(clippy::too_many_arguments)]
    fn materialize_seed_input_for(
        self,
        profile: AccountProfileV2<'_>,
        join: Option<ValidatedProfileJoinV3<'_>>,
        plan: ActionPlanV3,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
        seed_ordinal: u8,
    ) -> Result<LifecycleSeedInputValueV3> {
        self.require_join(join, profile)?;
        validate_runtime_width(profile, tail_count, registers)?;
        let recipe = self.recipe(plan.recipe)?;
        validate_item(recipe.account.scope, tail_count, item_index)?;
        if seed_ordinal >= recipe.seed_count {
            return Err(Error::InvalidCoordinate);
        }
        let seed = self.seed(
            recipe
                .seed_start
                .checked_add(u16::from(seed_ordinal))
                .ok_or(Error::Arithmetic)?,
        )?;
        if seed.source == SeedSourceV3::CanonicalBump {
            Ok(LifecycleSeedInputValueV3::CanonicalBump)
        } else {
            seed.materialize(profile, tail_count, item_index, registers)
                .map(LifecycleSeedInputValueV3::Bytes)
        }
    }

    fn validate(self) -> Result<()> {
        let mut recipe_index = 0_u16;
        while recipe_index < self.recipes {
            let recipe = self.recipe(recipe_index)?;
            if recipe.seed_count == 0 || recipe.seed_count > MAX_SEEDS {
                return Err(Error::InvalidSeed);
            }
            let end = recipe
                .seed_start
                .checked_add(u16::from(recipe.seed_count))
                .ok_or(Error::InvalidCoordinate)?;
            if end > self.seeds || recipe.bump_offset.checked_add(1) != Some(recipe.seed_count) {
                return Err(Error::InvalidCoordinate);
            }
            let mut local_seed = 0_u8;
            while local_seed < recipe.seed_count {
                self.seed(
                    recipe
                        .seed_start
                        .checked_add(u16::from(local_seed))
                        .ok_or(Error::InvalidCoordinate)?,
                )?;
                local_seed = local_seed.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            let bump = self.seed(
                recipe
                    .seed_start
                    .checked_add(u16::from(recipe.bump_offset))
                    .ok_or(Error::InvalidCoordinate)?,
            )?;
            let bump_valid = match self.artifact_profile {
                ARTIFACT_PROFILE => matches!(
                    bump.source,
                    SeedSourceV3::CommonScalar | SeedSourceV3::ItemScalar
                ),
                PROTECTED_OUTPUT_ARTIFACT_PROFILE
                | SUCCESSOR_ARTIFACT_PROFILE
                | CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5 => {
                    bump.source == SeedSourceV3::CanonicalBump
                }
                _ => false,
            };
            if !bump_valid || bump.width != 1 {
                return Err(Error::InvalidSeed);
            }
            if recipe.data_base == 0 {
                return Err(Error::InvalidCoordinate);
            }
            recipe_index = recipe_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut plan_index = 0_u16;
        let mut previous = None;
        while plan_index < self.plans {
            let plan = self.plan(plan_index)?;
            if plan.recipe >= self.recipes {
                return Err(Error::InvalidCoordinate);
            }
            let order = (plan.action, operation_tag(plan.operation), plan.recipe);
            if previous.is_some_and(|prior| prior >= order) {
                return Err(Error::InvalidCoordinate);
            }
            let protected = self.protected_outputs(plan_index)?;
            if (plan.operation == LifecycleOperationV3::AuthenticateOrCreate) != protected.is_some()
                && matches!(
                    self.artifact_profile,
                    PROTECTED_OUTPUT_ARTIFACT_PROFILE
                        | SUCCESSOR_ARTIFACT_PROFILE
                        | CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5
                )
            {
                return Err(Error::NonCanonicalReserved);
            }
            previous = Some(order);
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut previous_binding = None;
        let mut binding_index = 0_u16;
        while binding_index < self.immutable_identity_bindings {
            let (bound_plan, binding) = self.immutable_identity_binding(binding_index)?;
            let plan = self.plan(bound_plan)?;
            let recipe = self.recipe(plan.recipe)?;
            if !matches!(
                self.artifact_profile,
                SUCCESSOR_ARTIFACT_PROFILE | CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5
            ) || plan.operation != LifecycleOperationV3::AuthenticateOrCreate
                || self.protected_outputs(bound_plan)?.is_none()
                || binding
                    .data_offset
                    .checked_add(32)
                    .is_none_or(|end| end > recipe.data_base)
            {
                return Err(Error::InvalidCoordinate);
            }
            let order = (bound_plan, binding.data_offset);
            if previous_binding.is_some_and(|previous: (u16, u32)| {
                previous >= order
                    || (previous.0 == bound_plan
                        && previous
                            .1
                            .checked_add(32)
                            .is_none_or(|end| end > binding.data_offset))
            }) {
                return Err(Error::InvalidCoordinate);
            }
            previous_binding = Some(order);
            binding_index = binding_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        let mut previous_destination = None;
        let mut quote_index = 0_u16;
        while quote_index < self.current_rent_quotes {
            let quote = self.current_rent_quote(quote_index)?;
            if previous_destination.is_some_and(|previous| previous >= quote.scalar_destination) {
                return Err(Error::InvalidRentQuote);
            }
            previous_destination = Some(quote.scalar_destination);
            quote_index = quote_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
    }

    fn recipe(self, index: u16) -> Result<RecipeV3> {
        if index >= self.recipes {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(index)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let scope = CoordinateScopeV3::decode(read_u8(self.bytes, offset)?)?;
        require_zero(self.bytes, offset + 1, 1)?;
        let value = RecipeV3 {
            account: AccountCoordinateV3 {
                scope,
                index: read_u16(self.bytes, offset + 2)?,
            },
            seed_start: read_u16(self.bytes, offset + 4)?,
            seed_count: read_u8(self.bytes, offset + 6)?,
            bump_offset: read_u8(self.bytes, offset + 7)?,
            data_base: read_u32(self.bytes, offset + 8)?,
            data_stride: read_u32(self.bytes, offset + 12)?,
        };
        Ok(value)
    }

    fn seed(self, index: u16) -> Result<SeedOperationV3<'a>> {
        if index >= self.seeds {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(self.recipes)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(SEED_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .ok_or(Error::InvalidLength)?;
        SeedOperationV3::decode(self.bytes, offset)
    }

    fn plan(self, index: u16) -> Result<ActionPlanV3> {
        if index >= self.plans {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(self.recipes)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|base| {
                usize::from(self.seeds)
                    .checked_mul(SEED_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(ACTION_PLAN_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .ok_or(Error::InvalidLength)?;
        let operation = LifecycleOperationV3::decode(read_u8(self.bytes, offset + 4)?)?;
        require_zero(self.bytes, offset + 5, 1)?;
        let recipe = read_u16(self.bytes, offset + 6)?;
        let payer = decode_optional_account(self.bytes, offset + 8)?;
        let rent_credit = decode_optional_account(self.bytes, offset + 12)?;
        let principal = decode_optional_register(self.bytes, offset + 16)?;
        let beneficiary = decode_optional_register(self.bytes, offset + 20)?;
        let guard = match read_u8(self.bytes, offset + 24)? {
            GUARD_ALWAYS => {
                require_zero(self.bytes, offset + 25, 15)?;
                PlanGuardV3::Always
            }
            GUARD_SCALAR_EQ => {
                let source = match read_u8(self.bytes, offset + 25)? {
                    SOURCE_COMMON => false,
                    SOURCE_ITEM => true,
                    _ => return Err(Error::UnknownTag),
                };
                let index = read_u16(self.bytes, offset + 26)?;
                let expected = read_u64(self.bytes, offset + 28)?;
                require_zero(self.bytes, offset + 36, 4)?;
                PlanGuardV3::ScalarEq {
                    source: RegisterSourceV3 {
                        item: source,
                        index,
                    },
                    expected,
                }
            }
            _ => return Err(Error::UnknownTag),
        };
        match operation {
            LifecycleOperationV3::Authenticate => {
                if payer.is_some()
                    || rent_credit.is_some()
                    || principal.is_some()
                    || beneficiary.is_some()
                {
                    return Err(Error::NonCanonicalReserved);
                }
            }
            LifecycleOperationV3::Create => {
                if payer.is_none()
                    || rent_credit.is_none()
                    || principal.is_none()
                    || beneficiary.is_none()
                {
                    return Err(Error::InvalidFunding);
                }
            }
            LifecycleOperationV3::Close => {
                if payer.is_some()
                    || rent_credit.is_none()
                    || principal.is_none()
                    || beneficiary.is_none()
                {
                    return Err(Error::InvalidFunding);
                }
            }
            LifecycleOperationV3::AuthenticateOrCreate => {
                if payer.is_none()
                    || rent_credit.is_none()
                    || principal.is_none()
                    || beneficiary.is_none()
                    || guard != PlanGuardV3::Always
                {
                    return Err(Error::InvalidFunding);
                }
            }
        }
        Ok(ActionPlanV3 {
            action: read_u32(self.bytes, offset)?,
            operation,
            recipe,
            payer,
            rent_credit,
            principal,
            beneficiary,
            guard,
        })
    }

    fn protected_outputs(self, index: u16) -> Result<Option<LifecycleProtectedOutputsV3>> {
        if index >= self.plans {
            return Err(Error::InvalidCoordinate);
        }
        if self.artifact_profile == ARTIFACT_PROFILE {
            return Ok(None);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(self.recipes)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|base| {
                usize::from(self.seeds)
                    .checked_mul(SEED_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.plans)
                    .checked_mul(ACTION_PLAN_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(PROTECTED_OUTPUT_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .ok_or(Error::InvalidLength)?;
        match read_u8(self.bytes, offset)? {
            PROTECTED_OUTPUT_NONE => {
                require_zero(self.bytes, offset + 1, PROTECTED_OUTPUT_BYTES - 1)?;
                Ok(None)
            }
            PROTECTED_OUTPUT_AUTHENTICATE_OR_CREATE => {
                require_zero(self.bytes, offset + 1, 1)?;
                require_zero(self.bytes, offset + 16, PROTECTED_OUTPUT_BYTES - 16)?;
                let recipe = self.recipe(self.plan(index)?.recipe)?;
                Ok(Some(LifecycleProtectedOutputsV3 {
                    scope: recipe.account.scope,
                    created: read_u16(self.bytes, offset + 2)?,
                    bump_observation: read_u16(self.bytes, offset + 4)?,
                    bump: read_u16(self.bytes, offset + 6)?,
                    historical_rent_principal: read_u16(self.bytes, offset + 8)?,
                    beneficiary: read_u16(self.bytes, offset + 10)?,
                    state: read_u16(self.bytes, offset + 12)?,
                    owner: read_u16(self.bytes, offset + 14)?,
                }))
            }
            _ => Err(Error::UnknownTag),
        }
    }

    fn immutable_identity_binding_count(self, plan_index: u16) -> Result<u16> {
        if plan_index >= self.plans {
            return Err(Error::InvalidCoordinate);
        }
        let mut found = 0_u16;
        let mut index = 0_u16;
        while index < self.immutable_identity_bindings {
            if self.immutable_identity_binding(index)?.0 == plan_index {
                found = found.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(found)
    }

    fn immutable_identity_binding_for_plan(
        self,
        plan_index: u16,
        ordinal: u16,
    ) -> Result<LifecycleImmutableIdentityBindingV4> {
        let mut seen = 0_u16;
        let mut index = 0_u16;
        while index < self.immutable_identity_bindings {
            let (candidate_plan, binding) = self.immutable_identity_binding(index)?;
            if candidate_plan == plan_index {
                if seen == ordinal {
                    return Ok(binding);
                }
                seen = seen.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Err(Error::InvalidCoordinate)
    }

    fn immutable_identity_binding(
        self,
        index: u16,
    ) -> Result<(u16, LifecycleImmutableIdentityBindingV4)> {
        if index >= self.immutable_identity_bindings {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(self.recipes)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|base| {
                usize::from(self.seeds)
                    .checked_mul(SEED_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.plans)
                    .checked_mul(ACTION_PLAN_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.protected_outputs)
                    .checked_mul(PROTECTED_OUTPUT_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(IMMUTABLE_IDENTITY_BINDING_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .ok_or(Error::InvalidLength)?;
        let plan_index = read_u16(self.bytes, offset)?;
        let item = match read_u8(self.bytes, offset + 2)? {
            SOURCE_COMMON => false,
            SOURCE_ITEM => true,
            _ => return Err(Error::UnknownTag),
        };
        require_zero(self.bytes, offset + 3, 1)?;
        let canonical = LifecycleRegisterTargetV3 {
            kind: LifecycleRegisterKindV3::Identity,
            scope: if item {
                CoordinateScopeV3::Item
            } else {
                CoordinateScopeV3::Fixed
            },
            index: read_u16(self.bytes, offset + 4)?,
        };
        require_zero(self.bytes, offset + 6, 2)?;
        let data_offset = read_u32(self.bytes, offset + 8)?;
        require_zero(self.bytes, offset + 12, 4)?;
        Ok((
            plan_index,
            LifecycleImmutableIdentityBindingV4 {
                data_offset,
                canonical,
            },
        ))
    }

    fn current_rent_quote(self, index: u16) -> Result<LifecycleCurrentRentQuoteV5> {
        if self.artifact_profile != CURRENT_RENT_QUOTE_ARTIFACT_PROFILE_V5
            || index >= self.current_rent_quotes
        {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(self.recipes)
                    .checked_mul(RECIPE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|base| {
                usize::from(self.seeds)
                    .checked_mul(SEED_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.plans)
                    .checked_mul(ACTION_PLAN_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.protected_outputs)
                    .checked_mul(PROTECTED_OUTPUT_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(self.immutable_identity_bindings)
                    .checked_mul(IMMUTABLE_IDENTITY_BINDING_BYTES)
                    .and_then(|width| base.checked_add(width))
            })
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(CURRENT_RENT_QUOTE_BYTES_V5)
                    .and_then(|width| base.checked_add(width))
            })
            .ok_or(Error::InvalidLength)?;
        let declaration = LifecycleCurrentRentQuoteV5 {
            exact_data_len: read_u32(self.bytes, offset)?,
            scalar_destination: read_u16(self.bytes, offset + 4)?,
        };
        require_zero(self.bytes, offset + 6, 10)?;
        if declaration.exact_data_len == 0 {
            return Err(Error::InvalidRentQuote);
        }
        Ok(declaration)
    }

    fn validate_seed_against_profile(
        self,
        seed: SeedOperationV3<'_>,
        profile: AccountProfileV2<'_>,
    ) -> Result<()> {
        match seed.source {
            SeedSourceV3::Literal | SeedSourceV3::ItemIndex => Ok(()),
            SeedSourceV3::CommonIdentity => {
                if seed.index < profile.common_identity_count() {
                    Ok(())
                } else {
                    Err(Error::ProfileMismatch)
                }
            }
            SeedSourceV3::ItemIdentity => {
                if seed.index < profile.item_identity_stride() {
                    Ok(())
                } else {
                    Err(Error::ProfileMismatch)
                }
            }
            SeedSourceV3::CommonScalar => {
                if seed.index < profile.common_scalar_count() {
                    Ok(())
                } else {
                    Err(Error::ProfileMismatch)
                }
            }
            SeedSourceV3::ItemScalar => {
                if seed.index < profile.item_scalar_stride() {
                    Ok(())
                } else {
                    Err(Error::ProfileMismatch)
                }
            }
            SeedSourceV3::CanonicalBump => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeedSourceV3 {
    Literal,
    CommonIdentity,
    ItemIdentity,
    CommonScalar,
    ItemScalar,
    ItemIndex,
    CanonicalBump,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeedOperationV3<'a> {
    source: SeedSourceV3,
    width: u8,
    index: u16,
    literal: &'a [u8],
}

impl<'a> SeedOperationV3<'a> {
    fn decode(bytes: &'a [u8], offset: usize) -> Result<Self> {
        let source = match read_u8(bytes, offset)? {
            SEED_LITERAL => SeedSourceV3::Literal,
            SEED_COMMON_IDENTITY => SeedSourceV3::CommonIdentity,
            SEED_ITEM_IDENTITY => SeedSourceV3::ItemIdentity,
            SEED_COMMON_SCALAR_LE => SeedSourceV3::CommonScalar,
            SEED_ITEM_SCALAR_LE => SeedSourceV3::ItemScalar,
            SEED_ITEM_INDEX_LE => SeedSourceV3::ItemIndex,
            SEED_CANONICAL_BUMP => SeedSourceV3::CanonicalBump,
            _ => return Err(Error::UnknownTag),
        };
        let width = read_u8(bytes, offset + 1)?;
        let index = read_u16(bytes, offset + 2)?;
        let literal = slice(bytes, offset + 4, 32)?;
        require_zero(bytes, offset + 36, 4)?;
        match source {
            SeedSourceV3::Literal => {
                if width == 0 || width > MAX_SEED_BYTES || index != 0 {
                    return Err(Error::InvalidSeed);
                }
                require_zero(
                    bytes,
                    offset + 4 + usize::from(width),
                    32 - usize::from(width),
                )?;
            }
            SeedSourceV3::CommonIdentity | SeedSourceV3::ItemIdentity => {
                if width != 32 || literal != [0; 32] {
                    return Err(Error::InvalidSeed);
                }
            }
            SeedSourceV3::CommonScalar | SeedSourceV3::ItemScalar => {
                if !matches!(width, 1 | 2 | 4 | 8) || literal != [0; 32] {
                    return Err(Error::InvalidSeed);
                }
            }
            SeedSourceV3::ItemIndex => {
                if !matches!(width, 1 | 2 | 4) || index != 0 || literal != [0; 32] {
                    return Err(Error::InvalidSeed);
                }
            }
            SeedSourceV3::CanonicalBump => {
                if width != 1 || index != 0 || literal != [0; 32] {
                    return Err(Error::InvalidSeed);
                }
            }
        }
        Ok(Self {
            source,
            width,
            index,
            literal,
        })
    }

    fn materialize(
        self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        item_index: Option<u32>,
        registers: LifecycleRegistersV3<'_>,
    ) -> Result<SeedValueV3> {
        let mut bytes = [0_u8; 32];
        match self.source {
            SeedSourceV3::Literal => {
                bytes
                    .get_mut(..usize::from(self.width))
                    .ok_or(Error::InvalidSeed)?
                    .copy_from_slice(
                        self.literal
                            .get(..usize::from(self.width))
                            .ok_or(Error::InvalidSeed)?,
                    );
            }
            SeedSourceV3::CommonIdentity | SeedSourceV3::ItemIdentity => {
                let source = identity_register(
                    profile,
                    tail_count,
                    item_index,
                    registers,
                    RegisterSourceV3 {
                        item: self.source == SeedSourceV3::ItemIdentity,
                        index: self.index,
                    },
                )?;
                bytes.copy_from_slice(&source);
            }
            SeedSourceV3::CommonScalar | SeedSourceV3::ItemScalar => {
                let value = scalar_register(
                    profile,
                    tail_count,
                    item_index,
                    registers,
                    RegisterSourceV3 {
                        item: self.source == SeedSourceV3::ItemScalar,
                        index: self.index,
                    },
                )?;
                encode_scalar(value, self.width, &mut bytes)?;
            }
            SeedSourceV3::ItemIndex => {
                let value = item_index.ok_or(Error::RuntimeWidth)?;
                encode_scalar(u64::from(value), self.width, &mut bytes)?;
            }
            SeedSourceV3::CanonicalBump => return Err(Error::InvalidSeed),
        }
        Ok(SeedValueV3 {
            bytes,
            length: self.width,
        })
    }
}

/// One owned, bounded seed slice returned to the PDA adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedValueV3 {
    bytes: [u8; 32],
    length: u8,
}

/// One policy seed input before the adapter derives the canonical bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleSeedInputValueV3 {
    /// Exact materialized non-bump seed bytes.
    Bytes(SeedValueV3),
    /// Sole final bump slot derived by the trusted PDA adapter.
    CanonicalBump,
}

impl SeedValueV3 {
    /// Exact seed bytes; never longer than 32.
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.length)).unwrap_or(&[])
    }

    /// Exact encoded seed width.
    pub const fn len(self) -> u8 {
        self.length
    }

    /// Whether this canonical seed is empty; always false for accepted policy bytes.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

/// Flat AccountProfile-projected runtime register banks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRegistersV3<'a> {
    /// Common prefix followed by every runtime item's scalar stride.
    pub scalars: &'a [u64],
    /// Common prefix followed by every runtime item's identity stride.
    pub identities: &'a [[u8; 32]],
}

/// Failure-atomic scratch and output banks for lifecycle-protected values.
pub struct LifecycleProtectedRegisterBuffersV3<'a> {
    /// Scalar scratch with exact runtime width.
    pub scalar_scratch: &'a mut [u64],
    /// Identity scratch with exact runtime width.
    pub identity_scratch: &'a mut [[u8; 32]],
    /// Scalar output committed only after every protected value validates.
    pub output_scalars: &'a mut [u64],
    /// Identity output committed only after every protected value validates.
    pub output_identities: &'a mut [[u8; 32]],
}

/// Adapter-authenticated permanent RentCredit facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRentCreditV3 {
    /// Exact RentCredit account key.
    pub key: [u8; 32],
    /// Immutable beneficiary decoded from the canonical RentCredit state.
    pub beneficiary: [u8; 32],
    /// Observed balance before this lifecycle operation.
    pub lamports: u64,
}

/// Adapter-authenticated current Rent minimum for one exact data width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRentMinimumV3 {
    /// Exact target data width passed to the current Rent sysvar calculation.
    pub data_bytes: u32,
    /// Current exact rent-exempt minimum for that width.
    pub lamports: u64,
}

/// Adapter-authenticated current Rent result for one V5 declaration.
///
/// The separately named runtime adapter obtains `current_minimum` from the
/// authenticated current Rent sysvar using exactly `exact_data_len`. Neither a
/// family request nor a projected account is a quote source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRentQuoteV5 {
    /// Exact declaration width passed to the current Rent calculation.
    pub exact_data_len: u32,
    /// Exact protected common scalar destination, repeated to prove ordering.
    pub scalar_destination: u16,
    /// Current exact rent-exempt minimum returned by the adapter.
    pub current_minimum: u64,
}

/// Failure-atomic scalar buffers for V5 current-Rent quote projection.
pub struct LifecycleRentQuoteBuffersV5<'a> {
    /// Full runtime scalar scratch bank.
    pub scalar_scratch: &'a mut [u64],
    /// Full runtime scalar output committed only after every quote validates.
    pub output_scalars: &'a mut [u64],
}

/// Runtime context supplied only after AccountProfile projection succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleContextV3<'a> {
    /// Sole AccountProfile owning account/register geometry.
    pub account_profile: AccountProfileV2<'a>,
    /// Product-authenticated runtime item count.
    pub tail_count: u32,
    /// Current item for an item-scoped plan; absent for fixed plans.
    pub item_index: Option<u32>,
    /// Exact expanded AccountProfile observations.
    pub accounts: &'a [AccountObservationV1<'a>],
    /// Exact projected output registers.
    pub registers: LifecycleRegistersV3<'a>,
    /// Current immutable Trading program identity.
    pub trading_program: [u8; 32],
    /// Current native System program identity.
    pub system_program: [u8; 32],
    /// PDA independently derived by the adapter from this policy's seeds.
    pub adapter_derived_pda: [u8; 32],
    /// Authenticated RentCredit facts for Create/Close; absent for Authenticate.
    pub rent_credit: Option<AuthenticatedRentCreditV3>,
    /// Current Rent result for Create only; absent for Authenticate/Close.
    pub current_rent_minimum: Option<AuthenticatedRentMinimumV3>,
}

/// Existing-state authentication result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticateStatePlanV3 {
    /// Exact Trading-owned PDA.
    pub state: [u8; 32],
    /// Exact authenticated data width.
    pub data_bytes: u32,
    /// Observed state balance, unchanged by authentication.
    pub lamports: u64,
    /// Exact PDA bump encoded by the policy's final seed.
    pub bump: u8,
}

/// Dust-tolerant state creation projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateStatePlanV3 {
    /// Exact vacant PDA to allocate and assign.
    pub state: [u8; 32],
    /// Exact payer account.
    pub payer: [u8; 32],
    /// Exact permanent RentCredit account authenticating the beneficiary.
    pub rent_credit: [u8; 32],
    /// Immutable beneficiary decoded from RentCredit and projected by AccountProfile.
    pub beneficiary: [u8; 32],
    /// Exact affine target data width.
    pub target_data_bytes: u32,
    /// Historical rent principal persisted by the created state.
    pub historical_rent_principal: u64,
    /// Harmless pre-existing System-owned dust.
    pub state_before: u64,
    /// Exact projected state balance: `max(dust, historical_rent_principal)`.
    pub state_after: u64,
    /// Exact payer top-up after dust.
    pub payer_debit: u64,
    /// Exact projected payer balance.
    pub payer_after: u64,
    /// Exact PDA bump encoded by the policy's final seed.
    pub bump: u8,
}

/// Full-balance state close projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseStatePlanV3 {
    /// Exact Trading-owned PDA to zero and close.
    pub state: [u8; 32],
    /// Exact permanent RentCredit destination.
    pub rent_credit: [u8; 32],
    /// Immutable authenticated beneficiary.
    pub beneficiary: [u8; 32],
    /// Exact state bytes the adapter must zero before resize/assign.
    pub source_data_bytes: u32,
    /// Persisted historical rent principal, not a fee or bounty.
    pub historical_rent_principal: u64,
    /// Entire observed state balance, including unsolicited donations.
    pub source_before: u64,
    /// Exact post-close source balance; always zero.
    pub source_after: u64,
    /// RentCredit balance before the close.
    pub rent_credit_before: u64,
    /// RentCredit balance after receiving the entire source balance.
    pub rent_credit_after: u64,
    /// Exact PDA bump encoded by the policy's final seed.
    pub bump: u8,
}

/// Typed lifecycle plan returned only after complete authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateLifecyclePlanV3 {
    /// Existing-state authentication only.
    Authenticate(AuthenticateStatePlanV3),
    /// System allocate/assign and payer-transfer projection.
    Create(CreateStatePlanV3),
    /// Zero/resize/assign and full RentCredit projection.
    Close(CloseStatePlanV3),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecycleProtectedValuesV3 {
    created: u64,
    bump: u64,
    historical_rent_principal: u64,
    beneficiary: [u8; 32],
    state: [u8; 32],
    owner: [u8; 32],
}

impl StateLifecyclePlanV3 {
    /// Effective state-data width after applying this plan.
    ///
    /// The outer uses this width for candidate EffectProgram writes before any
    /// CPI. Close returns zero; Authenticate returns the existing exact width.
    pub const fn effective_state_data_bytes(self) -> u32 {
        match self {
            Self::Authenticate(plan) => plan.data_bytes,
            Self::Create(plan) => plan.target_data_bytes,
            Self::Close(_) => 0,
        }
    }

    /// Exact selected Trading-owned state address.
    pub const fn state(self) -> [u8; 32] {
        match self {
            Self::Authenticate(plan) => plan.state,
            Self::Create(plan) => plan.state,
            Self::Close(plan) => plan.state,
        }
    }
}

/// Evidence that one exact policy's join to one exact AccountProfile has
/// already been validated.
///
/// `validate_account_profile` is a pure function of the two artifacts' bytes
/// and neither can change during an execution, yet the planner re-derived it
/// for every invocation of a batch: on the canonical Direct Profile14 lifecycle
/// that is the same 82,000-CU join recomputed once per planned state, twice
/// over, because the executor plans the batch and then replans it against the
/// transition's outputs.
///
/// The token records the two byte ranges it was proved for. The planner accepts
/// it only for the very same ranges -- same address, same length, and both are
/// immutable borrows for the whole execution -- so it can never carry a
/// validation of one artifact pair into a plan over another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedProfileJoinV3<'a> {
    policy: &'a [u8],
    profile: &'a [u8],
}

impl ValidatedProfileJoinV3<'_> {
    fn covers(self, policy: StateLifecyclePolicyV3<'_>, profile: AccountProfileV2<'_>) -> bool {
        let policy_bytes = policy.bytes();
        let profile_bytes = profile.bytes();
        core::ptr::eq(self.policy.as_ptr(), policy_bytes.as_ptr())
            && self.policy.len() == policy_bytes.len()
            && core::ptr::eq(self.profile.as_ptr(), profile_bytes.as_ptr())
            && self.profile.len() == profile_bytes.len()
    }
}

/// Authenticate and plan one exact selected lifecycle operation.
pub fn plan_lifecycle(
    selected: SelectedLifecycleV3<'_>,
    context: LifecycleContextV3<'_>,
) -> Result<StateLifecyclePlanV3> {
    Ok(plan_lifecycle_with_values(selected, context, None)?.0)
}

/// Plan lifecycle and seed its declared protected outputs atomically.
///
/// AccountProfile owns persisted observations. Lifecycle derives or
/// authenticates the branch flag, canonical bump, historical rent principal,
/// immutable beneficiary, state key, and Trading owner. Callers cannot supply
/// those values through request or transition registers.
pub fn plan_lifecycle_with_protected_outputs_atomic(
    selected: SelectedLifecycleV3<'_>,
    context: LifecycleContextV3<'_>,
    adapter_derived_bump: u8,
    buffers: LifecycleProtectedRegisterBuffersV3<'_>,
) -> Result<StateLifecyclePlanV3> {
    validate_runtime_width(
        context.account_profile,
        context.tail_count,
        context.registers,
    )?;
    if buffers.scalar_scratch.len() != context.registers.scalars.len()
        || buffers.identity_scratch.len() != context.registers.identities.len()
        || buffers.output_scalars.len() != context.registers.scalars.len()
        || buffers.output_identities.len() != context.registers.identities.len()
    {
        return Err(Error::RuntimeWidth);
    }
    buffers
        .scalar_scratch
        .copy_from_slice(context.registers.scalars);
    buffers
        .identity_scratch
        .copy_from_slice(context.registers.identities);
    let (plan, protected) =
        plan_lifecycle_with_values(selected, context, Some(adapter_derived_bump))?;
    if let Some((declaration, values)) = protected {
        write_lifecycle_scalar(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.scalar_scratch,
            declaration.created(),
            values.created,
        )?;
        write_lifecycle_scalar(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.scalar_scratch,
            declaration.bump(),
            values.bump,
        )?;
        write_lifecycle_scalar(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.scalar_scratch,
            declaration.historical_rent_principal(),
            values.historical_rent_principal,
        )?;
        write_lifecycle_identity(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.identity_scratch,
            declaration.beneficiary(),
            values.beneficiary,
        )?;
        write_lifecycle_identity(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.identity_scratch,
            declaration.state(),
            values.state,
        )?;
        write_lifecycle_identity(
            context.account_profile,
            context.tail_count,
            context.item_index,
            buffers.identity_scratch,
            declaration.owner(),
            values.owner,
        )?;
    }
    buffers
        .output_scalars
        .copy_from_slice(buffers.scalar_scratch);
    buffers
        .output_identities
        .copy_from_slice(buffers.identity_scratch);
    Ok(plan)
}

fn plan_lifecycle_with_values(
    selected: SelectedLifecycleV3<'_>,
    context: LifecycleContextV3<'_>,
    adapter_derived_bump: Option<u8>,
) -> Result<(
    StateLifecyclePlanV3,
    Option<(LifecycleProtectedOutputsV3, LifecycleProtectedValuesV3)>,
)> {
    let selected_record = selected;
    let policy = selected.policy;
    let protected = selected.protected_outputs()?;
    if !selected.is_enabled(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
    )? {
        return Err(Error::PlanDisabled);
    }
    let data_bytes = selected.target_data_bytes(context.tail_count)?;
    let selected = selected.plan;
    policy.require_join(selected_record.join, context.account_profile)?;
    validate_runtime_width(
        context.account_profile,
        context.tail_count,
        context.registers,
    )?;
    // The System Program's canonical address IS the all-zero pubkey, so an
    // all-zero `system_program` is the real identity and never an unset field.
    // Refusing it refused every lifecycle plan any adapter has ever submitted:
    // the sole live caller supplies `solana_sdk_ids::system_program::ID`, and
    // no test caught it because the fixtures below all substitute a made-up
    // non-zero address. The two fields that can meaningfully be defaulted are
    // still refused, as is an executing program that claims to be the builtin.
    if context.trading_program == [0; 32]
        || context.trading_program == context.system_program
        || context.adapter_derived_pda == [0; 32]
    {
        return Err(Error::IdentityMismatch);
    }
    let recipe = policy.recipe(selected.recipe)?;
    validate_item(recipe.account.scope, context.tail_count, context.item_index)?;
    let state_index = expanded_account_index(
        context.account_profile,
        context.tail_count,
        context.item_index,
        recipe.account,
    )?;
    if context.accounts.len() != account_width(context.account_profile, context.tail_count)? {
        return Err(Error::RuntimeWidth);
    }
    let state = *context
        .accounts
        .get(state_index)
        .ok_or(Error::RuntimeWidth)?;
    if *state.key != context.adapter_derived_pda || !state.writable || state.executable {
        return Err(Error::IdentityMismatch);
    }
    let bump_operation = policy.seed(
        recipe
            .seed_start
            .checked_add(u16::from(recipe.bump_offset))
            .ok_or(Error::InvalidSeed)?,
    )?;
    let bump = if bump_operation.source == SeedSourceV3::CanonicalBump {
        adapter_derived_bump.ok_or(Error::InvalidSeed)?
    } else {
        let bump_seed = policy.materialize_seed_for(
            context.account_profile,
            selected,
            context.tail_count,
            context.item_index,
            context.registers,
            recipe.bump_offset,
        )?;
        *bump_seed.as_slice().first().ok_or(Error::InvalidSeed)?
    };

    let plan = match selected.operation {
        LifecycleOperationV3::Authenticate => {
            if context.rent_credit.is_some()
                || context.current_rent_minimum.is_some()
                || *state.owner != context.trading_program
                || state.data.len() != usize::try_from(data_bytes).map_err(|_| Error::Arithmetic)?
            {
                return Err(Error::InvalidState);
            }
            StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                state: *state.key,
                data_bytes,
                lamports: state.lamports,
                bump,
            })
        }
        LifecycleOperationV3::Create => {
            plan_create(selected, recipe, state, data_bytes, bump, context, false)?
        }
        LifecycleOperationV3::Close => {
            plan_close(selected, recipe, state, data_bytes, bump, context)?
        }
        LifecycleOperationV3::AuthenticateOrCreate => {
            if *state.owner == context.trading_program
                && state.data.len() == usize::try_from(data_bytes).map_err(|_| Error::Arithmetic)?
            {
                StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                    state: *state.key,
                    data_bytes,
                    lamports: state.lamports,
                    bump,
                })
            } else if *state.owner == context.system_program && state.data.is_empty() {
                plan_create(
                    selected,
                    recipe,
                    state,
                    data_bytes,
                    bump,
                    context,
                    protected.is_some(),
                )?
            } else {
                return Err(Error::InvalidState);
            }
        }
    };
    validate_immutable_identity_bindings(selected_record, state, context, plan)?;
    let protected = protected
        .map(|declaration| {
            validate_protected_values(
                declaration,
                selected,
                state,
                data_bytes,
                bump,
                context,
                plan,
            )
            .map(|values| (declaration, values))
        })
        .transpose()?;
    Ok((plan, protected))
}

fn validate_immutable_identity_bindings(
    selected: SelectedLifecycleV3<'_>,
    state: AccountObservationV1<'_>,
    context: LifecycleContextV3<'_>,
    plan: StateLifecyclePlanV3,
) -> Result<()> {
    let count = selected.immutable_identity_binding_count()?;
    let mut ordinal = 0_u16;
    while ordinal < count {
        let binding = selected.immutable_identity_binding(ordinal)?;
        let canonical = identity_register(
            context.account_profile,
            context.tail_count,
            context.item_index,
            context.registers,
            register_source(binding.canonical),
        )?;
        if canonical == [0; 32] {
            return Err(Error::IdentityMismatch);
        }
        match plan {
            StateLifecyclePlanV3::Authenticate(_) => {
                let start = usize::try_from(binding.data_offset).map_err(|_| Error::Arithmetic)?;
                let end = start.checked_add(32).ok_or(Error::Arithmetic)?;
                if state.data.get(start..end) != Some(canonical.as_slice()) {
                    return Err(Error::IdentityMismatch);
                }
            }
            StateLifecyclePlanV3::Create(_) => {
                if !state.data.is_empty() {
                    return Err(Error::InvalidState);
                }
            }
            StateLifecyclePlanV3::Close(_) => return Err(Error::InvalidState),
        }
        ordinal = ordinal.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    Ok(())
}

fn plan_create(
    selected: ActionPlanV3,
    recipe: RecipeV3,
    state: AccountObservationV1<'_>,
    data_bytes: u32,
    bump: u8,
    context: LifecycleContextV3<'_>,
    lifecycle_owns_values: bool,
) -> Result<StateLifecyclePlanV3> {
    if *state.owner != context.system_program || !state.data.is_empty() {
        return Err(Error::InvalidState);
    }
    let payer_coordinate = selected.payer.ok_or(Error::InvalidFunding)?;
    let credit_coordinate = selected.rent_credit.ok_or(Error::InvalidFunding)?;
    let payer = account_at(context, payer_coordinate)?;
    let credit_account = account_at(context, credit_coordinate)?;
    require_permissions(
        context.account_profile,
        recipe.account,
        EFFECT_PERMISSION_CREDIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA,
    )?;
    require_permissions(
        context.account_profile,
        payer_coordinate,
        EFFECT_PERMISSION_DEBIT_LAMPORTS,
    )?;
    let credit = if lifecycle_owns_values {
        authenticate_credit_account(context, credit_account)?
    } else {
        authenticate_credit(selected, context, credit_account)?
    };
    if payer.key == state.key
        || credit_account.key == state.key
        || payer.key == credit_account.key
        || !payer.signer
        || !payer.writable
        || payer.executable
    {
        return Err(Error::InvalidFunding);
    }
    let current = context.current_rent_minimum.ok_or(Error::InvalidRent)?;
    let principal = if lifecycle_owns_values {
        current.lamports
    } else {
        scalar_register(
            context.account_profile,
            context.tail_count,
            context.item_index,
            context.registers,
            selected.principal.ok_or(Error::InvalidRent)?,
        )?
    };
    if principal == 0 || current.data_bytes != data_bytes || current.lamports != principal {
        return Err(Error::InvalidRent);
    }
    let state_after = core::cmp::max(state.lamports, principal);
    let payer_debit = state_after
        .checked_sub(state.lamports)
        .ok_or(Error::Arithmetic)?;
    let payer_after = payer
        .lamports
        .checked_sub(payer_debit)
        .ok_or(Error::InvalidFunding)?;
    Ok(StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        state: *state.key,
        payer: *payer.key,
        rent_credit: credit.key,
        beneficiary: credit.beneficiary,
        target_data_bytes: data_bytes,
        historical_rent_principal: principal,
        state_before: state.lamports,
        state_after,
        payer_debit,
        payer_after,
        bump,
    }))
}

fn plan_close(
    selected: ActionPlanV3,
    recipe: RecipeV3,
    state: AccountObservationV1<'_>,
    data_bytes: u32,
    bump: u8,
    context: LifecycleContextV3<'_>,
) -> Result<StateLifecyclePlanV3> {
    if *state.owner != context.trading_program
        || context.current_rent_minimum.is_some()
        || state.data.len() != usize::try_from(data_bytes).map_err(|_| Error::Arithmetic)?
    {
        return Err(Error::InvalidState);
    }
    let credit_coordinate = selected.rent_credit.ok_or(Error::InvalidFunding)?;
    let credit_account = account_at(context, credit_coordinate)?;
    require_permissions(
        context.account_profile,
        recipe.account,
        EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA,
    )?;
    require_permissions(
        context.account_profile,
        credit_coordinate,
        EFFECT_PERMISSION_CREDIT_LAMPORTS,
    )?;
    let credit = authenticate_credit(selected, context, credit_account)?;
    if credit_account.key == state.key || !credit_account.writable || credit_account.executable {
        return Err(Error::InvalidFunding);
    }
    let principal = scalar_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.principal.ok_or(Error::InvalidRent)?,
    )?;
    if principal == 0 || state.lamports < principal {
        return Err(Error::InvalidRent);
    }
    let rent_credit_after = credit
        .lamports
        .checked_add(state.lamports)
        .ok_or(Error::Arithmetic)?;
    Ok(StateLifecyclePlanV3::Close(CloseStatePlanV3 {
        state: *state.key,
        rent_credit: credit.key,
        beneficiary: credit.beneficiary,
        source_data_bytes: data_bytes,
        historical_rent_principal: principal,
        source_before: state.lamports,
        source_after: 0,
        rent_credit_before: credit.lamports,
        rent_credit_after,
        bump,
    }))
}

fn authenticate_credit(
    selected: ActionPlanV3,
    context: LifecycleContextV3<'_>,
    account: AccountObservationV1<'_>,
) -> Result<AuthenticatedRentCreditV3> {
    let credit = authenticate_credit_account(context, account)?;
    let beneficiary = identity_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.beneficiary.ok_or(Error::InvalidRent)?,
    )?;
    if credit.beneficiary != beneficiary {
        return Err(Error::InvalidRent);
    }
    Ok(credit)
}

fn authenticate_credit_account(
    context: LifecycleContextV3<'_>,
    account: AccountObservationV1<'_>,
) -> Result<AuthenticatedRentCreditV3> {
    let credit = context.rent_credit.ok_or(Error::InvalidFunding)?;
    if credit.beneficiary == [0; 32]
        || credit.key != *account.key
        || credit.lamports != account.lamports
    {
        return Err(Error::InvalidRent);
    }
    Ok(credit)
}

fn validate_protected_values(
    declaration: LifecycleProtectedOutputsV3,
    selected: ActionPlanV3,
    state: AccountObservationV1<'_>,
    data_bytes: u32,
    bump: u8,
    context: LifecycleContextV3<'_>,
    plan: StateLifecyclePlanV3,
) -> Result<LifecycleProtectedValuesV3> {
    if selected.operation != LifecycleOperationV3::AuthenticateOrCreate {
        return Err(Error::InvalidState);
    }
    let bump_observation = scalar_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        register_source(declaration.bump_observation()),
    )?;
    let principal_observation = scalar_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.principal.ok_or(Error::InvalidRent)?,
    )?;
    let beneficiary_observation = identity_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.beneficiary.ok_or(Error::InvalidRent)?,
    )?;
    let credit_coordinate = selected.rent_credit.ok_or(Error::InvalidFunding)?;
    let credit = authenticate_credit_account(context, account_at(context, credit_coordinate)?)?;
    let current = context.current_rent_minimum.ok_or(Error::InvalidRent)?;
    if current.data_bytes != data_bytes || current.lamports == 0 {
        return Err(Error::InvalidRent);
    }
    let (created, principal, beneficiary) = match plan {
        StateLifecyclePlanV3::Authenticate(value) => {
            if bump_observation != u64::from(value.bump)
                || principal_observation == 0
                || beneficiary_observation != credit.beneficiary
                || state.lamports < principal_observation
                || state.lamports < current.lamports
            {
                return Err(Error::InvalidRent);
            }
            (0, principal_observation, beneficiary_observation)
        }
        StateLifecyclePlanV3::Create(value) => {
            if bump_observation != 0
                || principal_observation != 0
                || beneficiary_observation != [0; 32]
            {
                return Err(Error::InvalidRent);
            }
            (1, value.historical_rent_principal, value.beneficiary)
        }
        StateLifecyclePlanV3::Close(_) => return Err(Error::InvalidState),
    };
    Ok(LifecycleProtectedValuesV3 {
        created,
        bump: u64::from(bump),
        historical_rent_principal: principal,
        beneficiary,
        state: *state.key,
        owner: context.trading_program,
    })
}

fn account_at(
    context: LifecycleContextV3<'_>,
    coordinate: AccountCoordinateV3,
) -> Result<AccountObservationV1<'_>> {
    let index = expanded_account_index(
        context.account_profile,
        context.tail_count,
        context.item_index,
        coordinate,
    )?;
    context
        .accounts
        .get(index)
        .copied()
        .ok_or(Error::RuntimeWidth)
}

fn validate_runtime_width(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    registers: LifecycleRegistersV3<'_>,
) -> Result<()> {
    let scalars = affine_width(
        profile.common_scalar_count(),
        profile.item_scalar_stride(),
        tail_count,
    )?;
    let identities = affine_width(
        profile.common_identity_count(),
        profile.item_identity_stride(),
        tail_count,
    )?;
    if registers.scalars.len() != scalars || registers.identities.len() != identities {
        Err(Error::RuntimeWidth)
    } else {
        Ok(())
    }
}

fn scalar_register(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    item_index: Option<u32>,
    registers: LifecycleRegistersV3<'_>,
    source: RegisterSourceV3,
) -> Result<u64> {
    validate_scalar_source(profile, source)?;
    validate_runtime_width(profile, tail_count, registers)?;
    let index = register_index(
        profile.common_scalar_count(),
        profile.item_scalar_stride(),
        tail_count,
        item_index,
        source,
    )?;
    registers
        .scalars
        .get(index)
        .copied()
        .ok_or(Error::RuntimeWidth)
}

fn identity_register(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    item_index: Option<u32>,
    registers: LifecycleRegistersV3<'_>,
    source: RegisterSourceV3,
) -> Result<[u8; 32]> {
    validate_identity_source(profile, source)?;
    validate_runtime_width(profile, tail_count, registers)?;
    let index = register_index(
        profile.common_identity_count(),
        profile.item_identity_stride(),
        tail_count,
        item_index,
        source,
    )?;
    registers
        .identities
        .get(index)
        .copied()
        .ok_or(Error::RuntimeWidth)
}

fn register_index(
    common: u16,
    stride: u16,
    tail_count: u32,
    item_index: Option<u32>,
    source: RegisterSourceV3,
) -> Result<usize> {
    if source.item {
        let item = item_index.ok_or(Error::RuntimeWidth)?;
        if item >= tail_count {
            return Err(Error::RuntimeWidth);
        }
        usize::from(common)
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| Error::RuntimeWidth)?
                    .checked_mul(usize::from(stride))
                    .ok_or(Error::Arithmetic)?,
            )
            .and_then(|value| value.checked_add(usize::from(source.index)))
            .ok_or(Error::Arithmetic)
    } else {
        Ok(usize::from(source.index))
    }
}

fn validate_scalar_source(profile: AccountProfileV2<'_>, source: RegisterSourceV3) -> Result<()> {
    let limit = if source.item {
        profile.item_scalar_stride()
    } else {
        profile.common_scalar_count()
    };
    if source.index < limit {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn validate_identity_source(profile: AccountProfileV2<'_>, source: RegisterSourceV3) -> Result<()> {
    let limit = if source.item {
        profile.item_identity_stride()
    } else {
        profile.common_identity_count()
    };
    if source.index < limit {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn lifecycle_target(
    kind: LifecycleRegisterKindV3,
    source: RegisterSourceV3,
) -> LifecycleRegisterTargetV3 {
    LifecycleRegisterTargetV3 {
        kind,
        scope: if source.item {
            CoordinateScopeV3::Item
        } else {
            CoordinateScopeV3::Fixed
        },
        index: source.index,
    }
}

fn register_source(target: LifecycleRegisterTargetV3) -> RegisterSourceV3 {
    RegisterSourceV3 {
        item: target.scope == CoordinateScopeV3::Item,
        index: target.index,
    }
}

fn protected_output_targets(
    outputs: LifecycleProtectedOutputsV3,
) -> [LifecycleRegisterTargetV3; 6] {
    [
        outputs.created(),
        outputs.bump(),
        outputs.historical_rent_principal(),
        outputs.beneficiary(),
        outputs.state(),
        outputs.owner(),
    ]
}

fn write_lifecycle_scalar(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    item_index: Option<u32>,
    output: &mut [u64],
    target: LifecycleRegisterTargetV3,
    value: u64,
) -> Result<()> {
    if target.kind != LifecycleRegisterKindV3::Scalar {
        return Err(Error::InvalidCoordinate);
    }
    let index = expanded_register_index(
        profile.common_scalar_count(),
        profile.item_scalar_stride(),
        tail_count,
        item_index,
        target,
    )?;
    *output.get_mut(index).ok_or(Error::RuntimeWidth)? = value;
    Ok(())
}

fn write_lifecycle_identity(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    item_index: Option<u32>,
    output: &mut [[u8; 32]],
    target: LifecycleRegisterTargetV3,
    value: [u8; 32],
) -> Result<()> {
    if target.kind != LifecycleRegisterKindV3::Identity {
        return Err(Error::InvalidCoordinate);
    }
    let index = expanded_register_index(
        profile.common_identity_count(),
        profile.item_identity_stride(),
        tail_count,
        item_index,
        target,
    )?;
    *output.get_mut(index).ok_or(Error::RuntimeWidth)? = value;
    Ok(())
}

fn expanded_register_index(
    common: u16,
    item_stride: u16,
    tail_count: u32,
    item_index: Option<u32>,
    target: LifecycleRegisterTargetV3,
) -> Result<usize> {
    match target.scope {
        CoordinateScopeV3::Fixed => {
            if item_index.is_some() && target.index >= common {
                return Err(Error::InvalidCoordinate);
            }
            if target.index >= common {
                return Err(Error::InvalidCoordinate);
            }
            Ok(usize::from(target.index))
        }
        CoordinateScopeV3::Item => {
            let item = item_index.ok_or(Error::RuntimeWidth)?;
            if item >= tail_count || target.index >= item_stride {
                return Err(Error::RuntimeWidth);
            }
            usize::from(common)
                .checked_add(
                    usize::try_from(item)
                        .map_err(|_| Error::Arithmetic)?
                        .checked_mul(usize::from(item_stride))
                        .ok_or(Error::Arithmetic)?,
                )
                .and_then(|base| base.checked_add(usize::from(target.index)))
                .ok_or(Error::Arithmetic)
        }
    }
}

fn profile_target(target: LifecycleRegisterTargetV3) -> ProjectionTargetV2 {
    ProjectionTargetV2 {
        kind: match target.kind {
            LifecycleRegisterKindV3::Scalar => ProjectionRegisterKindV2::Scalar,
            LifecycleRegisterKindV3::Identity => ProjectionRegisterKindV2::Identity,
        },
        space: match target.scope {
            CoordinateScopeV3::Fixed => ProjectionRegisterSpaceV2::Common,
            CoordinateScopeV3::Item => ProjectionRegisterSpaceV2::Item,
        },
        index: target.index,
    }
}

fn validate_lifecycle_target(
    profile: AccountProfileV2<'_>,
    target: LifecycleRegisterTargetV3,
) -> Result<()> {
    let source = RegisterSourceV3 {
        item: target.scope == CoordinateScopeV3::Item,
        index: target.index,
    };
    match target.kind {
        LifecycleRegisterKindV3::Scalar => validate_scalar_source(profile, source),
        LifecycleRegisterKindV3::Identity => validate_identity_source(profile, source),
    }
}

fn validate_account_coordinate(
    profile: AccountProfileV2<'_>,
    coordinate: AccountCoordinateV3,
) -> Result<()> {
    let limit = match coordinate.scope {
        CoordinateScopeV3::Fixed => profile.fixed_account_count(),
        CoordinateScopeV3::Item => profile.item_account_stride(),
    };
    if coordinate.index < limit {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn rule_for_coordinate(
    profile: AccountProfileV2<'_>,
    coordinate: AccountCoordinateV3,
) -> Result<AccountRuleV2> {
    validate_account_coordinate(profile, coordinate)?;
    profile
        .rule(
            coordinate.scope == CoordinateScopeV3::Item,
            coordinate.index,
        )
        .map_err(|_| Error::ProfileMismatch)
}

fn validate_invocation_reference(
    invocation: CoordinateScopeV3,
    reference: CoordinateScopeV3,
) -> Result<()> {
    if reference == CoordinateScopeV3::Fixed || invocation == CoordinateScopeV3::Item {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn validate_invocation_source(
    invocation: CoordinateScopeV3,
    source: RegisterSourceV3,
) -> Result<()> {
    if !source.item || invocation == CoordinateScopeV3::Item {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn require_permissions(
    profile: AccountProfileV2<'_>,
    coordinate: AccountCoordinateV3,
    required: u8,
) -> Result<()> {
    let rule = profile
        .rule(
            coordinate.scope == CoordinateScopeV3::Item,
            coordinate.index,
        )
        .map_err(|_| Error::ProfileMismatch)?;
    if rule.effect_permissions() & required == required {
        Ok(())
    } else {
        Err(Error::ProfileMismatch)
    }
}

fn expanded_account_index(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    item_index: Option<u32>,
    coordinate: AccountCoordinateV3,
) -> Result<usize> {
    validate_account_coordinate(profile, coordinate)?;
    match coordinate.scope {
        CoordinateScopeV3::Fixed => Ok(usize::from(coordinate.index)),
        CoordinateScopeV3::Item => {
            let item = item_index.ok_or(Error::RuntimeWidth)?;
            if item >= tail_count {
                return Err(Error::RuntimeWidth);
            }
            usize::from(profile.fixed_account_count())
                .checked_add(
                    usize::try_from(item)
                        .map_err(|_| Error::RuntimeWidth)?
                        .checked_mul(usize::from(profile.item_account_stride()))
                        .ok_or(Error::Arithmetic)?,
                )
                .and_then(|value| value.checked_add(usize::from(coordinate.index)))
                .ok_or(Error::Arithmetic)
        }
    }
}

fn account_width(profile: AccountProfileV2<'_>, tail_count: u32) -> Result<usize> {
    affine_width(
        profile.fixed_account_count(),
        profile.item_account_stride(),
        tail_count,
    )
}

fn affine_width(common: u16, stride: u16, tail_count: u32) -> Result<usize> {
    usize::try_from(tail_count)
        .map_err(|_| Error::RuntimeWidth)?
        .checked_mul(usize::from(stride))
        .and_then(|tail| usize::from(common).checked_add(tail))
        .ok_or(Error::Arithmetic)
}

fn validate_item(scope: CoordinateScopeV3, tail_count: u32, item_index: Option<u32>) -> Result<()> {
    match (scope, item_index) {
        (CoordinateScopeV3::Fixed, None) => Ok(()),
        (CoordinateScopeV3::Item, Some(item)) if item < tail_count => Ok(()),
        _ => Err(Error::RuntimeWidth),
    }
}

fn encode_scalar(value: u64, width: u8, output: &mut [u8; 32]) -> Result<()> {
    let max = match width {
        1 => u64::from(u8::MAX),
        2 => u64::from(u16::MAX),
        4 => u64::from(u32::MAX),
        8 => u64::MAX,
        _ => return Err(Error::InvalidSeed),
    };
    if value > max {
        return Err(Error::InvalidSeed);
    }
    output
        .get_mut(..usize::from(width))
        .ok_or(Error::InvalidSeed)?
        .copy_from_slice(
            value
                .to_le_bytes()
                .get(..usize::from(width))
                .ok_or(Error::InvalidSeed)?,
        );
    Ok(())
}

fn decode_optional_account(bytes: &[u8], offset: usize) -> Result<Option<AccountCoordinateV3>> {
    let scope = read_u8(bytes, offset)?;
    require_zero(bytes, offset + 1, 1)?;
    let index = read_u16(bytes, offset + 2)?;
    if scope == u8::MAX {
        if index == 0 {
            Ok(None)
        } else {
            Err(Error::NonCanonicalReserved)
        }
    } else {
        Ok(Some(AccountCoordinateV3 {
            scope: CoordinateScopeV3::decode(scope)?,
            index,
        }))
    }
}

fn decode_optional_register(bytes: &[u8], offset: usize) -> Result<Option<RegisterSourceV3>> {
    let source = read_u8(bytes, offset)?;
    require_zero(bytes, offset + 1, 1)?;
    let index = read_u16(bytes, offset + 2)?;
    if source == u8::MAX {
        if index == 0 {
            Ok(None)
        } else {
            Err(Error::NonCanonicalReserved)
        }
    } else {
        Ok(Some(RegisterSourceV3 {
            item: match source {
                SOURCE_COMMON => false,
                SOURCE_ITEM => true,
                _ => return Err(Error::UnknownTag),
            },
            index,
        }))
    }
}

const fn operation_tag(operation: LifecycleOperationV3) -> u8 {
    match operation {
        LifecycleOperationV3::Authenticate => PLAN_AUTHENTICATE,
        LifecycleOperationV3::Create => PLAN_CREATE,
        LifecycleOperationV3::Close => PLAN_CLOSE,
        LifecycleOperationV3::AuthenticateOrCreate => PLAN_AUTHENTICATE_OR_CREATE,
    }
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, offset, 8)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use super::*;
    use crate::v2::{
        self, AccountPrestateV2, ProjectionRegistersV2, TrustedBuiltinIdentityV2,
        TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2, IdentityCoordinateV2,
            RegisterGeometryV2, ScalarCoordinateV2,
            encode_account_profile_with_dynamic_fixed_span_v2_atomic,
            encode_account_profile_with_lifecycle_v2_atomic,
        },
        project_atomic, project_dynamic_fixed_spans_atomic,
    };

    const DIRECT_MAKER_DOMAIN: &[u8] = b"dclutch/direct-replay/v2";
    const DIRECT_RECORD_DOMAIN: &[u8] = b"dclutch/direct-intent/v2";
    const POLICY_ID: [u8; 32] = [0x71; 32];
    const TRADING: [u8; 32] = [0x91; 32];
    /// The real System Program address. Every fixture below used a made-up
    /// non-zero identity here, which is why no test ever exercised the guard
    /// that refused the genuine builtin.
    const SYSTEM: [u8; 32] = [0; 32];
    const PRODUCT_DATA: [u8; 4] = 3_u32.to_le_bytes();
    const CLOSED_STATE_DATA: [u8; 148] = [0x55; 148];

    #[test]
    fn lifecycle_bound_prestate_authenticates_live_or_creates_vacant_only() {
        let profile_rules = [
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(false, true, false),
                    effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 152,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::LifecycleBound,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(true, true, false),
                    effect_permissions: AccountEffectPermissionsV2::new(true, false, false),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(false, false, false),
                    effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
        ];
        let profile_operations = [
            AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(1),
                expected: IdentityCoordinateV2::common(1),
            },
            AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(2),
                data_offset: 8,
            },
        ];
        let profile_width = v2::HEADER_BYTES
            + profile_rules.len() * v2::RULE_BYTES
            + profile_operations.len() * v2::OPERATION_BYTES;
        let mut profile_scratch = vec![0_u8; profile_width];
        let mut profile_bytes = vec![0_u8; profile_width];
        encode_account_profile_with_lifecycle_v2_atomic(
            TrustedEnvironmentV2::None,
            &profile_rules,
            &[],
            &profile_operations,
            &[],
            RegisterGeometryV2 {
                common_scalars: 3,
                item_scalar_stride: 0,
                common_identities: 2,
                item_identity_stride: 0,
            },
            &mut profile_scratch,
            &mut profile_bytes,
        )
        .expect("profile");
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile decode");

        let recipes = [encode::LifecycleRecipeInputV3 {
            state: encode::LifecycleAccountCoordinateV3::fixed(0),
            seed_start: 0,
            seed_count: 2,
            bump_offset: 1,
            data_base: 152,
            data_stride: 0,
        }];
        let seeds = [
            encode::LifecycleSeedInputV3::Literal(b"maker-replay"),
            encode::LifecycleSeedInputV3::CommonScalar { index: 0, width: 1 },
        ];
        let plans = [encode::LifecyclePlanInputV3 {
            action: 1,
            operation: encode::LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 0,
            payer: Some(encode::LifecycleAccountCoordinateV3::fixed(1)),
            rent_credit: Some(encode::LifecycleAccountCoordinateV3::fixed(2)),
            principal: Some(encode::LifecycleRegisterCoordinateV3::common(1)),
            beneficiary: Some(encode::LifecycleRegisterCoordinateV3::common(0)),
            guard: encode::LifecycleGuardInputV3::Always,
        }];
        let policy_width = HEADER_BYTES
            + recipes.len() * RECIPE_BYTES
            + seeds.len() * SEED_BYTES
            + plans.len() * ACTION_PLAN_BYTES;
        let mut policy_scratch = vec![0_u8; policy_width];
        let mut policy_bytes = vec![0_u8; policy_width];
        encode::encode_lifecycle_policy_v3_atomic(
            &recipes,
            &seeds,
            &plans,
            &mut policy_scratch,
            &mut policy_bytes,
        )
        .expect("policy");
        let policy = StateLifecyclePolicyV3::decode(&policy_bytes).expect("policy decode");
        policy
            .validate_account_profile(profile)
            .expect("profile-policy join");
        let unbound_plans = [encode::LifecyclePlanInputV3 {
            action: 1,
            operation: encode::LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            guard: encode::LifecycleGuardInputV3::Always,
        }];
        let mut unbound_scratch = vec![0_u8; policy_width];
        let mut unbound_bytes = vec![0_u8; policy_width];
        encode::encode_lifecycle_policy_v3_atomic(
            &recipes,
            &seeds,
            &unbound_plans,
            &mut unbound_scratch,
            &mut unbound_bytes,
        )
        .expect("unbound policy bytes");
        assert_eq!(
            StateLifecyclePolicyV3::decode(&unbound_bytes)
                .expect("unbound decode")
                .validate_account_profile(profile),
            Err(Error::ProfileMismatch)
        );
        let selected = policy.action_plan(1, 0).expect("selected");
        assert_eq!(selected.uses_canonical_bump(), Ok(false));

        let state = [9_u8; 32];
        let payer = [2_u8; 32];
        let payer_owner = [6_u8; 32];
        let credit = [3_u8; 32];
        let beneficiary = [4_u8; 32];
        let registers = LifecycleRegistersV3 {
            scalars: &[9, 30, 0],
            identities: &[beneficiary, payer_owner],
        };
        let rent_credit = AuthenticatedRentCreditV3 {
            key: credit,
            lamports: 50,
            beneficiary,
        };
        let minimum = AuthenticatedRentMinimumV3 {
            data_bytes: 152,
            lamports: 30,
        };
        let plan = |state_owner: [u8; 32], state_data: &[u8], state_key: [u8; 32]| {
            let accounts = [
                AccountObservationV1::new(
                    &state_key,
                    &state_owner,
                    5,
                    state_data,
                    false,
                    true,
                    false,
                ),
                AccountObservationV1::new(&payer, &payer_owner, 100, &[], true, true, false),
                AccountObservationV1::new(&credit, &[7; 32], 50, &[], false, false, false),
            ];
            plan_lifecycle(
                selected,
                LifecycleContextV3 {
                    account_profile: profile,
                    tail_count: 0,
                    item_index: None,
                    accounts: &accounts,
                    registers,
                    trading_program: TRADING,
                    system_program: SYSTEM,
                    adapter_derived_pda: state,
                    rent_credit: Some(rent_credit),
                    current_rent_minimum: Some(minimum),
                },
            )
        };
        assert!(matches!(
            plan(SYSTEM, &[], state),
            Ok(StateLifecyclePlanV3::Create(CreateStatePlanV3 {
                target_data_bytes: 152,
                state_before: 5,
                state_after: 30,
                payer_debit: 25,
                payer_after: 75,
                ..
            }))
        ));
        let live = [0_u8; 152];
        assert!(matches!(
            plan(TRADING, &live, state),
            Ok(StateLifecyclePlanV3::Authenticate(
                AuthenticateStatePlanV3 {
                    data_bytes: 152,
                    ..
                }
            ))
        ));
        assert_eq!(plan([0x44; 32], &[], state), Err(Error::InvalidState));
        assert_eq!(plan(TRADING, &live[..151], state), Err(Error::InvalidState));
        assert_eq!(plan(SYSTEM, &[], [0x55; 32]), Err(Error::IdentityMismatch));

        // The `Create` acceptance above already runs against the genuine
        // all-zero System Program address, which is the identity every live
        // adapter supplies. What the guard can still establish is refused: a
        // defaulted executing program, a defaulted derived address, and an
        // executing program that claims to be the builtin.
        let identities = |trading: [u8; 32], system: [u8; 32], derived: [u8; 32]| {
            let accounts = [
                AccountObservationV1::new(&state, &SYSTEM, 5, &[], false, true, false),
                AccountObservationV1::new(&payer, &payer_owner, 100, &[], true, true, false),
                AccountObservationV1::new(&credit, &[7; 32], 50, &[], false, false, false),
            ];
            plan_lifecycle(
                selected,
                LifecycleContextV3 {
                    account_profile: profile,
                    tail_count: 0,
                    item_index: None,
                    accounts: &accounts,
                    registers,
                    trading_program: trading,
                    system_program: system,
                    adapter_derived_pda: derived,
                    rent_credit: Some(rent_credit),
                    current_rent_minimum: Some(minimum),
                },
            )
            .map(|_| ())
        };
        assert_eq!(identities(TRADING, SYSTEM, state), Ok(()));
        assert_eq!(
            identities([0; 32], SYSTEM, state),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(
            identities(TRADING, SYSTEM, [0; 32]),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(
            identities(TRADING, TRADING, state),
            Err(Error::IdentityMismatch)
        );

        let observations = [AccountObservationV1::new(
            &state,
            &SYSTEM,
            5,
            &[],
            false,
            true,
            false,
        )];
        let mut scalar_scratch = [0_u64; 3];
        let mut scalar_output = [99_u64; 3];
        let mut identity_scratch = [[0_u8; 32]; 2];
        let mut identity_output = [[9_u8; 32]; 2];
        assert_eq!(
            project_atomic(
                profile,
                0,
                &observations,
                ProjectionRegistersV2 {
                    input_scalars: &[9, 30, 88],
                    input_identities: &[beneficiary, payer_owner],
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: &mut scalar_output,
                    output_identities: &mut identity_output,
                },
            ),
            Err(v2::Error::WidthMismatch)
        );
        assert_eq!(scalar_output, [99; 3]);
        assert_eq!(identity_output, [[9; 32]; 2]);
    }

    #[test]
    fn protected_authenticate_or_create_derives_both_branches_atomically() {
        let rules = [
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(false, true, false),
                    effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 152,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::LifecycleBound,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(true, true, false),
                    effect_permissions: AccountEffectPermissionsV2::new(true, false, false),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(false, false, false),
                    effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
        ];
        let operations = [
            AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(1),
                expected: IdentityCoordinateV2::common(4),
            },
            AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 0,
            },
            AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(1),
                data_offset: 8,
            },
            AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(0),
                destination: IdentityCoordinateV2::common(0),
                data_offset: 16,
            },
        ];
        let profile_width = v2::HEADER_BYTES
            + rules.len() * v2::RULE_BYTES
            + operations.len() * v2::OPERATION_BYTES;
        let mut profile_scratch = vec![0; profile_width];
        let mut profile_bytes = vec![0; profile_width];
        encode_account_profile_with_lifecycle_v2_atomic(
            TrustedEnvironmentV2::None,
            &rules,
            &[],
            &operations,
            &[],
            RegisterGeometryV2 {
                common_scalars: 5,
                item_scalar_stride: 0,
                common_identities: 5,
                item_identity_stride: 0,
            },
            &mut profile_scratch,
            &mut profile_bytes,
        )
        .expect("profile");
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile decode");

        let recipes = [encode::LifecycleRecipeInputV3 {
            state: encode::LifecycleAccountCoordinateV3::fixed(0),
            seed_start: 0,
            seed_count: 3,
            bump_offset: 2,
            data_base: 152,
            data_stride: 0,
        }];
        let seeds = [
            encode::LifecycleSeedInputV3::Literal(b"maker-replay"),
            encode::LifecycleSeedInputV3::CommonIdentity(4),
            encode::LifecycleSeedInputV3::CanonicalBump,
        ];
        let plans = [encode::LifecyclePlanInputV3 {
            action: 1,
            operation: encode::LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 0,
            payer: Some(encode::LifecycleAccountCoordinateV3::fixed(1)),
            rent_credit: Some(encode::LifecycleAccountCoordinateV3::fixed(2)),
            principal: Some(encode::LifecycleRegisterCoordinateV3::common(1)),
            beneficiary: Some(encode::LifecycleRegisterCoordinateV3::common(0)),
            guard: encode::LifecycleGuardInputV3::Always,
        }];
        let protected = [Some(encode::LifecycleProtectedOutputsInputV3 {
            created: 2,
            bump_observation: 0,
            bump: 3,
            historical_rent_principal: 4,
            beneficiary: 1,
            state: 2,
            owner: 3,
        })];
        let bindings = [encode::LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 48,
            canonical: encode::LifecycleRegisterCoordinateV3::common(4),
        }];
        let policy_width = HEADER_BYTES
            + RECIPE_BYTES
            + 3 * SEED_BYTES
            + ACTION_PLAN_BYTES
            + PROTECTED_OUTPUT_BYTES
            + IMMUTABLE_IDENTITY_BINDING_BYTES;
        let mut policy_scratch = vec![0; policy_width];
        let mut policy_bytes = vec![0; policy_width];
        encode::encode_lifecycle_policy_v4_atomic(
            &recipes,
            &seeds,
            &plans,
            &protected,
            &bindings,
            &mut policy_scratch,
            &mut policy_bytes,
        )
        .expect("protected policy");
        let policy = StateLifecyclePolicyV4::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("successor policy decode");
        policy
            .validate_account_profile(profile)
            .expect("static profile join");
        let selected = policy.action_plan(1, 0).expect("selected");
        assert_eq!(selected.uses_canonical_bump(), Ok(true));
        assert_eq!(selected.protected_observation_count(), Ok(3));
        assert_eq!(selected.protected_output_count(), Ok(6));
        assert_eq!(selected.immutable_identity_binding_count(), Ok(1));
        assert_eq!(
            selected.immutable_identity_binding(0),
            Ok(LifecycleImmutableIdentityBindingV4 {
                data_offset: 48,
                canonical: LifecycleRegisterTargetV3 {
                    kind: LifecycleRegisterKindV3::Identity,
                    scope: CoordinateScopeV3::Fixed,
                    index: 4,
                },
            })
        );
        assert_eq!(
            selected.seed_register_target(1),
            Ok(Some(LifecycleRegisterTargetV3 {
                kind: LifecycleRegisterKindV3::Identity,
                scope: CoordinateScopeV3::Fixed,
                index: 4,
            }))
        );
        assert_eq!(
            selected.materialize_seed_input(
                profile,
                0,
                None,
                LifecycleRegistersV3 {
                    scalars: &[0; 5],
                    identities: &[[0; 32]; 5],
                },
                2,
            ),
            Ok(LifecycleSeedInputValueV3::CanonicalBump)
        );

        let mut guarded_policy = policy_bytes.clone();
        let plan_offset = HEADER_BYTES + RECIPE_BYTES + 3 * SEED_BYTES;
        *guarded_policy.get_mut(plan_offset + 24).expect("guard tag") = GUARD_SCALAR_EQ;
        guarded_policy
            .get_mut(plan_offset + 28..plan_offset + 36)
            .expect("guard expected")
            .copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            StateLifecyclePolicyV4::decode_selected(POLICY_ID, POLICY_ID, &guarded_policy),
            Err(Error::InvalidFunding)
        );

        let state = [0x41; 32];
        let payer = [0x42; 32];
        let credit = [0x43; 32];
        let payer_owner = [0x51; 32];
        let beneficiary = [0x52; 32];
        let credit_owner = [0x53; 32];
        let rent_credit = AuthenticatedRentCreditV3 {
            key: credit,
            beneficiary,
            lamports: 7,
        };
        let minimum = AuthenticatedRentMinimumV3 {
            data_bytes: 152,
            lamports: 100,
        };
        let exercise = |state_owner: [u8; 32],
                        state_lamports: u64,
                        state_data: &[u8],
                        scalar_output: &mut [u64; 5],
                        identity_output: &mut [[u8; 32]; 5]| {
            let accounts = [
                AccountObservationV1::new(
                    &state,
                    &state_owner,
                    state_lamports,
                    state_data,
                    false,
                    true,
                    false,
                ),
                AccountObservationV1::new(&payer, &payer_owner, 1_000, &[], true, true, false),
                AccountObservationV1::new(&credit, &credit_owner, 7, &[], false, false, false),
            ];
            let mut projected_scalars = [9_u64; 5];
            let mut projected_identities = [[9_u8; 32]; 5];
            let mut projection_scalar_scratch = [0_u64; 5];
            let mut projection_identity_scratch = [[0_u8; 32]; 5];
            let input_identities = [[9; 32], [9; 32], [9; 32], [9; 32], payer_owner];
            project_atomic(
                profile,
                0,
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &[9; 5],
                    input_identities: &input_identities,
                    scratch_scalars: &mut projection_scalar_scratch,
                    scratch_identities: &mut projection_identity_scratch,
                    output_scalars: &mut projected_scalars,
                    output_identities: &mut projected_identities,
                },
            )
            .expect("account projection");
            let mut scalar_scratch = [0_u64; 5];
            let mut identity_scratch = [[0_u8; 32]; 5];
            let join = selected
                .policy
                .validate_account_profile_join(profile)
                .expect("validated join");
            plan_lifecycle_with_protected_outputs_atomic(
                selected.with_validated_join(join),
                LifecycleContextV3 {
                    account_profile: profile,
                    tail_count: 0,
                    item_index: None,
                    accounts: &accounts,
                    registers: LifecycleRegistersV3 {
                        scalars: &projected_scalars,
                        identities: &projected_identities,
                    },
                    trading_program: TRADING,
                    system_program: SYSTEM,
                    adapter_derived_pda: state,
                    rent_credit: Some(rent_credit),
                    current_rent_minimum: Some(minimum),
                },
                242,
                LifecycleProtectedRegisterBuffersV3 {
                    scalar_scratch: &mut scalar_scratch,
                    identity_scratch: &mut identity_scratch,
                    output_scalars: scalar_output,
                    output_identities: identity_output,
                },
            )
        };

        let mut create_scalars = [77; 5];
        let mut create_identities = [[77; 32]; 5];
        assert!(matches!(
            exercise(SYSTEM, 5, &[], &mut create_scalars, &mut create_identities),
            Ok(StateLifecyclePlanV3::Create(_))
        ));
        assert_eq!(create_scalars, [0, 0, 1, 242, 100]);
        assert_eq!(
            create_identities,
            [[0; 32], beneficiary, state, TRADING, payer_owner]
        );

        let mut live = [0_u8; 152];
        live[..8].copy_from_slice(&242_u64.to_le_bytes());
        live[8..16].copy_from_slice(&100_u64.to_le_bytes());
        live[16..48].copy_from_slice(&beneficiary);
        live[48..80].copy_from_slice(&payer_owner);
        let mut authenticate_scalars = [88; 5];
        let mut authenticate_identities = [[88; 32]; 5];
        assert!(matches!(
            exercise(
                TRADING,
                100,
                &live,
                &mut authenticate_scalars,
                &mut authenticate_identities
            ),
            Ok(StateLifecyclePlanV3::Authenticate(_))
        ));
        assert_eq!(authenticate_scalars, [242, 100, 0, 242, 100]);
        assert_eq!(
            authenticate_identities,
            [beneficiary, beneficiary, state, TRADING, payer_owner]
        );

        live[..8].copy_from_slice(&241_u64.to_le_bytes());
        let mut hostile_scalars = [66; 5];
        let hostile_scalars_before = hostile_scalars;
        let mut hostile_identities = [[66; 32]; 5];
        let hostile_identities_before = hostile_identities;
        assert_eq!(
            exercise(
                TRADING,
                100,
                &live,
                &mut hostile_scalars,
                &mut hostile_identities
            ),
            Err(Error::InvalidRent)
        );
        assert_eq!(hostile_scalars, hostile_scalars_before);
        assert_eq!(hostile_identities, hostile_identities_before);

        live[..8].copy_from_slice(&242_u64.to_le_bytes());
        live[48..80].fill(0x99);
        assert_eq!(
            exercise(
                TRADING,
                100,
                &live,
                &mut hostile_scalars,
                &mut hostile_identities
            ),
            Err(Error::IdentityMismatch)
        );
        assert_eq!(hostile_scalars, hostile_scalars_before);
        assert_eq!(hostile_identities, hostile_identities_before);

        let colliding = [Some(encode::LifecycleProtectedOutputsInputV3 {
            bump: 2,
            ..protected[0].expect("protected")
        })];
        let mut colliding_scratch = vec![0; policy_width];
        let mut colliding_bytes = vec![0; policy_width];
        encode::encode_lifecycle_policy_v4_atomic(
            &recipes,
            &seeds,
            &plans,
            &colliding,
            &bindings,
            &mut colliding_scratch,
            &mut colliding_bytes,
        )
        .expect("colliding wire remains hostile-decodable");
        assert_eq!(
            StateLifecyclePolicyV4::decode_selected(POLICY_ID, POLICY_ID, &colliding_bytes)
                .expect("decode successor")
                .validate_account_profile(profile),
            Err(Error::ProfileMismatch)
        );

        let out_of_range = [Some(encode::LifecycleProtectedOutputsInputV3 {
            created: 5,
            ..protected[0].expect("protected")
        })];
        let mut out_of_range_scratch = vec![0; policy_width];
        let mut out_of_range_bytes = vec![0; policy_width];
        encode::encode_lifecycle_policy_v4_atomic(
            &recipes,
            &seeds,
            &plans,
            &out_of_range,
            &bindings,
            &mut out_of_range_scratch,
            &mut out_of_range_bytes,
        )
        .expect("out-of-range wire remains hostile-decodable");
        assert_eq!(
            StateLifecyclePolicyV4::decode_selected(POLICY_ID, POLICY_ID, &out_of_range_bytes)
                .expect("decode successor")
                .validate_account_profile(profile),
            Err(Error::ProfileMismatch)
        );

        let invalid_bindings = [encode::LifecycleImmutableIdentityBindingInputV4 {
            data_offset: 121,
            ..bindings[0]
        }];
        let mut invalid_binding_scratch = vec![0; policy_width];
        let mut invalid_binding_output = vec![55; policy_width];
        let invalid_binding_before = invalid_binding_output.clone();
        assert_eq!(
            encode::encode_lifecycle_policy_v4_atomic(
                &recipes,
                &seeds,
                &plans,
                &protected,
                &invalid_bindings,
                &mut invalid_binding_scratch,
                &mut invalid_binding_output,
            ),
            Err(Error::InvalidCoordinate)
        );
        assert_eq!(invalid_binding_output, invalid_binding_before);
    }

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        let end = offset.checked_add(bytes.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture slice")
            .copy_from_slice(bytes);
    }

    fn account_profile_bytes() -> Vec<u8> {
        let fixed_accounts = 4_u16;
        let item_accounts = 1_u16;
        let rules = usize::from(fixed_accounts) + usize::from(item_accounts);
        let fixed_operations = 4_usize;
        let item_operations = 1_usize;
        let mut output = vec![
            0_u8;
            v2::HEADER_BYTES
                + rules * v2::RULE_BYTES
                + (fixed_operations + item_operations) * v2::OPERATION_BYTES
        ];
        put(&mut output, 0, &v2::MAGIC);
        for (offset, value) in [
            (8, v2::VERSION),
            (10, v2::ARTIFACT_PROFILE),
            (12, fixed_accounts),
            (14, item_accounts),
            (
                16,
                u16::try_from(fixed_operations).expect("fixed operations"),
            ),
            (18, u16::try_from(item_operations).expect("item operations")),
            (20, 3),
            (22, 5),
            (24, 5),
            (26, 1),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        // The independently authenticated Product prefix owns runtime width.
        put(&mut output, v2::HEADER_BYTES + 8, &4_u32.to_le_bytes());
        // State/payer/RentCredit permissions are exact and owner-anchored.
        for (rule, privileges, permissions) in [
            (1_usize, 0x02_u8, 0x07_u8),
            (2, 0x03, EFFECT_PERMISSION_DEBIT_LAMPORTS),
            (3, 0x02, EFFECT_PERMISSION_CREDIT_LAMPORTS),
            (4, 0x02, 0x07),
        ] {
            let offset = v2::HEADER_BYTES + rule * v2::RULE_BYTES;
            *output.get_mut(offset).expect("rule privileges") = privileges;
            *output.get_mut(offset + 1).expect("rule permissions") = permissions;
        }
        let operations = v2::HEADER_BYTES + rules * v2::RULE_BYTES;
        let mut operation = [0_u8; v2::OPERATION_BYTES];
        operation[0] = 8; // Product-owned tail count -> common scalar zero.
        put(&mut output, operations, &operation);
        for (ordinal, account, identity) in [(1_usize, 1_u16, 2_u16), (2, 2, 3), (3, 3, 4)] {
            let mut owner = [0_u8; v2::OPERATION_BYTES];
            owner[0] = 1;
            put(&mut owner, 2, &account.to_le_bytes());
            put(&mut owner, 6, &identity.to_le_bytes());
            put(
                &mut output,
                operations + ordinal * v2::OPERATION_BYTES,
                &owner,
            );
        }
        let mut item_owner = [0_u8; v2::OPERATION_BYTES];
        item_owner[0] = 1;
        item_owner[1] = 1;
        put(&mut item_owner, 6, &2_u16.to_le_bytes());
        put(
            &mut output,
            operations + fixed_operations * v2::OPERATION_BYTES,
            &item_owner,
        );
        output
    }

    fn recipe(
        scope: u8,
        account: u16,
        seed_start: u16,
        seed_count: u8,
        data_base: u32,
        data_stride: u32,
    ) -> [u8; RECIPE_BYTES] {
        let mut output = [0_u8; RECIPE_BYTES];
        output[0] = scope;
        put(&mut output, 2, &account.to_le_bytes());
        put(&mut output, 4, &seed_start.to_le_bytes());
        output[6] = seed_count;
        output[7] = seed_count.checked_sub(1).expect("nonempty seeds");
        put(&mut output, 8, &data_base.to_le_bytes());
        put(&mut output, 12, &data_stride.to_le_bytes());
        output
    }

    fn seed(source: u8, width: u8, index: u16, literal: &[u8]) -> [u8; SEED_BYTES] {
        let mut output = [0_u8; SEED_BYTES];
        output[0] = source;
        output[1] = width;
        put(&mut output, 2, &index.to_le_bytes());
        put(&mut output, 4, literal);
        output
    }

    fn optional_account(output: &mut [u8], offset: usize, value: Option<(u8, u16)>) {
        match value {
            Some((scope, index)) => {
                *output.get_mut(offset).expect("account scope") = scope;
                put(output, offset + 2, &index.to_le_bytes());
            }
            None => *output.get_mut(offset).expect("absent account") = u8::MAX,
        }
    }

    fn optional_register(output: &mut [u8], offset: usize, value: Option<(u8, u16)>) {
        match value {
            Some((source, index)) => {
                *output.get_mut(offset).expect("register source") = source;
                put(output, offset + 2, &index.to_le_bytes());
            }
            None => *output.get_mut(offset).expect("absent register") = u8::MAX,
        }
    }

    fn action_plan(
        action: u32,
        operation: u8,
        recipe: u16,
        payer: Option<(u8, u16)>,
        rent_credit: Option<(u8, u16)>,
        principal: Option<(u8, u16)>,
        beneficiary: Option<(u8, u16)>,
    ) -> [u8; ACTION_PLAN_BYTES] {
        let mut output = [0_u8; ACTION_PLAN_BYTES];
        put(&mut output, 0, &action.to_le_bytes());
        output[4] = operation;
        put(&mut output, 6, &recipe.to_le_bytes());
        optional_account(&mut output, 8, payer);
        optional_account(&mut output, 12, rent_credit);
        optional_register(&mut output, 16, principal);
        optional_register(&mut output, 20, beneficiary);
        output
    }

    fn scalar_guarded(
        mut plan: [u8; ACTION_PLAN_BYTES],
        source: u8,
        index: u16,
        expected: u64,
    ) -> [u8; ACTION_PLAN_BYTES] {
        plan[24] = GUARD_SCALAR_EQ;
        plan[25] = source;
        put(&mut plan, 26, &index.to_le_bytes());
        put(&mut plan, 28, &expected.to_le_bytes());
        plan
    }

    fn policy_bytes() -> Vec<u8> {
        let recipes = [
            recipe(SCOPE_FIXED, 1, 0, 5, 144, 0),
            recipe(SCOPE_ITEM, 0, 5, 7, 100, 16),
        ];
        let seeds = [
            seed(
                SEED_LITERAL,
                u8::try_from(DIRECT_MAKER_DOMAIN.len()).expect("seed width"),
                0,
                DIRECT_MAKER_DOMAIN,
            ),
            seed(SEED_COMMON_IDENTITY, 32, 0, &[]),
            seed(SEED_COMMON_SCALAR_LE, 8, 0, &[]),
            seed(SEED_COMMON_IDENTITY, 32, 1, &[]),
            seed(SEED_COMMON_SCALAR_LE, 1, 2, &[]),
            seed(
                SEED_LITERAL,
                u8::try_from(DIRECT_RECORD_DOMAIN.len()).expect("seed width"),
                0,
                DIRECT_RECORD_DOMAIN,
            ),
            seed(SEED_COMMON_IDENTITY, 32, 0, &[]),
            seed(SEED_COMMON_SCALAR_LE, 8, 0, &[]),
            seed(SEED_ITEM_IDENTITY, 32, 0, &[]),
            seed(SEED_ITEM_INDEX_LE, 4, 0, &[]),
            seed(SEED_ITEM_SCALAR_LE, 8, 1, &[]),
            seed(SEED_ITEM_SCALAR_LE, 1, 2, &[]),
        ];
        let plans = [
            action_plan(1, PLAN_AUTHENTICATE, 0, None, None, None, None),
            action_plan(
                2,
                PLAN_CREATE,
                0,
                Some((SCOPE_FIXED, 2)),
                Some((SCOPE_FIXED, 3)),
                Some((SOURCE_COMMON, 1)),
                Some((SOURCE_COMMON, 1)),
            ),
            action_plan(
                3,
                PLAN_CREATE,
                1,
                Some((SCOPE_FIXED, 2)),
                Some((SCOPE_FIXED, 3)),
                Some((SOURCE_ITEM, 3)),
                Some((SOURCE_COMMON, 1)),
            ),
            action_plan(4, PLAN_AUTHENTICATE, 1, None, None, None, None),
            scalar_guarded(
                action_plan(
                    5,
                    PLAN_CLOSE,
                    1,
                    None,
                    Some((SCOPE_FIXED, 3)),
                    Some((SOURCE_ITEM, 3)),
                    Some((SOURCE_COMMON, 1)),
                ),
                SOURCE_ITEM,
                4,
                1,
            ),
        ];
        let mut output = vec![
            0_u8;
            HEADER_BYTES
                + recipes.len() * RECIPE_BYTES
                + seeds.len() * SEED_BYTES
                + plans.len() * ACTION_PLAN_BYTES
        ];
        put(&mut output, 0, &MAGIC);
        put(&mut output, 8, &VERSION.to_le_bytes());
        put(&mut output, 10, &ARTIFACT_PROFILE.to_le_bytes());
        put(
            &mut output,
            12,
            &u16::try_from(recipes.len()).expect("recipes").to_le_bytes(),
        );
        put(
            &mut output,
            14,
            &u16::try_from(seeds.len()).expect("seeds").to_le_bytes(),
        );
        put(
            &mut output,
            16,
            &u16::try_from(plans.len()).expect("plans").to_le_bytes(),
        );
        let mut offset = HEADER_BYTES;
        for value in recipes {
            put(&mut output, offset, &value);
            offset += RECIPE_BYTES;
        }
        for value in seeds {
            put(&mut output, offset, &value);
            offset += SEED_BYTES;
        }
        for value in plans {
            put(&mut output, offset, &value);
            offset += ACTION_PLAN_BYTES;
        }
        output
    }

    fn registers() -> (Vec<u64>, Vec<[u8; 32]>) {
        (
            vec![
                3, 100, 250, // common: generation/count, rent, maker bump
                0, 10, 240, 80, 0, // item zero
                1, 11, 241, 90, 0, // item one
                2, 12, 242, 100, 1, // item two: terminal flag
            ],
            vec![
                [0x11; 32], [0x22; 32], SYSTEM, SYSTEM, [7; 32], [0x31; 32], [0x32; 32], [0x33; 32],
            ],
        )
    }

    fn create_accounts() -> [AccountObservationV1<'static>; 7] {
        [
            AccountObservationV1::new(&[1; 32], &[7; 32], 1, &PRODUCT_DATA, false, false, false),
            AccountObservationV1::new(&[0x41; 32], &SYSTEM, 5, &[], false, true, false),
            AccountObservationV1::new(&[0x42; 32], &SYSTEM, 1_000, &[], true, true, false),
            AccountObservationV1::new(&[0x43; 32], &[7; 32], 7, &[], false, true, false),
            AccountObservationV1::new(&[0x50; 32], &SYSTEM, 0, &[], false, true, false),
            AccountObservationV1::new(&[0x51; 32], &SYSTEM, 0, &[], false, true, false),
            AccountObservationV1::new(&[0x52; 32], &SYSTEM, 20, &[], false, true, false),
        ]
    }

    /// Evidence that a policy joins one AccountProfile must never stand in for
    /// a plan over a different one.
    ///
    /// The planner skips re-deriving the join only for the exact byte ranges the
    /// evidence was proved against. A token is therefore neither transferable to
    /// another profile -- where the join genuinely fails and must still be
    /// refused -- nor to a byte-identical profile at another address, where the
    /// join is simply re-derived.
    #[test]
    fn validated_join_evidence_never_covers_another_artifact() {
        let policy_bytes = policy_bytes();
        let policy = StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("policy");
        let profile_bytes = account_profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
        let join = policy
            .validate_account_profile_join(profile)
            .expect("join validates");
        assert!(join.covers(policy, profile));

        // A byte-identical profile at another address is not the artifact the
        // evidence names, so the join is re-derived rather than assumed.
        let copied_bytes = account_profile_bytes();
        assert_eq!(copied_bytes, profile_bytes);
        let copied = AccountProfileV2::decode(&copied_bytes).expect("copied profile");
        assert!(!join.covers(policy, copied));

        // A profile this policy does not join is refused, evidence or not.
        let mut narrowed_bytes = account_profile_bytes();
        put(&mut narrowed_bytes, 26, &0_u16.to_le_bytes());
        let narrowed = AccountProfileV2::decode(&narrowed_bytes).expect("narrowed profile");
        assert_eq!(
            policy.validate_account_profile(narrowed),
            Err(Error::ProfileMismatch)
        );
        assert!(!join.covers(policy, narrowed));

        // A second policy carrying identical bytes is likewise not the artifact
        // the evidence names.
        let copied_policy_bytes = policy_bytes.clone();
        let copied_policy =
            StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &copied_policy_bytes)
                .expect("policy");
        assert_eq!(copied_policy.bytes(), policy.bytes());
        assert!(!join.covers(copied_policy, profile));
    }

    #[test]
    fn direct_maker_and_runtime_item_seeds_are_exact() {
        let policy_bytes = policy_bytes();
        let policy = StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("policy");
        let profile_bytes = account_profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
        policy.validate_account_profile(profile).expect("geometry");
        assert_eq!(policy.action_plan_count(3), Ok(1));
        let (scalars, identities) = registers();
        let registers = LifecycleRegistersV3 {
            scalars: &scalars,
            identities: &identities,
        };

        let maker = policy.action_plan(2, 0).expect("maker create");
        assert_eq!(maker.invocation_scope(), Ok(CoordinateScopeV3::Fixed));
        assert_eq!(maker.invocation_count(3), Ok(1));
        assert_eq!(maker.target_data_bytes(3), Ok(144));
        assert_eq!(maker.invocation_item(3, 0), Ok(None));
        assert_eq!(maker.invocation_item(3, 1), Err(Error::InvalidCoordinate));
        assert_eq!(
            maker.project_account_indices(profile, 3, None),
            Ok(LifecycleAccountIndicesV3 {
                state: 1,
                payer: Some(2),
                rent_credit: Some(3),
            })
        );
        assert_eq!(maker.seed_count(), Ok(5));
        assert_eq!(
            maker
                .materialize_seed(profile, 3, None, registers, 0)
                .expect("maker domain")
                .as_slice(),
            DIRECT_MAKER_DOMAIN
        );
        assert_eq!(
            maker
                .materialize_seed(profile, 3, None, registers, 4)
                .expect("maker bump")
                .as_slice(),
            &[250]
        );

        let record = policy.action_plan(3, 0).expect("record create");
        assert_eq!(record.invocation_scope(), Ok(CoordinateScopeV3::Item));
        assert_eq!(record.invocation_count(3), Ok(3));
        assert_eq!(record.target_data_bytes(3), Ok(148));
        assert_eq!(record.target_data_bytes(u32::MAX), Err(Error::Arithmetic));
        assert_eq!(record.invocation_item(3, 2), Ok(Some(2)));
        assert_eq!(
            record.project_account_indices(profile, 3, Some(2)),
            Ok(LifecycleAccountIndicesV3 {
                state: 6,
                payer: Some(2),
                rent_credit: Some(3),
            })
        );
        assert_eq!(record.seed_count(), Ok(7));
        assert_eq!(
            record
                .materialize_seed(profile, 3, Some(2), registers, 3)
                .expect("item identity")
                .as_slice(),
            &[0x33; 32]
        );
        assert_eq!(
            record
                .materialize_seed(profile, 3, Some(2), registers, 4)
                .expect("item index")
                .as_slice(),
            &2_u32.to_le_bytes()
        );
        assert_eq!(
            record
                .materialize_seed(profile, 3, Some(2), registers, 5)
                .expect("nonce")
                .as_slice(),
            &12_u64.to_le_bytes()
        );
        let close = policy.action_plan(5, 0).expect("record close");
        assert_eq!(close.is_enabled(profile, 3, Some(2), registers), Ok(true));
        assert_eq!(close.is_enabled(profile, 3, Some(1), registers), Ok(false));
    }

    #[test]
    fn dust_tolerant_create_projects_exact_affine_width_and_funding() {
        let policy_bytes = policy_bytes();
        let policy = StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("policy");
        let profile_bytes = account_profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
        let (scalars, identities) = registers();
        let accounts = create_accounts();
        let plan = plan_lifecycle(
            policy.action_plan(3, 0).expect("create"),
            LifecycleContextV3 {
                account_profile: profile,
                tail_count: 3,
                item_index: Some(2),
                accounts: &accounts,
                registers: LifecycleRegistersV3 {
                    scalars: &scalars,
                    identities: &identities,
                },
                trading_program: TRADING,
                system_program: SYSTEM,
                adapter_derived_pda: [0x52; 32],
                rent_credit: Some(AuthenticatedRentCreditV3 {
                    key: [0x43; 32],
                    beneficiary: [0x22; 32],
                    lamports: 7,
                }),
                current_rent_minimum: Some(AuthenticatedRentMinimumV3 {
                    data_bytes: 148,
                    lamports: 100,
                }),
            },
        )
        .expect("dust create");
        assert_eq!(
            plan,
            StateLifecyclePlanV3::Create(CreateStatePlanV3 {
                state: [0x52; 32],
                payer: [0x42; 32],
                rent_credit: [0x43; 32],
                beneficiary: [0x22; 32],
                target_data_bytes: 148,
                historical_rent_principal: 100,
                state_before: 20,
                state_after: 100,
                payer_debit: 80,
                payer_after: 920,
                bump: 242,
            })
        );
        assert_eq!(plan.effective_state_data_bytes(), 148);
    }

    #[test]
    fn close_credits_full_balance_not_only_historical_rent() {
        let policy_bytes = policy_bytes();
        let policy = StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("policy");
        let profile_bytes = account_profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
        let (scalars, identities) = registers();
        let mut accounts = create_accounts();
        accounts[6] = AccountObservationV1::new(
            &[0x52; 32],
            &TRADING,
            135,
            &CLOSED_STATE_DATA,
            false,
            true,
            false,
        );
        let plan = plan_lifecycle(
            policy.action_plan(5, 0).expect("close"),
            LifecycleContextV3 {
                account_profile: profile,
                tail_count: 3,
                item_index: Some(2),
                accounts: &accounts,
                registers: LifecycleRegistersV3 {
                    scalars: &scalars,
                    identities: &identities,
                },
                trading_program: TRADING,
                system_program: SYSTEM,
                adapter_derived_pda: [0x52; 32],
                rent_credit: Some(AuthenticatedRentCreditV3 {
                    key: [0x43; 32],
                    beneficiary: [0x22; 32],
                    lamports: 7,
                }),
                current_rent_minimum: None,
            },
        )
        .expect("close");
        assert_eq!(
            plan,
            StateLifecyclePlanV3::Close(CloseStatePlanV3 {
                state: [0x52; 32],
                rent_credit: [0x43; 32],
                beneficiary: [0x22; 32],
                source_data_bytes: 148,
                historical_rent_principal: 100,
                source_before: 135,
                source_after: 0,
                rent_credit_before: 7,
                rent_credit_after: 142,
                bump: 242,
            })
        );
        assert_eq!(plan.effective_state_data_bytes(), 0);
    }

    #[test]
    fn runtime_width_lifecycle_recipe_joins_exact_profile_geometry() {
        let state_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 16,
                data_item_stride: 8,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
        let payer_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(true, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(true, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let readonly_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let opaque_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        };
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 3,
            count_scalar: 1,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 258,
            step: 1,
        }];
        let fixed_rules = [state_rule, payer_rule, readonly_rule];
        let span_rules = [opaque_rule];
        let operations = [
            AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(1),
                expected: IdentityCoordinateV2::common(0),
            },
            AccountOperationInputV2::ProjectTailCountU32 {
                account: AccountCoordinateV2::fixed(2),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 0,
            },
        ];
        let profile_width = v2::DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + v2::DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed_rules.len() + span_rules.len()) * v2::RULE_BYTES
            + operations.len() * v2::OPERATION_BYTES;
        let mut profile_scratch = vec![0_u8; profile_width];
        let mut profile_bytes = vec![0_u8; profile_width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &fixed_rules,
            &span_rules,
            &operations,
            RegisterGeometryV2 {
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut profile_scratch,
            &mut profile_bytes,
        )
        .expect("runtime-width lifecycle Profile13");
        let profile = AccountProfileV2::decode(&profile_bytes).expect("decode Profile13");

        let policy_bytes_for = |data_base, data_stride| {
            let recipes = [encode::LifecycleRecipeInputV3 {
                state: encode::LifecycleAccountCoordinateV3::fixed(0),
                seed_start: 0,
                seed_count: 1,
                bump_offset: 0,
                data_base,
                data_stride,
            }];
            let seeds = [encode::LifecycleSeedInputV3::CanonicalBump];
            let plans = [encode::LifecyclePlanInputV3 {
                action: 1,
                operation: encode::LifecycleOperationInputV3::AuthenticateOrCreate,
                recipe: 0,
                payer: Some(encode::LifecycleAccountCoordinateV3::fixed(1)),
                rent_credit: Some(encode::LifecycleAccountCoordinateV3::fixed(2)),
                principal: Some(encode::LifecycleRegisterCoordinateV3::common(0)),
                beneficiary: Some(encode::LifecycleRegisterCoordinateV3::common(0)),
                guard: encode::LifecycleGuardInputV3::Always,
            }];
            let width = HEADER_BYTES + RECIPE_BYTES + SEED_BYTES + ACTION_PLAN_BYTES;
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0_u8; width];
            encode::encode_lifecycle_policy_v3_atomic(
                &recipes,
                &seeds,
                &plans,
                &mut scratch,
                &mut output,
            )
            .expect("lifecycle policy");
            output
        };
        let policy_bytes = policy_bytes_for(16, 8);
        let policy = StateLifecyclePolicyV3::decode(&policy_bytes).expect("decode lifecycle");
        policy
            .validate_account_profile(profile)
            .expect("exact affine geometry joins");
        let selected = policy.action_plan(1, 0).expect("selected lifecycle");
        assert_eq!(selected.target_data_bytes(3), Ok(40));
        assert_eq!(selected.target_data_bytes(u32::MAX), Err(Error::Arithmetic));

        for hostile in [policy_bytes_for(15, 8), policy_bytes_for(16, 7)] {
            assert_eq!(
                StateLifecyclePolicyV3::decode(&hostile)
                    .expect("hostile policy remains decodable")
                    .validate_account_profile(profile),
                Err(Error::ProfileMismatch)
            );
        }

        let payer_owner = [0x33; 32];
        let opaque_data = [0x55];
        let count_data = 3_u32.to_le_bytes();
        let project = |state_data: &[u8]| {
            let accounts = [
                AccountObservationV1::new(&[0x10; 32], &TRADING, 1, state_data, false, true, false),
                AccountObservationV1::new(&[0x11; 32], &payer_owner, 1, &[], true, true, false),
                AccountObservationV1::new(
                    &[0x12; 32],
                    &[0x44; 32],
                    1,
                    &count_data,
                    false,
                    false,
                    false,
                ),
                AccountObservationV1::new(
                    &[0x13; 32],
                    &[0x55; 32],
                    1,
                    &opaque_data,
                    false,
                    false,
                    false,
                ),
            ];
            let input_scalars = [0_u64, 1];
            let input_identities = [payer_owner];
            let mut scratch_scalars = [0_u64; 2];
            let mut scratch_identities = [[0_u8; 32]; 1];
            let mut output_scalars = [9_u64; 2];
            let mut output_identities = [[9_u8; 32]; 1];
            project_dynamic_fixed_spans_atomic(
                profile,
                3,
                &[1],
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
        };
        assert_eq!(project(&[]), Ok(()));
        assert_eq!(project(&[0_u8; 40]), Ok(()));
        assert_eq!(project(&[0_u8; 39]), Err(v2::Error::DataLengthMismatch));
        assert_eq!(project(&[0_u8; 41]), Err(v2::Error::DataLengthMismatch));
    }

    #[test]
    fn truncation_overflow_and_substitution_refuse() {
        let policy_bytes = policy_bytes();
        let policy = StateLifecyclePolicyV3::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
            .expect("policy");
        let profile_bytes = account_profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
        let (mut scalars, identities) = registers();
        let record = policy.action_plan(3, 0).expect("record");
        *scalars.get_mut(15).expect("item bump") = 256;
        assert_eq!(
            record.materialize_seed(
                profile,
                3,
                Some(2),
                LifecycleRegistersV3 {
                    scalars: &scalars,
                    identities: &identities,
                },
                6,
            ),
            Err(Error::InvalidSeed)
        );

        let accounts = create_accounts();
        let canonical = registers();
        let context = LifecycleContextV3 {
            account_profile: profile,
            tail_count: 3,
            item_index: Some(2),
            accounts: &accounts,
            registers: LifecycleRegistersV3 {
                scalars: &canonical.0,
                identities: &canonical.1,
            },
            trading_program: TRADING,
            system_program: SYSTEM,
            adapter_derived_pda: [0x99; 32],
            rent_credit: Some(AuthenticatedRentCreditV3 {
                key: [0x43; 32],
                beneficiary: [0x22; 32],
                lamports: 7,
            }),
            current_rent_minimum: Some(AuthenticatedRentMinimumV3 {
                data_bytes: 148,
                lamports: 100,
            }),
        };
        assert_eq!(
            plan_lifecycle(record, context),
            Err(Error::IdentityMismatch)
        );
        let mut bad_accounts = create_accounts();
        bad_accounts[6] =
            AccountObservationV1::new(&[0x52; 32], &SYSTEM, 20, &[], false, true, false);
        let bad_credit = LifecycleContextV3 {
            accounts: &bad_accounts,
            adapter_derived_pda: [0x52; 32],
            rent_credit: Some(AuthenticatedRentCreditV3 {
                key: [0x43; 32],
                beneficiary: [0x77; 32],
                lamports: 7,
            }),
            ..context
        };
        assert_eq!(plan_lifecycle(record, bad_credit), Err(Error::InvalidRent));
    }

    #[test]
    fn hostile_bytes_and_content_selection_refuse() {
        let canonical = policy_bytes();
        assert_eq!(
            StateLifecyclePolicyV3::decode_selected([1; 32], [2; 32], &canonical),
            Err(Error::ContentIdentity)
        );
        assert_eq!(
            StateLifecyclePolicyV3::decode(
                canonical
                    .get(..canonical.len() - 1)
                    .expect("truncated policy"),
            ),
            Err(Error::InvalidLength)
        );
        let recipe_bytes = 2 * RECIPE_BYTES;
        let first_literal = HEADER_BYTES + recipe_bytes;
        let mut overlong = canonical.clone();
        *overlong.get_mut(first_literal + 1).expect("literal length") = 33;
        assert_eq!(
            StateLifecyclePolicyV3::decode(&overlong),
            Err(Error::InvalidSeed)
        );
        let mut dirty_literal_tail = canonical.clone();
        *dirty_literal_tail
            .get_mut(first_literal + 4 + DIRECT_MAKER_DOMAIN.len())
            .expect("literal tail") = 1;
        assert_eq!(
            StateLifecyclePolicyV3::decode(&dirty_literal_tail),
            Err(Error::NonCanonicalReserved)
        );
        let mut too_many_seeds = canonical;
        *too_many_seeds
            .get_mut(HEADER_BYTES + 6)
            .expect("recipe seed count") = 17;
        assert_eq!(
            StateLifecyclePolicyV3::decode(&too_many_seeds),
            Err(Error::InvalidSeed)
        );
    }
}
