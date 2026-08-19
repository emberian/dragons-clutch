//! Shared System-program construction for the market's founding state plane.
//!
//! `CreateMarket` used to require its state PDAs to arrive as correctly sized,
//! zero-filled, program-owned genesis accounts.  A wallet cannot produce that
//! prestate: only this program can sign for its PDAs.  This module owns the
//! mechanical repair and deliberately owns no founding values.  It derives,
//! preflights, rent-funds, allocates, and assigns the seven production state
//! targets; [`super::market_init`] remains the semantic owner of every byte
//! subsequently encoded into them.
//!
//! The legacy per-owner external shadow is intentionally absent.  Actual
//! Token-2022 mints and token accounts are the external claim truth.  The seven
//! targets are Market, Hoard, founding Position, kernel aggregate, founding
//! Replay, market-wide SupplyLedger, and Resolution.
//!
//! ## Atomicity and refusal order
//!
//! Every address and every absence condition is checked before the first CPI.
//! A target is absent only if it is writable, non-executable, System-owned,
//! zero-lamport, and zero-data.  A pre-funded system account is not treated as
//! a helpful donation: accepting it would make the System program's deeper
//! `CreateAccount` behavior the re-initialization rule and would let a third
//! party influence which refusal a market sees.
//!
//! Once the preflight passes, each account is created with the shared
//! [`super::genesis::create_pda_account`] primitive.  If any CPI or any later
//! market/token write refuses, SVM transaction atomicity rolls all earlier
//! creations back.  Off-chain `invoke_signed` is a silent no-op, so host tests
//! cover the exact preflight and size inventory while the real-SVM gate must
//! establish successful creation and rollback.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::ClutchError;
use crate::seeds;
use clutch_solana_layout::account_len;
use clutch_solana_reference::{KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

#[cfg(test)]
use super::genesis::MAX_PERMITTED_DATA_INCREASE;
use super::genesis::{create_pda_account, RentParameters, SYSTEM_PROGRAM_ID};

/// Exact number of program-owned state accounts a production market founds.
pub const MARKET_STATE_TARGET_COUNT: usize = 7;

/// Canonical bumps for the seven founding state accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketStateBumps {
    /// Market PDA bump.
    pub market: u8,
    /// Hoard state PDA bump.
    pub hoard: u8,
    /// Founding Position PDA bump.
    pub position: u8,
    /// Kernel aggregate PDA bump.
    pub kernel: u8,
    /// Founding Replay PDA bump.
    pub replay: u8,
    /// Market-wide SupplyLedger PDA bump.
    pub supply: u8,
    /// Resolution PDA bump.
    pub resolution: u8,
}

/// Identity preimages that derive the founding state addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketStateIdentity {
    /// Realm identity bytes used by the Market seed.
    pub realm: [u8; 32],
    /// Canonical market identity bytes.
    pub market: [u8; 32],
    /// Founding owner's wallet bytes.
    pub owner: [u8; 32],
    /// Founding Position generation; V1 market creation supplies zero.
    pub generation: u64,
}

/// The seven absent target accounts, named rather than indexed.
#[derive(Debug)]
pub struct MarketStateTargets<'accounts, 'info> {
    /// Market target.
    pub market: &'accounts AccountInfo<'info>,
    /// Hoard state target.
    pub hoard: &'accounts AccountInfo<'info>,
    /// Founding Position target.
    pub position: &'accounts AccountInfo<'info>,
    /// Kernel aggregate target.
    pub kernel: &'accounts AccountInfo<'info>,
    /// Founding Replay target.
    pub replay: &'accounts AccountInfo<'info>,
    /// Market-wide SupplyLedger target.
    pub supply: &'accounts AccountInfo<'info>,
    /// Resolution target.
    pub resolution: &'accounts AccountInfo<'info>,
}

/// Canonical bumps for one owner's Position and Replay lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerStateBumps {
    /// Position PDA bump.
    pub position: u8,
    /// Generation-scoped Replay PDA bump.
    pub replay: u8,
}

