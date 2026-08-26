//! Trading-owned canonical terminal-obligation projection for Dealer V3.
//!
//! The account is the one persisted owner of Dealer terminal obligations.  It
//! is a PDA of the canonical Trading program beneath the immutable child root;
//! callers may borrow its runtime-width vector, but may not supply a parallel
//! obligation DTO. The vector includes all admitted liabilities, while
//! `total_equity_shares` records the outstanding junior ownership supply.
//! Equity shares are not obligations: their value is derived exclusively from
//! the authenticated residual vector `capital + Claims - obligations`.
//! Realized fees have no field in this account.

use dclutch_dealer_codec::scenario::ScenarioSolvencyDescriptor;
use solana_program::{hash::hash, pubkey::Pubkey};

/// PDA domain for the one Dealer V3 obligation account beneath a Trading root.
pub const DEALER_OBLIGATION_PDA_DOMAIN_V3: &[u8] = b"dclutch:dealer-obligation:v3";
/// Exact fixed header before the runtime-width obligation vector.
pub const DEALER_OBLIGATION_HEADER_BYTES_V3: usize = 192;
/// Wire magic for the Trading-owned obligation account.
pub const DEALER_OBLIGATION_MAGIC_V3: [u8; 8] = *b"DCLDOB03";
/// Current wire version.
pub const DEALER_OBLIGATION_VERSION_V3: u16 = 2;

const _: () = assert!(DEALER_OBLIGATION_PDA_DOMAIN_V3.len() <= 32);

/// Stable refusal at the canonical obligation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObligationErrorV3 {
    /// Account bytes had an impossible or noncanonical shape.
    InvalidBytes,
    /// A required identity was zero.
    ZeroIdentity,
    /// The account key or owner was not the canonical Trading PDA/program.
    AccountMismatch,
    /// Market, Product, basis, Position owner, child root, or width differed.
    CoordinateMismatch,
    /// Optimistic revision or exact state digest was stale.
    StaleState,
    /// An equity-share supply transition was zero, stale, or underflowed.
    InvalidShareSupply,
    /// Checked obligation or revision arithmetic failed.
    Arithmetic,
}

/// Result alias for Dealer V3 obligation projection.
pub type ObligationResultV3<T> = core::result::Result<T, ObligationErrorV3>;

/// Exact immutable coordinates expected by a Dealer V3 action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObligationExpectationV3 {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Stable semantic Product identity.
    pub product: [u8; 32],
    /// Stable semantic LiabilityBasis identity.
    pub liability_basis: [u8; 32],
    /// Canonical Dealer Claims Position owner.
    pub position_owner: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Optimistic obligation revision.
    pub revision: u64,
    /// Runtime Product width.
    pub width: u32,
    /// Digest of the exact prestate bytes bound by the parent request.
    pub state_digest: [u8; 32],
}

/// Hostile account observation supplied by the small SVM adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObligationAccountObservationV3<'a> {
    /// Observed account address.
    pub address: [u8; 32],
    /// Observed SVM account owner.
    pub owner: [u8; 32],
    /// Current account data.
    pub data: &'a [u8],
}

/// Immutable coordinates for first creation of the Dealer obligation PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObligationOpenInputV3 {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Stable semantic Product identity.
    pub product: [u8; 32],
    /// Stable semantic LiabilityBasis identity.
    pub liability_basis: [u8; 32],
    /// Canonical Dealer Claims Position owner, normally the Trading child root.
    pub position_owner: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Runtime Product outcome width.
    pub width: u32,
}

/// Exact write-last candidate for one vacant obligation PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObligationOpenPlanV3 {
    /// Canonical Trading PDA allocated by the common lifecycle executor.
    pub obligation: [u8; 32],
    /// Exact count-derived account data width.
    pub data_bytes: usize,
    /// Digest of the exact initial all-zero-obligation state.
    pub initial_digest: [u8; 32],
    /// Initial optimistic revision.
    pub initial_revision: u64,
}

/// Exact quiescent reclamation candidate for the obligation PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObligationClosePlanV3 {
    /// Canonical Trading PDA reclaimed by the common lifecycle executor.
    pub obligation: [u8; 32],
    /// Digest of the exact terminal zero-obligation prestate.
    pub prestate_digest: [u8; 32],
    /// Final optimistic revision.
    pub terminal_revision: u64,
    /// Exact count-derived data width being reclaimed.
    pub data_bytes: usize,
}

