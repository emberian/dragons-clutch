//! Commit last: the root write, the ordered non-root effects and the lamport
//! outputs, each checked to persist.

use super::*;

pub(super) fn require_root_write_is_state_only(
    resolved: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let (account, offset) = match resolved {
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteIdentity {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU8 {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU16 {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => (account, offset),
        _ => return Ok(()),
    };
    let representative = *aliases.get(account).ok_or(TradingSbfError::Transition)?;
    if representative == 0
        && usize::try_from(offset).map_err(|_| TradingSbfError::Transition)?
            < CAPABILITY_ROOT_HEADER_BYTES_V1
    {
        Err(TradingSbfError::Commit.into())
    } else {
        Ok(())
    }
}

/// Data writes retained by the mandatory pre-child Effect projection.
///
/// Resolution is a pure function of the immutable Effect and candidate output
/// registers. The projection validates every operation, including request and
/// lamport effects, before any child runs. Its visitor retains each accepted
/// data write here in execution order; the commit never re-resolves it.
///
/// `ordinals` pins the complete Effect geometry. `bits` instead indexes this
/// compact data-write bank: the non-root pass marks writes whose authenticated
/// representative is the root, and the final pass applies exactly those writes.
/// A projection refusal drops the entire plan before it can reach either pass.
pub(super) struct RootCommitPlanV3 {
    pub(super) ordinals: u32,
    pub(super) bits: Vec<u8>,
    writes: Vec<ResolvedEffectV3>,
    write_limit: usize,
}

impl RootCommitPlanV3 {
    pub(super) fn for_geometry(
        effect: SelectedEffectProgramV4<'_>,
        tail_count: u32,
        write_limit: usize,
    ) -> Result<Self, ProgramError> {
        let ordinals = root_commit_ordinal_count_v3(effect, tail_count)?;
        if write_limit > usize::try_from(ordinals).map_err(|_| TradingSbfError::Commit)? {
            return Err(TradingSbfError::Commit.into());
        }
        let bytes = write_limit.div_ceil(8);
        let mut bits = Vec::new();
        bits.try_reserve_exact(bytes)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        bits.resize(bytes, 0);
        let mut writes = Vec::new();
        writes
            .try_reserve_exact(write_limit)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        Ok(Self {
            ordinals,
            bits,
            writes,
            write_limit,
        })
    }

    /// Retain only actual data writes offered by the successful projection.
    /// All other operation kinds have their effect in its lamport/request banks.
    pub(super) fn record_projected(
        &mut self,
        effect: ResolvedEffectV3,
    ) -> Result<(), ProgramError> {
        if matches!(
            effect,
            ResolvedEffectV3::WriteScalar { .. }
                | ResolvedEffectV3::WriteIdentity { .. }
                | ResolvedEffectV3::WriteU8 { .. }
                | ResolvedEffectV3::WriteU16 { .. }
                | ResolvedEffectV3::WriteU32 { .. }
        ) {
            if self.writes.len() >= self.write_limit {
                return Err(TradingSbfError::Commit.into());
            }
            self.writes.push(effect);
        }
        Ok(())
    }

    fn record(&mut self, index: usize) -> Result<(), ProgramError> {
        *self
            .bits
            .get_mut(index / 8)
            .ok_or(TradingSbfError::Commit)? |= 1_u8 << (index % 8);
        Ok(())
    }
}

/// Total local-effect ordinals for one execution's geometry.
fn root_commit_ordinal_count_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
) -> Result<u32, ProgramError> {
    u32::from(effect.item_operation_count())
        .checked_mul(tail_count)
        .and_then(|items| items.checked_add(u32::from(effect.fixed_operation_count())))
        .ok_or_else(|| TradingSbfError::Commit.into())
}

/// Commit every non-root coordinate and record what the commit-last pass owns.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_non_root_effects_into_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    output_lamports: &[u64],
    participation: Option<&[CoordinateParticipationV3]>,
    plan: &mut RootCommitPlanV3,
) -> Result<(), ProgramError> {
    if plan.ordinals != root_commit_ordinal_count_v3(effect, tail_count)?
        || plan.bits.iter().any(|byte| *byte != 0)
    {
        return Err(TradingSbfError::Commit.into());
    }
    commit_output_lamports_v3(
        effect,
        accounts,
        aliases,
        output_lamports,
        participation,
        false,
    )?;
    for index in 0..plan.writes.len() {
        let resolved = *plan.writes.get(index).ok_or(TradingSbfError::Commit)?;
        if commit_data_effect(resolved, accounts, aliases, false)? {
            plan.record(index)?;
        }
    }
    require_committed_accounts_persist_v3(accounts, aliases, false)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_non_root_effects_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    output_lamports: &[u64],
    participation: Option<&[CoordinateParticipationV3]>,
) -> Result<RootCommitPlanV3, ProgramError> {
    let mut plan = RootCommitPlanV3::for_geometry(
        effect,
        tail_count,
        effect
            .successor
            .data_write_operation_count(tail_count)
            .map_err(|_| TradingSbfError::Commit)?,
    )?;
    // Test fixture construction. Production retains these values directly
    // from the mandatory pre-child projection's visitor.
    for fixed in 0..effect.fixed_operation_count() {
        plan.record_projected(
            effect
                .resolved_fixed_effect(fixed, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Commit)?,
        )?;
    }
    for item in 0..tail_count {
        for operation in 0..effect.item_operation_count() {
            plan.record_projected(
                effect
                    .resolved_item_effect(item, operation, tail_count, scalars, identities)
                    .map_err(|_| TradingSbfError::Commit)?,
            )?;
        }
    }
    commit_non_root_effects_into_v3(
        effect,
        tail_count,
        accounts,
        aliases,
        output_lamports,
        participation,
        &mut plan,
    )?;
    Ok(plan)
}

/// Commit the root coordinate last from the recorded projection writes.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_root_effects_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    output_lamports: &[u64],
    participation: Option<&[CoordinateParticipationV3]>,
    plan: &RootCommitPlanV3,
) -> Result<(), ProgramError> {
    if plan.ordinals != root_commit_ordinal_count_v3(effect, tail_count)? {
        return Err(TradingSbfError::Commit.into());
    }
    commit_output_lamports_v3(
        effect,
        accounts,
        aliases,
        output_lamports,
        participation,
        true,
    )?;
    for (byte_index, byte) in plan.bits.iter().enumerate() {
        let mut remaining = *byte;
        while remaining != 0 {
            let bit = remaining.trailing_zeros();
            remaining &= remaining.wrapping_sub(1);
            let ordinal = u32::try_from(byte_index)
                .ok()
                .and_then(|index| index.checked_mul(8))
                .and_then(|base| base.checked_add(bit))
                .ok_or(TradingSbfError::Commit)?;
            let resolved = *plan
                .writes
                .get(usize::try_from(ordinal).map_err(|_| TradingSbfError::Commit)?)
                .ok_or(TradingSbfError::Commit)?;
            commit_data_effect(resolved, accounts, aliases, true)?;
        }
    }
    require_committed_accounts_persist_v3(accounts, aliases, true)
}

