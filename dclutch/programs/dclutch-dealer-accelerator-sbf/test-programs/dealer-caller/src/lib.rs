#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Read-only ProgramTest caller for the Dealer admitted accelerator.
//!
//! The caller stands in for common Trading on the admitted-AOT path. It is
//! installed at the Trading fixed slot of the frame it forwards, so the
//! caller-authority PDA it signs under its own `program_id` is the same PDA
//! `authenticate_accelerator_caller_authority_v4` re-derives under
//! `frame.trading_program.key`. It owns no protocol semantics, mutates no
//! account, and grants no child authority.
//!
//! # Top-level account layout
//!
//! `authenticate_accelerator_top_level_v4` reads the *top-level* instruction
//! out of the Instructions sysvar and requires its account metas to be the
//! canonical admitted-AOT Trading Hot layout, positionally:
//!
//! ```text
//!   [0 .. 39)                    common Hot fixed frame (root writable, rest read-only)
//!   [39 .. 47)                   admitted strategy evidence
//!   [47 .. 47 + chunk_count)     one caller authority per canonical output chunk
//!   [47 + chunk_count .. len-2)  AccountProfile runtime suffix (logical coordinates 5..)
//!   [len-2]                      exact AcceleratorRequestV2 body account   (test-only)
//!   [len-1]                      the accelerator program                   (test-only)
//! ```
//!
//! The first three spans are what Trading itself emits and are what the
//! authentication chain positionally checks; the two trailing accounts are
//! this caller's own affordance and sit past everything Trading inspects.
//!
//! The previous layout put the request body and the accelerator *first*, so
//! the top-level metas began `[request, accelerator, authority, ...]` while
//! `metas_range(0, HOT_FIXED_ACCOUNT_COUNT_V3)` demanded the Hot fixed frame
//! at offset zero. No frame could satisfy it, so no invocation through this
//! caller could ever authenticate. Only the refusal test existed, so nothing
//! ever noticed.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3,
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_CONFIG_COORDINATE_V3,
    HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
    HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, HOT_RUNTIME_PRODUCT_COORDINATE_V3,
    HOT_RUNTIME_ROOT_COORDINATE_V3, HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::AcceleratorRequestV2;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading_sbf::admitted_composition_v3::{
    ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4, ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Number of accounts this caller appends past everything Trading inspects.
///
/// The exact request body, then the accelerator program.
pub const DEALER_ACCELERATOR_TEST_CALLER_SUFFIX_ACCOUNTS_V1: usize = 2;

/// First caller-authority account in the top-level instruction.
pub const DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1: usize =
    ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4 - 1;

/// Stable refusal from the test-only caller.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorTestCallerErrorV1 {
    /// The request, accelerator, or forwarded frame was malformed.
    Frame = 0x10_8000,
    /// The canonical caller-authority PDA or privilege set differed.
    Authority = 0x10_8001,
    /// The accelerator returned no typed bytes or another producer returned.
    ReturnData = 0x10_8002,
}

impl DealerAcceleratorTestCallerErrorV1 {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`DealerAcceleratorTestCallerErrorV1::ordinal`], whose match is exhaustive: a variant added
    /// to the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 3] = [Self::Frame, Self::Authority, Self::ReturnData];

    /// This refusal's position in [`DealerAcceleratorTestCallerErrorV1::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a fourth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Frame => 0,
            Self::Authority => 1,
            Self::ReturnData => 2,
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
        DealerAcceleratorTestCallerErrorV1::ALL[0] as u32
            == dclutch_refusal_registry::TEST_DEALER_ACCELERATOR_CALLER_BASE,
        "DealerAcceleratorTestCallerErrorV1 must start at its registered refusal band base"
    );
    let mut index: u32 = 0;
    let mut rest = DealerAcceleratorTestCallerErrorV1::ALL.as_slice();
    while let [variant, tail @ ..] = rest {
        let variant = *variant;
        assert!(
            variant.ordinal() == index as usize,
            "DealerAcceleratorTestCallerErrorV1::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == dclutch_refusal_registry::TEST_DEALER_ACCELERATOR_CALLER_BASE + index,
            "DealerAcceleratorTestCallerErrorV1 discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_DEALER_ACCELERATOR_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "DealerAcceleratorTestCallerErrorV1 must not run past its registered refusal band"
        );
        index += 1;
        rest = tail;
    }
};

// The caller authority block starts exactly where the strategy evidence ends,
// and the accelerator's own runtime slice starts one further along because it
// is offset by the authority the CPI prepends. Deriving both from the composer
// constants is what keeps this stand-in honest when the frame widens again.
const _: () = assert!(
    DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1
        > ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4,
    "the caller authority block must follow the admitted strategy evidence"
);

