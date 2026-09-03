//! Core-authorized atomic Claims founding over the sole LBV2 ledger.
//!
//! The parent Core route has already executed and producer-authenticated the
//! exact Custody source-to-Hoard transfer. This adapter independently joins
//! the current release-set authority, Custody post-observations, finalized
//! Product/basis graph, Founding Core Market, canonical Claims PDAs, and exact
//! prepaid rent before allocating or committing any Claims state.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};
use core::convert::TryFrom;

use dclutch_claims_svm::{
    founding_v5::{
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, ClaimsFoundingAggregateSeedsV5,
        ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
    },
    series_founding_transport_v1::SeriesClaimsFoundingTransportV1,
};
use dclutch_custody_contract::{CallerRoleV1, CustodyReplayV1};
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyLockReceiptV1, ProjectedCustodyReceiptV1,
};
use dclutch_market_core_codec::{
    CoreState, FoundingIntentV5, Identity, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES,
    SeriesFoundingPermitV1, SeriesPermitJoinMismatchV1,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV3};
use dclutch_registry_activation_auth_v1::ActivationAuthErrorV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_source_contract::MarketPrincipalCapSetsV1;
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::Instruction,
    log::sol_log,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign};

use super::affine_batch_v2::authenticate_runtime_product_basis_core_with_rent_v3;
use crate::claims_cu_checkpoint;
use crate::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2, MarketViewV2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2, vector_width,
};
use crate::market_admission_v1::CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1;
use dclutch_claims_svm::liability_basis_state_v2::{
    put_liability_basis_market_bump_v2, put_liability_basis_position_bump_v2,
};

pub use dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_ACCOUNT_COUNT_V5;
/// Exact request plus typed projected-Custody receipt instruction width.
pub const CLAIMS_FOUNDING_INSTRUCTION_BYTES_V5: usize =
    dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
        + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1
        + PROJECTED_CUSTODY_RECEIPT_BYTES_V1;

const AUTHORITY: usize = 0;
const PERMIT: usize = 1;
const AGGREGATE: usize = 2;
const POSITION: usize = 3;
const ADMISSION: usize = 4;
const FUNDING_SOURCE: usize = 5;
const HOARD: usize = 6;
const CUSTODY_REPLAY: usize = 7;
const BASIS_RECORD: usize = 8;
const BASIS_STAGING: usize = 9;
const PRODUCT_RECORD: usize = 10;
const PRODUCT_STAGING: usize = 11;
const RESULT_RECORD: usize = 12;
const RESULT_STAGING: usize = 13;
const PORTFOLIO_RECORD: usize = 14;
const PORTFOLIO_STAGING: usize = 15;
const SYSTEM: usize = 16;
const CORE_MARKET: usize = 17;
const CACHE: usize = 18;
const REGISTRY: usize = 19;
const CLAIMS_PROGRAM: usize = 20;
const CLAIMS_PROGRAMDATA: usize = 21;
const CORE_PROGRAM: usize = 22;
const CORE_PROGRAMDATA: usize = 23;
const TRADING_PROGRAM: usize = 24;
const TRADING_PROGRAMDATA: usize = 25;
const CUSTODY_PROGRAM: usize = 26;
const CUSTODY_PROGRAMDATA: usize = 27;
const FOUNDER: usize = 28;
const RENT_CREDIT: usize = 29;
const RENT_PROGRAM: usize = 30;

/// Stable FoundingV5 adapter refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsFoundingSbfErrorV5 {
    /// Instruction bytes did not decode as the sole FoundingV5 ABI.
    Instruction = 0x5180,
    /// Account count, privileges, executable flags, or aliases refused.
    Accounts = 0x5181,
    /// An activated role's own receipt names a different release set than the
    /// one this request executes under.
    ///
    /// Narrow on purpose. This code used to mean "Core caller authority or
    /// current release selection refused" and was raised at nineteen sites
    /// covering four unrelated subsystems, nine of them `map_err(|_| ..)`
    /// wrappers that threw away a cause the callee had already computed. The
    /// other eighteen now have their own names below.
    Release = 0x5182,
    /// Custody source, Hoard, or replay post-observations refused.
    Custody = 0x5183,
    /// Product graph, linked basis, or Founding Core Market refused.
    ProductBasis = 0x5184,
    /// Claims aggregate, Position, or admission PDA/vacancy refused.
    ClaimsState = 0x5185,
    /// Rent sysvar, exact principals, target lamports, or RentCredit refused.
    Rent = 0x5186,
    /// System allocation or assignment refused.
    Allocation = 0x5187,
    /// Candidate receipt or post-resource digest refused.
    Receipt = 0x5188,
    /// State-last copy or immutable postcondition refused.
    Commit = 0x5189,
    /// The founding principal exceeded the Market's carried manipulation-capacity
    /// cap, or that cap was never stated.
    ///
    /// Separate from [`Self::ProductBasis`] because it is the one refusal in this
    /// family that is a *policy* answer rather than a malformed-input answer: the
    /// records all decoded, the graph all joined, and the founding was refused
    /// because the principal it asked for is larger than the venue behind its
    /// Source can be manipulated for. A reader of a validator log that cannot
    /// tell those two apart cannot tell a broken founding from a refused one.
    PrincipalCapacity = 0x518A,
    /// The Trading-signed caller-authority PDA did not reproduce.
    ///
    /// The seeds are the request's own release set, Market, execution role,
    /// founding intent and request digest, and the program they are derived
    /// under is the request's Trading program. A refusal here says the account
    /// presented as the authority is not the one those seeds address, or that
    /// one of the coordinates was the zero identity.
    CallerAuthority = 0x518B,
    /// The Core-owned Series founding permit account, its canonical address, or
    /// the bump its intent carries for that address refused.
    ///
    /// About the ACCOUNT, not its content: owner, exact width, hostile decode,
    /// the PDA the permit's own seeds address, and the bump byte the intent
    /// states for it. [`Self::PermitBody`] is the content.
    Permit = 0x518C,
    /// The permit's authorization of this intent and request, or the intent's
    /// own agreement with the request it authorizes, refused.
    ///
    /// This is the eighteen-conjunct join, and it is where the founding's
    /// digests meet: a single byte moved anywhere upstream of the projected
    /// Custody receipt reaches this comparison as two unequal digests and
    /// nothing else. It carries a named conjunct on the refusing path for
    /// exactly that reason.
    PermitBody = 0x518D,
    /// The account handed as the Registry activation cache is not the canonical
    /// cache for this request's release set, or its body did not decode.
    ///
    /// Surfaced from [`ActivationAuthErrorV1::ActivationCache`].
    ActivationCache = 0x518E,
    /// An activated role's observed on-chain deployment is not the one its
    /// activation admitted.
    ///
    /// Surfaced from [`ActivationAuthErrorV1::Deployment`]. Distinct from
    /// [`Self::ReleaseSuperseded`]: this one says the program, ProgramData,
    /// loader or ELF digest disagrees with the activation record, which is a
    /// wrong-account or stale-plan answer rather than an upgrade.
    RoleDeployment = 0x518F,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    /// Every open market on the superseded release generation refuses until a
    /// re-release re-authenticates the new deployment and re-pins its slot.
    ///
    /// WORD-FOR-WORD the sentence the other seven bands register, and it has to
    /// be. Decision 0012 gave this one event a name in every program that can
    /// observe it, `apps/dclutch-web/lib/refusals.ts::releaseSupersededMeaningV1`
    /// reads the meaning out of the generated registry rather than writing its
    /// own, and it THROWS if the rows disagree. An eighth row phrased in this
    /// author's own words is a second authority on what a supersession means,
    /// which is exactly the defect that check exists to refuse -- it caught this
    /// variant's first draft on 2026-09-03.
    ///
    /// Surfaced here from [`ActivationAuthErrorV1::ReleaseSuperseded`]: the
    /// remedy is a re-release, not an investigation, and folding it into a
    /// generic release refusal was what made it unsayable on this route.
    ReleaseSuperseded = 0x5190,
}

