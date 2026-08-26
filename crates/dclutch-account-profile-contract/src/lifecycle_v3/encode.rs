//! Safe, allocation-free StateLifecyclePolicy V3 artifact encoder.
//!
//! Typed inputs retain seed, coordinate, operation, and guard tag authority in
//! this semantic-owner crate. The encoder builds into caller scratch,
//! hostile-decodes the complete candidate, and copies to output only on
//! success.

use super::{
    ACTION_PLAN_BYTES, ARTIFACT_PROFILE, Error, GUARD_ALWAYS, GUARD_SCALAR_EQ, HEADER_BYTES, MAGIC,
    MAX_SEED_BYTES, PLAN_AUTHENTICATE, PLAN_AUTHENTICATE_OR_CREATE, PLAN_CLOSE, PLAN_CREATE,
    RECIPE_BYTES, SCOPE_FIXED, SCOPE_ITEM, SEED_BYTES, SEED_COMMON_IDENTITY, SEED_COMMON_SCALAR_LE,
    SEED_ITEM_IDENTITY, SEED_ITEM_INDEX_LE, SEED_ITEM_SCALAR_LE, SEED_LITERAL,
    StateLifecyclePolicyV3, VERSION,
};

/// Fixed-prefix or per-Product-item lifecycle coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleCoordinateSpaceV3 {
    /// Fixed-prefix coordinate.
    Fixed,
    /// Per-Product-item coordinate.
    Item,
}

impl LifecycleCoordinateSpaceV3 {
    const fn tag(self) -> u8 {
        match self {
            Self::Fixed => SCOPE_FIXED,
            Self::Item => SCOPE_ITEM,
        }
    }
}

/// One account coordinate in the authenticated AccountProfile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleAccountCoordinateV3 {
    space: LifecycleCoordinateSpaceV3,
    index: u16,
}

impl LifecycleAccountCoordinateV3 {
    /// Address one fixed-prefix account.
    pub const fn fixed(index: u16) -> Self {
        Self {
            space: LifecycleCoordinateSpaceV3::Fixed,
            index,
        }
    }

    /// Address one account within every Product-item subframe.
    pub const fn item(index: u16) -> Self {
        Self {
            space: LifecycleCoordinateSpaceV3::Item,
            index,
        }
    }
}

/// One scalar or identity register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRegisterCoordinateV3 {
    space: LifecycleCoordinateSpaceV3,
    index: u16,
}

impl LifecycleRegisterCoordinateV3 {
    /// Address one common register.
    pub const fn common(index: u16) -> Self {
        Self {
            space: LifecycleCoordinateSpaceV3::Fixed,
            index,
        }
    }

    /// Address one register within every Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self {
            space: LifecycleCoordinateSpaceV3::Item,
            index,
        }
    }
}

/// One Trading-owned PDA recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRecipeInputV3 {
    /// State account coordinate.
    pub state: LifecycleAccountCoordinateV3,
    /// First seed in the global seed table.
    pub seed_start: u16,
    /// Exact nonzero seed count.
    pub seed_count: u8,
    /// Seed ordinal containing the one-byte PDA bump.
    pub bump_offset: u8,
    /// Fixed state-data width.
    pub data_base: u32,
    /// Additional state-data bytes per Product item.
    pub data_stride: u32,
}

/// One bounded PDA seed source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleSeedInputV3<'a> {
    /// Exact nonempty literal of at most 32 bytes.
    Literal(&'a [u8]),
    /// One common 32-byte identity register.
    CommonIdentity(u16),
    /// One per-item 32-byte identity register.
    ItemIdentity(u16),
    /// Little-endian low 1, 2, 4, or 8 bytes of one common scalar.
    CommonScalar {
        /// Common scalar index.
        index: u16,
        /// Exact encoded width.
        width: u8,
    },
    /// Little-endian low 1, 2, 4, or 8 bytes of one per-item scalar.
    ItemScalar {
        /// Item scalar index.
        index: u16,
        /// Exact encoded width.
        width: u8,
    },
    /// Canonical Product item index encoded as 1, 2, or 4 bytes.
    ItemIndex(u8),
}

/// Generic lifecycle operation selected by a family action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleOperationInputV3 {
    /// Authenticate an existing Trading-owned PDA.
    Authenticate,
    /// Allocate and assign a vacant dust-tolerant PDA.
    Create,
    /// Close a terminal PDA to its permanent RentCredit.
    Close,
    /// Authenticate an existing PDA or create its exact vacant prestate.
    AuthenticateOrCreate,
}

