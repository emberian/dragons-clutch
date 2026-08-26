//! Data-defined Trading-owned PDA derivation and state lifecycle planning.
//!
//! The authenticated policy describes bounded Solana seed slices, but this
//! pure kernel neither hashes nor derives an address.  A separately named
//! adapter materializes the seeds, derives the PDA under the authenticated
//! Trading program, and returns that derived key to [`plan_lifecycle`].

use core::convert::{TryFrom, TryInto};

use super::{
    AccountObservationV1, EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA, v2::AccountProfileV2,
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
/// Canonical schema version.
pub const VERSION: u16 = 3;
/// Canonical physical artifact profile.
pub const ARTIFACT_PROFILE: u16 = 1;
/// Exact header width.
pub const HEADER_BYTES: usize = 40;
/// Exact derivation-recipe width.
pub const RECIPE_BYTES: usize = 16;
/// Exact seed-operation width.
pub const SEED_BYTES: usize = 40;
/// Exact action-plan width.
pub const ACTION_PLAN_BYTES: usize = 40;
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

const PLAN_AUTHENTICATE: u8 = 0;
const PLAN_CREATE: u8 = 1;
const PLAN_CLOSE: u8 = 2;

const SOURCE_COMMON: u8 = 0;
const SOURCE_ITEM: u8 = 1;

const GUARD_ALWAYS: u8 = 0;
const GUARD_SCALAR_EQ: u8 = 1;

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
}

impl LifecycleOperationV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            PLAN_AUTHENTICATE => Ok(Self::Authenticate),
            PLAN_CREATE => Ok(Self::Create),
            PLAN_CLOSE => Ok(Self::Close),
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
        self.policy.validate_account_profile(profile)?;
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
        self.policy.validate_account_profile(profile)?;
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
    recipes: u16,
    seeds: u16,
    plans: u16,
    bytes: &'a [u8],
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
        if read_u16(bytes, 8)? != VERSION || read_u16(bytes, 10)? != ARTIFACT_PROFILE {
            return Err(Error::UnsupportedProfile);
        }
        require_zero(bytes, 18, 22)?;
        let value = Self {
            recipes: read_u16(bytes, 12)?,
            seeds: read_u16(bytes, 14)?,
            plans: read_u16(bytes, 16)?,
            bytes,
        };
        if value.recipes == 0 || value.seeds == 0 || value.plans == 0 {
            return Err(Error::EmptyPolicy);
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
                    return Ok(SelectedLifecycleV3 { policy: self, plan });
                }
                seen = seen.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Err(Error::InvalidCoordinate)
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
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        Ok(())
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
            if !matches!(
                bump.source,
                SeedSourceV3::CommonScalar | SeedSourceV3::ItemScalar
            ) || bump.width != 1
            {
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
            previous = Some(order);
            plan_index = plan_index.checked_add(1).ok_or(Error::Arithmetic)?;
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

/// Authenticate and plan one exact selected lifecycle operation.
pub fn plan_lifecycle(
    selected: SelectedLifecycleV3<'_>,
    context: LifecycleContextV3<'_>,
) -> Result<StateLifecyclePlanV3> {
    let policy = selected.policy;
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
    policy.validate_account_profile(context.account_profile)?;
    validate_runtime_width(
        context.account_profile,
        context.tail_count,
        context.registers,
    )?;
    if context.trading_program == [0; 32]
        || context.system_program == [0; 32]
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
    if state.key != context.adapter_derived_pda || !state.writable || state.executable {
        return Err(Error::IdentityMismatch);
    }
    let bump_seed = policy.materialize_seed_for(
        context.account_profile,
        selected,
        context.tail_count,
        context.item_index,
        context.registers,
        recipe.bump_offset,
    )?;
    let bump = *bump_seed.as_slice().first().ok_or(Error::InvalidSeed)?;

    match selected.operation {
        LifecycleOperationV3::Authenticate => {
            if context.rent_credit.is_some()
                || context.current_rent_minimum.is_some()
                || state.owner != context.trading_program
                || state.data.len() != usize::try_from(data_bytes).map_err(|_| Error::Arithmetic)?
            {
                return Err(Error::InvalidState);
            }
            Ok(StateLifecyclePlanV3::Authenticate(
                AuthenticateStatePlanV3 {
                    state: state.key,
                    data_bytes,
                    lamports: state.lamports,
                    bump,
                },
            ))
        }
        LifecycleOperationV3::Create => {
            plan_create(selected, recipe, state, data_bytes, bump, context)
        }
        LifecycleOperationV3::Close => {
            plan_close(selected, recipe, state, data_bytes, bump, context)
        }
    }
}

fn plan_create(
    selected: ActionPlanV3,
    recipe: RecipeV3,
    state: AccountObservationV1<'_>,
    data_bytes: u32,
    bump: u8,
    context: LifecycleContextV3<'_>,
) -> Result<StateLifecyclePlanV3> {
    if state.owner != context.system_program || !state.data.is_empty() {
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
    let credit = authenticate_credit(selected, context, credit_account)?;
    if payer.key == state.key
        || credit_account.key == state.key
        || payer.key == credit_account.key
        || !payer.signer
        || !payer.writable
        || payer.executable
    {
        return Err(Error::InvalidFunding);
    }
    let principal = scalar_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.principal.ok_or(Error::InvalidRent)?,
    )?;
    if principal == 0 {
        return Err(Error::InvalidRent);
    }
    let current = context.current_rent_minimum.ok_or(Error::InvalidRent)?;
    if current.data_bytes != data_bytes || current.lamports != principal {
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
        state: state.key,
        payer: payer.key,
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
    if state.owner != context.trading_program
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
        state: state.key,
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
    let credit = context.rent_credit.ok_or(Error::InvalidFunding)?;
    let beneficiary = identity_register(
        context.account_profile,
        context.tail_count,
        context.item_index,
        context.registers,
        selected.beneficiary.ok_or(Error::InvalidRent)?,
    )?;
    if beneficiary == [0; 32]
        || credit.key != account.key
        || credit.lamports != account.lamports
        || credit.beneficiary != beneficiary
    {
        return Err(Error::InvalidRent);
    }
    Ok(credit)
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
    use crate::v2;

    const DIRECT_MAKER_DOMAIN: &[u8] = b"dclutch/direct-replay/v2";
    const DIRECT_RECORD_DOMAIN: &[u8] = b"dclutch/direct-intent/v2";
    const POLICY_ID: [u8; 32] = [0x71; 32];
    const TRADING: [u8; 32] = [0x91; 32];
    const SYSTEM: [u8; 32] = [0x81; 32];
    const PRODUCT_DATA: [u8; 4] = 3_u32.to_le_bytes();
    const CLOSED_STATE_DATA: [u8; 148] = [0x55; 148];

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
            AccountObservationV1::new([1; 32], [7; 32], 1, &PRODUCT_DATA, false, false, false),
            AccountObservationV1::new([0x41; 32], SYSTEM, 5, &[], false, true, false),
            AccountObservationV1::new([0x42; 32], SYSTEM, 1_000, &[], true, true, false),
            AccountObservationV1::new([0x43; 32], [7; 32], 7, &[], false, true, false),
            AccountObservationV1::new([0x50; 32], SYSTEM, 0, &[], false, true, false),
            AccountObservationV1::new([0x51; 32], SYSTEM, 0, &[], false, true, false),
            AccountObservationV1::new([0x52; 32], SYSTEM, 20, &[], false, true, false),
        ]
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
            [0x52; 32],
            TRADING,
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
            AccountObservationV1::new([0x52; 32], SYSTEM, 20, &[], false, true, false);
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