/// Exact count-derived obligation account width.
pub fn obligation_account_bytes_v3(width: u32) -> ObligationResultV3<usize> {
    if width == 0 {
        return Err(ObligationErrorV3::InvalidBytes);
    }
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(8))
        .and_then(|body| DEALER_OBLIGATION_HEADER_BYTES_V3.checked_add(body))
        .ok_or(ObligationErrorV3::InvalidBytes)
}

/// Prepare the exact initial state of one vacant obligation PDA.
///
/// Allocation, assignment, prepaid-rent validation, and immutable refund
/// selection remain owned by the common state-lifecycle executor. This
/// family adapter owns only the PDA identity and semantic bytes committed last.
pub fn prepare_obligation_open_v3(
    trading_program: [u8; 32],
    observed: ObligationAccountObservationV3<'_>,
    input: ObligationOpenInputV3,
    output: &mut [u8],
) -> ObligationResultV3<ObligationOpenPlanV3> {
    require_identity(trading_program)?;
    for identity in [
        input.market,
        input.product,
        input.liability_basis,
        input.position_owner,
        input.child_root,
    ] {
        require_identity(identity)?;
    }
    let bytes = obligation_account_bytes_v3(input.width)?;
    let expected_address = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &input.child_root],
        &Pubkey::new_from_array(trading_program),
    )
    .0
    .to_bytes();
    if observed.address != expected_address
        || observed.owner != solana_system_interface::program::ID.to_bytes()
        || !observed.data.is_empty()
        || output.len() != bytes
    {
        return Err(ObligationErrorV3::AccountMismatch);
    }
    output.fill(0);
    write_bytes(output, 0, &DEALER_OBLIGATION_MAGIC_V3)?;
    write_bytes(output, 8, &DEALER_OBLIGATION_VERSION_V3.to_le_bytes())?;
    write_bytes(output, 12, &input.width.to_le_bytes())?;
    write_u64(output, 16, 1)?;
    for (offset, identity) in [
        (24, input.market),
        (56, input.product),
        (88, input.liability_basis),
        (120, input.position_owner),
        (152, input.child_root),
    ] {
        write_bytes(output, offset, &identity)?;
    }
    DealerObligationProjectionV3::decode(output)?;
    Ok(ObligationOpenPlanV3 {
        obligation: expected_address,
        data_bytes: bytes,
        initial_digest: hash(output).to_bytes(),
        initial_revision: 1,
    })
}

/// Admit reclamation only after all equity shares and scenario obligations are zero.
pub fn prepare_obligation_close_v3(
    trading_program: [u8; 32],
    observed: ObligationAccountObservationV3<'_>,
    expected: ObligationExpectationV3,
) -> ObligationResultV3<ObligationClosePlanV3> {
    let projection =
        DealerObligationProjectionV3::authenticate(trading_program, observed, expected)?;
    if projection.total_equity_shares() != 0 || projection.obligations().any(|value| value != 0) {
        return Err(ObligationErrorV3::InvalidShareSupply);
    }
    Ok(ObligationClosePlanV3 {
        obligation: observed.address,
        prestate_digest: projection.state_digest(),
        terminal_revision: projection.revision(),
        data_bytes: observed.data.len(),
    })
}

/// Authenticated borrowed canonical obligation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerObligationProjectionV3<'a> {
    bytes: &'a [u8],
    market: [u8; 32],
    product: [u8; 32],
    liability_basis: [u8; 32],
    position_owner: [u8; 32],
    child_root: [u8; 32],
    revision: u64,
    total_equity_shares: u64,
    width: u32,
}

