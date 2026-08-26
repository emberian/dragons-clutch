//! Chain-derived unsigned construction for Dealer V3 multi-LP actions.
//!
//! The operator accepts only an authenticated chain projection.  User choice
//! is limited to a positive principal amount (or no amount for open/close),
//! while every identity, digest, revision, generation, and expiry coordinate
//! is copied from chain state.  The family-neutral CapabilityProgramSetV1 then
//! selects the complete action bundle from the exact request selector.

use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_core_contract::ContentId;
use solana_program::hash::hash;

use super::v3_multi_lp::DealerLpPositionV3;
use super::v3_obligation::DealerObligationProjectionV3;

/// Exact unsigned multi-LP request width.
pub const DEALER_MULTI_LP_REQUEST_BYTES_V3: usize = 320;
/// Request magic.
pub const DEALER_MULTI_LP_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLMLP03";
/// Current request version.
pub const DEALER_MULTI_LP_REQUEST_VERSION_V3: u16 = 1;
/// CapabilityProgramSet selector offset in every multi-LP request.
pub const DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3: u32 = 10;

/// Exact action selector; no executable dispatch is encoded here.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpRequestActionV3 {
    /// Create one vacant prepaid Trading-owned LP Position.
    Open = 1,
    /// Deposit present external collateral and admit equal par obligations.
    Add = 2,
    /// Remove equal par obligations and return present collateral.
    Remove = 3,
    /// Reclaim one zero-share LP Position to its immutable refund recipient.
    Close = 4,
}

/// Stable refusal from unsigned request construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpOperatorErrorV3 {
    /// The authenticated chain projection was internally inconsistent.
    InvalidProjection,
    /// The economic choice was not admitted in the current lifecycle state.
    InvalidChoice,
    /// The family-neutral program set did not admit the exact action request.
    ProgramSelection,
}

/// Authenticated chain snapshot for one request-construction batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpChainProjectionV3<'a> {
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical LP Position PDA.
    pub lp_position_address: [u8; 32],
    /// Exact decoded LP Position; absent only for Open.
    pub lp_position: Option<DealerLpPositionV3>,
    /// Exact LP Position data when present.
    pub lp_position_bytes: Option<&'a [u8]>,
    /// Authenticated canonical obligation projection.
    pub obligation: DealerObligationProjectionV3<'a>,
    /// Exact obligation account address.
    pub obligation_address: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// Current slot/time coordinate copied into the expiry bound.
    pub now: u64,
    /// Last admitted slot/time for this unsigned request.
    pub expires_at: u64,
    /// Whether Core has entered terminal settlement.
    pub terminal: bool,
}

/// Exact request bytes and selected CapabilityProgramV3 identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedMultiLpRequestV3 {
    bytes: [u8; DEALER_MULTI_LP_REQUEST_BYTES_V3],
    selected_program: ContentId,
}

impl UnsignedMultiLpRequestV3 {
    /// Borrow the exact request bytes for wallet signing.
    pub const fn as_bytes(&self) -> &[u8; DEALER_MULTI_LP_REQUEST_BYTES_V3] {
        &self.bytes
    }

    /// Exact CapabilityProgramV3 content selected by the authenticated set.
    pub const fn selected_program(self) -> ContentId {
        self.selected_program
    }
}

/// Construct Open for a chain-derived vacant LP PDA.
pub fn build_open_lp_v3(
    chain: MultiLpChainProjectionV3<'_>,
    lp_owner: [u8; 32],
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    if chain.lp_position.is_some() || chain.lp_position_bytes.is_some() || lp_owner == [0; 32] {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, lp_owner, MultiLpRequestActionV3::Open, 0, set)
}

/// Construct an exact present-capital deposit.
pub fn build_add_lp_v3(
    chain: MultiLpChainProjectionV3<'_>,
    amount: u64,
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    if chain.terminal || amount == 0 {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    let lp = current_lp(chain)?;
    build(chain, lp.lp_owner, MultiLpRequestActionV3::Add, amount, set)
}

/// Construct an exact principal withdrawal or terminal par redemption.
pub fn build_remove_lp_v3(
    chain: MultiLpChainProjectionV3<'_>,
    amount: u64,
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    let lp = current_lp(chain)?;
    if amount == 0 || amount > lp.principal_shares {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(
        chain,
        lp.lp_owner,
        MultiLpRequestActionV3::Remove,
        amount,
        set,
    )
}

/// Construct close after all principal shares have been returned.
pub fn build_close_lp_v3(
    chain: MultiLpChainProjectionV3<'_>,
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    let lp = current_lp(chain)?;
    if lp.principal_shares != 0 {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, lp.lp_owner, MultiLpRequestActionV3::Close, 0, set)
}

fn current_lp(
    chain: MultiLpChainProjectionV3<'_>,
) -> Result<DealerLpPositionV3, MultiLpOperatorErrorV3> {
    let lp = chain
        .lp_position
        .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?;
    let bytes = chain
        .lp_position_bytes
        .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?;
    if DealerLpPositionV3::decode(bytes) != Ok(lp) {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    Ok(lp)
}

fn build(
    chain: MultiLpChainProjectionV3<'_>,
    lp_owner: [u8; 32],
    action: MultiLpRequestActionV3,
    amount: u64,
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    for identity in [
        chain.release_set,
        chain.market,
        chain.child_root,
        chain.lp_position_address,
        lp_owner,
        chain.obligation_address,
    ] {
        if identity == [0; 32] {
            return Err(MultiLpOperatorErrorV3::InvalidProjection);
        }
    }
    if chain.expires_at < chain.now
        || chain.obligation.child_root() != chain.child_root
        || chain.generation == 0
    {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    let (lp_revision, lp_digest) = match chain.lp_position_bytes {
        Some(bytes) => (
            chain
                .lp_position
                .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?
                .revision,
            hash(bytes).to_bytes(),
        ),
        None if action == MultiLpRequestActionV3::Open => (0, [0; 32]),
        None => return Err(MultiLpOperatorErrorV3::InvalidProjection),
    };
    let mut bytes = [0; DEALER_MULTI_LP_REQUEST_BYTES_V3];
    bytes[..8].copy_from_slice(&DEALER_MULTI_LP_REQUEST_MAGIC_V3);
    bytes[8..10].copy_from_slice(&DEALER_MULTI_LP_REQUEST_VERSION_V3.to_le_bytes());
    bytes[10..12].copy_from_slice(&(action as u16).to_le_bytes());
    for (offset, identity) in [
        (16, chain.release_set),
        (48, chain.market),
        (80, chain.child_root),
        (112, chain.lp_position_address),
        (144, lp_owner),
        (176, chain.obligation_address),
        (208, chain.obligation.state_digest()),
        (240, lp_digest),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&identity);
    }
    for (offset, value) in [
        (272, amount),
        (280, chain.obligation.revision()),
        (288, lp_revision),
        (296, chain.generation),
        (304, chain.expires_at),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    let selected_program = set
        .select(&bytes)
        .map_err(|_| MultiLpOperatorErrorV3::ProgramSelection)?;
    Ok(UnsignedMultiLpRequestV3 {
        bytes,
        selected_program,
    })
}
