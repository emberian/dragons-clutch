//! Token-2022 outcome-mint truth at the adapter boundary.
//!
//! Internal claims are aggregated in [`SupplyLedgerAccount::internal_supply`].
//! External claims are ordinary bearer tokens, so their aggregate is the
//! authenticated Token-2022 mint supply and never an owner-local program
//! account.  This module admits the complete bounded mint vector and joins the
//! two terms to the kernel aggregate.
//!
//! The frozen `external_supply` field survives this ABI only as the last mint
//! supply this program observed.  A lower current supply is an ordinary holder
//! burn and is synchronized as a safe liability donation.  A higher supply is
//! impossible without the program's mint-authority PDA and refuses.  Current
//! Token-2022 bytes, not the cache, own the balance truth.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::{seeds, token};
use clutch_solana_layout::{Hash32, SupplyLedgerAccount, MAX_OUTCOMES};
use clutch_solana_reference::KernelAccount;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Exact observed Token-2022 supply for every active outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedMintSupplies {
    /// Supply by outcome index; padding is canonical zero.
    pub values: [u64; MAX_OUTCOMES],
    /// Active prefix length.
    pub outcome_count: u8,
}

/// Admit the canonical complete outcome-mint suffix and read every supply.
///
/// `first` is the suffix's first account index.  The caller owns exact total
/// account count and global alias checks.  `writable_outcome` names the one mint
/// a mint/burn CPI will change; every other mint must be read-only.  Passing
/// `None` makes the entire suffix read-only.
#[inline(never)]
pub fn observe_outcome_mints(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    first: usize,
    market_account_key: Pubkey,
    market: Hash32,
    outcome_count: u8,
    writable_outcome: Option<u8>,
) -> Outcome<ObservedMintSupplies> {
    observe_outcome_mints_with(
        accounts,
        first,
        market_account_key,
        outcome_count,
        writable_outcome,
        |outcome| seeds::outcome_mint_pda(program_id, &market.bytes(), outcome),
    )
}

/// Admit the canonical full-width MarketInstanceV2 outcome-mint suffix.
///
/// This successor uses a fresh mint seed domain and the authenticated stable
/// MarketRuntime PDA as mint authority. It never treats the full content ID as
/// a legacy lowered Market coordinate or asks a Product artifact account to
/// sign claim issuance.
#[inline(never)]
pub fn observe_outcome_mints_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    first: usize,
    market_runtime_authority: Pubkey,
    market_instance_v2_id: [u8; 32],
    outcome_count: u8,
    writable_outcome: Option<u8>,
) -> Outcome<ObservedMintSupplies> {
    observe_outcome_mints_with(
        accounts,
        first,
        market_runtime_authority,
        outcome_count,
        writable_outcome,
        |outcome| seeds::outcome_mint_v2_pda(program_id, &market_instance_v2_id, outcome),
    )
}

/// Boxed [`observe_outcome_mints`]: the observed vector lives in this helper's
/// frame and only a heap pointer crosses back into the caller's bounded SBF
/// frame (the `direct_selection_v3::common` discipline).
#[inline(never)]
pub fn observe_outcome_mints_boxed(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    first: usize,
    market_account_key: Pubkey,
    market: Hash32,
    outcome_count: u8,
    writable_outcome: Option<u8>,
) -> Outcome<Box<ObservedMintSupplies>> {
    Ok(Box::new(observe_outcome_mints(
        program_id,
        accounts,
        first,
        market_account_key,
        market,
        outcome_count,
        writable_outcome,
    )?))
}

