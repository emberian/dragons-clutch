//! Schema-bound unsigned construction for Dealer selector 9.
//!
//! The request wire and all economic/Claims composition remain owned by
//! [`super::v3_trade`]. This adapter changes only release selection: the sole
//! mixed Dealer [`CapabilityProgramSetV2`] must select the exact finalized V4
//! descriptor schema and content supplied by the release artifact owner.

use dclutch_capability_program_contract::{
    set_v2::{CapabilityDescriptorReferenceV2, CapabilityProgramSetV2, SelectorWidthV2},
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
};

use super::v3_trade::{
    DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
    ScenarioTradeChainProjectionV3, ScenarioTradeErrorV3, ScenarioTradeIntentV3,
    encode_scenario_trade_request_v3, validate_scenario_trade_construction_v3,
};

/// Exact unsigned scenario request plus its authenticated V4 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedScenarioTradeRequestV4 {
    request_bytes: usize,
    selected_descriptor: CapabilityDescriptorReferenceV2,
}

impl UnsignedScenarioTradeRequestV4 {
    /// Exact initialized request width in the caller-owned output.
    pub const fn request_bytes(self) -> usize {
        self.request_bytes
    }

    /// Exact finalized descriptor schema/content selected by the global set.
    pub const fn selected_descriptor(self) -> CapabilityDescriptorReferenceV2 {
        self.selected_descriptor
    }
}