/// Land the planned lamports, and leave a declared child route's poststate alone.
///
/// `participation` is `None` only when the Effect declares no child route, and
/// then every coordinate is treated as the plan's: with no child there is
/// nothing else in the transaction that could have moved a lamport, so the two
/// readings are the same walk.
fn commit_output_lamports_v3(
    effect: SelectedEffectProgramV4<'_>,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    output_lamports: &[u64],
    participation: Option<&[CoordinateParticipationV3]>,
    root_only: bool,
) -> Result<(), ProgramError> {
    for (coordinate, account) in accounts.iter().enumerate() {
        let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Commit)?;
        if representative != coordinate
            || (coordinate == 0) != root_only
            || effect
                .funding()
                .is_some_and(|funding| funding_owns_coordinate_v5(funding, coordinate))
        {
            continue;
        }
        let output = *output_lamports
            .get(coordinate)
            .ok_or(TradingSbfError::Commit)?;
        let participation = match participation {
            Some(bank) => *bank.get(coordinate).ok_or(TradingSbfError::Commit)?,
            None => CoordinateParticipationV3::PLAN_IS_SOLE_AUTHORITY,
        };
        match committed_lamports_v3(output, account.lamports(), participation) {
            CommittedLamportsV3::Apply => {
                if account.lamports() != output {
                    **account
                        .try_borrow_mut_lamports()
                        .map_err(|_| TradingSbfError::Commit)? = output;
                }
            }
            CommittedLamportsV3::Settled | CommittedLamportsV3::ChildPoststate => {}
            CommittedLamportsV3::Unexplained => return Err(TradingSbfError::Commit.into()),
        }
    }
    Ok(())
}