impl ClaimsFoundingSbfErrorV5 {
    /// Every refusal this request family can raise, in discriminant order.
    ///
    /// This is what the sub-band assertions below read. It is kept honest by
    /// [`ClaimsFoundingSbfErrorV5::ordinal`], whose match is exhaustive: a variant added to the
    /// enum does not compile until its author writes an arm here, and the only arm that satisfies
    /// the assertions is its own index in this array.
    pub const ALL: [Self; 17] = [
        Self::Instruction,
        Self::Accounts,
        Self::Release,
        Self::Custody,
        Self::ProductBasis,
        Self::ClaimsState,
        Self::Rent,
        Self::Allocation,
        Self::Receipt,
        Self::Commit,
        Self::PrincipalCapacity,
        Self::CallerAuthority,
        Self::Permit,
        Self::PermitBody,
        Self::ActivationCache,
        Self::RoleDeployment,
        Self::ReleaseSuperseded,
    ];

    /// This refusal's position in [`ClaimsFoundingSbfErrorV5::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a twelfth variant is
    /// a COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::Accounts => 1,
            Self::Release => 2,
            Self::Custody => 3,
            Self::ProductBasis => 4,
            Self::ClaimsState => 5,
            Self::Rent => 6,
            Self::Allocation => 7,
            Self::Receipt => 8,
            Self::Commit => 9,
            Self::PrincipalCapacity => 10,
            Self::CallerAuthority => 11,
            Self::Permit => 12,
            Self::PermitBody => 13,
            Self::ActivationCache => 14,
            Self::RoleDeployment => 15,
            Self::ReleaseSuperseded => 16,
        }
    }

    /// The exact line this refusal writes to the validator log.
    ///
    /// A `&'static str` per variant rather than a `{:?}` format: `sol_log` takes
    /// a `&str` with no allocation, and this program's refusing paths run inside
    /// a founding whose compute margin is already the binding constraint. The
    /// match is exhaustive, so an eighteenth variant does not compile until its
    /// author says what a reader of a validator log should see.
    const fn log_line(self) -> &'static str {
        match self {
            Self::Instruction => "claims founding v5: refused, instruction bytes",
            Self::Accounts => "claims founding v5: refused, account frame",
            Self::Release => "claims founding v5: refused, role names another release set",
            Self::Custody => "claims founding v5: refused, custody post-observations",
            Self::ProductBasis => "claims founding v5: refused, product/basis graph",
            Self::ClaimsState => "claims founding v5: refused, claims PDA or vacancy",
            Self::Rent => "claims founding v5: refused, rent",
            Self::Allocation => "claims founding v5: refused, allocation",
            Self::Receipt => "claims founding v5: refused, receipt",
            Self::Commit => "claims founding v5: refused, commit postcondition",
            Self::PrincipalCapacity => "claims founding v5: refused, principal capacity cap",
            Self::CallerAuthority => "claims founding v5: refused, caller-authority PDA",
            Self::Permit => "claims founding v5: refused, permit account",
            Self::PermitBody => "claims founding v5: refused, permit body join",
            Self::ActivationCache => "claims founding v5: refused, activation cache",
            Self::RoleDeployment => "claims founding v5: refused, role deployment",
            Self::ReleaseSuperseded => "claims founding v5: refused, release superseded",
        }
    }
}

/// Name an activation-cache refusal, keeping the callee's own distinction.
///
/// `authenticate_activated_role_and_bump_v1` already computed WHICH conjunct
/// failed. Until this impl existed the four sites that call it wrote
/// `map_err(|_| Release)`, so a frame the caller built wrong, a cache belonging
/// to another release set, a deployment the activation never admitted, and an
/// upgraded substrate all published one code — and the reader who had to tell
/// them apart had to bisect a route with no diagnostic in it.
///
/// `AccountFrame` folds into [`ClaimsFoundingSbfErrorV5::Accounts`] rather than
/// taking a new discriminant because it is exactly that code's stated
/// accusation: privileges and executable flags on the three-account read-only
/// frame this adapter itself assembled.
impl From<ActivationAuthErrorV1> for ClaimsFoundingSbfErrorV5 {
    fn from(value: ActivationAuthErrorV1) -> Self {
        match value {
            ActivationAuthErrorV1::AccountFrame => Self::Accounts,
            ActivationAuthErrorV1::ActivationCache => Self::ActivationCache,
            ActivationAuthErrorV1::Deployment => Self::RoleDeployment,
            ActivationAuthErrorV1::ReleaseSuperseded => Self::ReleaseSuperseded,
        }
    }
}

/// Refuse with a named conjunct, on the refusing path only.
///
/// The wire carries one `u32`, so the code names the accusation and these two
/// static lines name the conjunct inside it. `#[cold]` and `#[inline(never)]`
/// keep both the branch and the strings off the accepting path, which is why a
/// named refusal costs the founding nothing it was not already paying.
#[cold]
#[inline(never)]
fn refuse(error: ClaimsFoundingSbfErrorV5, conjunct: &'static str) -> ProgramError {
    sol_log(error.log_line());
    sol_log(conjunct);
    error.into()
}

/// Require one named conjunct of a joined authentication.
///
/// Sequential `require`s evaluate exactly what a `||` chain evaluates and stop
/// at exactly the same place; what changes is that the one that stopped it can
/// say so.
#[inline(always)]
fn require(
    holds: bool,
    error: ClaimsFoundingSbfErrorV5,
    conjunct: &'static str,
) -> Result<(), ProgramError> {
    if holds {
        Ok(())
    } else {
        Err(refuse(error, conjunct))
    }
}

/// The line naming which execution role a founding refusal is about.
///
/// Which of the four roles the activation loop was on cannot ride a `u32` that
/// is already spent naming the cause, so it rides a static string instead.
const fn role_log_line(role: ExecutionRoleV1) -> &'static str {
    match role {
        ExecutionRoleV1::Core => "claims founding v5: role core",
        ExecutionRoleV1::Claims => "claims founding v5: role claims",
        ExecutionRoleV1::Trading => "claims founding v5: role trading",
        ExecutionRoleV1::Resolution => "claims founding v5: role resolution",
        ExecutionRoleV1::Custody => "claims founding v5: role custody",
    }
}

/// Refuse one role's activation under the cache's own cause and this role's name.
#[cold]
#[inline(never)]
fn refuse_activated_role(role: ExecutionRoleV1, error: ActivationAuthErrorV1) -> ProgramError {
    refuse(ClaimsFoundingSbfErrorV5::from(error), role_log_line(role))
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing about
// the variants after it and goes stale silently every single time the family
// grows -- the failure is not that the name is wrong, it is that nothing can
// notice. Claims' own top-level band proved it the expensive way: its bound went
// on naming `ReleaseSuperseded` after a later variant landed, so for as long as
// that stood, the newest refusal in the program was checked by nothing.
//
// So the sub-band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x180;
    assert!(
        ClaimsFoundingSbfErrorV5::ALL[0] as u32 == SUB_BAND,
        "ClaimsFoundingSbfErrorV5 must start at its registered sub-band offset"
    );
    let mut index: u32 = 0;
    let mut rest = ClaimsFoundingSbfErrorV5::ALL.as_slice();
    while let [variant, tail @ ..] = rest {
        let variant = *variant;
        assert!(
            variant.ordinal() == index as usize,
            "ClaimsFoundingSbfErrorV5::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == SUB_BAND + index,
            "ClaimsFoundingSbfErrorV5 discriminants are not the contiguous run from the sub-band offset that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "ClaimsFoundingSbfErrorV5 must not run past its registered refusal band"
        );
        index += 1;
        rest = tail;
    }
};

