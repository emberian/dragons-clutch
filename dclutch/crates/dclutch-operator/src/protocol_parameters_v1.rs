//! Host-only construction for the governable parameter surface's four acts.
//!
//! Decision 0024's amendment asks for "a governable parameter surface so we are
//! not stuck (prototype the policy we intend to deploy)".
//! `dclutch_market::protocol_parameters` owns the record, the bands and the
//! delay; `programs/dclutch-custody-sbf/src/protocol_parameters_v1.rs` owns the
//! accounts. This owns neither. It exists because until it did, the record's
//! four routes could be reached by nothing at all: no client anywhere
//! constructed a `ProtocolParametersRequestV1`, so the record that makes a
//! ruled value governable could not be founded, and a surface nobody can found
//! is a surface nobody is governed by.
//!
//! # The founding is the one act a host cannot fully check
//!
//! Ruling R5: the record is founded only by the Custody program's CURRENT
//! upgrade authority, read off its ProgramData inside the route. A host can
//! derive the ProgramData address — it does, below — but it cannot know who the
//! upgrade authority is without reading the account, and this crate does no
//! RPC. So the founder is the caller's claim and the chain's conjunct; a
//! founding submitted by anyone else refuses as
//! `ProtocolParametersSbfErrorV1::FoundingAuthority`, by name, having spent a
//! fee and nothing else.

use dclutch_market::protocol_parameters::{
    Error as ParametersError, GovernanceActV1, PROTOCOL_PARAMETERS_APPLY_ACCOUNT_COUNT_V1,
    PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1, PROTOCOL_PARAMETERS_FOUND_ACCOUNT_COUNT_V1,
    ProtocolParametersReceiptSeedsV1, ProtocolParametersRecordSeedsV1, ProtocolParametersRequestV1,
    ProtocolParametersV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

/// Stable refusal from host-side governance construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolParametersBuildErrorV1 {
    /// The Custody program or the acting key was the zero key.
    InvalidIdentity,
    /// The body the caller asked to propose is not one the record admits; the
    /// cause is the contract's own, and it includes
    /// [`ParametersError::TakeBeforeMainnet`] — a proposal that would take a
    /// protocol fee before mainnet refuses HERE, at the host, by name, and
    /// again on chain if anyone assembles the bytes another way.
    Parameters(ParametersError),
}

/// The one record address under one Custody deployment.
///
/// Ruling R4: the seeds are the domain alone, with no generation in them, so a
/// consumer finds the record without knowing which generation it is at. The
/// generation appears only in a receipt's address.
#[must_use]
pub fn protocol_parameters_record_address_v1(custody_program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &ProtocolParametersRecordSeedsV1.as_slices(),
        &custody_program,
    )
    .0
}

/// The address of the receipt one applied change writes.
///
/// One per generation, so the census reads a stream rather than a latest value:
/// a change that could overwrite its predecessor's receipt would be a change
/// with no record that it happened.
#[must_use]
pub fn protocol_parameters_receipt_address_v1(custody_program: Pubkey, generation: u64) -> Pubkey {
    Pubkey::find_program_address(
        &ProtocolParametersReceiptSeedsV1::new(generation).as_slices(),
        &custody_program,
    )
    .0
}