/// Require every account this commit could have changed to still exist.
///
/// **Only the writable ones.** This is a POSTCONDITION of the commit, and the
/// commit can only reach an account the transaction made writable: both writes
/// it performs -- `commit_output_lamports_v3`'s `try_borrow_mut_lamports` and
/// `commit_data_effect`'s `try_borrow_mut_data` -- refuse on a readonly
/// account, and so does every child CPI. Asserting exemption of a readonly
/// coordinate is therefore never a fact about this execution; it is a fact
/// about someone else's account, and it is one that is false on every cluster
/// for the coordinates a Direct frame carries.
///
/// **It asked for EXEMPTION at the live rate until 2026-09-04, and that was a
/// question about the cluster rather than about this commit.** The runtime
/// already forbids the transition this wanted to prevent -- `transition_allowed`
/// refuses `RentExempt -> RentPaying`, and a creation from `Uninitialized` must
/// land exempt -- so a writable account this commit wrote either ends exempt,
/// ends at zero, or the transaction fails with `InsufficientFundsForRent`
/// without reaching any refusal of ours. What exemption at TODAY's rate added
/// on top of that was a refusal of the one account nobody had touched: one
/// funded when a byte cost less. So the postcondition kept here is the part
/// this commit is answerable for -- that it drained nothing it wrote.
pub(super) fn require_committed_accounts_persist_v3(
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    root_only: bool,
) -> Result<(), ProgramError> {
    for (coordinate, account) in accounts.iter().enumerate() {
        if *aliases.get(coordinate).ok_or(TradingSbfError::Commit)? == coordinate
            && (coordinate == 0) == root_only
            && account.is_writable
            && account.data_len() != 0
            && !funded_rent_persists_v1(account.lamports())
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

/// Apply one resolved local effect if it belongs to this pass, and report
/// whether it belongs to the commit-last pass at all.
///
/// The answer is what [`RootCommitPlanV3`] records: an effect that writes
/// nothing, or writes a coordinate whose representative is not the root, is
/// `false` and the second pass never has to resolve it again.
pub(super) fn commit_data_effect(
    resolved: ResolvedEffectV3,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    root_only: bool,
) -> Result<bool, ProgramError> {
    // Fixed writes are at most one identity wide. Keep their bytes on the
    // stack: allocating one temporary `Vec` per effect permanently consumed
    // bump-heap space during a long Hot commit even though each value dies at
    // the end of this call.
    let mut fixed = [0_u8; 32];
    let (coordinate, offset, width): (usize, usize, usize) = match resolved {
        ResolvedEffectV3::WriteScalar {
            account,
            offset,
            value,
        } => {
            fixed[..8].copy_from_slice(&value.to_le_bytes());
            (
                account,
                usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
                8,
            )
        }
        ResolvedEffectV3::WriteIdentity {
            account,
            offset,
            value,
        } => {
            fixed = value;
            (
                account,
                usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
                32,
            )
        }
        ResolvedEffectV3::WriteU8 {
            account,
            offset,
            value,
        } => {
            fixed[0] = value;
            (
                account,
                usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
                1,
            )
        }
        ResolvedEffectV3::WriteU16 {
            account,
            offset,
            value,
        } => {
            fixed[..2].copy_from_slice(&value.to_le_bytes());
            (
                account,
                usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
                2,
            )
        }
        ResolvedEffectV3::WriteU32 {
            account,
            offset,
            value,
        } => {
            fixed[..4].copy_from_slice(&value.to_le_bytes());
            (
                account,
                usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
                4,
            )
        }
        ResolvedEffectV3::Noop
        | ResolvedEffectV3::TransferLamports { .. }
        | ResolvedEffectV3::RequireLamportsEq { .. }
        | ResolvedEffectV3::WriteRequest { .. } => return Ok(false),
    };
    let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Commit)?;
    let commits_last = representative == 0;
    if commits_last != root_only {
        return Ok(commits_last);
    }
    let account = accounts
        .get(representative)
        .ok_or(TradingSbfError::Commit)?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    let bytes = fixed.get(..width).ok_or(TradingSbfError::Commit)?;
    let end = offset
        .checked_add(bytes.len())
        .ok_or(TradingSbfError::Commit)?;
    data.get_mut(offset..end)
        .ok_or(TradingSbfError::Commit)?
        .copy_from_slice(bytes);
    Ok(commits_last)
}
