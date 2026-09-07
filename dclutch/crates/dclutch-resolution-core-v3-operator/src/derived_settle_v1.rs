//! The unsigned derived-settle instruction: a child market settled from its
//! parents' certificates (`programs/dclutch-resolution-proof-sbf/src/derived_transport_v1.rs`).
//!
//! The frame is `ParentReferenceV1Abi.settleFrame`, in the emitted order with
//! the emitted privileges (`DERIVED_SETTLE_ACCOUNT_ROLES_V1`), so this builder
//! cannot disagree with the program about who signs or who is written. Every
//! PDA is derived here from the same seeds the program re-derives: record
//! pairs under the Registry, the child's and each parent's Source state under
//! the Resolution program, and each certificate seat from its Source state,
//! kind seed and terminal sequence.

use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_source::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2;
use dclutch_source::parent_reference_v1::{
    DERIVED_SETTLE_ACCOUNT_COUNT_V1, DERIVED_SETTLE_ACCOUNT_ROLES_V1,
    DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1, DerivedSettleRequestV1,
};
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// A finalized record pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPairV1 {
    /// Registry-owned raw record.
    pub raw: Pubkey,
    /// Vacant staging cursor.
    pub staging: Pubkey,
}

impl RecordPairV1 {
    /// Derive the pair for `(schema, digest)` under the Registry.
    #[must_use]
    pub fn derive(registry: &Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Self {
        Self {
            raw: Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                registry,
            )
            .0,
            staging: Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                registry,
            )
            .0,
        }
    }
}

/// One parent's three accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentAddressesV1 {
    /// The parent's Core Market.
    pub market: Pubkey,
    /// The parent's Source state under the Resolution program.
    pub source_state: Pubkey,
    /// The parent's terminal certificate seat.
    pub certificate: Pubkey,
}

impl ParentAddressesV1 {
    /// Derive a parent's Source state and certificate seat from what its
    /// Market and Source state say: generation, terminal kind and sequence.
    #[must_use]
    pub fn derive(
        resolution_program: &Pubkey,
        market: Pubkey,
        generation: u64,
        terminal_kind: ResolutionCertificateKindV2,
        terminal_sequence: u64,
    ) -> Self {
        let source_state = source_state_address_v1(resolution_program, &market, generation);
        Self {
            market,
            source_state,
            certificate: certificate_seat_v1(
                resolution_program,
                &source_state,
                terminal_kind,
                terminal_sequence,
            ),
        }
    }
}

/// The Source state of `(market, generation)` under the Resolution program.
#[must_use]
pub fn source_state_address_v1(
    resolution_program: &Pubkey,
    market: &Pubkey,
    generation: u64,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        resolution_program,
    )
    .0
}

/// The certificate seat a Source state writes at one kind and sequence.
#[must_use]
pub fn certificate_seat_v1(
    resolution_program: &Pubkey,
    source_state: &Pubkey,
    kind: ResolutionCertificateKindV2,
    terminal_sequence: u64,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[kind.kind_seed()],
            &terminal_sequence.to_le_bytes(),
        ],
        resolution_program,
    )
    .0
}

/// Every account of the settle frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedSettleAddressesV1 {
    /// The fee payer.
    pub worker: Pubkey,
    /// The child's Core Market.
    pub market: Pubkey,
    /// The Core program.
    pub core_program: Pubkey,
    /// The Registry-owned activation cache of the child's release set.
    pub activation_cache: Pubkey,
    /// The child's Source state.
    pub source_state: Pubkey,
    /// The child's certificate seat, prefunded.
    pub certificate: Pubkey,
    /// `SourceMaterialV3`.
    pub material: RecordPairV1,
    /// `SourceSpecV1`.
    pub source_spec: RecordPairV1,
    /// `ProviderReleaseV1`.
    pub provider_release: RecordPairV1,
    /// `WindowSpecV1`.
    pub window_spec: RecordPairV1,
    /// `StatisticSpecV1`.
    pub statistic_spec: RecordPairV1,
    /// `ParentReferenceV1`.
    pub parent_reference: RecordPairV1,
    /// The Product record.
    pub product: RecordPairV1,
    /// The result domain.
    pub result_domain: RecordPairV1,
    /// The portfolio.
    pub portfolio: RecordPairV1,
    /// Parent A.
    pub parent_a: ParentAddressesV1,
    /// Parent B, absent for a conditional child off its condition.
    pub parent_b: Option<ParentAddressesV1>,
}