/// Host-testable core of [`observe_outcome_mints`].
///
/// PDA derivation is a Solana syscall and is deliberately injected here so
/// host tests can exercise every admission check without pretending that a
/// caller-supplied key is production authority.
#[inline(never)]
fn observe_outcome_mints_with<D>(
    accounts: &[AccountInfo],
    first: usize,
    market_account_key: Pubkey,
    outcome_count: u8,
    writable_outcome: Option<u8>,
    derive: D,
) -> Outcome<ObservedMintSupplies>
where
    D: Fn(u8) -> (Pubkey, u8),
{
    let count = usize::from(outcome_count);
    require(count <= MAX_OUTCOMES, ClutchError::NonCanonical)?;
    let end = first
        .checked_add(count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(end <= accounts.len(), ClutchError::AccountCount)?;
    if let Some(outcome) = writable_outcome {
        require(usize::from(outcome) < count, ClutchError::NonCanonical)?;
    }

    let mut values = [0_u64; MAX_OUTCOMES];
    let mut outcome = 0_usize;
    while outcome < count {
        let mint = &accounts[first + outcome];
        require(!mint.executable, ClutchError::ExecutableAccount)?;
        let must_write = writable_outcome == Some(outcome as u8);
        require(
            mint.is_writable == must_write,
            if must_write {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        let derived = derive(outcome as u8);
        expect_pda(mint.key, derived, None)?;
        let observation = token::admit_mint(
            mint,
            &token::MintPolicy::outcome(*mint.key, market_account_key),
        )?;
        values[outcome] = observation.supply;
        outcome += 1;
    }
    Ok(ObservedMintSupplies {
        values,
        outcome_count,
    })
}

/// Synchronize direct holder burns into the cached ledger and kernel total.
///
/// The pre-sync cache must already close against the persisted kernel.  Actual
/// mint supply may only stay equal or fall: only this program can sign as the
/// mint authority, and its own increases are persisted atomically with the CPI.
/// All checks finish before either byte slice is written.
#[inline(never)]
pub fn synchronize_external_truth(
    supply_data: &mut [u8],
    kernel_data: &mut [u8],
    market: Hash32,
    realm: Hash32,
    outcome_count: u8,
    observed: &ObservedMintSupplies,
) -> Outcome<()> {
    let cached_total = read_kernel_totals(kernel_data, market, outcome_count)?;
    let next_total = synchronize_supply_cache(
        supply_data,
        market,
        realm,
        outcome_count,
        observed,
        &cached_total,
    )?;
    write_kernel_totals(kernel_data, market, outcome_count, &next_total)
}

#[inline(never)]
fn read_kernel_totals(
    kernel_data: &[u8],
    market: Hash32,
    outcome_count: u8,
) -> Outcome<[u64; MAX_OUTCOMES]> {
    let kernel = KernelAccount::decode(kernel_data)?;
    require(
        kernel.market == market && kernel.payouts.outcomes == outcome_count,
        ClutchError::MismatchedState,
    )?;
    Ok(kernel.total_supply)
}

#[inline(never)]
fn synchronize_supply_cache(
    supply_data: &mut [u8],
    market: Hash32,
    realm: Hash32,
    outcome_count: u8,
    observed: &ObservedMintSupplies,
    cached_total: &[u64; MAX_OUTCOMES],
) -> Outcome<[u64; MAX_OUTCOMES]> {
    let mut supply = SupplyLedgerAccount::decode(supply_data)?;
    require(
        supply.market == market
            && supply.realm == realm
            && supply.outcome_count == outcome_count
            && observed.outcome_count == outcome_count,
        ClutchError::MismatchedState,
    )?;

    let mut next_total = *cached_total;
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let accounted_total = supply.internal_supply[outcome]
            .checked_add(supply.external_supply[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            accounted_total == cached_total[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        require(
            observed.values[outcome] <= supply.external_supply[outcome],
            ClutchError::ShadowSupplyMismatch,
        )?;
        next_total[outcome] = supply.internal_supply[outcome]
            .checked_add(observed.values[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        outcome += 1;
    }

    supply.external_supply = observed.values;
    supply.encode(supply_data)?;
    Ok(next_total)
}

#[inline(never)]
fn write_kernel_totals(
    kernel_data: &mut [u8],
    market: Hash32,
    outcome_count: u8,
    next_total: &[u64; MAX_OUTCOMES],
) -> Outcome<()> {
    let mut kernel = KernelAccount::decode(kernel_data)?;
    require(
        kernel.market == market && kernel.payouts.outcomes == outcome_count,
        ClutchError::MismatchedState,
    )?;
    kernel.total_supply = *next_total;
    kernel.encode(kernel_data)?;
    Ok(())
}

/// Boxed [`KernelAccount`] decode: the decode temporary lives in this helper's
/// frame and only a heap pointer crosses back into the caller's bounded SBF
/// frame (the `direct_selection_v3::common` discipline).
#[inline(never)]
fn decode_kernel_boxed(kernel_data: &[u8]) -> Outcome<Box<KernelAccount>> {
    Ok(Box::new(KernelAccount::decode(kernel_data)?))
}

/// Persist the exact post-CPI supplies and re-close them against the kernel.
///
/// This never repairs a kernel discrepancy.  The transition has already
/// decided its aggregate effect; Token-2022 must have produced the exact mint
/// vector whose internal-plus-external sum is that effect.
#[inline(never)]
pub fn commit_observed_supplies(
    supply_data: &mut [u8],
    kernel_data: &[u8],
    market: Hash32,
    realm: Hash32,
    outcome_count: u8,
    observed: &ObservedMintSupplies,
) -> Outcome<()> {
    let mut supply = SupplyLedgerAccount::decode(supply_data)?;
    let kernel = decode_kernel_boxed(kernel_data)?;
    require(
        supply.market == market
            && supply.realm == realm
            && supply.outcome_count == outcome_count
            && observed.outcome_count == outcome_count
            && kernel.market == market,
        ClutchError::MismatchedState,
    )?;
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let total = supply.internal_supply[outcome]
            .checked_add(observed.values[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            total == kernel.total_supply[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }
    supply.external_supply = observed.values;
    supply.encode(supply_data)?;
    Ok(())
}

/// Compare pre/post complete mint vectors with one exact optional delta.
pub fn require_exact_mint_vector_delta(
    before: &ObservedMintSupplies,
    after: &ObservedMintSupplies,
    changed: Option<(u8, bool, u64)>,
) -> Outcome<()> {
    require(
        before.outcome_count == after.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let mut outcome = 0_usize;
    while outcome < usize::from(before.outcome_count) {
        let expected = match changed {
            Some((index, minting, quantity)) if usize::from(index) == outcome => {
                if minting {
                    before.values[outcome]
                        .checked_add(quantity)
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                } else {
                    before.values[outcome]
                        .checked_sub(quantity)
                        .ok_or(Refusal::Adapter(ClutchError::TokenDeltaMismatch))?
                }
            }
            _ => before.values[outcome],
        };
        require(
            after.values[outcome] == expected,
            ClutchError::TokenDeltaMismatch,
        )?;
        outcome += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS};

    struct MintCell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        writable: bool,
        executable: bool,
    }

    impl MintCell {
        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                self.writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn payouts() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut first = [0; MAX_OUTCOMES];
        first[0] = 1;
        let mut second = [0; MAX_OUTCOMES];
        second[1] = 1;
        vectors[0] = PayoutVector::new(1, first);
        vectors[1] = PayoutVector::new(1, second);
        PayoutSet::new(2, 2, vectors)
    }

    fn state() -> (Vec<u8>, Vec<u8>) {
        let mut supply_bytes = vec![0; clutch_solana_layout::account_len::SUPPLY_LEDGER];
        let mut internal = [0; MAX_OUTCOMES];
        internal[0] = 7;
        internal[1] = 8;
        let mut external = [0; MAX_OUTCOMES];
        external[0] = 5;
        external[1] = 4;
        SupplyLedgerAccount {
            market: h(1),
            realm: h(2),
            generation: 0,
            outcome_count: 2,
            internal_supply: internal,
            external_supply: external,
            stored_bump: 9,
            flags: 0,
        }
        .encode(&mut supply_bytes)
        .unwrap();
        let mut kernel_bytes = vec![0; clutch_solana_reference::KERNEL_ACCOUNT_LEN];
        let mut total = [0; MAX_OUTCOMES];
        total[0] = 12;
        total[1] = 12;
        KernelAccount {
            market: h(1),
            phase: 0,
            basis_mode: clutch_kernel::BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: payouts(),
            total_supply: total,
        }
        .encode(&mut kernel_bytes)
        .unwrap();
        (supply_bytes, kernel_bytes)
    }

    #[test]
    fn a_direct_burn_lowers_recognized_liability_without_touching_internal() {
        let (mut supply, mut kernel) = state();
        let observed = ObservedMintSupplies {
            values: {
                let mut value = [0; MAX_OUTCOMES];
                value[0] = 2;
                value[1] = 4;
                value
            },
            outcome_count: 2,
        };
        synchronize_external_truth(&mut supply, &mut kernel, h(1), h(2), 2, &observed).unwrap();
        let supply = SupplyLedgerAccount::decode(&supply).unwrap();
        let kernel = KernelAccount::decode(&kernel).unwrap();
        assert_eq!(supply.internal_supply[0], 7);
        assert_eq!(supply.external_supply[0], 2);
        assert_eq!(kernel.total_supply[0], 9);
        assert_eq!(kernel.total_supply[1], 12);
    }

    #[test]
    fn an_unaccounted_mint_increase_refuses_without_writing() {
        let (mut supply, mut kernel) = state();
        let before_supply = supply.clone();
        let before_kernel = kernel.clone();
        let observed = ObservedMintSupplies {
            values: {
                let mut value = [0; MAX_OUTCOMES];
                value[0] = 6;
                value[1] = 4;
                value
            },
            outcome_count: 2,
        };
        assert_eq!(
            synchronize_external_truth(&mut supply, &mut kernel, h(1), h(2), 2, &observed),
            Err(ClutchError::ShadowSupplyMismatch.into())
        );
        assert_eq!(supply, before_supply);
        assert_eq!(kernel, before_kernel);
    }

    #[test]
    fn whole_vector_delta_requires_untouched_mints_to_stay_exact() {
        let before = ObservedMintSupplies {
            values: {
                let mut value = [0; MAX_OUTCOMES];
                value[0] = 2;
                value[1] = 4;
                value
            },
            outcome_count: 2,
        };
        let mut after = before;
        after.values[0] = 5;
        assert_eq!(
            require_exact_mint_vector_delta(&before, &after, Some((0, true, 3))),
            Ok(())
        );
        after.values[1] = 5;
        assert_eq!(
            require_exact_mint_vector_delta(&before, &after, Some((0, true, 3))),
            Err(ClutchError::TokenDeltaMismatch.into())
        );
    }

    #[test]
    fn canonical_suffix_authenticates_order_authority_and_exact_mutability() {
        let market_key = Pubkey::new_from_array([9; 32]);
        let first_key = Pubkey::new_from_array([10; 32]);
        let second_key = Pubkey::new_from_array([11; 32]);
        let mut first = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: false,
            executable: false,
        };
        let mut second = MintCell {
            key: second_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 5),
            writable: true,
            executable: false,
        };
        let accounts = [first.info(), second.info()];
        let observed = observe_outcome_mints_with(
            &accounts,
            0,
            market_key,
            2,
            Some(1),
            |outcome| match outcome {
                0 => (first_key, 1),
                _ => (second_key, 2),
            },
        )
        .unwrap();
        assert_eq!(&observed.values[..2], &[3, 5]);
    }

    #[test]
    fn a_swapped_suffix_and_writable_untouched_mint_refuse() {
        let market_key = Pubkey::new_from_array([9; 32]);
        let first_key = Pubkey::new_from_array([10; 32]);
        let second_key = Pubkey::new_from_array([11; 32]);
        let mut first = MintCell {
            key: second_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: false,
            executable: false,
        };
        let mut second = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 5),
            writable: true,
            executable: false,
        };
        let accounts = [first.info(), second.info()];
        assert_eq!(
            observe_outcome_mints_with(
                &accounts,
                0,
                market_key,
                2,
                Some(1),
                |outcome| match outcome {
                    0 => (first_key, 1),
                    _ => (second_key, 2),
                }
            ),
            Err(ClutchError::WrongPda.into())
        );

        let mut first = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: true,
            executable: false,
        };
        let mut second = MintCell {
            key: second_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 5),
            writable: true,
            executable: false,
        };
        let accounts = [first.info(), second.info()];
        assert_eq!(
            observe_outcome_mints_with(
                &accounts,
                0,
                market_key,
                2,
                Some(1),
                |outcome| match outcome {
                    0 => (first_key, 1),
                    _ => (second_key, 2),
                }
            ),
            Err(ClutchError::UnexpectedWritable.into())
        );
    }

    #[test]
    fn malformed_or_incomplete_mint_vectors_fail_closed() {
        let market_key = Pubkey::new_from_array([9; 32]);
        let first_key = Pubkey::new_from_array([10; 32]);
        let second_key = Pubkey::new_from_array([11; 32]);
        let mut first = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: false,
            executable: false,
        };
        let accounts = [first.info()];
        assert_eq!(
            observe_outcome_mints_with(&accounts, 0, market_key, 2, None, |outcome| {
                match outcome {
                    0 => (first_key, 1),
                    _ => (second_key, 2),
                }
            }),
            Err(ClutchError::AccountCount.into())
        );

        let mut malformed = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: vec![0; token::BASE_MINT_LEN - 1],
            writable: false,
            executable: false,
        };
        let accounts = [malformed.info()];
        assert_eq!(
            observe_outcome_mints_with(&accounts, 0, market_key, 1, None, |_| (first_key, 1)),
            Err(ClutchError::TokenExtensionNotAllowed.into())
        );

        assert_eq!(
            observe_outcome_mints_with(&[], 0, market_key, (MAX_OUTCOMES + 1) as u8, None, |_| {
                (first_key, 1)
            }),
            Err(ClutchError::NonCanonical.into())
        );
    }

    #[test]
    fn hostile_runtime_roles_and_mint_authority_fail_closed() {
        let market_key = Pubkey::new_from_array([9; 32]);
        let first_key = Pubkey::new_from_array([10; 32]);
        let mut wrong_owner = MintCell {
            key: first_key,
            owner: Pubkey::new_from_array([77; 32]),
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: false,
            executable: false,
        };
        let accounts = [wrong_owner.info()];
        assert_eq!(
            observe_outcome_mints_with(&accounts, 0, market_key, 1, None, |_| (first_key, 1)),
            Err(ClutchError::WrongTokenProgram.into())
        );

        let mut executable = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(market_key.to_bytes(), 3),
            writable: false,
            executable: true,
        };
        let accounts = [executable.info()];
        assert_eq!(
            observe_outcome_mints_with(&accounts, 0, market_key, 1, None, |_| (first_key, 1)),
            Err(ClutchError::ExecutableAccount.into())
        );

        let wrong_authority = Pubkey::new_from_array([88; 32]);
        let mut authority_mismatch = MintCell {
            key: first_key,
            owner: token::TOKEN_2022_PROGRAM_ID,
            lamports: 1,
            data: token::fixtures::outcome_mint_bytes(wrong_authority.to_bytes(), 3),
            writable: false,
            executable: false,
        };
        let accounts = [authority_mismatch.info()];
        assert_eq!(
            observe_outcome_mints_with(&accounts, 0, market_key, 1, None, |_| (first_key, 1)),
            Err(ClutchError::MintNotAdmitted.into())
        );
    }
}