/// The Custody program's ProgramData, whose current upgrade authority is the
/// only key the founding admits.
#[must_use]
pub fn custody_programdata_address_v1(custody_program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[custody_program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// `[record (w), founder (signer, w), custody_programdata, system, rent]`.
///
/// The body is `ProtocolParametersV1::genesis(authority)` and the route checks
/// that it is: a founding that carried anything else would be a founding that
/// chose its own economics, which is the thing the delay exists to prevent.
/// The authority is a placeholder by decision 0024 §3's own words — "today its
/// authority is the deployer key, named as a placeholder rather than pretended
/// to be governance" — and passing the zero key founds a FROZEN record, which
/// is what an immutable Custody deployment gets.
pub fn found_protocol_parameters_instruction_v1(
    custody_program: Pubkey,
    founder: Pubkey,
    governance_authority: [u8; 32],
) -> Result<Instruction, ProtocolParametersBuildErrorV1> {
    if custody_program == Pubkey::default() || founder == Pubkey::default() {
        return Err(ProtocolParametersBuildErrorV1::InvalidIdentity);
    }
    let data = ProtocolParametersRequestV1 {
        act: GovernanceActV1::Found,
        body: ProtocolParametersV1::genesis(governance_authority),
    }
    .to_bytes();
    let accounts = vec![
        AccountMeta::new(
            protocol_parameters_record_address_v1(custody_program),
            false,
        ),
        AccountMeta::new(founder, true),
        AccountMeta::new_readonly(custody_programdata_address_v1(custody_program), false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];
    debug_assert_eq!(accounts.len(), PROTOCOL_PARAMETERS_FOUND_ACCOUNT_COUNT_V1);
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

/// `[record (w), authority (signer)]`: stage a change, to mature after the
/// record's own delay.
///
/// The body is checked against the bands and the take conjunct HERE as well as
/// on chain, and both call the same `in_band` / `take_admissible` — one author,
/// asked twice, so a proposal that cannot land is refused before a fee rather
/// than discovered after one.
pub fn propose_protocol_parameters_instruction_v1(
    custody_program: Pubkey,
    authority: Pubkey,
    body: ProtocolParametersV1,
) -> Result<Instruction, ProtocolParametersBuildErrorV1> {
    governance_act_instruction_v1(custody_program, authority, GovernanceActV1::Propose, body)
}

/// `[record (w), authority (signer)]`: withdraw the standing proposal.
///
/// The body is not read, and the wire carries zeros rather than the value that
/// is being abandoned — a withdraw that echoed a body would be a second,
/// unauthenticated statement of what was proposed.
pub fn withdraw_protocol_parameters_instruction_v1(
    custody_program: Pubkey,
    authority: Pubkey,
) -> Result<Instruction, ProtocolParametersBuildErrorV1> {
    if custody_program == Pubkey::default() || authority == Pubkey::default() {
        return Err(ProtocolParametersBuildErrorV1::InvalidIdentity);
    }
    let data = ProtocolParametersRequestV1::WITHDRAW.to_bytes();
    let accounts = vec![
        AccountMeta::new(
            protocol_parameters_record_address_v1(custody_program),
            false,
        ),
        AccountMeta::new_readonly(authority, true),
    ];
    debug_assert_eq!(
        accounts.len(),
        PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1
    );
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

/// `[record (w), receipt (w), payer (signer, w), system, rent]`: install a
/// matured proposal, permissionlessly.
///
/// A governed change is still a crank. The authority that had to show up twice
/// could propose and then decline to finish, so anybody may finish it; the
/// payer funds the receipt's rent and gains nothing but the receipt.
///
/// `next_generation` is the generation the receipt will carry — the record's
/// current generation plus one — because the receipt's address is seeded with
/// it and a host that guessed would address the wrong account.
pub fn apply_protocol_parameters_instruction_v1(
    custody_program: Pubkey,
    payer: Pubkey,
    body: ProtocolParametersV1,
    next_generation: u64,
) -> Result<Instruction, ProtocolParametersBuildErrorV1> {
    if custody_program == Pubkey::default() || payer == Pubkey::default() {
        return Err(ProtocolParametersBuildErrorV1::InvalidIdentity);
    }
    let data = admissible_body_bytes_v1(GovernanceActV1::Apply, body)?;
    let accounts = vec![
        AccountMeta::new(
            protocol_parameters_record_address_v1(custody_program),
            false,
        ),
        AccountMeta::new(
            protocol_parameters_receipt_address_v1(custody_program, next_generation),
            false,
        ),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];
    debug_assert_eq!(accounts.len(), PROTOCOL_PARAMETERS_APPLY_ACCOUNT_COUNT_V1);
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

fn governance_act_instruction_v1(
    custody_program: Pubkey,
    authority: Pubkey,
    act: GovernanceActV1,
    body: ProtocolParametersV1,
) -> Result<Instruction, ProtocolParametersBuildErrorV1> {
    if custody_program == Pubkey::default() || authority == Pubkey::default() {
        return Err(ProtocolParametersBuildErrorV1::InvalidIdentity);
    }
    let data = admissible_body_bytes_v1(act, body)?;
    let accounts = vec![
        AccountMeta::new(
            protocol_parameters_record_address_v1(custody_program),
            false,
        ),
        AccountMeta::new_readonly(authority, true),
    ];
    debug_assert_eq!(
        accounts.len(),
        PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1
    );
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

/// The two conjuncts the record applies to a body, applied here to the same
/// body, by the record's own predicates AND IN THE RECORD'S OWN ORDER.
///
/// Bands first, then the take, exactly as `ProtocolParametersRecordV1::propose`
/// and `::apply_change` order them. The order is not decoration: a body can
/// fail both, and a host that named the other one would tell a caller a
/// different reason than the chain will, for the same bytes.
fn admissible_body_bytes_v1(
    act: GovernanceActV1,
    body: ProtocolParametersV1,
) -> Result<
    [u8; dclutch_market::protocol_parameters::PROTOCOL_PARAMETERS_REQUEST_BYTES_V1],
    ProtocolParametersBuildErrorV1,
> {
    if !body.in_band() {
        return Err(ProtocolParametersBuildErrorV1::Parameters(
            ParametersError::ParameterOutOfBand,
        ));
    }
    if !body.take_admissible() {
        return Err(ProtocolParametersBuildErrorV1::Parameters(
            ParametersError::TakeBeforeMainnet,
        ));
    }
    Ok(ProtocolParametersRequestV1 { act, body }.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market::protocol_parameters::{
        PROTOCOL_PARAMETERS_REQUEST_BYTES_V1, PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1,
    };

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    /// The founding frame is the adapter's, and its body is the genesis the
    /// route independently reconstructs — decoded back through the contract,
    /// never compared to a literal.
    #[test]
    fn the_founding_frame_carries_the_genesis_and_the_programdata() {
        let custody = key(0xc0);
        let founder = key(0x51);
        let instruction =
            found_protocol_parameters_instruction_v1(custody, founder, founder.to_bytes())
                .expect("founding builds");
        assert_eq!(
            instruction.accounts.len(),
            PROTOCOL_PARAMETERS_FOUND_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            instruction.accounts[0].pubkey,
            protocol_parameters_record_address_v1(custody)
        );
        assert!(instruction.accounts[0].is_writable && !instruction.accounts[0].is_signer);
        assert_eq!(instruction.accounts[1].pubkey, founder);
        assert!(instruction.accounts[1].is_signer && instruction.accounts[1].is_writable);
        assert_eq!(
            instruction.accounts[2].pubkey,
            custody_programdata_address_v1(custody),
            "the founding authority is read off THIS program's ProgramData"
        );
        assert_eq!(instruction.accounts[3].pubkey, system_program::ID);
        assert_eq!(instruction.accounts[4].pubkey, sysvar::rent::ID);
        assert_eq!(instruction.data.len(), PROTOCOL_PARAMETERS_REQUEST_BYTES_V1);
        assert_eq!(
            instruction.data.get(..8),
            Some(PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1.as_slice())
        );
        let decoded =
            ProtocolParametersRequestV1::decode(&instruction.data).expect("canonical founding");
        assert_eq!(decoded.act, GovernanceActV1::Found);
        assert_eq!(
            decoded.body,
            ProtocolParametersV1::genesis(founder.to_bytes())
        );
    }

    /// RULING R6 at the host: a body that would take a protocol fee before
    /// mainnet cannot be built into a request at all, and the refusal is the
    /// contract's own name for it rather than a second name invented here.
    #[test]
    fn a_take_before_mainnet_cannot_even_be_proposed_from_here() {
        let taking = ProtocolParametersV1 {
            protocol_take_basis_points: 1,
            protocol_beneficiary: [0x9a; 32],
            ..ProtocolParametersV1::genesis(key(0x51).to_bytes())
        };
        // IN BAND and still refused: one basis point under a 500-point ceiling
        // with a named beneficiary satisfies every band, so this reaches the
        // take conjunct rather than being caught by the coarser one on the way.
        assert!(taking.in_band());
        assert_eq!(
            propose_protocol_parameters_instruction_v1(key(0xc0), key(0x51), taking),
            Err(ProtocolParametersBuildErrorV1::Parameters(
                ParametersError::TakeBeforeMainnet
            ))
        );
        assert_eq!(
            apply_protocol_parameters_instruction_v1(key(0xc0), key(0x51), taking, 1),
            Err(ProtocolParametersBuildErrorV1::Parameters(
                ParametersError::TakeBeforeMainnet
            ))
        );
    }

    /// A body outside the bands refuses by the band's name, distinctly from the
    /// take's — two conjuncts, two accusations, in the record's own order, so
    /// this host and the chain name the same one for the same bytes.
    #[test]
    fn a_body_outside_the_bands_refuses_by_the_bands_name() {
        let wide = ProtocolParametersV1 {
            max_fee_basis_points: 10_000,
            ..ProtocolParametersV1::genesis(key(0x51).to_bytes())
        };
        assert_eq!(
            propose_protocol_parameters_instruction_v1(key(0xc0), key(0x51), wide),
            Err(ProtocolParametersBuildErrorV1::Parameters(
                ParametersError::ParameterOutOfBand
            ))
        );
    }

    /// A proposal a record would accept builds, and its two-account frame is
    /// the authority frame: the record writable, the authority signing and
    /// writing nothing.
    #[test]
    fn a_lawful_proposal_builds_a_two_account_authority_frame() {
        let custody = key(0xc0);
        let authority = key(0x51);
        let raised = ProtocolParametersV1 {
            closer_reward_cap_lamports: 200_000,
            ..ProtocolParametersV1::genesis(authority.to_bytes())
        };
        let instruction =
            propose_protocol_parameters_instruction_v1(custody, authority, raised).expect("builds");
        assert_eq!(
            instruction.accounts.len(),
            PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1
        );
        assert!(instruction.accounts[0].is_writable);
        assert!(instruction.accounts[1].is_signer && !instruction.accounts[1].is_writable);
        let decoded = ProtocolParametersRequestV1::decode(&instruction.data).expect("canonical");
        assert_eq!(decoded.act, GovernanceActV1::Propose);
        assert_eq!(decoded.body.closer_reward_cap_lamports, 200_000);
    }

    /// A withdraw carries zeros, and an apply addresses the receipt of the
    /// generation it will write — not the record's current one.
    #[test]
    fn a_withdraw_carries_nothing_and_an_apply_addresses_its_own_generation() {
        let custody = key(0xc0);
        let decoded = ProtocolParametersRequestV1::decode(
            &withdraw_protocol_parameters_instruction_v1(custody, key(0x51))
                .expect("builds")
                .data,
        )
        .expect("canonical withdraw");
        assert_eq!(decoded, ProtocolParametersRequestV1::WITHDRAW);

        let apply = apply_protocol_parameters_instruction_v1(
            custody,
            key(0x77),
            ProtocolParametersV1::genesis(key(0x51).to_bytes()),
            7,
        )
        .expect("builds");
        assert_eq!(
            apply.accounts[1].pubkey,
            protocol_parameters_receipt_address_v1(custody, 7)
        );
        assert_ne!(
            apply.accounts[1].pubkey,
            protocol_parameters_receipt_address_v1(custody, 6),
            "a receipt address is seeded with its own generation and nothing else"
        );
        assert!(apply.accounts[2].is_signer && apply.accounts[2].is_writable);
    }
}