/// Build the unsigned settle instruction.
///
/// Refuses a frame whose width is not one of the two the program admits.
pub fn build_derived_settle_instruction_v1(
    resolution_program: Pubkey,
    addresses: DerivedSettleAddressesV1,
    request: DerivedSettleRequestV1,
) -> Result<Instruction, dclutch_source::Error> {
    let data = request.to_bytes()?;
    let mut keys = vec![
        addresses.worker,
        addresses.market,
        addresses.core_program,
        addresses.activation_cache,
        addresses.source_state,
        addresses.certificate,
        addresses.material.raw,
        addresses.material.staging,
        addresses.source_spec.raw,
        addresses.source_spec.staging,
        addresses.provider_release.raw,
        addresses.provider_release.staging,
        addresses.window_spec.raw,
        addresses.window_spec.staging,
        addresses.statistic_spec.raw,
        addresses.statistic_spec.staging,
        addresses.parent_reference.raw,
        addresses.parent_reference.staging,
        addresses.product.raw,
        addresses.product.staging,
        addresses.result_domain.raw,
        addresses.result_domain.staging,
        addresses.portfolio.raw,
        addresses.portfolio.staging,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        addresses.parent_a.market,
        addresses.parent_a.source_state,
        addresses.parent_a.certificate,
    ];
    if let Some(parent_b) = addresses.parent_b {
        keys.extend([parent_b.market, parent_b.source_state, parent_b.certificate]);
    }
    let expected = if addresses.parent_b.is_some() {
        DERIVED_SETTLE_ACCOUNT_COUNT_V1
    } else {
        DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1
    };
    if keys.len() != expected {
        return Err(dclutch_source::Error::InvalidLength);
    }
    let accounts = keys
        .into_iter()
        .zip(DERIVED_SETTLE_ACCOUNT_ROLES_V1)
        .map(|(key, (writable, signer))| {
            if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            }
        })
        .collect();
    Ok(Instruction {
        program_id: resolution_program,
        accounts,
        data: data.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn pair(tag: u8) -> RecordPairV1 {
        RecordPairV1 {
            raw: pk(tag),
            staging: pk(tag + 1),
        }
    }

    fn addresses(with_b: bool) -> DerivedSettleAddressesV1 {
        DerivedSettleAddressesV1 {
            worker: pk(1),
            market: pk(2),
            core_program: pk(3),
            activation_cache: pk(4),
            source_state: pk(5),
            certificate: pk(6),
            material: pair(10),
            source_spec: pair(12),
            provider_release: pair(14),
            window_spec: pair(16),
            statistic_spec: pair(18),
            parent_reference: pair(20),
            product: pair(22),
            result_domain: pair(24),
            portfolio: pair(26),
            parent_a: ParentAddressesV1 {
                market: pk(30),
                source_state: pk(31),
                certificate: pk(32),
            },
            parent_b: with_b.then_some(ParentAddressesV1 {
                market: pk(40),
                source_state: pk(41),
                certificate: pk(42),
            }),
        }
    }

    fn request() -> DerivedSettleRequestV1 {
        DerivedSettleRequestV1 {
            generation: 2,
            terminal_sequence: 1,
            parent_reference: [0xA1; 32],
            source_material: [0xB2; 32],
            source_spec: [0xC3; 32],
        }
    }

    #[test]
    fn the_two_widths_are_the_emitted_ones() {
        let with_b =
            build_derived_settle_instruction_v1(pk(9), addresses(true), request()).expect("with B");
        assert_eq!(with_b.accounts.len(), DERIVED_SETTLE_ACCOUNT_COUNT_V1);
        let off = build_derived_settle_instruction_v1(pk(9), addresses(false), request())
            .expect("off condition");
        assert_eq!(
            off.accounts.len(),
            DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1
        );
        assert!(with_b.accounts[0].is_signer && with_b.accounts[0].is_writable);
        assert!(with_b.accounts[4].is_writable && with_b.accounts[5].is_writable);
        assert!(with_b.accounts[27..].iter().all(|meta| !meta.is_writable));
        assert_eq!(with_b.accounts[24].pubkey, sysvar::clock::ID);
        assert_eq!(with_b.accounts[26].pubkey, system_program::ID);
    }

    #[test]
    fn the_seat_is_kind_seeded() {
        let state = pk(5);
        let success = certificate_seat_v1(
            &pk(9),
            &state,
            ResolutionCertificateKindV2::ResolutionSuccess,
            1,
        );
        let failure = certificate_seat_v1(
            &pk(9),
            &state,
            ResolutionCertificateKindV2::ResolutionFailure,
            1,
        );
        assert_ne!(success, failure);
    }
}
