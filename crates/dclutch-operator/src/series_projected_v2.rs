//! Host-only construction of compact projected-Market Series Consume data.
//!
//! The caller first derives an exact recurring-Series Consume family request
//! from finalized Template/Occurrence/Ticket/replay observations.  This module
//! wraps that already-admitted header and proof once in the production
//! family-neutral projected executor wire.  It does not construct account
//! authority, sign, submit, or treat the bounded Funding count as attested;
//! current Core promotes that hint only through `SeriesCoreFoundAckV2`.

use dclutch_series_v3_kernel::request::{
    SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3,
};
use dclutch_trading_sbf::projected_market_v2::{
    PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2, ProjectedMarketExecutionV2,
    encode_projected_market_execution_v2,
};
use solana_program::hash::hash;

/// Stable refusal from compact projected-Series data construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesProjectedOperatorErrorV2 {
    /// Family bytes were not one exact recurring-Series request.
    Request,
    /// The request selected another action or omitted its occurrence proof.
    Action,
    /// The pre-Core Funding span hint was outside the protocol bound.
    FundingCount,
    /// Checked width arithmetic or canonical projected encoding refused.
    Encoding,
}

/// Unsigned compact data for the production projected-Market Trading executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedSeriesProjectedConsumeV2 {
    data: Vec<u8>,
    family_request_digest: [u8; 32],
    funding_count_hint: u8,
}

impl UnsignedSeriesProjectedConsumeV2 {
    /// Borrow the exact projected-executor instruction bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// SHA-256 of the exact Series header and proof supplied once in the wire.
    pub const fn family_request_digest(&self) -> [u8; 32] {
        self.family_request_digest
    }

    /// Bounded routing hint before current Core authenticates the Funding list.
    pub const fn funding_count_hint(&self) -> u8 {
        self.funding_count_hint
    }
}