impl From<ClaimsFoundingSbfErrorV5> for ProgramError {
    fn from(value: ClaimsFoundingSbfErrorV5) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct FoundingAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    permit: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    funding_source: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_record: &'accounts AccountInfo<'info>,
    result_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    founder: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FoundingAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CLAIMS_FOUNDING_ACCOUNT_COUNT_V5 {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY)?,
            permit: account(accounts, PERMIT)?,
            aggregate: account(accounts, AGGREGATE)?,
            position: account(accounts, POSITION)?,
            admission: account(accounts, ADMISSION)?,
            funding_source: account(accounts, FUNDING_SOURCE)?,
            hoard: account(accounts, HOARD)?,
            custody_replay: account(accounts, CUSTODY_REPLAY)?,
            basis_record: account(accounts, BASIS_RECORD)?,
            basis_staging: account(accounts, BASIS_STAGING)?,
            product_record: account(accounts, PRODUCT_RECORD)?,
            product_staging: account(accounts, PRODUCT_STAGING)?,
            result_record: account(accounts, RESULT_RECORD)?,
            result_staging: account(accounts, RESULT_STAGING)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING)?,
            system: account(accounts, SYSTEM)?,
            core_market: account(accounts, CORE_MARKET)?,
            cache: account(accounts, CACHE)?,
            registry: account(accounts, REGISTRY)?,
            claims_program: account(accounts, CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA)?,
            core_program: account(accounts, CORE_PROGRAM)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA)?,
            trading_program: account(accounts, TRADING_PROGRAM)?,
            trading_programdata: account(accounts, TRADING_PROGRAMDATA)?,
            custody_program: account(accounts, CUSTODY_PROGRAM)?,
            custody_programdata: account(accounts, CUSTODY_PROGRAMDATA)?,
            founder: account(accounts, FOUNDER)?,
            rent_credit: account(accounts, RENT_CREDIT)?,
            rent_program: account(accounts, RENT_PROGRAM)?,
        })
    }
}

/// Execute one exact atomic Claims founding request.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    claims_cu_checkpoint!("found-enter");
    let decoded = decode_instruction(instruction_data)?;
    let accounts = FoundingAccounts::parse(account_infos)?;
    let rent = Rent::get().map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    authenticate_privileges(program_id, accounts, &decoded.request)?;
    claims_cu_checkpoint!("found-frame");
    authenticate_authority(accounts, &decoded.request, decoded.request_digest)?;
    claims_cu_checkpoint!("found-authority");
    process_authenticated(program_id, accounts, decoded, rent)
}

/// Execute one root-independent recurring-Series founding transport.
#[inline(never)]
pub(crate) fn process_series_transport(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let transient = decode_series_transport_instruction(instruction_data)?;
    let accounts = FoundingAccounts::parse(account_infos)?;
    if transient.transport.permit() != accounts.permit.key.to_bytes() {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "transport names a different permit than the account presented",
        ));
    }
    authenticate_series_transport_authority(accounts, &transient.transport, transient.digest)?;
    let permit = decode_permit_account(accounts)?;
    let request = Box::new(
        transient
            .transport
            .reconstruct_v5(
                permit.claims_intent_digest().to_bytes(),
                transient.lock_receipt.request_digest,
                transient.lock_receipt_digest,
            )
            .map_err(|_| ClaimsFoundingSbfErrorV5::Instruction)?,
    );
    let request_bytes = request.to_bytes();
    let decoded = DecodedFounding {
        request,
        lock_receipt: transient.lock_receipt,
        projected_receipt: transient.projected_receipt,
        request_digest: hash(&request_bytes).to_bytes(),
        lock_receipt_digest: transient.lock_receipt_digest,
        projected_receipt_digest: transient.projected_receipt_digest,
    };
    let rent = Rent::get().map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    authenticate_privileges(program_id, accounts, &decoded.request)?;
    process_authenticated(program_id, accounts, decoded, rent)
}

#[inline(never)]
fn process_authenticated(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    decoded: DecodedFounding,
    rent: Rent,
) -> Result<(), ProgramError> {
    let request = decoded.request;
    let lock_receipt = decoded.lock_receipt;
    let projected_receipt = decoded.projected_receipt;
    let request_digest = decoded.request_digest;
    let lock_receipt_digest = decoded.lock_receipt_digest;
    let projected_receipt_digest = decoded.projected_receipt_digest;
    // The four-role activation loop. It is the FIRST thing this route does and
    // the reason these marks exist: a Claims child is one `consumed` line in
    // its caller's log, and one number cannot say whether it was spent on work
    // only Claims can do or on re-establishing a release set its caller had
    // already established. `found-releases` minus `found-authority` is that
    // question's answer for this route, and the open question in
    // `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md` is asked about
    // exactly this quantity on the SignedDelta route next door.
    authenticate_releases(accounts, &request)?;
    claims_cu_checkpoint!("found-releases");
    let custody_context = authenticate_permit_and_projection(
        accounts,
        &request,
        request_digest,
        &lock_receipt,
        lock_receipt_digest,
        &projected_receipt,
        projected_receipt_digest,
    )?;
    claims_cu_checkpoint!("found-permit");
    authenticate_custody_poststate(
        accounts,
        &request,
        &projected_receipt,
        projected_receipt_digest,
    )?;
    claims_cu_checkpoint!("found-custody");
    let market = authenticate_product_core(program_id, accounts, &request, custody_context, &rent)?;
    claims_cu_checkpoint!("found-product-core");
    authenticate_rent_and_vacancy(program_id, accounts, &request, market, &rent)?;
    claims_cu_checkpoint!("found-rent-vacancy");

    let candidates =
        build_candidates_boxed(program_id, accounts, &request, market, request_digest)?;
    let receipt = build_receipt(&request, request_digest, &candidates)?;
    claims_cu_checkpoint!("found-candidates");

    allocate_all(program_id, accounts, &request, &candidates)?;
    claims_cu_checkpoint!("found-allocate");
    commit_candidates(accounts, &candidates)?;
    set_return_data(receipt.as_slice());
    claims_cu_checkpoint!("found-commit");
    Ok(())
}

