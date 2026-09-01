#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly stateless admitted-AOT accelerator for Dealer execution.
//!
//! Common Trading authenticates the release, action descriptor, Product,
//! execution artifacts, exact request, Profile13 account expansion, and input
//! register bank. This program independently rejoins every Dealer semantic
//! account through that public view, evaluates the selected LP-lifecycle or
//! scenario-solvency transition, and returns one canonical candidate-bank
//! chunk. It never writes an account, invokes a child, or owns protocol state.

extern crate alloc;
extern crate std;

use alloc::vec;

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_HEADER_BYTES_V2, ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, AcceleratorAckV2,
    AcceleratorRequestV2,
};
use dclutch_trading_sbf::{
    dealer::{
        v3_accelerator_accounts::evaluate_authenticated_dealer_scenario_v4,
        v3_equity_operator::DEALER_EQUITY_REQUEST_MAGIC_V3,
        v3_operator::DEALER_MULTI_LP_REQUEST_MAGIC_V3,
        v3_trade::{DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_MAGIC_V3},
        v4_equity_accelerator_accounts::evaluate_authenticated_dealer_equity_v4,
        v4_lp_accelerator_accounts::evaluate_authenticated_dealer_lp_v4,
    },
    hot_v3::authenticate_accelerator_invocation_v4,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

/// Stable physical refusal from the Dealer accelerator boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorSbfErrorV4 {
    /// AcceleratorRequestV2 transport or candidate-bank width differed.
    InvalidRequest = 0xD000,
    /// Common Trading could not authenticate the release/artifact/runtime view.
    InvalidInvocation = 0xD001,
    /// A canonical acknowledgement could not be constructed.
    InvalidAcknowledgement = 0xD002,
}

impl DealerAcceleratorSbfErrorV4 {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`DealerAcceleratorSbfErrorV4::ordinal`], whose match is exhaustive: a variant added to the
    /// enum does not compile until its author writes an arm here, and the only
    /// arm that satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 3] = [
        Self::InvalidRequest,
        Self::InvalidInvocation,
        Self::InvalidAcknowledgement,
    ];

    /// This refusal's position in [`DealerAcceleratorSbfErrorV4::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism:
    /// a fourth variant is a COMPILE ERROR here rather than a discriminant no
    /// assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::InvalidRequest => 0,
            Self::InvalidInvocation => 1,
            Self::InvalidAcknowledgement => 2,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing
// about the variants after it and goes stale silently every single time the
// enum grows -- the failure is not that the name is wrong, it is that nothing
// can notice. Claims proved it the expensive way: its bound went on naming
// `ReleaseSuperseded` after a later variant landed, so for as long as that
// stood, the newest refusal in the program was checked by nothing.
//
// So the band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    assert!(
        DealerAcceleratorSbfErrorV4::ALL[0] as u32
            == dclutch_refusal_registry::DEALER_ACCELERATOR_REFUSAL_BASE,
        "DealerAcceleratorSbfErrorV4 must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < DealerAcceleratorSbfErrorV4::ALL.len() {
        let variant = DealerAcceleratorSbfErrorV4::ALL[index];
        assert!(
            variant.ordinal() == index,
            "DealerAcceleratorSbfErrorV4::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::DEALER_ACCELERATOR_REFUSAL_BASE + index as u32,
            "DealerAcceleratorSbfErrorV4 discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::DEALER_ACCELERATOR_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "DealerAcceleratorSbfErrorV4 must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<DealerAcceleratorSbfErrorV4> for ProgramError {
    fn from(value: DealerAcceleratorSbfErrorV4) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Evaluate one authenticated Dealer candidate chunk.
///
/// Physical authentication failures return a program error with no return
/// data. A fully authenticated invocation whose Dealer semantics refuse emits
/// the canonical refused acknowledgement; common Trading therefore retains
/// sole authority over effects and write-last commitment.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = AcceleratorRequestV2::decode(instruction_data)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let bank_bytes = usize::try_from(request.total_bank_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let invocation = authenticate_accelerator_invocation_v4(program_id, accounts, instruction_data)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidInvocation)?;
    let mut candidate = vec![0_u8; bank_bytes];
    let family_magic = invocation.family_request().get(..8);
    let selected_action = invocation.selected_action();
    // Dispatch only after common Hot authenticated the top-level request,
    // ProgramSet selection and invocation context. Both magic and selected
    // action must name the same family wire; a cross-family combination is a
    // semantic refusal rather than an alternate decoder choice.
    let accepted = if family_magic == Some(DEALER_MULTI_LP_REQUEST_MAGIC_V3.as_slice())
        && matches!(selected_action, 7 | 8)
    {
        evaluate_authenticated_dealer_lp_v4(&invocation, &mut candidate).is_ok()
    } else if family_magic == Some(DEALER_EQUITY_REQUEST_MAGIC_V3.as_slice())
        && matches!(selected_action, 1..=6)
    {
        evaluate_authenticated_dealer_equity_v4(&invocation, &mut candidate).is_ok()
    } else if family_magic == Some(DEALER_SCENARIO_TRADE_MAGIC_V3.as_slice())
        && selected_action == u32::from(DEALER_SCENARIO_TRADE_ACTION_V3)
    {
        evaluate_authenticated_dealer_scenario_v4(&invocation, &mut candidate).is_ok()
    } else {
        false
    };
    let request_digest = content(instruction_data)?;
    let acknowledgement = if accepted {
        let bank_digest = content(&candidate)?;
        let start = usize::try_from(request.chunk_offset())
            .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
        let remaining = candidate
            .len()
            .checked_sub(start)
            .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
        let payload_bytes = remaining.min(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
        let end = start
            .checked_add(payload_bytes)
            .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
        let payload = candidate
            .get(start..end)
            .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
        AcceleratorAckV2::accepted(request, request_digest, bank_digest, payload)
            .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?
    } else {
        AcceleratorAckV2::refused(request, request_digest)
    };
    let output_bytes = ACCELERATOR_ACK_HEADER_BYTES_V2
        .checked_add(acknowledgement.payload().len())
        .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
    let mut output = vec![0_u8; output_bytes];
    acknowledgement
        .encode_into(&mut output)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
    set_return_data(&output);
    Ok(())
}

fn content(bytes: &[u8]) -> Result<ContentId, DealerAcceleratorSbfErrorV4> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)
}
