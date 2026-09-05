//! Schema-bound unsigned construction for Dealer LP Open and Close.
//!
//! The request wire and chain authentication remain owned by `lp_request`.
//! This successor adapter changes only release selection: the sole mixed
//! Dealer `CapabilityProgramSetV2` returns the exact descriptor schema/content
//! pair rather than an untyped descriptor content identity.

use dclutch_capability_program_contract::set_v2::{
    CapabilityDescriptorReferenceV2, CapabilityProgramSetV2, SelectorWidthV2,
};

use super::lp_request::{
    DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3, DEALER_MULTI_LP_REQUEST_BYTES_V3,
    MultiLpChainProjectionV3, MultiLpOperatorErrorV3, MultiLpRequestActionV3,
    build_multi_lp_request_bytes_v3,
};

/// Exact unsigned LP request and schema-bound V4 descriptor selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedMultiLpRequestV4 {
    bytes: [u8; DEALER_MULTI_LP_REQUEST_BYTES_V3],
    selected_descriptor: CapabilityDescriptorReferenceV2,
}

impl UnsignedMultiLpRequestV4 {
    /// Borrow the exact request bytes for wallet signing or Hot construction.
    pub const fn as_bytes(&self) -> &[u8; DEALER_MULTI_LP_REQUEST_BYTES_V3] {
        &self.bytes
    }

    /// Exact finalized descriptor schema/content selected by the global set.
    pub const fn selected_descriptor(self) -> CapabilityDescriptorReferenceV2 {
        self.selected_descriptor
    }
}

/// Construct Open for a chain-derived vacant LP PDA under the global SetV2.
pub fn build_open_lp_v4(
    chain: MultiLpChainProjectionV3<'_>,
    lp_owner: [u8; 32],
    set: CapabilityProgramSetV2<'_>,
) -> Result<UnsignedMultiLpRequestV4, MultiLpOperatorErrorV3> {
    if chain.terminal
        || chain.lp_position.is_some()
        || chain.lp_position_bytes.is_some()
        || lp_owner == [0; 32]
    {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, lp_owner, MultiLpRequestActionV3::Open, set)
}

/// Construct Close for a chain-derived live zero-share LP PDA under SetV2.
pub fn build_close_lp_v4(
    chain: MultiLpChainProjectionV3<'_>,
    set: CapabilityProgramSetV2<'_>,
) -> Result<UnsignedMultiLpRequestV4, MultiLpOperatorErrorV3> {
    let position = chain
        .lp_position
        .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?;
    let bytes = chain
        .lp_position_bytes
        .ok_or(MultiLpOperatorErrorV3::InvalidProjection)?;
    if super::multi_lp::DealerLpPositionV3::decode(bytes) != Ok(position) {
        return Err(MultiLpOperatorErrorV3::InvalidProjection);
    }
    if position.equity_shares != 0 {
        return Err(MultiLpOperatorErrorV3::InvalidChoice);
    }
    build(chain, position.lp_owner, MultiLpRequestActionV3::Close, set)
}

fn build(
    chain: MultiLpChainProjectionV3<'_>,
    lp_owner: [u8; 32],
    action: MultiLpRequestActionV3,
    set: CapabilityProgramSetV2<'_>,
) -> Result<UnsignedMultiLpRequestV4, MultiLpOperatorErrorV3> {
    if set.selector_offset() != DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U16
    {
        return Err(MultiLpOperatorErrorV3::ProgramSelection);
    }
    let bytes = build_multi_lp_request_bytes_v3(chain, lp_owner, action)?;
    let selected_descriptor = set
        .select_descriptor(&bytes)
        .map_err(|_| MultiLpOperatorErrorV3::ProgramSelection)?;
    Ok(UnsignedMultiLpRequestV4 {
        bytes,
        selected_descriptor,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::set_v2::{
        CapabilityProgramSetEntryV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    };
    use dclutch_core_contract::ContentId;
    use solana_program::pubkey::Pubkey;
    use std::{vec, vec::Vec};

    use super::*;
    use crate::dealer::obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    };

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero identity")
    }

    fn program_set(offset: u32) -> Vec<u8> {
        let entries = (1_u32..=9)
            .map(|selector| {
                CapabilityProgramSetEntryV2::new(
                    selector,
                    CapabilityDescriptorReferenceV2::new(
                        id(0x40 + u8::try_from(selector).expect("selector")),
                        id(0x60 + u8::try_from(selector).expect("selector")),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let mut output = vec![0; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
        encode_program_set_v2(offset, SelectorWidthV2::U16, &entries, &mut output)
            .expect("program set");
        output
    }

    fn obligation_bytes(child_root: [u8; 32]) -> Vec<u8> {
        let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + 16];
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

    #[test]
    fn open_selects_the_schema_bound_global_entry_and_refuses_selector_drift() {
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
            &[
                super::super::multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
                &child_root,
                &lp_owner,
            ],
            &trading,
        )
        .0
        .to_bytes();
        let obligation_bytes = obligation_bytes(child_root);
        let chain = MultiLpChainProjectionV3 {
            trading_program,
            release_set,
            market,
            child_root,
            lp_position_address,
            lp_position: None,
            lp_position_bytes: None,
            obligation: DealerObligationProjectionV3::decode(&obligation_bytes)
                .expect("obligation"),
            obligation_address,
            generation: 11,
            now: 20,
            expires_at: 25,
            lp_position_rent_principal: 50,
            terminal: false,
        };
        let canonical_bytes = program_set(DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3);
        let canonical = CapabilityProgramSetV2::decode(&canonical_bytes).expect("set");
        let request = build_open_lp_v4(chain, lp_owner, canonical).expect("open");
        assert_eq!(
            request.selected_descriptor(),
            CapabilityDescriptorReferenceV2::new(id(0x47), id(0x67))
        );
        assert_eq!(request.as_bytes()[10..12], 7_u16.to_le_bytes());

        let drifted_bytes = program_set(DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3 + 1);
        let drifted = CapabilityProgramSetV2::decode(&drifted_bytes).expect("drifted set");
        assert_eq!(
            build_open_lp_v4(chain, lp_owner, drifted),
            Err(MultiLpOperatorErrorV3::ProgramSelection)
        );
    }
}