#[inline(never)]
fn decode_instruction(instruction_data: &[u8]) -> Result<DecodedFounding, ProgramError> {
    if instruction_data.len() != CLAIMS_FOUNDING_INSTRUCTION_BYTES_V5 {
        return Err(ClaimsFoundingSbfErrorV5::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(..dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let projected_receipt_bytes = instruction_data
        .get(
            dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                ..dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                    + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
        )
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let realized_receipt_bytes = instruction_data
        .get(
            dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1..,
        )
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    Ok(DecodedFounding {
        request: Box::new(
            ClaimsFoundingRequestV5::decode(request_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Instruction)?,
        ),
        lock_receipt: Box::new(
            ProjectedCustodyLockReceiptV1::decode(projected_receipt_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        projected_receipt: Box::new(
            ProjectedCustodyReceiptV1::decode(realized_receipt_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        request_digest: hash(request_bytes).to_bytes(),
        lock_receipt_digest: hash(projected_receipt_bytes).to_bytes(),
        projected_receipt_digest: hash(realized_receipt_bytes).to_bytes(),
    })
}

#[inline(never)]
fn build_receipt(
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    candidates: &FoundingCandidates,
) -> Result<
    Box<[u8; dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_RECEIPT_BYTES_V5]>,
    ProgramError,
> {
    let receipt = ClaimsFoundingReceiptV5::new(
        *request,
        request_digest,
        hash(&candidates.aggregate).to_bytes(),
        hash(&candidates.position).to_bytes(),
        hash(&candidates.admission).to_bytes(),
        hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
            &candidates.aggregate,
            &candidates.position,
            &candidates.admission,
        ])
        .to_bytes(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::Receipt)?;
    Ok(Box::new(receipt.to_bytes()))
}

struct FoundingCandidates {
    aggregate: Vec<u8>,
    position: Vec<u8>,
    admission: [u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2],
    /// The two PDA bumps this route both SIGNS with and RECORDS in the bodies.
    ///
    /// Derived once, in `build_candidates_boxed`, because the bump has to be in
    /// the candidate before `build_receipt` hashes it and before `allocate_all`
    /// signs with it -- and deriving it twice would put back one of the searches
    /// persisting it exists to remove.
    aggregate_bump: u8,
    position_bump: u8,
}

struct DecodedFounding {
    request: Box<ClaimsFoundingRequestV5>,
    lock_receipt: Box<ProjectedCustodyLockReceiptV1>,
    projected_receipt: Box<ProjectedCustodyReceiptV1>,
    request_digest: [u8; 32],
    lock_receipt_digest: [u8; 32],
    projected_receipt_digest: [u8; 32],
}

struct DecodedSeriesTransport {
    transport: SeriesClaimsFoundingTransportV1,
    lock_receipt: Box<ProjectedCustodyLockReceiptV1>,
    projected_receipt: Box<ProjectedCustodyReceiptV1>,
    digest: [u8; 32],
    lock_receipt_digest: [u8; 32],
    projected_receipt_digest: [u8; 32],
}

#[inline(never)]
fn decode_series_transport_instruction(
    instruction_data: &[u8],
) -> Result<DecodedSeriesTransport, ProgramError> {
    if instruction_data.len() != CLAIMS_FOUNDING_INSTRUCTION_BYTES_V5 {
        return Err(ClaimsFoundingSbfErrorV5::Instruction.into());
    }
    let request_end =
        dclutch_claims_svm::series_founding_transport_v1::SERIES_CLAIMS_FOUNDING_TRANSPORT_BYTES_V1;
    let lock_end = request_end
        .checked_add(PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let request_bytes = instruction_data
        .get(..request_end)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let lock_bytes = instruction_data
        .get(request_end..lock_end)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let projected_bytes = instruction_data
        .get(lock_end..)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    Ok(DecodedSeriesTransport {
        transport: SeriesClaimsFoundingTransportV1::decode(request_bytes)
            .map_err(|_| ClaimsFoundingSbfErrorV5::Instruction)?,
        lock_receipt: Box::new(
            ProjectedCustodyLockReceiptV1::decode(lock_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        projected_receipt: Box::new(
            ProjectedCustodyReceiptV1::decode(projected_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        digest: hash(request_bytes).to_bytes(),
        lock_receipt_digest: hash(lock_bytes).to_bytes(),
        projected_receipt_digest: hash(projected_bytes).to_bytes(),
    })
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || accounts.permit.is_signer
        || accounts.permit.is_writable
        || accounts.permit.executable
        || !accounts.aggregate.is_writable
        || !accounts.position.is_writable
        || !accounts.admission.is_writable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.key.to_bytes() != request.claims_program()
        || accounts.trading_program.key.to_bytes() != request.trading_program()
        || !accounts.claims_program.executable
        || !accounts.core_program.executable
        || !accounts.trading_program.executable
        || !accounts.custody_program.executable
        || !accounts.registry.executable
        || !accounts.rent_program.executable
        || accounts.system.key != &system_program::ID
        || !accounts.system.executable
    {
        return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
    }
    for readonly in [
        accounts.permit,
        accounts.funding_source,
        accounts.hoard,
        accounts.custody_replay,
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_record,
        accounts.result_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
        accounts.system,
        accounts.core_market,
        accounts.cache,
        accounts.registry,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.core_programdata,
        accounts.trading_program,
        accounts.trading_programdata,
        accounts.custody_program,
        accounts.custody_programdata,
        accounts.founder,
        accounts.rent_credit,
        accounts.rent_program,
    ] {
        if readonly.is_signer || readonly.is_writable {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
    }
    require_distinct(&[
        accounts.authority,
        accounts.permit,
        accounts.aggregate,
        accounts.position,
        accounts.admission,
        accounts.funding_source,
        accounts.hoard,
        accounts.custody_replay,
        accounts.core_market,
        accounts.registry,
        accounts.claims_program,
        accounts.core_program,
        accounts.trading_program,
        accounts.custody_program,
        accounts.founder,
        accounts.rent_credit,
        accounts.rent_program,
    ])
}

#[inline(never)]
fn authenticate_authority(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set(),
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        request_digest,
    )
    .map_err(|_| {
        refuse(
            ClaimsFoundingSbfErrorV5::CallerAuthority,
            "a caller-authority seed coordinate is zero or malformed",
        )
    })?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.trading_program.key).0;
    if accounts.authority.key != &expected {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::CallerAuthority,
            "authority is not the PDA these seeds address under the trading program",
        ));
    }
    Ok(())
}

#[inline(never)]
fn authenticate_series_transport_authority(
    accounts: FoundingAccounts<'_, '_>,
    transport: &SeriesClaimsFoundingTransportV1,
    transport_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || accounts.trading_program.key.to_bytes() != transport.trading_program()
        || !accounts.trading_program.executable
    {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::CallerAuthority,
            "transport authority privileges, or the trading program it names",
        ));
    }
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        transport.release_set(),
        transport.market(),
        ExecutionRoleV1::Trading,
        transport.permit(),
        transport_digest,
    )
    .map_err(|_| {
        refuse(
            ClaimsFoundingSbfErrorV5::CallerAuthority,
            "a transport caller-authority seed coordinate is zero or malformed",
        )
    })?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.trading_program.key).0;
    if accounts.authority.key != &expected {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::CallerAuthority,
            "transport authority is not the PDA these seeds address",
        ));
    }
    Ok(())
}

#[inline(never)]
fn decode_permit_account(
    accounts: FoundingAccounts<'_, '_>,
) -> Result<SeriesFoundingPermitV1, ProgramError> {
    if accounts.permit.owner != accounts.core_program.key
        || accounts.permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
    {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "permit account is not Core-owned at the exact permit width",
        ));
    }
    let permit_data = accounts
        .permit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    SeriesFoundingPermitV1::decode(&permit_data).map_err(|_| {
        refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "permit account body did not hostile-decode",
        )
    })
}

#[inline(never)]
fn authenticate_permit_and_projection(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_digest: [u8; 32],
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_receipt_digest: [u8; 32],
) -> Result<[u8; 32], ProgramError> {
    if accounts.permit.owner != accounts.core_program.key
        || accounts.permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
    {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "permit account is not Core-owned at the exact permit width",
        ));
    }
    let permit_data = accounts
        .permit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let permit = SeriesFoundingPermitV1::decode(&permit_data).map_err(|_| {
        refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "permit account body did not hostile-decode",
        )
    })?;
    drop(permit_data);
    let intent = authenticate_permit_body(permit, request)?;
    let permit_seeds = permit.seeds();
    let seed_slices = permit_seeds.as_slices();
    let (expected_permit, expected_bump) =
        Pubkey::find_program_address(&seed_slices, accounts.core_program.key);
    if expected_permit != *accounts.permit.key {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "permit is not the PDA its own seeds address under the core program",
        ));
    }
    if expected_bump != intent.bump() {
        return Err(refuse(
            ClaimsFoundingSbfErrorV5::Permit,
            "the intent states a different bump for the permit than the canonical search found",
        ));
    }
    let projected_context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        intent.ticket_context().to_bytes().as_slice(),
    ])
    .to_bytes();
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core_digest = hash(&core_data).to_bytes();
    drop(core_data);
    authenticate_lock_receipt(
        intent,
        request,
        lock_receipt,
        projected_context,
        lock_receipt_digest,
        projected_receipt,
    )?;
    authenticate_projected_receipt(
        intent,
        request,
        projected_receipt,
        projected_context,
        projected_receipt_digest,
        core_digest,
    )?;
    authenticate_permit_authorization(permit, intent, request, request_digest)?;
    // The Market's Custody namespace, and the only authenticated statement of
    // it this instruction has. It comes from the Core-owned permit's intent,
    // is cross-checked against the Lock receipt, the realization receipt, and
    // (in `authenticate_custody_poststate`) the live replay account's own
    // `context`. `authenticate_product_core` persists it; nothing downstream
    // may assume it.
    Ok(projected_context)
}