/// Build selector 9 from authenticated chain state under the sole global SetV2.
///
/// `expected_descriptor` must be derived from the finalized selector-9 V4
/// artifact. Requiring it here prevents a syntactically valid SetV2 from
/// silently substituting another descriptor schema or content identity.
pub fn build_scenario_trade_request_v4(
    chain: ScenarioTradeChainProjectionV3<'_>,
    intent: ScenarioTradeIntentV3<'_>,
    set: CapabilityProgramSetV2<'_>,
    expected_descriptor: CapabilityDescriptorReferenceV2,
    output: &mut [u8],
) -> Result<UnsignedScenarioTradeRequestV4, ScenarioTradeErrorV3> {
    validate_scenario_trade_construction_v3(chain, intent)?;
    if set.selector_offset() != DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U16
        || expected_descriptor.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4
    {
        return Err(ScenarioTradeErrorV3::ProgramSelection);
    }

    let mut selector = [0_u8; 12];
    selector[10..12].copy_from_slice(&DEALER_SCENARIO_TRADE_ACTION_V3.to_le_bytes());
    let selected_descriptor = set
        .select_descriptor(&selector)
        .map_err(|_| ScenarioTradeErrorV3::ProgramSelection)?;
    if selected_descriptor != expected_descriptor {
        return Err(ScenarioTradeErrorV3::ProgramSelection);
    }

    let request_bytes = encode_scenario_trade_request_v3(chain, intent, output)?;
    Ok(UnsignedScenarioTradeRequestV4 {
        request_bytes,
        selected_descriptor,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::{
        set_v1::CapabilityProgramSetV1,
        set_v2::{
            CapabilityProgramSetEntryV2, encode_program_set_v2, encoded_program_set_bytes_v2,
        },
    };
    use dclutch_core_contract::ContentId;
    use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
    use solana_program::pubkey::Pubkey;
    use std::{vec, vec::Vec};

    use super::*;
    use crate::dealer::{
        v3_obligation::{
            DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
            DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
            DealerObligationProjectionV3,
        },
        v3_trade::{
            DEALER_SCENARIO_TRADE_HEADER_BYTES_V3, ScenarioTradeDirectionV3,
            build_scenario_trade_request_v3, scenario_trade_max_request_bytes_v3,
        },
    };

    fn content(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero test identity")
    }

    fn expected_descriptor() -> CapabilityDescriptorReferenceV2 {
        CapabilityDescriptorReferenceV2::new(
            content(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4),
            content([0x79; 32]),
        )
    }

    fn set_v2(offset: u32, selector_nine: CapabilityDescriptorReferenceV2) -> Vec<u8> {
        let entries = (1_u32..=9)
            .map(|selector| {
                CapabilityProgramSetEntryV2::new(
                    selector,
                    if selector == u32::from(DEALER_SCENARIO_TRADE_ACTION_V3) {
                        selector_nine
                    } else {
                        CapabilityDescriptorReferenceV2::new(
                            content([0x40 + u8::try_from(selector).expect("selector"); 32]),
                            content([0x60 + u8::try_from(selector).expect("selector"); 32]),
                        )
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut output = vec![0; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
        encode_program_set_v2(offset, SelectorWidthV2::U16, &entries, &mut output).expect("SetV2");
        output
    }

    fn set_v1() -> Vec<u8> {
        let mut bytes = vec![0; 72];
        bytes[..8].copy_from_slice(b"DCLTCPS1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3.to_le_bytes());
        bytes[16] = 2;
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
        bytes[32..36].copy_from_slice(&u32::from(DEALER_SCENARIO_TRADE_ACTION_V3).to_le_bytes());
        bytes[36..68].copy_from_slice(&[0x79; 32]);
        bytes
    }

    fn obligation_bytes(child_root: [u8; 32]) -> Vec<u8> {
        let obligations = [12_u64, 20, 10];
        let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + obligations.len() * 8];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&3_u32.to_le_bytes());
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
        for (index, obligation) in obligations.into_iter().enumerate() {
            let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
            bytes[offset..offset + 8].copy_from_slice(&obligation.to_le_bytes());
        }
        bytes
    }

    fn fixture<'a>(
        current_bytes: &'a [u8],
        dealer_inventory: &'a [u64],
        counterparty_inventory: &'a [u64],
    ) -> (
        ScenarioTradeChainProjectionV3<'a>,
        ScenarioTradeIntentV3<'static>,
    ) {
        let trading_program = [1; 32];
        let child_root = [7; 32];
        let obligation_address = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child_root],
            &Pubkey::new_from_array(trading_program),
        )
        .0
        .to_bytes();
        (
            ScenarioTradeChainProjectionV3 {
                trading_program,
                release_set: [6; 32],
                market: [2; 32],
                child_root,
                obligation_address,
                current_obligation: DealerObligationProjectionV3::decode(current_bytes)
                    .expect("current obligation"),
                dealer_position: ClaimsInventoryObservation {
                    market_id: [2; 32],
                    product_id: [3; 32],
                    liability_basis_id: [4; 32],
                    position_owner: [5; 32],
                    revision: 9,
                    inventory: dealer_inventory,
                },
                counterparty_position: ClaimsInventoryObservation {
                    market_id: [2; 32],
                    product_id: [3; 32],
                    liability_basis_id: [4; 32],
                    position_owner: [8; 32],
                    revision: 11,
                    inventory: counterparty_inventory,
                },
                product_record_digest: [10; 32],
                linked_basis_record_digest: [11; 32],
                counterparty_account: [9; 32],
                principal_balance: 100,
                locked_capital_floor: 0,
                claims_revision: 0,
                generation: 17,
                now: 20,
                expires_at: 25,
                terminal: false,
                basis_scale: 1,
            },
            ScenarioTradeIntentV3 {
                direction: ScenarioTradeDirectionV3::CounterpartyPaysDealer,
                principal: 10,
                realized_fee: 1,
                acquired: &[3, 0, 4],
                delivered: &[0, 1, 0],
                candidate_obligations: &[10, 19, 13],
            },
        )
    }

    #[test]
    fn set_v2_selects_exact_v4_descriptor_without_changing_v3_request_bytes() {
        let current = obligation_bytes([7; 32]);
        let dealer = [2_u64, 10, 0];
        let counterparty = [20_u64, 5, 9];
        let (chain, intent) = fixture(&current, &dealer, &counterparty);
        let mut v3_output = vec![0; scenario_trade_max_request_bytes_v3(3).expect("bound")];
        let legacy_bytes = set_v1();
        let legacy = CapabilityProgramSetV1::decode(&legacy_bytes).expect("SetV1");
        let v3 = build_scenario_trade_request_v3(chain, intent, legacy, &mut v3_output)
            .expect("legacy request");

        let exact = expected_descriptor();
        let set_bytes = set_v2(DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3, exact);
        let set = CapabilityProgramSetV2::decode(&set_bytes).expect("SetV2");
        let mut v4_output = vec![0; scenario_trade_max_request_bytes_v3(3).expect("bound")];
        let v4 = build_scenario_trade_request_v4(chain, intent, set, exact, &mut v4_output)
            .expect("schema-bound request");

        assert!(v4.request_bytes() > DEALER_SCENARIO_TRADE_HEADER_BYTES_V3);
        assert_eq!(v4.selected_descriptor(), exact);
        assert_eq!(v4.request_bytes(), v3.request_bytes);
        assert_eq!(
            &v4_output[..v4.request_bytes()],
            &v3_output[..v3.request_bytes],
            "SetV2 selection must not fork the canonical request wire"
        );
    }

    #[test]
    fn set_v2_refuses_legacy_offset_and_schema_or_content_substitution() {
        let current = obligation_bytes([7; 32]);
        let dealer = [2_u64, 10, 0];
        let counterparty = [20_u64, 5, 9];
        let (chain, intent) = fixture(&current, &dealer, &counterparty);
        let expected = expected_descriptor();
        let mut output = vec![0; scenario_trade_max_request_bytes_v3(3).expect("bound")];

        let offset_bytes = set_v2(DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3 + 1, expected);
        let offset = CapabilityProgramSetV2::decode(&offset_bytes).expect("offset SetV2");
        assert_eq!(
            build_scenario_trade_request_v4(chain, intent, offset, expected, &mut output),
            Err(ScenarioTradeErrorV3::ProgramSelection)
        );

        let substituted_schema =
            CapabilityDescriptorReferenceV2::new(content([0x55; 32]), expected.program());
        let schema_bytes = set_v2(DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3, substituted_schema);
        let schema = CapabilityProgramSetV2::decode(&schema_bytes).expect("schema SetV2");
        assert_eq!(
            build_scenario_trade_request_v4(chain, intent, schema, expected, &mut output),
            Err(ScenarioTradeErrorV3::ProgramSelection)
        );

        let substituted_content =
            CapabilityDescriptorReferenceV2::new(expected.schema(), content([0x7a; 32]));
        let content_bytes = set_v2(
            DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
            substituted_content,
        );
        let content_set = CapabilityProgramSetV2::decode(&content_bytes).expect("content SetV2");
        assert_eq!(
            build_scenario_trade_request_v4(chain, intent, content_set, expected, &mut output),
            Err(ScenarioTradeErrorV3::ProgramSelection)
        );

        let wrong_expected =
            CapabilityDescriptorReferenceV2::new(content([0x56; 32]), expected.program());
        let exact_bytes = set_v2(DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3, expected);
        let exact_set = CapabilityProgramSetV2::decode(&exact_bytes).expect("exact SetV2");
        assert_eq!(
            build_scenario_trade_request_v4(chain, intent, exact_set, wrong_expected, &mut output,),
            Err(ScenarioTradeErrorV3::ProgramSelection)
        );
        assert!(output.iter().all(|byte| *byte == 0));
    }
}