/// The two absent targets needed to admit a new market participant.
#[derive(Debug)]
pub struct OwnerStateTargets<'accounts, 'info> {
    /// Owner's Position target.
    pub position: &'accounts AccountInfo<'info>,
    /// Owner's generation-scoped Replay target.
    pub replay: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> MarketStateTargets<'accounts, 'info> {
    /// Return the target accounts in canonical creation order.
    pub fn ordered(&self) -> [&'accounts AccountInfo<'info>; MARKET_STATE_TARGET_COUNT] {
        [
            self.market,
            self.hoard,
            self.position,
            self.kernel,
            self.replay,
            self.supply,
            self.resolution,
        ]
    }
}

/// Refuse anything other than a genuinely absent System-owned account slot.
pub fn require_absent_target(account: &AccountInfo<'_>) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        *account.owner == SYSTEM_PROGRAM_ID
            && **account.lamports.borrow() == 0
            && account.data_is_empty(),
        ClutchError::AlreadyInitialized,
    )
}

/// Authenticate all target addresses before any lamports move.
#[inline(never)]
pub fn validate_market_state_addresses(
    program_id: &Pubkey,
    targets: &MarketStateTargets<'_, '_>,
    identity: &MarketStateIdentity,
    bumps: &MarketStateBumps,
) -> Outcome<()> {
    expect_pda(
        targets.market.key,
        seeds::market_pda(program_id, &identity.realm, &identity.market),
        Some(bumps.market),
    )?;
    expect_pda(
        targets.hoard.key,
        seeds::hoard_pda(program_id, &identity.market),
        Some(bumps.hoard),
    )?;
    expect_pda(
        targets.position.key,
        seeds::position_pda(program_id, &identity.market, &identity.owner),
        Some(bumps.position),
    )?;
    expect_pda(
        targets.kernel.key,
        seeds::kernel_pda(program_id, &identity.market),
        Some(bumps.kernel),
    )?;
    expect_pda(
        targets.replay.key,
        seeds::replay_pda(
            program_id,
            &identity.market,
            &identity.owner,
            identity.generation,
        ),
        Some(bumps.replay),
    )?;
    expect_pda(
        targets.supply.key,
        seeds::supply_pda(program_id, &identity.market),
        Some(bumps.supply),
    )?;
    expect_pda(
        targets.resolution.key,
        seeds::resolution_pda(program_id, &identity.market),
        Some(bumps.resolution),
    )
}

/// Require all seven targets absent before the first account-creation CPI.
pub fn preflight_absent_market_state(targets: &MarketStateTargets<'_, '_>) -> Outcome<()> {
    for target in targets.ordered() {
        require_absent_target(target)?;
    }
    Ok(())
}

/// Authenticate and create one absent owner plane.
///
/// This is used by the first backed `Endow` for a wallet. It performs both
/// absence checks before the first CPI, so a mixed existing/absent plane cannot
/// partially allocate state.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn create_owner_state_plane<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &RentParameters,
    targets: &OwnerStateTargets<'_, 'info>,
    market: &[u8; 32],
    owner: &[u8; 32],
    generation: u64,
    bumps: &OwnerStateBumps,
) -> Outcome<()> {
    expect_pda(
        targets.position.key,
        seeds::position_pda(program_id, market, owner),
        Some(bumps.position),
    )?;
    expect_pda(
        targets.replay.key,
        seeds::replay_pda(program_id, market, owner, generation),
        Some(bumps.replay),
    )?;
    require_absent_target(targets.position)?;
    require_absent_target(targets.replay)?;

    let position_bump = [bumps.position];
    create_pda_account(
        program_id,
        payer,
        targets.position,
        system_program,
        rent,
        account_len::POSITION,
        &[seeds::SEED_POSITION, market, owner, &position_bump],
    )?;

    let generation_bytes = generation.to_le_bytes();
    let replay_bump = [bumps.replay];
    create_pda_account(
        program_id,
        payer,
        targets.replay,
        system_program,
        rent,
        REPLAY_ACCOUNT_LEN,
        &[
            seeds::SEED_REPLAY,
            market,
            owner,
            &generation_bytes,
            &replay_bump,
        ],
    )
}