/// Join the permit's authorization against this intent and these request bytes.
///
/// SPLIT FROM `authenticate_permit_body`, AND RUN AFTER THE RECEIPTS, because
/// these two comparisons bind whole bodies in single digests and therefore
/// subsume every named conjunct anywhere in this route. While they ran first,
/// the coordinate joins in `authenticate_permit_body` and the four
/// intent-versus-receipt joins in `authenticate_projected_receipt` -- which are
/// this route's ONLY reachable statement about the intent coordinates the
/// request does not carry -- could never fire on the input they were written
/// for. Measured 2026-09-03 on the tier-1 campaign: the founding refused
/// `PermitBody`, "intent digest is not the request's founding_intent_digest",
/// which says the intents differ and says nothing about which coordinate.
///
/// Nothing is ADMITTED that was not admitted before: every conjunct still runs
/// and still runs before `allocate_all`, which is the first statement in this
/// instruction that writes anything. What moved is that the general join is
/// asked last, so a reader is told the specific thing first.
#[inline(never)]
fn authenticate_permit_authorization(
    permit: SeriesFoundingPermitV1,
    intent: FoundingIntentV5,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let body = ClaimsFoundingSbfErrorV5::PermitBody;
    let intent_bytes = intent
        .encode()
        .map_err(|_| refuse(body, "the permit's own intent did not re-encode"))?;
    let intent_digest = hash(&intent_bytes).to_bytes();
    require(
        intent_digest == request.founding_intent_digest(),
        body,
        "intent digest is not the request's founding_intent_digest",
    )?;
    permit
        .join_for_intent_and_request(
            intent,
            Identity::new(intent_digest)
                .map_err(|_| refuse(body, "the intent's own digest is the zero identity"))?,
            Identity::new(request_digest)
                .map_err(|_| refuse(body, "the founding request digest is the zero identity"))?,
        )
        .map_err(|mismatch| {
            refuse(
                body,
                match mismatch {
                    SeriesPermitJoinMismatchV1::Intent => {
                        "the permit was issued for a different founding intent"
                    }
                    SeriesPermitJoinMismatchV1::IntentDigest => {
                        "the permit records a different intent digest than its own intent hashes to"
                    }
                    SeriesPermitJoinMismatchV1::RequestDigest => {
                        "the permit records a different request digest than these request bytes hash to; the request Core compiled is not the request this founding was handed"
                    }
                },
            )
        })
}

#[inline(never)]
fn authenticate_lock_receipt(
    intent: FoundingIntentV5,
    request: &ClaimsFoundingRequestV5,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    projected_context: [u8; 32],
    lock_receipt_digest: [u8; 32],
    projected_receipt: &ProjectedCustodyReceiptV1,
) -> Result<(), ProgramError> {
    let body = ClaimsFoundingSbfErrorV5::Custody;
    require(
        lock_receipt.market == request.market(),
        body,
        "lock receipt market",
    )?;
    require(
        lock_receipt.release_set == request.release_set(),
        body,
        "lock receipt release set",
    )?;
    require(
        lock_receipt.context_digest == projected_context,
        body,
        "lock receipt custody context",
    )?;
    require(
        lock_receipt.source_vault == request.funding_source(),
        body,
        "lock receipt source vault",
    )?;
    require(
        lock_receipt.hoard_vault == request.hoard(),
        body,
        "lock receipt hoard vault",
    )?;
    require(
        lock_receipt.rent_credit == request.rent_credit(),
        body,
        "lock receipt rent credit",
    )?;
    require(
        lock_receipt.request_digest == request.custody_request_digest(),
        body,
        "lock receipt names another Lock request than the one the founding request carries",
    )?;
    require(
        lock_receipt_digest == request.custody_receipt_digest(),
        body,
        "the Lock receipt handed here is not the one the founding request commits to",
    )?;
    require(
        lock_receipt.amount == request.collateral_transferred(),
        body,
        "lock receipt collateral amount",
    )?;
    require(
        lock_receipt.resulting_revision.checked_add(1)
            == Some(intent.projected_resulting_revision()),
        body,
        "the Lock revision does not step to the intent's projected resulting revision",
    )?;
    require(
        projected_receipt.resulting_revision == intent.projected_resulting_revision(),
        body,
        "the realization receipt's revision is not the intent's projected resulting revision",
    )?;
    Ok(())
}

#[inline(never)]
fn authenticate_permit_body(
    permit: SeriesFoundingPermitV1,
    request: &ClaimsFoundingRequestV5,
) -> Result<FoundingIntentV5, ProgramError> {
    let intent = permit.intent();
    // Eighteen named conjuncts, not one `||` chain, AND THEY RUN FIRST. Every
    // founding digest in the tree converges here: a byte moved anywhere
    // upstream -- a Core bump written where the projection had no field for
    // it, a revision off by one, a stale release id -- arrives as two unequal
    // 32-byte strings and nothing else. Naming the one that disagreed is the
    // difference between reading a log line and bisecting five programs.
    //
    // ORDER IS THE WHOLE DIAGNOSTIC. The permit's own authorization join binds
    // the complete request body in one digest, so it subsumes every conjunct
    // below it: while it ran first, all eighteen names were unreachable on
    // exactly the input that needed them, and the founding could only say that
    // the pair did not authorize. Measured 2026-09-03 on the tier-1 campaign's
    // last transaction. So the ladder now descends from the specific to the
    // general -- coordinate, then intent, then the permit pair -- and nothing
    // about what is ADMITTED changes: the same conjunctions, all of them, in
    // an order the accepting path pays nothing for.
    let body = ClaimsFoundingSbfErrorV5::PermitBody;
    require(
        intent.release_set().to_bytes() == request.release_set(),
        body,
        "intent release set",
    )?;
    require(
        intent.market().to_bytes() == request.market(),
        body,
        "intent market",
    )?;
    require(
        intent.product_record().to_bytes() == request.product_record_digest(),
        body,
        "intent product record digest",
    )?;
    require(
        intent.founder().to_bytes() == request.founder(),
        body,
        "intent founder",
    )?;
    require(
        intent.projected_replay().to_bytes() == request.custody_replay(),
        body,
        "intent projected custody replay",
    )?;
    require(
        intent.funding_source().to_bytes() == request.funding_source(),
        body,
        "intent funding source",
    )?;
    require(
        intent.hoard().to_bytes() == request.hoard(),
        body,
        "intent hoard",
    )?;
    // Deliberately an INEQUALITY: the request carries the REALIZED custody
    // digests and the intent the PROJECTED ones, so equality here would mean
    // the realization never happened.
    require(
        intent.projected_request_digest().to_bytes() != request.custody_request_digest(),
        body,
        "realized custody request digest equals the projected one",
    )?;
    require(
        intent.projected_receipt_digest().to_bytes() != request.custody_receipt_digest(),
        body,
        "realized custody receipt digest equals the projected one",
    )?;
    require(
        intent.trading_program().to_bytes() == request.trading_program(),
        body,
        "intent trading program",
    )?;
    require(
        intent.claims_program().to_bytes() == request.claims_program(),
        body,
        "intent claims program",
    )?;
    require(
        intent.rent_credit().to_bytes() == request.rent_credit(),
        body,
        "intent rent credit",
    )?;
    require(
        intent.generation() == request.generation(),
        body,
        "intent generation",
    )?;
    require(
        intent.quantity() == request.quantity(),
        body,
        "intent quantity",
    )?;
    require(
        intent.basis_scale() == request.basis_scale(),
        body,
        "intent basis scale",
    )?;
    require(
        intent.normal_replay_revision() == request.post_custody_revision(),
        body,
        "intent normal replay revision is not the request's post-custody revision",
    )?;
    require(
        request.pre_custody_revision().checked_add(1) == Some(intent.normal_replay_revision()),
        body,
        "pre-custody revision does not step to the intent's normal replay revision",
    )?;
    Ok(intent)
}

