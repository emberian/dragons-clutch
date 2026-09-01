#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for the fractional claim-check compaction route.
//!
//! The Claims handler `fractional_claim_check_v1::process_fractional_compaction`
//! is reachable only from a program that can sign the Trading-owned Fractional
//! capability root. In production that program is Trading, whose
//! `fractional_root_signer` marks the root's meta a signer after authenticating
//! the root's bytes against the same request. Nothing in this repository could
//! produce that signature for the compaction frame, so the shipped handler had
//! never executed. This program is that caller and owns no protocol state or
//! production ABI. For the representative whole-life campaign it also
//! delegates the sibling atomic wrapper byte-for-byte to that wrapper's
//! existing implementation. One Trading identity can therefore wrap and later
//! compact the same root without making this compaction branch invent a
//! caller-authority account.
//!
//! The width is deliberately not named here. This caller reads
//! `FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1` and the role coordinates from the
//! declaration itself, so the ruled fiftieth account (WAVE `b4546291`, the Rent
//! program) cost this file one sentence and no code -- which is the property the
//! declaration exists to give.
//!
//! # One signature, and that is the whole point
//!
//! Its sibling `fractional-atomic-caller` signs **two** PDAs -- the release-
//! scoped Trading caller authority at coordinate zero and the Fractional root.
//! This one signs **one**, and the difference is design §17.8 ruling 2 rather
//! than a simplification. `TradingCallerAuthority` was declared a required
//! signer for a close that turns out to be owner-signed without it, and the
//! role is now *declared and refused*. A caller that signed one anyway would be
//! quietly re-supplying the ceremony the ruling removed, and the campaign built
//! on it would prove the frame worked *with* an account the frame says is not
//! there. So the compaction branch cannot sign a caller authority: there is no
//! code in that branch that derives one. Atomic requests are a separate,
//! width-disjoint dispatch into the existing two-signature atomic caller.
//!
//! That is what makes witness w7 an observation rather than an assertion. The
//! route runs, end to end, with no caller-authority account anywhere in frame,
//! because this caller has none to pass.
//!
//! # The residual, named rather than papered over
//!
//! `invoke_signed` signs only for the calling program's own addresses, so the
//! root this signs for is derived under **this** program id and not under
//! Trading's. What is under test is therefore the Claims route's behaviour given
//! a correctly-signed root, not Trading's derivation of it -- which
//! `fractional_root_signer`'s own witnesses cover, at the level its three
//! neighbours are covered. The two halves meet in the design, not in one test.

extern crate alloc;

use alloc::vec::Vec;
use dclutch_capability_program_contract::CapabilityRootHeaderV1;
use dclutch_claims_svm::fractional_claim_check_compaction_receipt_v1::FractionalClaimCheckCompactionReceiptV1;
use dclutch_claims_svm::fractional_claim_check_compaction_request_v1::{
    FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1, FractionalCompactToClaimCheckRequestV1,
};
use dclutch_claims_svm::fractional_claim_check_v1::{
    FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1, FractionalCompactionRoleV1,
};
use dclutch_fractional_claim_contract::FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Coordinate of the Claims program account this caller invokes.
pub const FRACTIONAL_COMPACTION_TEST_CLAIMS_PROGRAM_COORDINATE: usize = 0;

/// Exact instruction width: one action byte, then the request verbatim.
pub const FRACTIONAL_COMPACTION_TEST_WRAPPER_BYTES: usize =
    1 + FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1;

const _: () = assert!(
    FRACTIONAL_COMPACTION_TEST_WRAPPER_BYTES
        != dclutch_fractional_atomic_test_caller_sbf::FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES,
    "atomic and compaction wrappers must remain width-disjoint"
);

const REQUEST_OFFSET: usize = 1;

/// What this caller does with the root's signature on one forwarding.
///
/// Three arms, and the second is the reason this enum exists at all: the
/// security property design §17.8 rests on is that a compaction arriving
/// *without* Trading dies at the `SetAuthority` hand-off. Proving that needs a
/// run identical to the admitted one in every respect except the signature, and
/// a separate hand-built transaction would differ in more than the one fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FractionalCompactionCallerActionV1 {
    /// Sign the root and forward. The admitted path.
    Signed = 0,
    /// Forward WITHOUT signing the root: witness w8, the direct-entry hostile.
    ///
    /// Everything else is identical -- same frame, same accounts, same request
    /// bytes, same program. Only the root's meta arrives unsigned, so the
    /// `SetAuthority` that re-points `PermissionedBurn` is refused by Token-2022
    /// for want of the current authority's signature. That refusal is what
    /// "Trading-composed" enforces on this route, and it is enforced by the root
    /// signer alone.
    UnsignedRoot = 1,
    /// Sign, forward, then refuse: a late failure, so rollback is provable.
    FailAfterCommit = 2,
}