impl<'a> DealerObligationProjectionV3<'a> {
    /// Decode and authenticate the sole Trading-owned obligation account.
    pub fn authenticate(
        trading_program: [u8; 32],
        observation: ObligationAccountObservationV3<'a>,
        expected: ObligationExpectationV3,
    ) -> ObligationResultV3<Self> {
        require_identity(trading_program)?;
        for identity in [
            expected.market,
            expected.product,
            expected.liability_basis,
            expected.position_owner,
            expected.child_root,
            expected.state_digest,
        ] {
            require_identity(identity)?;
        }
        let expected_address = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &expected.child_root],
            &Pubkey::new_from_array(trading_program),
        )
        .0
        .to_bytes();
        if observation.owner != trading_program || observation.address != expected_address {
            return Err(ObligationErrorV3::AccountMismatch);
        }
        if hash(observation.data).to_bytes() != expected.state_digest {
            return Err(ObligationErrorV3::StaleState);
        }
        let value = Self::decode(observation.data)?;
        if value.market != expected.market
            || value.product != expected.product
            || value.liability_basis != expected.liability_basis
            || value.position_owner != expected.position_owner
            || value.child_root != expected.child_root
            || value.width != expected.width
        {
            return Err(ObligationErrorV3::CoordinateMismatch);
        }
        if value.revision != expected.revision {
            return Err(ObligationErrorV3::StaleState);
        }
        Ok(value)
    }

    /// Hostile-decode exact bytes after account authentication.
    pub fn decode(bytes: &'a [u8]) -> ObligationResultV3<Self> {
        if bytes.len() < DEALER_OBLIGATION_HEADER_BYTES_V3
            || bytes.get(..8) != Some(DEALER_OBLIGATION_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_OBLIGATION_VERSION_V3
            || bytes.get(10..12).is_none_or(|reserved| reserved != [0, 0])
        {
            return Err(ObligationErrorV3::InvalidBytes);
        }
        let width = read_u32(bytes, 12)?;
        let vector_bytes = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(8))
            .and_then(|body| DEALER_OBLIGATION_HEADER_BYTES_V3.checked_add(body))
            .ok_or(ObligationErrorV3::InvalidBytes)?;
        if width == 0 || bytes.len() != vector_bytes {
            return Err(ObligationErrorV3::InvalidBytes);
        }
        let value = Self {
            bytes,
            revision: read_u64(bytes, 16)?,
            market: read_identity(bytes, 24)?,
            product: read_identity(bytes, 56)?,
            liability_basis: read_identity(bytes, 88)?,
            position_owner: read_identity(bytes, 120)?,
            child_root: read_identity(bytes, 152)?,
            total_equity_shares: read_u64(bytes, 184)?,
            width,
        };
        if value.revision == 0 {
            return Err(ObligationErrorV3::InvalidBytes);
        }
        Ok(value)
    }

    /// Project the exact descriptor consumed by the sole scenario planner.
    pub const fn descriptor(self, locked_capital_floor: u64) -> ScenarioSolvencyDescriptor {
        ScenarioSolvencyDescriptor {
            market_id: self.market,
            product_id: self.product,
            liability_basis_id: self.liability_basis,
            position_owner: self.position_owner,
            locked_capital_floor,
        }
    }

    /// Borrow the exact runtime-width obligation bytes.
    pub fn obligations(self) -> ObligationIterV3<'a> {
        ObligationIterV3 {
            bytes: self.bytes,
            index: 0,
            width: self.width,
        }
    }

    /// Decode one indexed terminal obligation.
    pub fn obligation(self, index: u32) -> ObligationResultV3<u64> {
        if index >= self.width {
            return Err(ObligationErrorV3::InvalidBytes);
        }
        let offset = usize::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(8))
            .and_then(|index| DEALER_OBLIGATION_HEADER_BYTES_V3.checked_add(index))
            .ok_or(ObligationErrorV3::InvalidBytes)?;
        read_u64(self.bytes, offset)
    }

    /// Current optimistic revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Runtime terminal-scenario width.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Exact outstanding junior equity-share supply.
    pub const fn total_equity_shares(self) -> u64 {
        self.total_equity_shares
    }

    /// Canonical Dealer Claims Position owner.
    pub const fn position_owner(self) -> [u8; 32] {
        self.position_owner
    }

    /// Immutable Trading child root.
    pub const fn child_root(self) -> [u8; 32] {
        self.child_root
    }

    /// Hash of the exact authenticated state bytes.
    pub fn state_digest(self) -> [u8; 32] {
        hash(self.bytes).to_bytes()
    }
}

/// Exact junior equity-share supply delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityShareDeltaV3 {
    /// Mint shares for an exact proportional scenario contribution.
    Mint(u64),
    /// Burn shares for the canonical pro-rata residual payout.
    Burn(u64),
}

/// Stage a complete write-last share-supply candidate.
///
/// `output` remains byte-for-byte unchanged on every refusal.
pub fn stage_equity_share_supply_v3(
    current: DealerObligationProjectionV3<'_>,
    delta: EquityShareDeltaV3,
    output: &mut [u8],
) -> ObligationResultV3<()> {
    if output.len() != current.bytes.len() {
        return Err(ObligationErrorV3::InvalidBytes);
    }
    let amount = match delta {
        EquityShareDeltaV3::Mint(amount) | EquityShareDeltaV3::Burn(amount) => amount,
    };
    if amount == 0 {
        return Err(ObligationErrorV3::InvalidShareSupply);
    }
    let next_revision = current
        .revision
        .checked_add(1)
        .ok_or(ObligationErrorV3::Arithmetic)?;
    let next_supply = match delta {
        EquityShareDeltaV3::Mint(_) => current.total_equity_shares.checked_add(amount),
        EquityShareDeltaV3::Burn(_) => current.total_equity_shares.checked_sub(amount),
    }
    .ok_or(ObligationErrorV3::InvalidShareSupply)?;

    output.copy_from_slice(current.bytes);
    write_u64(output, 16, next_revision)?;
    write_u64(output, 184, next_supply)?;
    DealerObligationProjectionV3::decode(output).map(|_| ())
}