#[inline(never)]
fn authenticate_projected_receipt(
    intent: FoundingIntentV5,
    request: &ClaimsFoundingRequestV5,
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_context: [u8; 32],
    projected_receipt_digest: [u8; 32],
    core_digest: [u8; 32],
) -> Result<(), ProgramError> {
    // Thirteen conjuncts, and four of them are the only reachable statement
    // this route has about intent coordinates the founding REQUEST does not
    // carry: the parent capability root, the projected Realize request digest,
    // the projected resulting revision, and the projected Realize receipt
    // digest. A `||` chain over them publishes one code for all thirteen, so a
    // Core-compiled intent that disagrees with the host's by one of those four
    // arrives as `Custody` and nothing else -- which is the same defect the
    // permit body had one level up.
    let body = ClaimsFoundingSbfErrorV5::Custody;
    require(
        projected_receipt.realized,
        body,
        "the realization receipt is not realized",
    )?;
    require(
        !projected_receipt.aborted_open,
        body,
        "the realization receipt aborted its open",
    )?;
    require(
        projected_receipt.market == request.market(),
        body,
        "realization receipt market",
    )?;
    require(
        projected_receipt.release_set == request.release_set(),
        body,
        "realization receipt release set",
    )?;
    require(
        projected_receipt.parent_capability_root == intent.parent_root().to_bytes(),
        body,
        "the intent's parent capability root is not the realization receipt's",
    )?;
    require(
        projected_receipt.context_digest == projected_context,
        body,
        "realization receipt custody context",
    )?;
    require(
        projected_receipt.hoard_vault == request.hoard(),
        body,
        "realization receipt hoard vault",
    )?;
    require(
        projected_receipt.amount == request.collateral_transferred(),
        body,
        "realization receipt collateral amount",
    )?;
    require(
        projected_receipt.request_digest == intent.projected_request_digest().to_bytes(),
        body,
        "the intent's projected Realize request digest is not the receipt's",
    )?;
    require(
        projected_receipt.market_state_digest == core_digest,
        body,
        "the realization receipt names a Market state digest that is not this Market account's",
    )?;
    require(
        projected_receipt.rent_credit == request.rent_credit(),
        body,
        "realization receipt rent credit",
    )?;
    require(
        projected_receipt.resulting_revision == intent.projected_resulting_revision(),
        body,
        "the intent's projected resulting revision is not the receipt's",
    )?;
    require(
        projected_receipt_digest == intent.projected_receipt_digest().to_bytes(),
        body,
        "the intent's projected Realize receipt digest is not this receipt's",
    )?;
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    // One canonical cache search admits the first role and returns the bump
    // witness; the remaining roles reproduce the address from it instead of
    // each paying the 256-way search again. Same checks, one search - the
    // composed founding runs this batch against a 1.4M ceiling it exhausts.
    let mut bump = None;
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        ),
        (
            ExecutionRoleV1::Trading,
            accounts.trading_program,
            accounts.trading_programdata,
        ),
        (
            ExecutionRoleV1::Custody,
            accounts.custody_program,
            accounts.custody_programdata,
        ),
    ] {
        let receipt = match bump {
            None => {
                let (receipt, witness) =
                    dclutch_registry_activation_auth_v1::authenticate_activated_role_and_bump_v1(
                        accounts.registry,
                        accounts.cache,
                        &request.release_set(),
                        role,
                        program,
                        programdata,
                    )
                    .map_err(|error| refuse_activated_role(role, error))?;
                bump = Some(witness);
                receipt
            }
            Some(witness) => {
                dclutch_registry_activation_auth_v1::authenticate_activated_role_with_bump_v1(
                    accounts.registry,
                    accounts.cache,
                    &request.release_set(),
                    witness,
                    role,
                    program,
                    programdata,
                )
                .map_err(|error| refuse_activated_role(role, error))?
            }
        };
        if receipt.execution_release_set_id().as_bytes() != &request.release_set() {
            return Err(refuse(
                ClaimsFoundingSbfErrorV5::Release,
                role_log_line(role),
            ));
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_custody_poststate(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if accounts.funding_source.key.to_bytes() != request.funding_source()
        || accounts.hoard.key.to_bytes() != request.hoard()
        || accounts.custody_replay.key.to_bytes() != request.custody_replay()
        || accounts.funding_source.owner != &system_program::ID
        || accounts.funding_source.lamports() != 0
        || !accounts.funding_source.data_is_empty()
        || accounts.funding_source.executable
        || TokenProgram::parse(accounts.hoard.owner.to_bytes()).is_err()
        || accounts.custody_replay.owner != accounts.custody_program.key
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    let hoard_data = accounts
        .hoard
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    if request.post_source_amount() != 0
        || request.pre_source_amount() != request.collateral_transferred()
        || hoard.amount != request.post_hoard_amount()
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    let replay_data = accounts
        .custody_replay
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let replay =
        CustodyReplayV1::decode(&replay_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set()
        || replay.market != request.market()
        || replay.realm != core.identity.realm_id.to_bytes()
        || replay.context != projected_receipt.context_digest
        || replay.generation != request.generation()
        || replay.caller_program != request.trading_program()
        || replay.rent_refund != request.rent_credit()
        || replay.open_vault_count != 1
        || replay.next_revision != request.post_custody_revision()
        || replay.last_request_digest != projected_receipt.request_digest
        || replay.last_poststate_commitment != projected_receipt_digest
        || replay.last_request_digest != projected_receipt.request_digest
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_product_core(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    custody_context: [u8; 32],
    rent: &Rent,
) -> Result<MarketViewV2, ProgramError> {
    if accounts.core_market.key.to_bytes() != request.market()
        || accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsFoundingSbfErrorV5::ProductBasis.into());
    }
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::ProductBasis)?;
    drop(core_data);
    // The manipulation-capacity bound, at the one moment principal first exists.
    // `outstanding` is 0 because a founding is the Market's first principal; the
    // same accessor re-runs at every later complete-set split, which is what makes
    // this a cap rather than a founding-time formality.
    MarketPrincipalCapSetsV1::read(core.principal_cap_sets)
        .admit_growth(0, request.quantity())
        .map_err(|_| ClaimsFoundingSbfErrorV5::PrincipalCapacity)?;
    let market = MarketViewV2 {
        claim_count: request.claim_count(),
        revision: request.post_aggregate_revision(),
        logical_market: request.market(),
        release_set: request.release_set(),
        registry_program: accounts.registry.key.to_bytes(),
        product_instance_id: request.product_instance_id(),
        basis_id: request.semantic_basis_id(),
        realm_id: core.identity.realm_id.to_bytes(),
        // The authenticated Custody namespace, not the Market address. The
        // founding creates the Hoard Vault and realizes the Market's normal
        // replay under
        // `SHA-256(PROJECTED_HOARD_CONTEXT_DOMAIN_V1 || permit.ticket_context)`,
        // and `GenericFoundingRequestV1::context` is caller-owned, so no
        // address reconciles with it. Writing `request.market()` here made the
        // aggregate lie about the one coordinate every payout route needs, in
        // the same instruction that had already authenticated the truth.
        custody_context,
        generation: request.generation(),
    };
    authenticate_runtime_product_basis_core_with_rent_v3(
        accounts.registry,
        rent,
        accounts.core_market,
        accounts.core_program,
        ProductRuntimeFrameV3 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_record,
                staging: accounts.result_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
            linked_basis: FinalizedRecordFrameV2 {
                raw: accounts.basis_record,
                staging: accounts.basis_staging,
            },
        },
        market,
        request.product_record_digest(),
        request.linked_basis_record_digest(),
        CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ProductBasis)?;
    let aggregate_seeds = ClaimsFoundingAggregateSeedsV5::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if Pubkey::find_program_address(&aggregate_seeds.as_slices(), program_id).0
        != *accounts.aggregate.key
        || accounts.aggregate.key.to_bytes() != request.aggregate()
    {
        return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
    }
    Ok(market)
}