/// System-CPI-create the complete seven-account founding state plane.
#[allow(clippy::too_many_arguments)] // explicit runtime roles and seed preimages
#[inline(never)]
pub fn create_market_state_plane<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &RentParameters,
    targets: &MarketStateTargets<'_, 'info>,
    identity: &MarketStateIdentity,
    bumps: &MarketStateBumps,
) -> Outcome<()> {
    validate_market_state_addresses(program_id, targets, identity, bumps)?;
    preflight_absent_market_state(targets)?;

    let market_bump = [bumps.market];
    create_pda_account(
        program_id,
        payer,
        targets.market,
        system_program,
        rent,
        account_len::MARKET,
        &[
            seeds::SEED_MARKET,
            &identity.realm,
            &identity.market,
            &market_bump,
        ],
    )?;

    let hoard_bump = [bumps.hoard];
    create_pda_account(
        program_id,
        payer,
        targets.hoard,
        system_program,
        rent,
        account_len::HOARD,
        &[seeds::SEED_HOARD, &identity.market, &hoard_bump],
    )?;

    let position_bump = [bumps.position];
    create_pda_account(
        program_id,
        payer,
        targets.position,
        system_program,
        rent,
        account_len::POSITION,
        &[
            seeds::SEED_POSITION,
            &identity.market,
            &identity.owner,
            &position_bump,
        ],
    )?;

    let kernel_bump = [bumps.kernel];
    create_pda_account(
        program_id,
        payer,
        targets.kernel,
        system_program,
        rent,
        KERNEL_ACCOUNT_LEN,
        &[seeds::SEED_KERNEL, &identity.market, &kernel_bump],
    )?;

    let generation = identity.generation.to_le_bytes();
    let replay_bump = [bumps.replay];
    create_pda_account(
        program_id,
        payer,
        targets.replay,
        system_program,
        rent,
        REPLAY_ACCOUNT_LEN,
        &[
            seeds::SEED_REPLAY,
            &identity.market,
            &identity.owner,
            &generation,
            &replay_bump,
        ],
    )?;

    let supply_bump = [bumps.supply];
    create_pda_account(
        program_id,
        payer,
        targets.supply,
        system_program,
        rent,
        account_len::SUPPLY_LEDGER,
        &[seeds::SEED_SUPPLY, &identity.market, &supply_bump],
    )?;

    let resolution_bump = [bumps.resolution];
    create_pda_account(
        program_id,
        payer,
        targets.resolution,
        system_program,
        rent,
        account_len::RESOLUTION,
        &[seeds::SEED_RESOLUTION, &identity.market, &resolution_bump],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        writable: bool,
        executable: bool,
    }

    impl Cell {
        fn absent() -> Self {
            Self {
                key: Pubkey::new_from_array([7; 32]),
                owner: SYSTEM_PROGRAM_ID,
                lamports: 0,
                data: Vec::new(),
                writable: true,
                executable: false,
            }
        }

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

    #[test]
    fn absence_is_stricter_than_zero_data() {
        let mut good = Cell::absent();
        assert_eq!(require_absent_target(&good.info()), Ok(()));

        let mut prefunded = Cell::absent();
        prefunded.lamports = 1;
        assert_eq!(
            require_absent_target(&prefunded.info()),
            Err(ClutchError::AlreadyInitialized.into())
        );

        let mut allocated = Cell::absent();
        allocated.data.push(0);
        assert_eq!(
            require_absent_target(&allocated.info()),
            Err(ClutchError::AlreadyInitialized.into())
        );

        let mut program_owned = Cell::absent();
        program_owned.owner = Pubkey::new_from_array([9; 32]);
        assert_eq!(
            require_absent_target(&program_owned.info()),
            Err(ClutchError::AlreadyInitialized.into())
        );

        let mut readonly = Cell::absent();
        readonly.writable = false;
        assert_eq!(
            require_absent_target(&readonly.info()),
            Err(ClutchError::NotWritable.into())
        );

        let mut executable = Cell::absent();
        executable.executable = true;
        assert_eq!(
            require_absent_target(&executable.info()),
            Err(ClutchError::ExecutableAccount.into())
        );
    }

    #[test]
    fn every_founding_state_target_fits_one_cpi() {
        let lengths = [
            account_len::MARKET,
            account_len::HOARD,
            account_len::POSITION,
            KERNEL_ACCOUNT_LEN,
            REPLAY_ACCOUNT_LEN,
            account_len::SUPPLY_LEDGER,
            account_len::RESOLUTION,
        ];
        assert_eq!(lengths.len(), MARKET_STATE_TARGET_COUNT);
        for length in lengths {
            assert!(length <= MAX_PERMITTED_DATA_INCREASE, "{length}");
        }
    }
}