impl From<DealerAcceleratorTestCallerErrorV1> for ProgramError {
    fn from(value: DealerAcceleratorTestCallerErrorV1) -> Self {
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

/// Invoke one admitted accelerator request without granting mutation authority.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let suffix_start = accounts
        .len()
        .checked_sub(DEALER_ACCELERATOR_TEST_CALLER_SUFFIX_ACCOUNTS_V1)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    if suffix_start < DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1 {
        return Err(DealerAcceleratorTestCallerErrorV1::Frame.into());
    }
    let request_account = accounts
        .get(suffix_start)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let accelerator = accounts
        .get(
            suffix_start
                .checked_add(1)
                .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
        )
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let fixed_and_evidence = accounts
        .get(..DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let root = accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let request_bytes = request_account
        .try_borrow_data()
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let request = AcceleratorRequestV2::decode(&request_bytes)
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let chunk_count = usize::try_from(request.chunk_count())
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let chunk_index = usize::try_from(request.chunk_index())
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    if chunk_index >= chunk_count {
        return Err(DealerAcceleratorTestCallerErrorV1::Frame.into());
    }
    let runtime_start = DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1
        .checked_add(chunk_count)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    if runtime_start > suffix_start {
        return Err(DealerAcceleratorTestCallerErrorV1::Frame.into());
    }
    let authority = accounts
        .get(
            DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_START_V1
                .checked_add(chunk_index)
                .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
        )
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let runtime_suffix = accounts
        .get(runtime_start..suffix_start)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let (expected_authority, seeds, bump) = dealer_accelerator_test_caller_authority_v1(
        program_id,
        instruction_data,
        root.key,
        &request_bytes,
    )?;
    if authority.key != &expected_authority
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || !accelerator.executable
        || request_account.is_signer
        || request_account.is_writable
        || request_account.executable
    {
        return Err(DealerAcceleratorTestCallerErrorV1::Authority.into());
    }

    // The logical runtime frame Trading hands an accelerator opens with the
    // five coordinates that are drawn from the fixed frame, so the top-level
    // instruction only carries the suffix. Rejoining them here is what makes
    // `context.account_count` and the runtime observation transcript the same
    // sequence the real producer commits.
    let runtime = runtime_accounts(accounts, runtime_suffix)?;

    let mut metas = Vec::with_capacity(
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4
            .checked_add(runtime.len())
            .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
    );
    metas.push(AccountMeta {
        pubkey: *authority.key,
        is_signer: true,
        is_writable: false,
    });
    metas.extend(
        fixed_and_evidence
            .iter()
            .chain(runtime.iter().copied())
            .map(|account| AccountMeta {
                pubkey: *account.key,
                is_signer: false,
                is_writable: false,
            }),
    );
    let instruction = Instruction {
        program_id: *accelerator.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4
            .checked_add(runtime.len())
            .and_then(|value| value.checked_add(1))
            .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
    );
    infos.push(authority.clone());
    infos.extend(fixed_and_evidence.iter().cloned());
    infos.extend(runtime.iter().map(|account| (*account).clone()));
    infos.push(accelerator.clone());
    let bump = [bump];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    drop(request_bytes);
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )?;
    let (producer, bytes) =
        get_return_data().ok_or(DealerAcceleratorTestCallerErrorV1::ReturnData)?;
    if producer != *accelerator.key || bytes.is_empty() {
        return Err(DealerAcceleratorTestCallerErrorV1::ReturnData.into());
    }
    set_return_data(&bytes);
    Ok(())
}

/// Rejoin the five fixed logical coordinates ahead of the runtime suffix.
fn runtime_accounts<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    suffix: &'a [AccountInfo<'info>],
) -> Result<Vec<&'a AccountInfo<'info>>, DealerAcceleratorTestCallerErrorV1> {
    let fixed = [
        (HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_ROOT_ACCOUNT_V3),
        (HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_CONFIG_RAW_ACCOUNT_V3),
        (
            HOT_RUNTIME_PRODUCT_COORDINATE_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ),
    ];
    let mut runtime = Vec::with_capacity(
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
            .checked_add(suffix.len())
            .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
    );
    for (coordinate, fixed_index) in fixed {
        if coordinate != runtime.len() {
            return Err(DealerAcceleratorTestCallerErrorV1::Frame);
        }
        runtime.push(
            accounts
                .get(fixed_index)
                .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
        );
    }
    if runtime.len() != HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 {
        return Err(DealerAcceleratorTestCallerErrorV1::Frame);
    }
    runtime.extend(suffix.iter());
    Ok(runtime)
}

/// Derive the canonical Trading caller-authority PDA for one test invocation.
pub fn dealer_accelerator_test_caller_authority_v1(
    program_id: &Pubkey,
    hot_instruction: &[u8],
    root: &Pubkey,
    request: &[u8],
) -> Result<(Pubkey, CallerAuthoritySeedsV1, u8), ProgramError> {
    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(hot_instruction)
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(envelope.release_set())
            .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?,
        envelope.market(),
        ExecutionRoleV1::Trading,
        root.to_bytes(),
        hash(request).to_bytes(),
    )
    .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    Ok((authority, seeds, bump))
}

/// Exact common Hot fixed-frame width this caller forwards.
pub const DEALER_ACCELERATOR_TEST_CALLER_FIXED_COUNT_V1: usize = HOT_FIXED_ACCOUNT_COUNT_V3;