/// Allocation-free iterator over exact obligation values.
pub struct ObligationIterV3<'a> {
    bytes: &'a [u8],
    index: u32,
    width: u32,
}

impl Iterator for ObligationIterV3<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.width {
            return None;
        }
        let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + usize::try_from(self.index).ok()? * 8;
        self.index = self.index.checked_add(1)?;
        read_u64(self.bytes, offset).ok()
    }
}

fn require_identity(identity: [u8; 32]) -> ObligationResultV3<()> {
    if identity == [0; 32] {
        Err(ObligationErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn read_identity(bytes: &[u8], offset: usize) -> ObligationResultV3<[u8; 32]> {
    let value: [u8; 32] = bytes
        .get(offset..offset + 32)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(ObligationErrorV3::InvalidBytes)?;
    require_identity(value)?;
    Ok(value)
}

fn read_u16(bytes: &[u8], offset: usize) -> ObligationResultV3<u16> {
    bytes
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ObligationErrorV3::InvalidBytes)
}

fn read_u32(bytes: &[u8], offset: usize) -> ObligationResultV3<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ObligationErrorV3::InvalidBytes)
}

fn read_u64(bytes: &[u8], offset: usize) -> ObligationResultV3<u64> {
    bytes
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ObligationErrorV3::InvalidBytes)
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> ObligationResultV3<()> {
    bytes
        .get_mut(offset..offset + 8)
        .ok_or(ObligationErrorV3::InvalidBytes)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> ObligationResultV3<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ObligationErrorV3::InvalidBytes)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ObligationErrorV3::InvalidBytes)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(obligations: &[u64], lp: u64) -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + obligations.len() * 8];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&(obligations.len() as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&7_u64.to_le_bytes());
        for (offset, value) in [
            (24, [1; 32]),
            (56, [2; 32]),
            (88, [3; 32]),
            (120, [4; 32]),
            (152, [5; 32]),
        ] {
            bytes[offset..offset + 32].copy_from_slice(&value);
        }
        bytes[184..192].copy_from_slice(&lp.to_le_bytes());
        for (index, obligation) in obligations.iter().enumerate() {
            let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
            bytes[offset..offset + 8].copy_from_slice(&obligation.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn authenticates_trading_pda_and_exact_digest() {
        let program = [9; 32];
        let data = bytes(&[10, 11, 12], 8);
        let address = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &[5; 32]],
            &Pubkey::new_from_array(program),
        )
        .0
        .to_bytes();
        let expected = ObligationExpectationV3 {
            market: [1; 32],
            product: [2; 32],
            liability_basis: [3; 32],
            position_owner: [4; 32],
            child_root: [5; 32],
            revision: 7,
            width: 3,
            state_digest: hash(&data).to_bytes(),
        };
        let projection = DealerObligationProjectionV3::authenticate(
            program,
            ObligationAccountObservationV3 {
                address,
                owner: program,
                data: &data,
            },
            expected,
        )
        .expect("canonical projection");
        assert_eq!(
            projection.obligations().collect::<std::vec::Vec<_>>(),
            [10, 11, 12]
        );
        assert_eq!(projection.total_equity_shares(), 8);
    }

    #[test]
    fn equity_supply_changes_without_rewriting_external_obligations() {
        let data = bytes(&[10, 11, 12], 8);
        let current = DealerObligationProjectionV3::decode(&data).expect("state");
        let mut post = std::vec![0; data.len()];
        stage_equity_share_supply_v3(current, EquityShareDeltaV3::Mint(5), &mut post)
            .expect("equity issue");
        let post = DealerObligationProjectionV3::decode(&post).expect("post");
        assert_eq!(post.total_equity_shares(), 13);
        assert_eq!(
            post.obligations().collect::<std::vec::Vec<_>>(),
            [10, 11, 12]
        );

        let mut untouched = std::vec![0xa5; data.len()];
        assert_eq!(
            stage_equity_share_supply_v3(current, EquityShareDeltaV3::Burn(9), &mut untouched),
            Err(ObligationErrorV3::InvalidShareSupply),
        );
        assert!(untouched.iter().all(|byte| *byte == 0xa5));
    }
}