/// Encode one exact compact projected-Series Consume instruction body.
pub fn build_series_projected_consume_v2(
    family_request: &[u8],
    funding_count_hint: u8,
) -> Result<UnsignedSeriesProjectedConsumeV2, SeriesProjectedOperatorErrorV2> {
    let request = SeriesActionRequestV3::decode(family_request)
        .map_err(|_| SeriesProjectedOperatorErrorV2::Request)?;
    if request.action() != SeriesActionV3::Consume || request.proof_count() == 0 {
        return Err(SeriesProjectedOperatorErrorV2::Action);
    }
    let header: &[u8; SERIES_ACTION_HEADER_BYTES_V3] = family_request
        .get(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesProjectedOperatorErrorV2::Request)?
        .try_into()
        .map_err(|_| SeriesProjectedOperatorErrorV2::Request)?;
    let witness = family_request
        .get(SERIES_ACTION_HEADER_BYTES_V3..)
        .ok_or(SeriesProjectedOperatorErrorV2::Request)?;
    let width = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2
        .checked_add(witness.len())
        .ok_or(SeriesProjectedOperatorErrorV2::Encoding)?;
    let mut data = vec![0_u8; width];
    encode_projected_market_execution_v2(&mut data, header, witness, funding_count_hint)
        .map_err(|error| match error {
            dclutch_trading_sbf::projected_market_v2::ProjectedMarketExecutionErrorV2::NonCanonical => {
                SeriesProjectedOperatorErrorV2::FundingCount
            }
            _ => SeriesProjectedOperatorErrorV2::Encoding,
        })?;
    let decoded = ProjectedMarketExecutionV2::decode(&data)
        .map_err(|_| SeriesProjectedOperatorErrorV2::Encoding)?;
    if decoded.family_request() != family_request
        || decoded.affine_count() != funding_count_hint
        || decoded.witness_words() != request.proof_count()
    {
        return Err(SeriesProjectedOperatorErrorV2::Encoding);
    }
    Ok(UnsignedSeriesProjectedConsumeV2 {
        data,
        family_request_digest: hash(family_request).to_bytes(),
        funding_count_hint,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_account_profile_contract::v2::AccountProfileV2;
    use dclutch_core_contract::ContentId;
    use dclutch_series_v3_kernel::request::encode_series_action_header_v3;
    use dclutch_trading_sbf::series::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    };
    use solana_compute_budget_interface::ComputeBudgetInstruction;
    use solana_hash::Hash;
    use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
    use solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    fn request_with_proof(action: SeriesActionV3, proof_count: u8) -> Vec<u8> {
        let occurrence_bound = action.occurrence_bound();
        let header = encode_series_action_header_v3(
            action,
            id(1),
            occurrence_bound.then_some(id(2)),
            (action != SeriesActionV3::Close).then_some(id(3)),
            4,
            if matches!(action, SeriesActionV3::Prepare | SeriesActionV3::Close) {
                0
            } else {
                5
            },
            if occurrence_bound { proof_count } else { 0 },
        )
        .expect("header");
        let mut bytes = header.to_vec();
        if occurrence_bound {
            bytes.extend_from_slice(&vec![9; usize::from(proof_count) * 32]);
        }
        bytes
    }

    fn request(action: SeriesActionV3) -> Vec<u8> {
        request_with_proof(action, 2)
    }

    #[test]
    fn consume_header_and_proof_are_encoded_once() {
        let family_request = request(SeriesActionV3::Consume);
        let built = build_series_projected_consume_v2(&family_request, 3).expect("projected data");
        let decoded = ProjectedMarketExecutionV2::decode(built.data()).expect("decode");
        assert_eq!(decoded.family_request(), family_request);
        assert_eq!(decoded.witness_words(), 2);
        assert_eq!(decoded.affine_count(), 3);
        assert_eq!(
            built.family_request_digest(),
            hash(&family_request).to_bytes()
        );
        assert_eq!(built.data().len(), 208);
    }

    #[test]
    fn action_funding_and_padding_substitution_refuse() {
        assert_eq!(
            build_series_projected_consume_v2(&request(SeriesActionV3::Prepare), 3),
            Err(SeriesProjectedOperatorErrorV2::Action)
        );
        let consume = request(SeriesActionV3::Consume);
        for count in [0, 17] {
            assert_eq!(
                build_series_projected_consume_v2(&consume, count),
                Err(SeriesProjectedOperatorErrorV2::FundingCount)
            );
        }
        let mut padded = consume;
        padded.push(0);
        assert_eq!(
            build_series_projected_consume_v2(&padded, 3),
            Err(SeriesProjectedOperatorErrorV2::Request)
        );
    }

    #[test]
    fn maximum_projected_runtime_subframe_has_a_v0_packet_margin() {
        const MAXIMUM_FUNDING_COUNT: u32 = 16;
        const SOLANA_PACKET_BYTES: usize = 1_232;
        const REQUIRED_PACKET_MARGIN: usize = 256;

        let lengths = [0_u32; 157];
        let mut profile_scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut profile_bytes = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &lengths,
            },
            &mut profile_scratch,
            &mut profile_bytes,
        )
        .expect("canonical Series Profile13");
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13 decode");
        let physical_accounts = profile
            .physical_account_count_with_dynamic_spans(0, &[MAXIMUM_FUNDING_COUNT])
            .expect("physical account count");
        assert_eq!(physical_accounts, 80);

        let family_request = request_with_proof(SeriesActionV3::Consume, 9);
        let projected = build_series_projected_consume_v2(
            &family_request,
            u8::try_from(MAXIMUM_FUNDING_COUNT).expect("bounded count"),
        )
        .expect("maximum projected Consume");
        assert_eq!(projected.data().len(), 432);

        let payer = Pubkey::new_from_array([1; 32]);
        let trading_program = Pubkey::new_from_array([2; 32]);
        let addresses = (0..physical_accounts)
            .map(|index| {
                Pubkey::new_from_array([
                    u8::try_from(index + 3).expect("bounded representative index");
                    32
                ])
            })
            .collect::<Vec<_>>();
        let accounts = addresses
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index.is_multiple_of(5) {
                    AccountMeta::new(*key, false)
                } else {
                    AccountMeta::new_readonly(*key, false)
                }
            })
            .collect::<Vec<_>>();
        let instruction = Instruction {
            program_id: trading_program,
            accounts,
            data: projected.data().to_vec(),
        };
        let lookup = AddressLookupTableAccount {
            key: Pubkey::new_from_array([254; 32]),
            addresses,
        };
        let message = v0::Message::try_compile(
            &payer,
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                ComputeBudgetInstruction::set_compute_unit_price(1),
                instruction,
            ],
            &[lookup],
            Hash::new_from_array([255; 32]),
        )
        .expect("maximum projected Consume v0 message");
        assert_eq!(message.account_keys.len(), 3);
        assert_eq!(message.address_table_lookups.len(), 1);
        let loaded_addresses = message.address_table_lookups[0]
            .writable_indexes
            .len()
            + message.address_table_lookups[0].readonly_indexes.len();
        assert_eq!(loaded_addresses, physical_accounts);
        let required_signatures = usize::from(message.header.num_required_signatures);
        let wire_bytes =
            1 + required_signatures * 64 + VersionedMessage::V0(message).serialize().len();
        assert_eq!(wire_bytes, 850);
        assert!(
            wire_bytes + REQUIRED_PACKET_MARGIN <= SOLANA_PACKET_BYTES,
            "{wire_bytes}B runtime-subframe packet leaves less than {REQUIRED_PACKET_MARGIN}B margin"
        );
    }
}