#[inline(never)]
fn authenticate_rent_and_vacancy(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    market: MarketViewV2,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let aggregate_width = vector_width(
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let position_width = vector_width(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if rent.minimum_balance(aggregate_width) != request.aggregate_rent_principal()
        || rent.minimum_balance(position_width) != request.position_rent_principal()
        || rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            != request.admission_rent_principal()
        || accounts.aggregate.lamports() != request.observed_aggregate_lamports()
        || accounts.position.lamports() != request.observed_position_lamports()
        || accounts.admission.lamports() != request.observed_admission_lamports()
    {
        return Err(ClaimsFoundingSbfErrorV5::Rent.into());
    }
    for vacant in [accounts.aggregate, accounts.position, accounts.admission] {
        if vacant.owner != &system_program::ID
            || !vacant.data_is_empty()
            || vacant.is_signer
            || !vacant.is_writable
            || vacant.executable
        {
            return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
        }
    }
    let position_seeds =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0
        != *accounts.position.key
        || Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0
            != *accounts.admission.key
        || accounts.position.key.to_bytes() != request.position()
        || accounts.admission.key.to_bytes() != request.admission()
        || accounts.founder.key.to_bytes() != request.founder()
        || accounts.founder.executable
        || accounts.rent_credit.key.to_bytes() != request.rent_credit()
        || accounts.rent_program.key.to_bytes() != request.rent_program()
        || accounts.rent_credit.owner != accounts.rent_program.key
    {
        return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
    }
    authenticate_rent_credit(accounts, market)?;
    Ok(())
}

#[inline(never)]
fn authenticate_rent_credit(
    accounts: FoundingAccounts<'_, '_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    let credit_data = accounts
        .rent_credit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    let seeds = credit.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation_seed = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market_seed.as_slice(),
            generation_seed.as_slice(),
            &bump,
        ],
        accounts.rent_program.key,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    if expected != *accounts.rent_credit.key
        || accounts.rent_credit.key.to_bytes() != core.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != market.logical_market
        || credit.release_set().to_bytes() != market.release_set
        || credit.generation() != core.identity.generation
        || core.identity.market_id.to_bytes() != market.logical_market
    {
        return Err(ClaimsFoundingSbfErrorV5::Rent.into());
    }
    Ok(())
}

#[inline(never)]
fn build_candidates_boxed(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    market: MarketViewV2,
    request_digest: [u8; 32],
) -> Result<Box<FoundingCandidates>, ProgramError> {
    let (mut aggregate, mut position) = build_liability_candidates(accounts, request, market)?;
    let admission = build_admission_candidate(program_id, accounts, request)?;
    if request_digest == [0; 32] {
        return Err(ClaimsFoundingSbfErrorV5::Receipt.into());
    }
    // Each account records the bump its own creator derived, so every later
    // reader reproduces the address instead of searching for it. This is the
    // ONLY derivation of either bump on this route: `allocate_all` signs with
    // what is recorded here.
    let aggregate_bump = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(request.market())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?
            .as_slices(),
        program_id,
    )
    .1;
    let position_bump = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?
            .as_slices(),
        program_id,
    )
    .1;
    put_liability_basis_market_bump_v2(&mut aggregate, aggregate_bump)
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    put_liability_basis_position_bump_v2(&mut position, position_bump)
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    Ok(Box::new(FoundingCandidates {
        aggregate,
        position,
        admission,
        aggregate_bump,
        position_bump,
    }))
}

#[inline(never)]
fn build_liability_candidates(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    market: MarketViewV2,
) -> Result<(Vec<u8>, Vec<u8>), ProgramError> {
    let count = usize::try_from(request.claim_count())
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let quantities = vec![request.quantity(); count];
    let aggregate = encode_liability_basis_market_v2(
        LiabilityBasisMarketInputV2 {
            revision: request.post_aggregate_revision(),
            logical_market: market.logical_market,
            release_set: market.release_set,
            registry_program: market.registry_program,
            product_instance_id: market.product_instance_id,
            basis_id: market.basis_id,
            realm_id: market.realm_id,
            custody_context: market.custody_context,
            generation: market.generation,
        },
        &quantities,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let position = encode_liability_basis_position_v2(
        LiabilityBasisPositionInputV2 {
            revision: request.post_position_revision(),
            market_account: accounts.aggregate.key.to_bytes(),
            owner: request.founder(),
            basis_id: market.basis_id,
        },
        &quantities,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    Ok((aggregate, position))
}

#[inline(never)]
fn build_admission_candidate(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<[u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2], ProgramError> {
    let admission_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: request.release_set(),
        market: request.market(),
        position_owner: request.founder(),
        parent_request_digest: request.founding_intent_digest(),
        rent_credit: request.rent_credit(),
        rent_program: request.rent_program(),
        generation: request.generation(),
        expected_market_revision: request.post_aggregate_revision(),
        expected_position_revision: request.pre_position_revision(),
        observed_position_lamports: request.observed_position_lamports(),
        observed_admission_lamports: request.observed_admission_lamports(),
        position_rent_principal: request.position_rent_principal(),
        admission_rent_principal: request.admission_rent_principal(),
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission_request_bytes = admission_request
        .to_bytes()
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission = ProtocolPositionAdmissionV2::new(
        admission_request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: request.product_record_digest(),
            semantic_basis_id: request.semantic_basis_id(),
            linked_basis_record_digest: request.linked_basis_record_digest(),
            request_digest: hash(&admission_request_bytes).to_bytes(),
            claims_program: program_id.to_bytes(),
            trading_program: accounts.trading_program.key.to_bytes(),
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: request.claim_count(),
        },
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?
    .to_state_bytes()
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    Ok(admission)
}

#[inline(never)]
fn allocate_all(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    candidates: &FoundingCandidates,
) -> Result<(), ProgramError> {
    let aggregate = ClaimsFoundingAggregateSeedsV5::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.aggregate,
        accounts.system,
        candidates.aggregate.len(),
        &aggregate.as_slices(),
        Some(candidates.aggregate_bump),
    )?;
    let position =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.position,
        accounts.system,
        candidates.position.len(),
        &position.as_slices(),
        Some(candidates.position_bump),
    )?;
    let admission =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.admission,
        accounts.system,
        candidates.admission.len(),
        &admission.as_slices(),
        // The admission record has no reserved byte to carry a bump, so it
        // still searches. It is not on the hot route.
        None,
    )
}