impl FractionalCompactionCallerActionV1 {
    /// Decode one action byte, refusing every value this caller does not define.
    ///
    /// Exhaustive rather than a range check, so a fourth action is a compile
    /// error at its author's hand rather than a byte that silently means
    /// something.
    const fn decode(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Signed),
            1 => Some(Self::UnsignedRoot),
            2 => Some(Self::FailAfterCommit),
            _ => None,
        }
    }

    /// Whether this action marks the capability root's meta a signer.
    const fn signs_the_root(self) -> bool {
        match self {
            Self::Signed | Self::FailAfterCommit => true,
            Self::UnsignedRoot => false,
        }
    }
}

/// Stable fractional compaction test-caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalCompactionTestCallerError {
    /// The wrapper width, the action byte, or the request refused.
    Instruction = 0x10_D000,
    /// The forwarded account frame was not the exact compaction frame.
    AccountFrame = 0x10_D001,
    /// The capability root did not decode, or did not derive under this id.
    Root = 0x10_D002,
    /// The Claims CPI refused, or returned a receipt this caller cannot admit.
    ClaimsCpi = 0x10_D003,
    /// The deliberate post-commit refusal, for proving rollback.
    DeliberateLateFailure = 0x10_D004,
}

impl FractionalCompactionTestCallerError {
    /// Every refusal this caller can raise, in discriminant order.
    pub const ALL: [Self; 5] = [
        Self::Instruction,
        Self::AccountFrame,
        Self::Root,
        Self::ClaimsCpi,
        Self::DeliberateLateFailure,
    ];

    /// This refusal's position in [`FractionalCompactionTestCallerError::ALL`].
    ///
    /// Exhaustive on purpose: a sixth variant is a COMPILE ERROR here rather
    /// than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::AccountFrame => 1,
            Self::Root => 2,
            Self::ClaimsCpi => 3,
            Self::DeliberateLateFailure => 4,
        }
    }
}

// The registered band, checked element by element against `ALL` for the reason
// the production tables are: a hand-named ceiling says nothing about the
// variants after it and goes stale silently every time the family grows.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::TEST_CLAIMS_FRACTIONAL_COMPACTION_CALLER_BASE;
    assert!(
        FractionalCompactionTestCallerError::ALL[0] as u32 == SUB_BAND,
        "FractionalCompactionTestCallerError must start at its registered band"
    );
    let mut index = 0;
    while index < FractionalCompactionTestCallerError::ALL.len() {
        let variant = FractionalCompactionTestCallerError::ALL[index];
        assert!(
            variant.ordinal() == index,
            "FractionalCompactionTestCallerError::ALL repeats a variant, skips one, or is out of order"
        );
        assert!(
            variant as u32 == SUB_BAND + index as u32,
            "FractionalCompactionTestCallerError discriminants are not the contiguous run ALL claims"
        );
        assert!(
            (variant as u32) < SUB_BAND + dclutch_refusal_registry::BAND_SPAN,
            "FractionalCompactionTestCallerError must not run past its registered band"
        );
        index += 1;
    }
};

