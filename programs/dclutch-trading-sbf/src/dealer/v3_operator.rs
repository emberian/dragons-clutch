//! Chain-derived unsigned construction for Dealer V3 LP-account lifecycle.
//!
//! Junior equity contribution/redemption has a separate runtime-width request
//! in `v3_equity_operator`; this fixed wire owns only vacant Open and quiescent
//! Close. Every coordinate is copied from authenticated chain state.

use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_core_contract::ContentId;
use solana_program::{hash::hash, pubkey::Pubkey};

use super::v3_multi_lp::{DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpPositionV3};
use super::v3_obligation::{DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3};

/// Exact unsigned multi-LP request width.
pub const DEALER_MULTI_LP_REQUEST_BYTES_V3: usize = 312;
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
    Open = 7,
    /// Reclaim one zero-share LP Position to its immutable refund recipient.
    Close = 8,
}

/// Stable refusal from unsigned request construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpOperatorErrorV3 {
    /// Exact request bytes, reserved fields, or action invariants refused.
    InvalidRequest,
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
    /// Current Registry-selected Trading program which owns both Dealer PDAs.
    pub trading_program: [u8; 32],
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

/// Hostile-decoded canonical multi-LP request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerMultiLpRequestV3 {
    /// Requested lifecycle action.
    pub action: MultiLpRequestActionV3,
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical Trading-owned LP Position PDA.
    pub lp_position: [u8; 32],
    /// LP authority and external capital owner.
    pub lp_owner: [u8; 32],
    /// Canonical Trading-owned obligation PDA.
    pub obligation: [u8; 32],
    /// Optimistic digest of the exact obligation prestate.
    pub obligation_digest: [u8; 32],
    /// Optimistic digest of the exact LP Position prestate; zero only for Open.
    pub lp_digest: [u8; 32],
    /// Optimistic obligation revision.
    pub obligation_revision: u64,
    /// Optimistic LP Position revision; zero only for Open.
    pub lp_revision: u64,
    /// Current Core Market generation.
    pub generation: u64,
    /// Last admitted slot/time coordinate.
    pub expires_at: u64,
}

impl DealerMultiLpRequestV3 {
    /// Hostile-decode one exact request and enforce action-dependent canonicality.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiLpOperatorErrorV3> {
        if bytes.len() != DEALER_MULTI_LP_REQUEST_BYTES_V3
            || bytes.get(..8) != Some(DEALER_MULTI_LP_REQUEST_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_MULTI_LP_REQUEST_VERSION_V3
            || bytes.get(12..16).is_none_or(|value| value != [0; 4])
            || bytes
                .get(304..312)
                .is_none_or(|value| value.iter().any(|byte| *byte != 0))
        {
            return Err(MultiLpOperatorErrorV3::InvalidRequest);
        }
        let action = match read_u16(bytes, 10)? {
            7 => MultiLpRequestActionV3::Open,
            8 => MultiLpRequestActionV3::Close,
            _ => return Err(MultiLpOperatorErrorV3::InvalidRequest),
        };
        let value = Self {
            action,
            release_set: read_identity(bytes, 16)?,
            market: read_identity(bytes, 48)?,
            child_root: read_identity(bytes, 80)?,
            lp_position: read_identity(bytes, 112)?,
            lp_owner: read_identity(bytes, 144)?,
            obligation: read_identity(bytes, 176)?,
            obligation_digest: read_identity(bytes, 208)?,
            lp_digest: read_identity_or_zero(bytes, 240)?,
            obligation_revision: read_u64(bytes, 272)?,
            lp_revision: read_u64(bytes, 280)?,
            generation: read_u64(bytes, 288)?,
            expires_at: read_u64(bytes, 296)?,
        };
        if value.obligation_revision == 0 || value.generation == 0 {
            return Err(MultiLpOperatorErrorV3::InvalidRequest);
        }
        let action_is_canonical = match action {
            MultiLpRequestActionV3::Open => value.lp_revision == 0 && value.lp_digest == [0; 32],
            MultiLpRequestActionV3::Close => value.lp_revision != 0 && value.lp_digest != [0; 32],
        };
        if !action_is_canonical {
            return Err(MultiLpOperatorErrorV3::InvalidRequest);
        }
        Ok(value)
    }
}