#[inline(never)]
fn allocate_one<'info>(
    program_id: &Pubkey,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    seeds: &[&[u8]],
    // The bump already derived for this address, where the body records one.
    // Signing with it rather than searching again is the same act: the runtime
    // will only produce a signature for the address these seeds and this bump
    // name, and the allocation is checked against the account it lands on.
    derived: Option<u8>,
) -> Result<(), ProgramError> {
    let bump = [match derived {
        Some(bump) => bump,
        None => Pubkey::find_program_address(seeds, program_id).1,
    }];
    let mut signer = Vec::with_capacity(seeds.len() + 1);
    signer.extend_from_slice(seeds);
    signer.push(&bump);
    let space = u64::try_from(width).map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    for instruction in [
        allocate(destination.key, space),
        assign(destination.key, program_id),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[destination.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    }
    if destination.owner != program_id || destination.data_len() != width {
        return Err(ClaimsFoundingSbfErrorV5::Allocation.into());
    }
    Ok(())
}

#[inline(never)]
fn commit_candidates(
    accounts: FoundingAccounts<'_, '_>,
    candidates: &FoundingCandidates,
) -> Result<(), ProgramError> {
    let mut aggregate = accounts
        .aggregate
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    let mut position = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    let mut admission = accounts
        .admission
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    if aggregate.len() != candidates.aggregate.len()
        || position.len() != candidates.position.len()
        || admission.len() != candidates.admission.len()
        || aggregate.iter().any(|byte| *byte != 0)
        || position.iter().any(|byte| *byte != 0)
        || admission.iter().any(|byte| *byte != 0)
    {
        return Err(ClaimsFoundingSbfErrorV5::Commit.into());
    }
    aggregate.copy_from_slice(&candidates.aggregate);
    position.copy_from_slice(&candidates.position);
    admission.copy_from_slice(&candidates.admission);
    Ok(())
}

fn require_distinct(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, candidate) in accounts.iter().enumerate() {
        if accounts
            .get(..index)
            .ok_or(ClaimsFoundingSbfErrorV5::Accounts)?
            .iter()
            .any(|prior| prior.key == candidate.key)
        {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsFoundingSbfErrorV5::Accounts.into())
}

#[cfg(test)]
mod tests {
    use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestInputV5;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn identity(value: [u8; 32]) -> Identity {
        Identity::new(value).expect("nonzero identity")
    }

    fn fixture() -> (
        ClaimsFoundingRequestV5,
        SeriesFoundingPermitV1,
        ProjectedCustodyLockReceiptV1,
        ProjectedCustodyReceiptV1,
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ) {
        let lock_request_digest = id(19);
        let projected_request_digest = id(20);
        let core_digest = id(24);
        let ticket_context = id(22);
        let projected_context =
            hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ticket_context.as_slice()]).to_bytes();
        let lock_receipt = ProjectedCustodyLockReceiptV1 {
            market: id(2),
            release_set: id(1),
            context_digest: projected_context,
            source_vault: id(12),
            source_replay: id(25),
            hoard_vault: id(13),
            rent_credit: id(15),
            request_digest: lock_request_digest,
            amount: 77,
            source_vault_rent_lamports: 30,
            source_replay_rent_lamports: 31,
            resulting_revision: 4,
        };
        let lock_receipt_digest =
            hash(&lock_receipt.encode().expect("canonical lock receipt")).to_bytes();
        let projected_receipt = ProjectedCustodyReceiptV1 {
            realized: true,
            aborted_open: false,
            market: id(2),
            release_set: id(1),
            parent_capability_root: id(23),
            context_digest: projected_context,
            hoard_vault: id(13),
            amount: 77,
            request_digest: projected_request_digest,
            market_state_digest: core_digest,
            rent_credit: id(15),
            resulting_revision: 5,
        };
        let projected_receipt_digest = hash(
            &projected_receipt
                .encode()
                .expect("canonical projected receipt"),
        )
        .to_bytes();
        let intent = FoundingIntentV5::new(
            255,
            identity(id(1)),
            identity(id(2)),
            identity(id(3)),
            identity(id(21)),
            identity(id(7)),
            identity(ticket_context),
            identity(id(23)),
            identity(id(14)),
            identity(id(12)),
            identity(id(13)),
            identity(projected_request_digest),
            identity(projected_receipt_digest),
            identity(id(18)),
            identity(id(17)),
            identity(id(15)),
            21,
            7,
            11,
            500,
            5,
            1,
        )
        .expect("canonical intent");
        let intent_digest = hash(&intent.encode().expect("intent bytes")).to_bytes();
        let request = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(3),
            product_instance_id: id(4),
            linked_basis_record_digest: id(5),
            semantic_basis_id: id(6),
            founder: id(7),
            founding_intent_digest: intent_digest,
            aggregate: id(9),
            position: id(10),
            admission: id(11),
            funding_source: id(12),
            hoard: id(13),
            custody_replay: id(14),
            rent_credit: id(15),
            rent_program: id(16),
            claims_program: id(17),
            trading_program: id(18),
            custody_request_digest: lock_request_digest,
            custody_receipt_digest: lock_receipt_digest,
            generation: 21,
            claim_count: 5,
            quantity: 7,
            basis_scale: 11,
            pre_source_amount: 77,
            post_source_amount: 0,
            pre_hoard_amount: 23,
            post_hoard_amount: 100,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("canonical request");
        let request_digest = hash(&request.to_bytes()).to_bytes();
        let permit =
            SeriesFoundingPermitV1::new(intent, identity(intent_digest), identity(request_digest))
                .expect("canonical permit");
        (
            request,
            permit,
            lock_receipt,
            projected_receipt,
            lock_receipt_digest,
            projected_receipt_digest,
            core_digest,
        )
    }

    #[test]
    fn exact_instruction_and_permit_projection_join() {
        let (request, permit, lock, projected, lock_digest, projected_digest, core_digest) =
            fixture();
        let request_bytes = request.to_bytes();
        let request_digest = hash(&request_bytes).to_bytes();
        let intent = authenticate_permit_body(permit, &request).expect("permit binds request");
        authenticate_permit_authorization(permit, intent, &request, request_digest)
            .expect("permit authorizes this intent and request");
        let context = hashv(&[
            PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
            intent.ticket_context().to_bytes().as_slice(),
        ])
        .to_bytes();
        authenticate_projected_receipt(
            intent,
            &request,
            &projected,
            context,
            projected_digest,
            core_digest,
        )
        .expect("projected receipt binds intent");
        authenticate_lock_receipt(intent, &request, &lock, context, lock_digest, &projected)
            .expect("lock receipt binds intent");
        let mut instruction = Vec::from(request_bytes);
        instruction.extend_from_slice(&lock.encode().expect("lock bytes"));
        instruction.extend_from_slice(&projected.encode().expect("projected bytes"));
        assert!(decode_instruction(&instruction).is_ok());
        let transport = SeriesClaimsFoundingTransportV1::from_canonical_v5(id(90), request)
            .expect("Series transport")
            .to_bytes();
        let mut transport_instruction = Vec::from(transport);
        transport_instruction.extend_from_slice(&lock.encode().expect("lock bytes"));
        transport_instruction.extend_from_slice(&projected.encode().expect("projected bytes"));
        let decoded = decode_series_transport_instruction(&transport_instruction)
            .expect("transient instruction");
        let reconstructed = decoded
            .transport
            .reconstruct_v5(
                permit.claims_intent_digest().to_bytes(),
                decoded.lock_receipt.request_digest,
                decoded.lock_receipt_digest,
            )
            .expect("canonical reconstruction");
        assert_eq!(reconstructed, request);
        let reconstructed_intent = authenticate_permit_body(permit, &reconstructed)
            .expect("permit binds reconstructed request");
        authenticate_permit_authorization(
            permit,
            reconstructed_intent,
            &reconstructed,
            hash(&reconstructed.to_bytes()).to_bytes(),
        )
        .expect("permit authorizes the reconstructed request");
        let short = instruction
            .get(..instruction.len().saturating_sub(1))
            .expect("short instruction slice");
        assert!(decode_instruction(short).is_err());
    }

    #[test]
    fn substituted_request_permit_and_projected_receipt_refuse() {
        let (request, permit, lock, projected, lock_digest, projected_digest, core_digest) =
            fixture();
        let request_digest = hash(&request.to_bytes()).to_bytes();
        // A coordinate the intent carries: one of the named joins owns it, and
        // the permit's own digests still agree because this hostile is handed
        // the honest request's digest. Named rather than `is_err()`, which
        // would also have accepted a refusal from anywhere else in the ladder.
        let mut hostile_input = request.input();
        hostile_input.trading_program = id(99);
        let hostile_request =
            ClaimsFoundingRequestV5::new(hostile_input).expect("same-shape hostile request");
        assert_eq!(
            authenticate_permit_body(permit, &hostile_request).unwrap_err(),
            ProgramError::Custom(ClaimsFoundingSbfErrorV5::PermitBody as u32),
        );

        // And a coordinate NO named join covers: `claim_count` is in the
        // request and not in the intent, so the only thing standing between it
        // and a founding is the permit's whole-body request digest. This is
        // what the named joins are in front of, and it is the case that would
        // go quiet if a later author ever reordered them past it.
        let mut uncovered_input = request.input();
        uncovered_input.claim_count = request.claim_count() + 1;
        let uncovered_request =
            ClaimsFoundingRequestV5::new(uncovered_input).expect("same-shape hostile request");
        let uncovered_digest = hash(&uncovered_request.to_bytes()).to_bytes();
        let intent = authenticate_permit_body(permit, &uncovered_request)
            .expect("no named coordinate join covers claim_count");
        assert_eq!(
            authenticate_permit_authorization(permit, intent, &uncovered_request, uncovered_digest)
                .unwrap_err(),
            ProgramError::Custom(ClaimsFoundingSbfErrorV5::PermitBody as u32),
        );

        let intent = authenticate_permit_body(permit, &request).expect("permit binds request");
        authenticate_permit_authorization(permit, intent, &request, request_digest)
            .expect("permit authorizes this intent and request");
        let context = hashv(&[
            PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
            intent.ticket_context().to_bytes().as_slice(),
        ])
        .to_bytes();
        // Both receipt hostiles reach their subject and now name the code they
        // find there. `parent_capability_root` is one of the four coordinates
        // the founding request cannot carry, so this join is the only thing in
        // the route that can refuse it at all.
        let mut hostile_projected = projected;
        hostile_projected.parent_capability_root = id(98);
        assert_eq!(
            authenticate_projected_receipt(
                intent,
                &request,
                &hostile_projected,
                context,
                projected_digest,
                core_digest,
            )
            .unwrap_err(),
            ProgramError::Custom(ClaimsFoundingSbfErrorV5::Custody as u32),
        );
        let mut hostile_lock = lock;
        hostile_lock.source_vault = id(97);
        assert_eq!(
            authenticate_lock_receipt(
                intent,
                &request,
                &hostile_lock,
                context,
                lock_digest,
                &projected,
            )
            .unwrap_err(),
            ProgramError::Custom(ClaimsFoundingSbfErrorV5::Custody as u32),
        );
        let mut obsolete = request.to_bytes();
        obsolete[..8]
            .copy_from_slice(&dclutch_claims_svm::founding_v4::CLAIMS_FOUNDING_REQUEST_MAGIC_V4);
        let mut instruction = Vec::from(obsolete);
        instruction.extend_from_slice(&lock.encode().expect("lock bytes"));
        instruction.extend_from_slice(&projected.encode().expect("projected bytes"));
        assert!(decode_instruction(&instruction).is_err());
    }
}