/// Data-defined plan enable guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleGuardInputV3 {
    /// Always execute this mandatory plan.
    Always,
    /// Execute only when one candidate post-transition scalar equals `expected`.
    ScalarEq {
        /// Candidate scalar source.
        source: LifecycleRegisterCoordinateV3,
        /// Exact enabling value.
        expected: u64,
    },
}

/// One action-selected lifecycle plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecyclePlanInputV3 {
    /// Family action selector.
    pub action: u32,
    /// Generic lifecycle operation.
    pub operation: LifecycleOperationInputV3,
    /// Recipe table index.
    pub recipe: u16,
    /// Create payer; absent for Authenticate and Close.
    pub payer: Option<LifecycleAccountCoordinateV3>,
    /// Permanent RentCredit; required for Create and Close.
    pub rent_credit: Option<LifecycleAccountCoordinateV3>,
    /// Historical rent-principal scalar; required for Create and Close.
    pub principal: Option<LifecycleRegisterCoordinateV3>,
    /// Immutable RentCredit-beneficiary identity; required for Create and Close.
    pub beneficiary: Option<LifecycleRegisterCoordinateV3>,
    /// Mandatory plan enable guard.
    pub guard: LifecycleGuardInputV3,
}

/// Encode one complete StateLifecyclePolicy V3 atomically.
pub fn encode_lifecycle_policy_v3_atomic(
    recipes: &[LifecycleRecipeInputV3],
    seeds: &[LifecycleSeedInputV3<'_>],
    plans: &[LifecyclePlanInputV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    let recipe_count = u16::try_from(recipes.len()).map_err(|_| Error::InvalidLength)?;
    let seed_count = u16::try_from(seeds.len()).map_err(|_| Error::InvalidLength)?;
    let plan_count = u16::try_from(plans.len()).map_err(|_| Error::InvalidLength)?;
    let expected = recipes
        .len()
        .checked_mul(RECIPE_BYTES)
        .and_then(|width| {
            seeds
                .len()
                .checked_mul(SEED_BYTES)
                .and_then(|seed_width| width.checked_add(seed_width))
        })
        .and_then(|width| {
            plans
                .len()
                .checked_mul(ACTION_PLAN_BYTES)
                .and_then(|plan_width| width.checked_add(plan_width))
        })
        .and_then(|body| HEADER_BYTES.checked_add(body))
        .ok_or(Error::InvalidLength)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &MAGIC)?;
    write(scratch, 8, &VERSION.to_le_bytes())?;
    write(scratch, 10, &ARTIFACT_PROFILE.to_le_bytes())?;
    for (offset, value) in [(12, recipe_count), (14, seed_count), (16, plan_count)] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = HEADER_BYTES;
    for recipe in recipes {
        encode_recipe(*recipe, scratch, cursor)?;
        cursor = add(cursor, RECIPE_BYTES)?;
    }
    for seed in seeds {
        encode_seed(*seed, scratch, cursor)?;
        cursor = add(cursor, SEED_BYTES)?;
    }
    for plan in plans {
        encode_plan(*plan, scratch, cursor)?;
        cursor = add(cursor, ACTION_PLAN_BYTES)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    StateLifecyclePolicyV3::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_recipe(
    recipe: LifecycleRecipeInputV3,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    write_byte(output, offset, recipe.state.space.tag())?;
    write(output, add(offset, 2)?, &recipe.state.index.to_le_bytes())?;
    write(output, add(offset, 4)?, &recipe.seed_start.to_le_bytes())?;
    write_byte(output, add(offset, 6)?, recipe.seed_count)?;
    write_byte(output, add(offset, 7)?, recipe.bump_offset)?;
    write(output, add(offset, 8)?, &recipe.data_base.to_le_bytes())?;
    write(output, add(offset, 12)?, &recipe.data_stride.to_le_bytes())
}

fn encode_seed(
    seed: LifecycleSeedInputV3<'_>,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    let (tag, width, index, literal) = match seed {
        LifecycleSeedInputV3::Literal(value) => (
            SEED_LITERAL,
            u8::try_from(value.len()).map_err(|_| Error::InvalidSeed)?,
            0,
            Some(value),
        ),
        LifecycleSeedInputV3::CommonIdentity(index) => (SEED_COMMON_IDENTITY, 32, index, None),
        LifecycleSeedInputV3::ItemIdentity(index) => (SEED_ITEM_IDENTITY, 32, index, None),
        LifecycleSeedInputV3::CommonScalar { index, width } => {
            (SEED_COMMON_SCALAR_LE, width, index, None)
        }
        LifecycleSeedInputV3::ItemScalar { index, width } => {
            (SEED_ITEM_SCALAR_LE, width, index, None)
        }
        LifecycleSeedInputV3::ItemIndex(width) => (SEED_ITEM_INDEX_LE, width, 0, None),
    };
    if width == 0 || width > MAX_SEED_BYTES {
        return Err(Error::InvalidSeed);
    }
    write_byte(output, offset, tag)?;
    write_byte(output, add(offset, 1)?, width)?;
    write(output, add(offset, 2)?, &index.to_le_bytes())?;
    if let Some(value) = literal {
        write(output, add(offset, 4)?, value)?;
    }
    Ok(())
}

fn encode_plan(plan: LifecyclePlanInputV3, output: &mut [u8], offset: usize) -> Result<(), Error> {
    let operation = match plan.operation {
        LifecycleOperationInputV3::Authenticate => PLAN_AUTHENTICATE,
        LifecycleOperationInputV3::Create => PLAN_CREATE,
        LifecycleOperationInputV3::Close => PLAN_CLOSE,
        LifecycleOperationInputV3::AuthenticateOrCreate => PLAN_AUTHENTICATE_OR_CREATE,
    };
    write(output, offset, &plan.action.to_le_bytes())?;
    write_byte(output, add(offset, 4)?, operation)?;
    write(output, add(offset, 6)?, &plan.recipe.to_le_bytes())?;
    encode_optional_account(plan.payer, output, add(offset, 8)?)?;
    encode_optional_account(plan.rent_credit, output, add(offset, 12)?)?;
    encode_optional_register(plan.principal, output, add(offset, 16)?)?;
    encode_optional_register(plan.beneficiary, output, add(offset, 20)?)?;
    match plan.guard {
        LifecycleGuardInputV3::Always => write_byte(output, add(offset, 24)?, GUARD_ALWAYS),
        LifecycleGuardInputV3::ScalarEq { source, expected } => {
            write_byte(output, add(offset, 24)?, GUARD_SCALAR_EQ)?;
            write_byte(output, add(offset, 25)?, source.space.tag())?;
            write(output, add(offset, 26)?, &source.index.to_le_bytes())?;
            write(output, add(offset, 28)?, &expected.to_le_bytes())
        }
    }
}

fn encode_optional_account(
    value: Option<LifecycleAccountCoordinateV3>,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    if let Some(value) = value {
        write_byte(output, offset, value.space.tag())?;
        write(output, add(offset, 2)?, &value.index.to_le_bytes())
    } else {
        write_byte(output, offset, u8::MAX)
    }
}

fn encode_optional_register(
    value: Option<LifecycleRegisterCoordinateV3>,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    if let Some(value) = value {
        write_byte(output, offset, value.space.tag())?;
        write(output, add(offset, 2)?, &value.index.to_le_bytes())
    } else {
        write_byte(output, offset, u8::MAX)
    }
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    let end = add(offset, bytes.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn typed_encoder_round_trips_guards_and_preserves_output() {
        let recipes = [LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(1),
            seed_start: 0,
            seed_count: 2,
            bump_offset: 1,
            data_base: 16,
            data_stride: 0,
        }];
        let seeds = [
            LifecycleSeedInputV3::Literal(b"state"),
            LifecycleSeedInputV3::CommonScalar { index: 0, width: 1 },
        ];
        let plans = [LifecyclePlanInputV3 {
            action: 7,
            operation: LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            guard: LifecycleGuardInputV3::ScalarEq {
                source: LifecycleRegisterCoordinateV3::common(1),
                expected: 9,
            },
        }];
        let width = HEADER_BYTES + RECIPE_BYTES + 2 * SEED_BYTES + ACTION_PLAN_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![9_u8; width];
        encode_lifecycle_policy_v3_atomic(&recipes, &seeds, &plans, &mut scratch, &mut output)
            .expect("encode");
        let policy = StateLifecyclePolicyV3::decode(&output).expect("decode encoded policy");
        assert_eq!(policy.action_plan_count(7), Ok(1));

        let invalid = [LifecyclePlanInputV3 {
            operation: LifecycleOperationInputV3::Create,
            ..plans[0]
        }];
        let mut hostile_scratch = std::vec![0_u8; width];
        let mut hostile_output = std::vec![7_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_lifecycle_policy_v3_atomic(
                &recipes,
                &seeds,
                &invalid,
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidFunding)
        );
        assert_eq!(hostile_output, before);
    }
}