/// Rejoin decoded request bytes to a current authenticated chain projection.
///
/// This is the physical adapter boundary used before any account creation,
/// Custody route, or Trading-owned write. It independently derives both PDAs,
/// rechecks every optimistic digest/revision, and applies the current expiry
/// and terminal lifecycle rules.
pub fn authenticate_multi_lp_request_v3(
    request: DealerMultiLpRequestV3,
    chain: MultiLpChainProjectionV3<'_>,
) -> Result<(), MultiLpOperatorErrorV3> {
    validate_projection(chain, request.lp_owner)?;
    if request.release_set != chain.release_set
        || request.market != chain.market
        || request.child_root != chain.child_root
        || request.lp_position != chain.lp_position_address
        || request.obligation != chain.obligation_address
        || request.obligation_digest != chain.obligation.state_digest()
        || request.obligation_revision != chain.obligation.revision()
        || request.generation != chain.generation
        || request.expires_at != chain.expires_at
        || chain.now > request.expires_at
    {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    match request.action {
        MultiLpRequestActionV3::Open => {
            if chain.terminal || chain.lp_position.is_some() || chain.lp_position_bytes.is_some() {
                return Err(MultiLpOperatorErrorV3::InvalidChoice);
            }
        }
        MultiLpRequestActionV3::Close => {
            let lp = authenticate_current_lp(request, chain)?;
            if lp.equity_shares != 0 {
                return Err(MultiLpOperatorErrorV3::InvalidChoice);
            }
        }
    }
    Ok(())
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
    if chain.terminal
        || chain.lp_position.is_some()
        || chain.lp_position_bytes.is_some()
        || lp_owner == [0; 32]
    {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, lp_owner, MultiLpRequestActionV3::Open, set)
}

/// Construct close after all equity shares have been burned.
pub fn build_close_lp_v3(
    chain: MultiLpChainProjectionV3<'_>,
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    let lp = current_lp(chain)?;
    if lp.equity_shares != 0 {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, lp.lp_owner, MultiLpRequestActionV3::Close, set)
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
    set: CapabilityProgramSetV1<'_>,
) -> Result<UnsignedMultiLpRequestV3, MultiLpOperatorErrorV3> {
    validate_projection(chain, lp_owner)?;
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
    write_bytes(&mut bytes, 0, &DEALER_MULTI_LP_REQUEST_MAGIC_V3)?;
    write_bytes(
        &mut bytes,
        8,
        &DEALER_MULTI_LP_REQUEST_VERSION_V3.to_le_bytes(),
    )?;
    write_bytes(&mut bytes, 10, &(action as u16).to_le_bytes())?;
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
        write_bytes(&mut bytes, offset, &identity)?;
    }
    for (offset, value) in [
        (272, chain.obligation.revision()),
        (280, lp_revision),
        (288, chain.generation),
        (296, chain.expires_at),
    ] {
        write_bytes(&mut bytes, offset, &value.to_le_bytes())?;
    }
    let selected_program = set
        .select(&bytes)
        .map_err(|_| MultiLpOperatorErrorV3::ProgramSelection)?;
    let request = DealerMultiLpRequestV3::decode(&bytes)?;
    authenticate_multi_lp_request_v3(request, chain)?;
    Ok(UnsignedMultiLpRequestV3 {
        bytes,
        selected_program,
    })
}

fn validate_projection(
    chain: MultiLpChainProjectionV3<'_>,
    lp_owner: [u8; 32],
) -> Result<(), MultiLpOperatorErrorV3> {
    for identity in [
        chain.trading_program,
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
    let trading = Pubkey::new_from_array(chain.trading_program);
    let expected_obligation = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &chain.child_root],
        &trading,
    )
    .0
    .to_bytes();
    let expected_lp = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &chain.child_root,
            &lp_owner,
        ],
        &trading,
    )
    .0
    .to_bytes();
    if chain.obligation_address != expected_obligation || chain.lp_position_address != expected_lp {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    Ok(())
}