impl From<FractionalCompactionTestCallerError> for ProgramError {
    fn from(value: FractionalCompactionTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Sign the Fractional capability root and forward one exact compaction request.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len()
        == dclutch_fractional_atomic_test_caller_sbf::FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES
    {
        return dclutch_fractional_atomic_test_caller_sbf::process_instruction(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if instruction_data.len() != FRACTIONAL_COMPACTION_TEST_WRAPPER_BYTES {
        return Err(FractionalCompactionTestCallerError::Instruction.into());
    }
    let action = FractionalCompactionCallerActionV1::decode(
        *instruction_data
            .first()
            .ok_or(FractionalCompactionTestCallerError::Instruction)?,
    )
    .ok_or(FractionalCompactionTestCallerError::Instruction)?;
    let request_bytes = instruction_data
        .get(REQUEST_OFFSET..)
        .ok_or(FractionalCompactionTestCallerError::Instruction)?;
    // Decoded here and forwarded VERBATIM. This caller never rewrites a
    // coordinate or a byte: the digest the child hashes has to be the digest
    // this caller hashes, or the receipt binding proves nothing.
    let request = FractionalCompactToClaimCheckRequestV1::decode(request_bytes)
        .map_err(|_| FractionalCompactionTestCallerError::Instruction)?;

    let claims_program = accounts
        .get(FRACTIONAL_COMPACTION_TEST_CLAIMS_PROGRAM_COORDINATE)
        .ok_or(FractionalCompactionTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(FractionalCompactionTestCallerError::AccountFrame)?;
    if !claims_program.executable
        || claims_program.is_signer
        || claims_program.is_writable
        || forwarded.len() != FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1
    {
        return Err(FractionalCompactionTestCallerError::AccountFrame.into());
    }

    // The root's coordinate comes from the frame declaration, never from
    // `accounts.len()` or from anything the caller of this caller chose. A test
    // program that inferred where to sign would sign wherever it was pointed.
    let root_coordinate = FractionalCompactionRoleV1::FractionalCapabilityRoot
        .index()
        .ok_or(FractionalCompactionTestCallerError::AccountFrame)?;
    let root_account = forwarded
        .get(root_coordinate)
        .ok_or(FractionalCompactionTestCallerError::AccountFrame)?;
    // The inversion, enforced here too. Compaction's root is (signer, NOT
    // writable) -- unlike its three exposure neighbours, whose effect programs
    // commit a revision. A caller that forwarded a writable root would be
    // handing the child privileges the frame does not declare, and the child
    // would refuse; refusing here says which of the two was wrong.
    if root_account.is_writable {
        return Err(FractionalCompactionTestCallerError::AccountFrame.into());
    }

    // The root's own immutable header is the sole seed source. Deriving from
    // anything the caller was told would let a test fabricate a root PDA.
    let root_seeds = {
        let root_data = root_account
            .try_borrow_data()
            .map_err(|_| FractionalCompactionTestCallerError::Root)?;
        let header_bytes = root_data
            .get(..FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4)
            .ok_or(FractionalCompactionTestCallerError::Root)?;
        CapabilityRootHeaderV1::decode(header_bytes)
            .map_err(|_| FractionalCompactionTestCallerError::Root)?
            .seeds()
    };
    let (expected_root, root_bump) =
        Pubkey::find_program_address(&root_seeds.as_slices(), program_id);
    if root_account.key != &expected_root {
        return Err(FractionalCompactionTestCallerError::Root.into());
    }

    // NO CALLER AUTHORITY IS DERIVED, AND THERE IS NO CODE HERE THAT COULD.
    // Design §17.8 ruling 2: a deadline-entitled permissionless crank takes no
    // parent's authority. Witness w7 is this absence, run rather than asserted.
    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = account.is_signer || (index == root_coordinate && action.signs_the_root());
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };

    let root_bump_seed = [root_bump];
    let [
        root_domain,
        root_market,
        root_generation,
        root_manifest,
        root_entry,
        root_kind,
        root_release,
        root_config,
    ] = root_seeds.as_slices();
    let root_signer: &[&[u8]] = &[
        root_domain,
        root_market,
        root_generation,
        root_manifest,
        root_entry,
        root_kind,
        root_release,
        root_config,
        &root_bump_seed,
    ];

    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    // On the w8 path the seed list is EMPTY, not merely unused. Passing the
    // root's seeds while declining to mark its meta would still hand the
    // runtime a signature for that address, and the hostile would be testing
    // the meta flag rather than the signature.
    let signers: &[&[&[u8]]] = if action.signs_the_root() {
        &[root_signer]
    } else {
        &[]
    };
    invoke_signed(&instruction, &infos, signers)
        .map_err(|_| FractionalCompactionTestCallerError::ClaimsCpi)?;

    // The receipt, verified by the caller as well as by the parent it stands in
    // for. Trading does this with `verify_fractional_claim_check_compaction_receipt`;
    // doing it here too is what makes a campaign failure point at the route
    // rather than at the harness.
    let (producer, receipt_bytes) =
        get_return_data().ok_or(FractionalCompactionTestCallerError::ClaimsCpi)?;
    if &producer != claims_program.key {
        return Err(FractionalCompactionTestCallerError::ClaimsCpi.into());
    }
    let receipt = FractionalClaimCheckCompactionReceiptV1::decode(&receipt_bytes)
        .map_err(|_| FractionalCompactionTestCallerError::ClaimsCpi)?;
    receipt
        .verify_for(request, hash(request_bytes).to_bytes())
        .map_err(|_| FractionalCompactionTestCallerError::ClaimsCpi)?;
    if receipt.root() != expected_root.to_bytes() {
        return Err(FractionalCompactionTestCallerError::ClaimsCpi.into());
    }

    if action == FractionalCompactionCallerActionV1::FailAfterCommit {
        return Err(FractionalCompactionTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}