fn authenticate_current_lp(
    request: DealerMultiLpRequestV3,
    chain: MultiLpChainProjectionV3<'_>,
) -> Result<DealerLpPositionV3, MultiLpOperatorErrorV3> {
    let lp = current_lp(chain)?;
    let bytes = chain
        .lp_position_bytes
        .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?;
    if lp.lp_owner != request.lp_owner
        || lp.revision != request.lp_revision
        || hash(bytes).to_bytes() != request.lp_digest
    {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    Ok(lp)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, MultiLpOperatorErrorV3> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(MultiLpOperatorErrorV3::InvalidRequest)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, MultiLpOperatorErrorV3> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(MultiLpOperatorErrorV3::InvalidRequest)
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], MultiLpOperatorErrorV3> {
    let value = read_identity_or_zero(bytes, offset)?;
    if value == [0; 32] {
        Err(MultiLpOperatorErrorV3::InvalidRequest)
    } else {
        Ok(value)
    }
}

fn read_identity_or_zero(bytes: &[u8], offset: usize) -> Result<[u8; 32], MultiLpOperatorErrorV3> {
    bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(MultiLpOperatorErrorV3::InvalidRequest)
}

fn write_bytes(
    bytes: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), MultiLpOperatorErrorV3> {
    let end = offset
        .checked_add(value.len())
        .ok_or(MultiLpOperatorErrorV3::InvalidRequest)?;
    bytes
        .get_mut(offset..end)
        .ok_or(MultiLpOperatorErrorV3::InvalidRequest)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dealer::{
        v3_multi_lp::DEALER_LP_POSITION_BYTES_V3,
        v3_obligation::{
            DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
            DEALER_OBLIGATION_VERSION_V3,
        },
    };

    fn program_set(action: MultiLpRequestActionV3) -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; 72];
        bytes[..8].copy_from_slice(b"DCLTCPS1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3.to_le_bytes());
        bytes[16] = 2;
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
        bytes[32..36].copy_from_slice(&(action as u32).to_le_bytes());
        bytes[36..68].copy_from_slice(&[42; 32]);
        bytes
    }

    fn obligation_bytes(child_root: [u8; 32]) -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + 16];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&2_u32.to_le_bytes());
        bytes[16..24].copy_from_slice(&7_u64.to_le_bytes());
        for (offset, identity) in [
            (24, [2; 32]),
            (56, [3; 32]),
            (88, [4; 32]),
            (120, [5; 32]),
            (152, child_root),
        ] {
            bytes[offset..offset + 32].copy_from_slice(&identity);
        }
        bytes[184..192].copy_from_slice(&10_u64.to_le_bytes());
        bytes[192..200].copy_from_slice(&20_u64.to_le_bytes());
        bytes[200..208].copy_from_slice(&21_u64.to_le_bytes());
        bytes
    }

    fn lp_bytes(
        release_set: [u8; 32],
        market: [u8; 32],
        child_root: [u8; 32],
        owner: [u8; 32],
        obligation: [u8; 32],
        shares: u64,
    ) -> [u8; DEALER_LP_POSITION_BYTES_V3] {
        let mut bytes = [0; DEALER_LP_POSITION_BYTES_V3];
        DealerLpPositionV3 {
            revision: 3,
            release_set,
            market,
            child_root,
            lp_owner: owner,
            rent_refund: [9; 32],
            obligation_account: obligation,
            equity_shares: shares,
            generation: 11,
            rent_principal: 50,
        }
        .encode_into(&mut bytes)
        .expect("LP state");
        bytes
    }

    #[test]
    fn open_is_chain_derived_and_hostile_bytes_refuse() {
        let trading_program = [1; 32];
        let release_set = [6; 32];
        let market = [2; 32];
        let child_root = [7; 32];
        let lp_owner = [8; 32];
        let trading = Pubkey::new_from_array(trading_program);
        let obligation_address =
            Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child_root], &trading)
                .0
                .to_bytes();
        let lp_position_address = Pubkey::find_program_address(
            &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &child_root, &lp_owner],
            &trading,
        )
        .0
        .to_bytes();
        let obligation_bytes = obligation_bytes(child_root);
        let obligation =
            DealerObligationProjectionV3::decode(&obligation_bytes).expect("obligation");
        let chain = MultiLpChainProjectionV3 {
            trading_program,
            release_set,
            market,
            child_root,
            lp_position_address,
            lp_position: None,
            lp_position_bytes: None,
            obligation,
            obligation_address,
            generation: 11,
            now: 20,
            expires_at: 25,
            terminal: false,
        };
        let set_bytes = program_set(MultiLpRequestActionV3::Open);
        let set = CapabilityProgramSetV1::decode(&set_bytes).expect("set");
        let unsigned = build_open_lp_v3(chain, lp_owner, set).expect("open");
        let request = DealerMultiLpRequestV3::decode(unsigned.as_bytes()).expect("request");
        assert_eq!(request.action, MultiLpRequestActionV3::Open);
        assert_eq!(request.lp_position, lp_position_address);
        assert_eq!(request.obligation, obligation_address);
        assert_eq!(unsigned.selected_program().to_bytes(), [42; 32]);

        for index in [0, 8, 12, 304] {
            let mut hostile = *unsigned.as_bytes();
            hostile[index] ^= 1;
            assert!(DealerMultiLpRequestV3::decode(&hostile).is_err());
        }
        for substitute in [
            |value: &mut DealerMultiLpRequestV3| value.lp_position[0] ^= 1,
            |value: &mut DealerMultiLpRequestV3| value.obligation_digest[0] ^= 1,
        ] {
            let mut substituted = request;
            substitute(&mut substituted);
            assert_eq!(
                authenticate_multi_lp_request_v3(substituted, chain),
                Err(MultiLpOperatorErrorV3::InvalidProjection)
            );
        }
    }

    #[test]
    fn close_binds_current_zero_share_position() {
        let trading_program = [1; 32];
        let release_set = [6; 32];
        let market = [2; 32];
        let child_root = [7; 32];
        let lp_owner = [8; 32];
        let trading = Pubkey::new_from_array(trading_program);
        let obligation_address =
            Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child_root], &trading)
                .0
                .to_bytes();
        let lp_position_address = Pubkey::find_program_address(
            &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &child_root, &lp_owner],
            &trading,
        )
        .0
        .to_bytes();
        let obligation_bytes = obligation_bytes(child_root);
        let obligation =
            DealerObligationProjectionV3::decode(&obligation_bytes).expect("obligation");
        let nonzero_lp_bytes = lp_bytes(
            release_set,
            market,
            child_root,
            lp_owner,
            obligation_address,
            10,
        );
        let lp = DealerLpPositionV3::decode(&nonzero_lp_bytes).expect("LP");
        let chain = MultiLpChainProjectionV3 {
            trading_program,
            release_set,
            market,
            child_root,
            lp_position_address,
            lp_position: Some(lp),
            lp_position_bytes: Some(&nonzero_lp_bytes),
            obligation,
            obligation_address,
            generation: 11,
            now: 20,
            expires_at: 25,
            terminal: false,
        };
        let close_set_bytes = program_set(MultiLpRequestActionV3::Close);
        let close_set = CapabilityProgramSetV1::decode(&close_set_bytes).expect("set");
        assert_eq!(
            build_close_lp_v3(chain, close_set),
            Err(MultiLpOperatorErrorV3::InvalidChoice)
        );
        let zero_bytes = lp_bytes(
            release_set,
            market,
            child_root,
            lp_owner,
            obligation_address,
            0,
        );
        let mut zero_chain = chain;
        zero_chain.lp_position = Some(DealerLpPositionV3::decode(&zero_bytes).expect("zero LP"));
        zero_chain.lp_position_bytes = Some(&zero_bytes);
        let close = build_close_lp_v3(zero_chain, close_set).expect("zero-share close");
        assert_eq!(
            DealerMultiLpRequestV3::decode(close.as_bytes())
                .expect("close request")
                .action,
            MultiLpRequestActionV3::Close
        );
    }
}
