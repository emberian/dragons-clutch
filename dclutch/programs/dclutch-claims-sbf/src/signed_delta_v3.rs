//! Authenticated family-neutral signed-delta batches over LiabilityBasisV2.
//!
//! The adapter consumes one canonical already-netted delta per touched
//! `(Position, outcome)` coordinate. It authenticates every immutable Product,
//! basis, release, Core, aggregate, and Position join, builds every candidate,
//! borrows every writable account, and commits the complete batch once.

extern crate alloc;

use alloc::vec::Vec;
use core::{
    cell::RefMut,
    convert::{TryFrom, TryInto},
};

use crate::claims_cu_checkpoint;
use dclutch_claims::{
    CallerRole,
    frame_spec_v1::{
        ClaimsFrameRoleV1,
        SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as SEMANTIC_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        SignedDeltaFrameSpecV3,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
    signed_delta_v3::{
        DeltaDirectionV3, SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3,
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3, SignedDeltaPlanV3, SignedDeltaReceiptV3,
        SignedDeltaV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_registry::activation_auth_v1::authenticate_activation_cache_identity_v1;
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_source::MarketPrincipalCapSetsV1;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use super::affine_batch_v2::authenticate_core_market_v3;
use crate::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, MarketViewV2, PositionViewV2,
};
use crate::market_admission_v1::CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2, LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2,
};
use dclutch_market::MarketAdmissionV1;

/// Exact fixed account count before the runtime Position tail.
pub const SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3: usize =
    SEMANTIC_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize;

/// What stands at the frame's `CallerAuthority` coordinate, and what it proves.
///
/// The coordinate's meaning has always been "the party entitled to move these
/// claims proved it, and the proof is a signature". There are exactly two kinds
/// of entitled party in this protocol, and until now only one of them had a
/// spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParentAuthorityV3 {
    /// A release-pinned `CallerAuthoritySeedsV1` PDA under the parent's caller
    /// program, which only the activated program of the parent's execution role
    /// can sign. The entitled party is a venue or lifecycle program.
    CallerProgramPda,
    /// The Position owner's own signature.
    ///
    /// A program-derived address has no private key, so producing this proof is
    /// itself the evidence that the Position is held by an ordinary identity and
    /// not by a Trading record or a Claims capability. Admissible only under
    /// [`CallerRole::Claims`], where there is no caller program to derive a PDA
    /// under in the first place.
    PositionOwner([u8; 32]),
    /// An enclosing Claims route already authenticated the exact external
    /// caller PDA against its own family request before deriving this child.
    ///
    /// This is not a public submission mode: the enum and execution entry are
    /// crate-private, and current-release authentication still runs before the
    /// derived SignedDelta commits.
    EnclosingClaimsRoute,
    /// A permissionless claim-check compaction crank, past its deadline.
    ///
    /// Coordinate 0 is the caller and nothing more. That is the point: a right
    /// only its beneficiary can exercise stalls when its beneficiary is absent,
    /// and this is the mode that lets somebody else finish the work without
    /// being able to take anything by doing so.
    ///
    /// The entitlement is not carried by the signature. It is proved before
    /// this mode is ever selected, by the compaction route, which requires the
    /// escrow's release-fixed deadline to have elapsed and DERIVES the payout's
    /// recipient from the market's own aggregate rather than accepting one. A
    /// caller therefore chooses only *whether* the crank turns, never where the
    /// collateral lands.
    ///
    /// One thing the owner's signature was silently also proving is not proved
    /// here: that the Position is wallet-held rather than owned by a Trading
    /// record or a Claims capability PDA, neither of which can sign. The
    /// compaction route replaces that inference with the persisted owner-kind
    /// tag, read off the admission record and refused explicitly.
    ClaimCheckCrank,
}

/// Exact already-authenticated parent request joined to one generated
/// SignedDeltaV3 plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSignedDeltaParentV3 {
    pub(crate) caller_role: CallerRole,
    pub(crate) authority: ParentAuthorityV3,
    pub(crate) release_set: [u8; 32],
    pub(crate) market: [u8; 32],
    pub(crate) parent_context: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
}

const MARKET_REVISION_OFFSET: usize = 16;
const POSITION_REVISION_OFFSET: usize = 16;
const SCALAR_BYTES: usize = 8;

/// Stable signed-delta SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SignedDeltaSbfErrorV3 {
    /// Instruction bytes did not decode as the canonical public ABI.
    Instruction = 0x5200,
    /// Account count, order, privileges, owners, or aliases refused.
    Accounts = 0x5201,
    /// Registry current-release authentication or caller authority refused.
    Release = 0x5202,
    /// Product graph, linked basis, semantic identity, or Core join refused.
    ProductBasis = 0x5203,
    /// Aggregate or Position PDA, width, identity, or revision refused.
    ClaimsState = 0x5204,
    /// An exact signed delta overflowed or underflowed a resource.
    Candidate = 0x5205,
    /// Complete candidate buffers could not all be borrowed and committed last.
    Commit = 0x5206,
    /// The canonical success receipt could not be constructed.
    Receipt = 0x5207,
    /// A positive aggregate delta would grow total principal past the Market's
    /// carried manipulation-capacity cap, or that cap was never stated.
    ///
    /// Separate from [`Self::Candidate`], which is arithmetic that does not fit a
    /// resource. This one fits perfectly well and is refused anyway, because the
    /// Market may not carry that much principal against the venue behind its
    /// Source. Splitting is exactly where a founding-time-only bound would leak.
    PrincipalCapacity = 0x5208,
}

dclutch_refusal_registry::pin_refusal_band!(
    SignedDeltaSbfErrorV3,
    dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x200,
    [
        Instruction,
        Accounts,
        Release,
        ProductBasis,
        ClaimsState,
        Candidate,
        Commit,
        Receipt,
        PrincipalCapacity
    ]
);

#[derive(Clone, Copy)]
struct SignedDeltaAccountsV3<'accounts, 'info> {
    all: &'accounts [AccountInfo<'info>],
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    caller_program: &'accounts AccountInfo<'info>,
    caller_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    positions: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> SignedDeltaAccountsV3<'accounts, 'info> {
    /// Bind the coordinates this route reads.
    ///
    /// Six of the frame's fixed coordinates -- the Product, ResultDomain,
    /// Portfolio and basis STAGING cursors, and the ResultDomain and Portfolio
    /// raw records -- are deliberately not bound here. They were the input to
    /// the eight-derivation Product runtime walk this route no longer makes,
    /// and nothing reads them now. They remain in `SignedDeltaFrameSpecV3`,
    /// because the frame is a wire contract this program shares with its
    /// callers, and [`authenticate_privileges`] still takes every coordinate's
    /// privileges by index -- so an unread account is still a refused writable
    /// or signer, it is simply not named twice. Binding a coordinate costs a
    /// full scan of the spec, which is why six unread names were 8,000 CU.
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        position_count: u32,
    ) -> Result<Self, ProgramError> {
        let spec = SignedDeltaFrameSpecV3::new(position_count)
            .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
        let count = usize::from(
            spec.account_count()
                .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?,
        );
        if accounts.len() != count {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
        Ok(Self {
            all: accounts,
            authority: account_for_role(accounts, spec, ClaimsFrameRoleV1::CallerAuthority)?,
            market: account_for_role(accounts, spec, ClaimsFrameRoleV1::ClaimsMarket)?,
            basis_record: account_for_role(accounts, spec, ClaimsFrameRoleV1::BasisRecord)?,
            product_record: account_for_role(accounts, spec, ClaimsFrameRoleV1::ProductRecord)?,
            rent: account_for_role(accounts, spec, ClaimsFrameRoleV1::RentSysvar)?,
            core_market: account_for_role(accounts, spec, ClaimsFrameRoleV1::CoreMarket)?,
            cache: account_for_role(accounts, spec, ClaimsFrameRoleV1::ActivationCache)?,
            registry: account_for_role(accounts, spec, ClaimsFrameRoleV1::RegistryProgram)?,
            caller_program: account_for_role(accounts, spec, ClaimsFrameRoleV1::CallerProgram)?,
            caller_programdata: account_for_role(
                accounts,
                spec,
                ClaimsFrameRoleV1::CallerProgramData,
            )?,
            claims_program: account_for_role(accounts, spec, ClaimsFrameRoleV1::ClaimsProgram)?,
            claims_programdata: account_for_role(
                accounts,
                spec,
                ClaimsFrameRoleV1::ClaimsProgramData,
            )?,
            core_program: account_for_role(accounts, spec, ClaimsFrameRoleV1::CoreProgram)?,
            core_programdata: account_for_role(accounts, spec, ClaimsFrameRoleV1::CoreProgramData)?,
            positions: accounts
                .get(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3..)
                .ok_or(SignedDeltaSbfErrorV3::Accounts)?,
        })
    }
}

/// Execute one authenticated runtime-width signed-delta batch.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    claims_cu_checkpoint!("sd-enter");
    let plan = SignedDeltaPlanV3::decode(instruction_data)
        .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
    claims_cu_checkpoint!("sd-plan-decoded");
    let accounts = SignedDeltaAccountsV3::parse(account_infos, plan.position_count())?;
    claims_cu_checkpoint!("sd-frame-parsed");
    authenticate_privileges(program_id, &accounts, plan.caller_role())?;
    claims_cu_checkpoint!("sd-privileges");
    let packet_digest = hash(instruction_data).to_bytes();
    claims_cu_checkpoint!("sd-packet-digest");
    authenticate_authority(&accounts, plan, packet_digest)?;
    claims_cu_checkpoint!("sd-authority");
    let receipt = execute_authenticated(
        program_id,
        &accounts,
        plan,
        packet_digest,
        false,
        CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
    )?;
    set_return_data(&receipt.to_bytes());
    claims_cu_checkpoint!("sd-return-data");
    Ok(())
}

/// Execute one generated SignedDeltaV3 plan from an enclosing Claims route
/// authenticated under the same caller authority and ProductRuntimeV3 graph.
///
/// The release/deployment chain is authenticated HERE, on the frame this
/// function has already parsed. It used to be a separate pre-pass
/// (`authenticate_parent_releases`) that every enclosing route called
/// immediately before this one, and that pre-pass re-decoded the plan,
/// re-parsed the whole account frame and re-took the privileges before it got
/// to the releases -- so a terminal settlement paid `SignedDeltaPlanV3::decode`,
/// `SignedDeltaAccountsV3::parse` and `authenticate_privileges` twice for one
/// execution. Measured 2026-09-02 on real Claims ELFs: the second parse alone
/// was 29,878 CU of a 36-account frame. The order the pre-pass established is
/// preserved exactly -- privileges, then releases, then the parent authority --
/// and `execute_authenticated`'s `parent_authenticated` arm still skips its own
/// release authentication, so the chain is authenticated once and never zero
/// times.
pub(crate) fn execute_parent_authenticated(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
    parent: AuthenticatedSignedDeltaParentV3,
    admission: MarketAdmissionV1,
) -> Result<SignedDeltaReceiptV3, ProgramError> {
    let plan = SignedDeltaPlanV3::decode(instruction_data)
        .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
    let accounts = SignedDeltaAccountsV3::parse(account_infos, plan.position_count())?;
    authenticate_privileges(program_id, &accounts, plan.caller_role())?;
    authenticate_releases(&accounts, plan)?;
    authenticate_parent_authority(&accounts, plan, parent)?;
    execute_authenticated(
        program_id,
        &accounts,
        plan,
        hash(instruction_data).to_bytes(),
        true,
        admission,
    )
}

fn execute_authenticated(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    packet_digest: [u8; 32],
    releases_authenticated: bool,
    admission: MarketAdmissionV1,
) -> Result<SignedDeltaReceiptV3, ProgramError> {
    if !releases_authenticated {
        authenticate_releases(accounts, plan)?;
    }
    claims_cu_checkpoint!("sd-releases");

    let market_before = accounts
        .market
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let market =
        MarketViewV2::decode(&market_before).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
    authenticate_market(program_id, accounts, plan, market, &market_before)?;
    claims_cu_checkpoint!("sd-market");
    authenticate_product_and_basis_digests(accounts, plan)?;
    authenticate_failure_escrow_deltas(program_id, accounts, plan, market)?;
    let principal_cap_sets = authenticate_core_market_v3(
        accounts.core_market,
        accounts.core_program,
        accounts.registry,
        market,
        plan.product_record_digest(),
        admission,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::ProductBasis)?;
    claims_cu_checkpoint!("sd-product-basis");
    admit_principal_growth(plan, &market_before, principal_cap_sets)?;
    let (mut market_candidate, mut position_candidates) =
        build_candidates(program_id, accounts, plan, market, &market_before)?;
    drop(market_before);
    claims_cu_checkpoint!("sd-candidates");

    apply_deltas(plan, &mut market_candidate, &mut position_candidates)?;
    let post_market_revision = plan
        .expected_market_revision()
        .checked_add(1)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    put_u64(
        &mut market_candidate,
        MARKET_REVISION_OFFSET,
        post_market_revision,
    )?;
    for candidate in &mut position_candidates {
        let revision = read_u64(candidate, POSITION_REVISION_OFFSET)?
            .checked_add(1)
            .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
        put_u64(candidate, POSITION_REVISION_OFFSET, revision)?;
    }

    claims_cu_checkpoint!("sd-deltas-applied");

    let (positions, aggregates, deltas) = plan.table_bytes();
    let table_digest = hashv(&[
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
        positions,
        aggregates,
        deltas,
    ])
    .to_bytes();
    let post_resource_digest = resource_digest(&market_candidate, &position_candidates);
    let receipt = SignedDeltaReceiptV3::new(
        plan,
        packet_digest,
        table_digest,
        program_id.to_bytes(),
        post_resource_digest,
        post_market_revision,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Receipt)?;
    claims_cu_checkpoint!("sd-digests-and-receipt");
    commit_candidates(accounts, &market_candidate, &position_candidates)?;
    claims_cu_checkpoint!("sd-committed");
    Ok(receipt)
}

/// The CallerAuthority coordinate's writability under an owner-signed plan.
///
/// The frame spec pins coordinate 0 to a READONLY signer, which is exactly right
/// for a `CallerAuthoritySeedsV1` PDA: a program-derived authority is never the
/// fee payer and is never written. An owner signature is the other case. A
/// wallet that authorizes its own redemption pays the transaction's fee, so it
/// is a WRITABLE signer, and an account appearing as both the fee payer and a
/// readonly signer compiles to one writable-signer entry -- there is no message
/// in which a single-wallet submitter can satisfy the readonly pin.
///
/// Writability at this coordinate carries no authority. This program never
/// borrows the account beyond `key` and `is_signer`, it is not the market and
/// cannot be a Position (both are Claims-owned PDAs that cannot sign), and the
/// SVM would refuse a write to an account this program does not own. So the pin
/// is relaxed for `CallerRole::Claims` ONLY, and only along that one axis:
/// signer stays required and executable stays refused.
const fn authority_writability_is_free(role: CallerRole) -> bool {
    matches!(role, CallerRole::Claims)
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    role: CallerRole,
) -> Result<(), ProgramError> {
    let position_count =
        u32::try_from(accounts.positions.len()).map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let spec =
        SignedDeltaFrameSpecV3::new(position_count).map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let authority_writable_free = authority_writability_is_free(role);
    for index in 0..spec
        .account_count()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?
    {
        let expected = spec
            .account(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?
            .privileges();
        let observed = account(accounts.all, usize::from(index))?;
        let writable_admitted = if index == 0 && authority_writable_free {
            !observed.executable
        } else {
            observed.is_writable == expected.writable()
        };
        if observed.is_signer != expected.signer()
            || !writable_admitted
            || observed.executable != expected.executable()
        {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
    }
    if accounts.claims_program.key != program_id || accounts.rent.key != &sysvar::rent::ID {
        return Err(SignedDeltaSbfErrorV3::Accounts.into());
    }
    for (left, position) in accounts.positions.iter().enumerate() {
        if !position.is_writable
            || position.is_signer
            || position.executable
            || position.key == accounts.market.key
            || accounts
                .positions
                .iter()
                .skip(left.saturating_add(1))
                .any(|right| right.key == position.key)
        {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
    }
    Ok(())
}

fn account_for_role<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    spec: SignedDeltaFrameSpecV3,
    role: ClaimsFrameRoleV1,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    let mut found = None;
    for index in 0..spec
        .account_count()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?
    {
        if spec
            .account(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?
            .role()
            == role
        {
            if found.is_some() {
                return Err(SignedDeltaSbfErrorV3::Accounts.into());
            }
            found = Some(account(accounts, usize::from(index))?);
        }
    }
    found.ok_or_else(|| SignedDeltaSbfErrorV3::Accounts.into())
}

fn authenticate_authority(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    packet_digest: [u8; 32],
) -> Result<(), ProgramError> {
    // A submitted plan is authorized by a caller program's PDA, full stop. Role
    // `Claims` names the case with no caller program, so it has nothing to
    // derive here -- its authority is the Position owner's signature, which only
    // an enclosing Claims route knows the owner for. Refused rather than left to
    // be unsatisfiable by accident: a PDA under this program is exactly what no
    // external submitter can sign, and an accidental refusal reads as a bug.
    if plan.caller_role() == CallerRole::Claims {
        return Err(SignedDeltaSbfErrorV3::Release.into());
    }
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).map_err(|_| SignedDeltaSbfErrorV3::Release)?,
        plan.market(),
        execution_role(plan.caller_role()),
        plan.request_id(),
        packet_digest,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    if accounts.authority.key
        != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
    {
        return Err(SignedDeltaSbfErrorV3::Release.into());
    }
    Ok(())
}

fn authenticate_parent_authority(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    parent: AuthenticatedSignedDeltaParentV3,
) -> Result<(), ProgramError> {
    if plan.caller_role() != parent.caller_role
        || plan.release_set() != parent.release_set
        || plan.market() != parent.market
        || plan.request_id() != parent.parent_request_digest
    {
        return Err(SignedDeltaSbfErrorV3::Release.into());
    }
    // The coordinate is a SIGNER either way -- `authenticate_privileges` has
    // already enforced the frame spec, so what remains is WHICH key had to sign.
    match (parent.caller_role, parent.authority) {
        (CallerRole::Claims, ParentAuthorityV3::PositionOwner(owner)) => {
            if !accounts.authority.is_signer || accounts.authority.key.to_bytes() != owner {
                return Err(SignedDeltaSbfErrorV3::Release.into());
            }
        }
        // A crank is anybody, so the only thing asked of coordinate 0 is that
        // somebody stood behind the transaction. Everything that makes the
        // crank safe was proved before this mode could be selected.
        (CallerRole::Claims, ParentAuthorityV3::ClaimCheckCrank) => {
            if !accounts.authority.is_signer {
                return Err(SignedDeltaSbfErrorV3::Release.into());
            }
        }
        // Role `Claims` has no caller program, and no other role may substitute
        // an owner signature for its program's authority. Nor may any other
        // role borrow the compaction crank's relaxation: it is admissible only
        // where the owner's own signature would otherwise have been required,
        // and never as a way around a caller program's authority PDA.
        (CallerRole::Claims, ParentAuthorityV3::CallerProgramPda)
        | (CallerRole::Claims, ParentAuthorityV3::EnclosingClaimsRoute)
        | (CallerRole::Core | CallerRole::Trading, ParentAuthorityV3::PositionOwner(_))
        | (CallerRole::Core | CallerRole::Trading, ParentAuthorityV3::ClaimCheckCrank) => {
            return Err(SignedDeltaSbfErrorV3::Release.into());
        }
        (CallerRole::Core | CallerRole::Trading, ParentAuthorityV3::EnclosingClaimsRoute) => {}
        (CallerRole::Core | CallerRole::Trading, ParentAuthorityV3::CallerProgramPda) => {
            let seeds = CallerAuthoritySeedsV1::new(
                ContentId::new(parent.release_set).map_err(|_| SignedDeltaSbfErrorV3::Release)?,
                parent.market,
                execution_role(parent.caller_role),
                parent.parent_context,
                parent.parent_request_digest,
            )
            .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
            if accounts.authority.key
                != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
            {
                return Err(SignedDeltaSbfErrorV3::Release.into());
            }
        }
    }
    Ok(())
}

/// Bind every role coordinate this action lends authority to, against the
/// activation its caller already authenticated.
///
/// ## What the caller's signature establishes
///
/// Nothing reaches this function without a `CallerAuthoritySeedsV1` PDA
/// signature: [`authenticate_authority`] on the public entry, and
/// [`authenticate_parent_authority`] on the in-process one. A program-derived
/// address has no private key, so that signature is a statement made by the
/// program sitting at `caller_program` that it is the one invoking this route,
/// and the seed order pins the release set, the Market, the caller's execution
/// role, the replay context, and the digest of these exact instruction bytes.
///
/// Under the standing ruling in `GOAL.md` -- a callee invoked by a PDA-signed
/// CPI takes the facts that signer's seeds pin as established -- the release
/// set is established, and with it the activation the Registry wrote for it,
/// which the caller authenticated in this same instruction before it built the
/// frame being read here. So this route no longer re-observes each role's
/// current deployment. It used to, three times, through
/// `authenticate_activated_role`, against the very activation cache account its
/// caller had read: measured 2026-09-02 at 76,245 CU of a 173,680-CU
/// invocation that spends 662 applying the deltas it exists to apply.
///
/// ## What the seeds do NOT establish, and is therefore still checked here
///
/// **Which program holds a role in that release set.** The seeds name a role,
/// not a key, and a signature under `caller_program` proves only that
/// `caller_program` signed -- any deployed program can sign a PDA under itself.
/// That is the whole hazard the old comment on this function recorded, and it
/// is not repaired by the ruling:
///
/// > Role `Core` used to assert in a comment that it was "already covered by
/// > the first entry" and pin nothing: the first entry authenticates
/// > `core_program`, a DIFFERENT coordinate. Any executable program could sit
/// > at the caller coordinate and the authority the route demanded was a PDA
/// > under it -- which is exactly the signature no external submitter is
/// > supposed to be able to produce.
///
/// So every coordinate is still pinned, and the pin is now the strongest form
/// this frame can state: each Program and ProgramData key must equal the one
/// the Registry's own activation for this release set names. A caller that is
/// not the activated Trading refuses at the Trading coordinate; a caller naming
/// a release set whose cache it does not hold refuses at the cache address,
/// which is derived from that release set; a submitter with no program behind
/// it cannot produce the signature at all.
///
/// The activation cache is decoded ONCE. The three
/// `authenticate_activated_role` calls each ran
/// `ActivatedExecutionReleaseSetViewV1::decode` -- the complete five-role
/// projection and every aliasing pair -- for one role, so the account was
/// hostile-decoded three times to answer three questions about it.
/// [`authenticate_activation_cache_identity_v1`] is the crate's own
/// already-decoded entry point and exists for exactly this shape.
///
/// ## What is given up, named as debt rather than left to be discovered
///
/// The per-role deployment observation was also the slot pin: decision 0012's
/// `ReleaseSuperseded`, raised when the substrate's upgrade authority ships new
/// bytes under an open market. This route now inherits that refusal from its
/// caller instead of raising it itself. It is not lost from the transaction --
/// the caller observes all five roles before it composes this child -- but a
/// future caller that does not would not be caught here.
fn authenticate_releases(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
) -> Result<(), ProgramError> {
    let release_set = plan.release_set();
    // Core and Claims are authenticated for every role, at their own
    // coordinates. The CALLER coordinates carry a third role only when the
    // caller really is Trading; for the other two roles they must equal a
    // coordinate this route already authenticates as exactly that role, and
    // [`caller_coordinate`] says which.
    //
    // This is not bookkeeping. `authenticate_authority` derives this route's
    // whole authority as a PDA under `accounts.caller_program` with
    // `execution_role(caller_role)` in its seeds, so an unpinned caller
    // coordinate is an unpinned authority. Role `Core` used to assert in a
    // comment that it was "already covered by the first entry" and pin nothing:
    // the first entry authenticates `core_program`, a DIFFERENT coordinate.
    // Any executable program could sit at the caller coordinate and the
    // authority the route demanded was a PDA under it -- which is exactly the
    // signature no external submitter is supposed to be able to produce.
    // `rational_representation_v2::authenticate_release_batch` already makes
    // this pin for the same role; the three sibling routes never omitted it.
    let caller_is_trading = match caller_coordinate(plan.caller_role()) {
        CallerCoordinateV3::Caller => true,
        CallerCoordinateV3::Core => {
            if accounts.caller_program.key != accounts.core_program.key
                || accounts.caller_programdata.key != accounts.core_programdata.key
            {
                return Err(SignedDeltaSbfErrorV3::Release.into());
            }
            false
        }
        CallerCoordinateV3::Claims => {
            if accounts.caller_program.key != accounts.claims_program.key
                || accounts.caller_programdata.key != accounts.claims_programdata.key
            {
                return Err(SignedDeltaSbfErrorV3::Release.into());
            }
            false
        }
    };
    let requested: [Option<RequestedRoleV3<'_, '_>>; 3] = [
        Some((
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        )),
        Some((
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        )),
        caller_is_trading.then_some((
            ExecutionRoleV1::Trading,
            accounts.caller_program,
            accounts.caller_programdata,
        )),
    ];
    let cache = accounts
        .cache
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache)
        .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    // Registry ownership and the one exact width before a byte is believed, the
    // body naming the very release set the seeds pin, and the address the two
    // seeds reproduce -- so no account the Registry did not open for exactly
    // this release set can stand in.
    authenticate_activation_cache_identity_v1(
        accounts.registry,
        accounts.cache,
        &release_set,
        activated,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    for (role, program, programdata) in requested.into_iter().flatten() {
        let release = activated
            .role(role)
            .map_err(|_| SignedDeltaSbfErrorV3::Release)?
            .release();
        if program.key.to_bytes() != release.program().to_bytes()
            || programdata.key.to_bytes() != release.programdata()
        {
            return Err(SignedDeltaSbfErrorV3::Release.into());
        }
    }
    Ok(())
}

type RequestedRoleV3<'accounts, 'info> = (
    ExecutionRoleV1,
    &'accounts AccountInfo<'info>,
    &'accounts AccountInfo<'info>,
);

fn authenticate_market(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
    market_before: &[u8],
) -> Result<(), ProgramError> {
    // Reproduced from the bump this program recorded when it founded the
    // aggregate, not searched for.
    let expected_market = derive_hinted(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        program_id,
        recorded_bump(market_before, LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2),
    );
    if accounts.market.owner != program_id
        || accounts.market.key != &expected_market
        || market.logical_market != plan.market()
        || market.release_set != plan.release_set()
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.product_instance_id == [0; 32]
        || market.basis_id != plan.semantic_basis_id()
        || market.claim_count != plan.claim_count()
        || market.revision != plan.expected_market_revision()
    {
        return Err(SignedDeltaSbfErrorV3::ClaimsState.into());
    }
    Ok(())
}

/// A refunding Market's failure coordinate may move only in the escrow's own
/// Position, on this route as well as on the complete-set route.
///
/// # The hole this closes
///
/// `signed_delta_v3` expresses an ARBITRARY conservative batch, so before this
/// it could credit a refunding Market's failure coordinate to any Position it
/// liked -- a stranger's, the founder's -- with no escrow check at all, and
/// could debit the escrow's. The complete-set gate
/// (`authenticate_failure_escrow`) does not see this route, and the founding
/// that seats the escrow cannot defend a coordinate after founding. Decision
/// 0025 section 6 named this route for the sibling immobility shape; under the
/// escrow shape it is the one waist the complete-set gate does not cover, and
/// leaving it open would have made the seating a formality -- the claims the
/// ruling exists to keep out of somebody's hands could simply be written there.
///
/// # Why it costs nothing on the route it is added to
///
/// The scan is over the plan's own delta table, which is already decoded and
/// in hand. A plan that touches no coordinate at the runtime width's last
/// index returns here having read no account and derived no address, and that
/// is every ordinary fill, every ordinary redemption and every batch on a
/// categorical Market's non-final outcome. Only a plan that reaches for the
/// failure coordinate pays for the basis decode and the one derivation --
/// which matters, because this route's compute is measured and defended
/// (`docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`), and a gate
/// that charged every batch for a rule about one coordinate would be paid for
/// by every trade.
///
/// # What it does NOT forbid
///
/// The escrow's own Position moving at the failure coordinate, which is how a
/// refunding merge burns the seated claims and how a failure settlement
/// retires them. And every coordinate of a CATEGORICAL Market, whose last
/// outcome is an ordinary tradeable claim -- cohort-13's failure walk paid it
/// to its holder, which was the protocol doing what it says it does.
#[inline(never)]
fn authenticate_failure_escrow_deltas(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    // A width that can seat no escrow has no failure coordinate to defend, so
    // this gate is INERT there rather than a refusal. Founding will not create
    // such a market any more; one founded before the seating still trades.
    let Ok(failure) = dclutch_product::economic_slice::refunding_failure_index(market.claim_count)
    else {
        return Ok(());
    };
    let Ok(failure) = u32::try_from(failure) else {
        return Ok(());
    };
    if !plan_touches_failure_coordinate_v1(plan, failure)? {
        return Ok(());
    }
    // The record's own answer, not this route's. The bytes were pinned to the
    // plan's `linked_basis_record_digest` one call above, so this decodes an
    // account the caller has already signed for and Core authenticated at
    // founding.
    let basis_bytes = accounts
        .basis_record
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let refunding =
        dclutch_product::payoff::runtime_v3::ProductBasisV3::decode(&basis_bytes)
            .map_err(|_| SignedDeltaSbfErrorV3::ProductBasis)?
            .refunds_on_failure();
    drop(basis_bytes);
    if !refunding {
        return Ok(());
    }
    let escrow = crate::FailureEscrowIdentityV1::derive(
        program_id,
        market.logical_market,
        market.claim_count,
    )
    .map_err(|_| crate::ClaimsSbfError::FailureEscrow)?;
    admit_failure_coordinate_owners_v1(plan, failure, escrow.owner)
}

/// Whether this plan moves any Position at the failure coordinate.
///
/// Split out from the gate because it is the whole of what a plan that does
/// NOT touch that coordinate pays, and a claim about cost is worth a test.
fn plan_touches_failure_coordinate_v1(
    plan: SignedDeltaPlanV3<'_>,
    failure: u32,
) -> Result<bool, ProgramError> {
    for index in 0..plan.position_delta_count() {
        let row = plan
            .position_delta(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        if row.outcome() == failure {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Every CREDIT at the failure coordinate must name the escrow's own Position.
///
/// CREDITS ONLY, and the asymmetry is the whole of the rule rather than a
/// gap in it. The hazard decision 0025 exists to close is a refunding Market's
/// worthless failure claims IN SOMEBODY'S HANDS -- worse than worthless
/// because they are sellable to a reader of a claim balance -- and only a
/// credit puts them there. A debit takes them away.
///
/// Refusing debits too would have been strictly worse, and not hypothetically:
/// cohort-16 founds markets whose RECORD refunds while their failure column
/// still sits with the founder, because the seating rides cohort-17. On those
/// markets every ordinary settlement and every retirement debit of that column
/// is a debit from a Position that is not the escrow, so a two-directional
/// gate would have frozen the failure column of every market founded between
/// the payout arm and the seating -- and a market that cannot retire leaks
/// rent forever (decision 0029 item 3).
fn admit_failure_coordinate_owners_v1(
    plan: SignedDeltaPlanV3<'_>,
    failure: u32,
    escrow_owner: [u8; 32],
) -> Result<(), ProgramError> {
    for index in 0..plan.position_delta_count() {
        let row = plan
            .position_delta(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        if row.outcome() != failure || row.delta().direction() != DeltaDirectionV3::Credit {
            continue;
        }
        let owner = plan
            .position(row.position_index())
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?
            .owner();
        if owner != escrow_owner {
            // 0x5010, the same code the complete-set gate and the founding
            // raise: a Position that is not this Market's escrow was OFFERED
            // the failure claims. Three routes, one accusation, one reader.
            return Err(crate::ClaimsSbfError::FailureEscrow.into());
        }
    }
    Ok(())
}

/// Preflight every positive aggregate delta against Core's set-denominated
/// principal cap before any candidate or account byte is changed.
fn admit_principal_growth(
    plan: SignedDeltaPlanV3<'_>,
    market: &[u8],
    principal_cap_sets: u64,
) -> Result<(), ProgramError> {
    let cap = MarketPrincipalCapSetsV1::read(principal_cap_sets);
    for outcome in 0..plan.claim_count() {
        let delta = plan
            .aggregate_delta(outcome)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        if delta.direction() != DeltaDirectionV3::Credit {
            continue;
        }
        let offset = usize::try_from(outcome)
            .ok()
            .and_then(|outcome| outcome.checked_mul(SCALAR_BYTES))
            .and_then(|relative| LIABILITY_BASIS_MARKET_HEADER_BYTES_V2.checked_add(relative))
            .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
        cap.admit_growth(read_u64(market, offset)?, delta.magnitude())
            .map_err(|_| SignedDeltaSbfErrorV3::PrincipalCapacity)?;
    }
    Ok(())
}

/// Rejoin the exact Product and ProductBasisV3 raw record digests the caller
/// has already authenticated and signed for.
///
/// ## Why this is the whole join, and what used to be here
///
/// This route used to derive the Product runtime graph and the linked
/// `ProductBasisV3` record from the Registry on every invocation --
/// `authenticate_runtime_product_basis_core_v3`, eight canonical record
/// derivations over four `FinalizedRecordFrameV2` pairs, measured 2026-09-02 at
/// **41,808 CU** on the SignedDelta child, which is 24.1% of a completing
/// invocation. The enclosing in-process arm did not: it hashed the two raw
/// records and compared them against the digests its parent had committed to,
/// because its parent had already done the derivation.
///
/// A CPI caller has committed to them at least as hard. The
/// `CallerAuthoritySeedsV1` PDA that authorizes this route carries
/// `hash(instruction_data)` as its last seed, so the signature is over the
/// EXACT plan bytes -- `product_record_digest` and `linked_basis_record_digest`
/// included. Under the ruling in `GOAL.md` those are established for exactly
/// what the seeds name, and what remains is to bind the frame's coordinates to
/// them, which is what this does.
///
/// ## The two conjuncts a signature cannot carry, and which therefore stay
///
/// [`authenticate_core_market_v3`] runs unconditionally beside this, and it is
/// not a formality: Core's persisted Market independently names the product
/// record digest, the product id, the selected release set, the Registry and
/// the generation, and carries the principal cap. A caller may pin its own plan
/// to whatever it likes; it may not author the Market's persisted cap, and the
/// Core join is where a plan that names a product the Market never selected
/// refuses. [`authenticate_market`] is the other: the Claims-owned Market PDA
/// pins the release set, the semantic basis id, the claim count and the
/// revision against this same plan.
fn authenticate_product_and_basis_digests(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
) -> Result<(), ProgramError> {
    let product = accounts
        .product_record
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let product_digest = hash(&product).to_bytes();
    drop(product);
    let basis = accounts
        .basis_record
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let basis_digest = hash(&basis).to_bytes();
    if product_digest != plan.product_record_digest()
        || basis_digest != plan.linked_basis_record_digest()
    {
        return Err(SignedDeltaSbfErrorV3::ProductBasis.into());
    }
    Ok(())
}

/// Widest hinted seed set on this route, plus the bump seed.
const HINTED_SEED_CAPACITY_V3: usize = 4;

/// One account body's own recorded bump. Zero is unrecorded and its reader
/// searches, so a body written before the byte existed is no worse off.
fn recorded_bump(body: &[u8], offset: usize) -> u8 {
    body.get(offset).copied().unwrap_or(0)
}

/// Reproduce one address from a recorded bump, degrading to the search this
/// route always ran.
///
/// Reading a hint must not be able to refuse: an unrecorded bump, or one whose
/// derivation fails outright, falls back to `find_program_address`. Only the
/// address equality the caller already had can refuse.
fn derive_hinted(seeds: &[&[u8]], program_id: &Pubkey, hint: u8) -> Pubkey {
    if hint != 0 && seeds.len() < HINTED_SEED_CAPACITY_V3 {
        let bump = [hint];
        let mut buffer: [&[u8]; HINTED_SEED_CAPACITY_V3] = [&[]; HINTED_SEED_CAPACITY_V3];
        for (slot, seed) in buffer.iter_mut().zip(seeds) {
            *slot = seed;
        }
        if let Some(slot) = buffer.get_mut(seeds.len()) {
            *slot = &bump;
        }
        if let Some(all) = buffer.get(..=seeds.len()) {
            if let Ok(address) = Pubkey::create_program_address(all, program_id) {
                return address;
            }
        }
    }
    Pubkey::find_program_address(seeds, program_id).0
}

fn build_candidates(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
    market_before: &[u8],
) -> Result<(Vec<u8>, Vec<Vec<u8>>), ProgramError> {
    let mut candidates = Vec::with_capacity(accounts.positions.len());
    for (index, account) in accounts.positions.iter().enumerate() {
        let table_index = u32::try_from(index).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        let expected = plan
            .position(table_index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        let seeds = ProtocolPositionSeedsV2::new(accounts.market.key.to_bytes(), expected.owner())
            .map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        let data = account
            .try_borrow_data()
            .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
        // Two Positions are authenticated per plan, so this is the same saving
        // twice; `sparse_native_transfer_v1` already reads the byte this reads.
        let expected_key = derive_hinted(
            &seeds.as_slices(),
            program_id,
            recorded_bump(&data, LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2),
        );
        let position =
            PositionViewV2::decode(&data).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        if account.owner != program_id
            || account.key != &expected_key
            || position.market_account != accounts.market.key.to_bytes()
            || position.owner != expected.owner()
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
            || position.revision != expected.expected_revision()
        {
            return Err(SignedDeltaSbfErrorV3::ClaimsState.into());
        }
        candidates.push(data.to_vec());
    }
    Ok((market_before.to_vec(), candidates))
}

fn apply_deltas(
    plan: SignedDeltaPlanV3<'_>,
    market: &mut [u8],
    positions: &mut [Vec<u8>],
) -> Result<(), ProgramError> {
    for outcome in 0..plan.claim_count() {
        apply_coordinate(
            market,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            outcome,
            plan.aggregate_delta(outcome)
                .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?,
        )?;
    }
    for index in 0..plan.position_delta_count() {
        let row = plan
            .position_delta(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        let position_index =
            usize::try_from(row.position_index()).map_err(|_| SignedDeltaSbfErrorV3::Candidate)?;
        let candidate = positions
            .get_mut(position_index)
            .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
        apply_coordinate(
            candidate,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            row.outcome(),
            row.delta(),
        )?;
    }
    Ok(())
}

fn apply_coordinate(
    bytes: &mut [u8],
    header: usize,
    outcome: u32,
    delta: SignedDeltaV3,
) -> Result<(), ProgramError> {
    let offset = usize::try_from(outcome)
        .ok()
        .and_then(|outcome| outcome.checked_mul(SCALAR_BYTES))
        .and_then(|relative| header.checked_add(relative))
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    let before = read_u64(bytes, offset)?;
    let after = match delta.direction() {
        DeltaDirectionV3::Neutral => Some(before),
        DeltaDirectionV3::Credit => before.checked_add(delta.magnitude()),
        DeltaDirectionV3::Debit => before.checked_sub(delta.magnitude()),
    }
    .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    put_u64(bytes, offset, after)
}

fn resource_digest(market: &[u8], positions: &[Vec<u8>]) -> [u8; 32] {
    let mut resources: Vec<&[u8]> = Vec::with_capacity(positions.len().saturating_add(2));
    resources.push(SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3);
    resources.push(market);
    for position in positions {
        resources.push(position);
    }
    hashv(&resources).to_bytes()
}

fn commit_candidates(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    market_candidate: &[u8],
    position_candidates: &[Vec<u8>],
) -> Result<(), ProgramError> {
    let mut market = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Commit)?;
    if market.len() != market_candidate.len()
        || position_candidates.len() != accounts.positions.len()
    {
        return Err(SignedDeltaSbfErrorV3::Commit.into());
    }
    let mut positions: Vec<RefMut<'_, &mut [u8]>> = Vec::with_capacity(accounts.positions.len());
    for (account, candidate) in accounts.positions.iter().zip(position_candidates) {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| SignedDeltaSbfErrorV3::Commit)?;
        if data.len() != candidate.len() {
            return Err(SignedDeltaSbfErrorV3::Commit.into());
        }
        positions.push(data);
    }
    market.copy_from_slice(market_candidate);
    for (mut position, candidate) in positions.into_iter().zip(position_candidates) {
        position.copy_from_slice(candidate);
    }
    Ok(())
}

/// Which coordinate of the frame must hold the caller program for one role.
///
/// The authority for this route is a PDA under whatever sits at the caller
/// coordinate, seeded with `execution_role(role)`. So that coordinate must hold
/// the program the Registry authenticated for exactly that role -- either
/// because the caller coordinates are themselves authenticated as a third role
/// ([`Self::Caller`], only Trading), or because they are pinned to the
/// coordinate that already is ([`Self::Core`], [`Self::Claims`]).
///
/// Being an enum rather than a pair of `if`s is the point: there is no variant
/// meaning "unpinned and unauthenticated", which is what role `Core` silently
/// was, and a fourth role cannot be added without choosing one of these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallerCoordinateV3 {
    /// The caller coordinates carry their own third Registry-authenticated role.
    Caller,
    /// The caller coordinates must equal the Core program coordinates.
    Core,
    /// The caller coordinates must equal the Claims program coordinates.
    Claims,
}

const fn caller_coordinate(role: CallerRole) -> CallerCoordinateV3 {
    match role {
        CallerRole::Core => CallerCoordinateV3::Core,
        CallerRole::Claims => CallerCoordinateV3::Claims,
        CallerRole::Trading => CallerCoordinateV3::Caller,
    }
}

const fn execution_role(role: CallerRole) -> ExecutionRoleV1 {
    match role {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Claims => ExecutionRoleV1::Claims,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| SignedDeltaSbfErrorV3::Accounts.into())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    let field: [u8; SCALAR_BYTES] = bytes
        .get(offset..end)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?
        .try_into()
        .map_err(|_| SignedDeltaSbfErrorV3::Candidate)?;
    Ok(u64::from_le_bytes(field))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    bytes
        .get_mut(offset..end)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use dclutch_claims::signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SIGNED_DELTA_PLAN_MAGIC_V3,
        SignedDeltaPlanInputV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    };

    /// Every caller role's coordinate is authenticated as that same role.
    ///
    /// The expectation here is derived from the AUTHORITY side --
    /// `execution_role`, the role that actually goes into the seeds the
    /// authority PDA is derived with -- and never from the pin under test, so
    /// this cannot agree with a wrong pin by construction. Under the code this
    /// replaced, `Core` was pinned to nothing and added no Registry entry, so
    /// its coordinate was authenticated as no role at all and this assertion
    /// had no true branch to take.
    ///
    /// Read as one sentence: whatever program the authority is derived under
    /// must be a program the Registry authenticated for exactly the role in
    /// that authority's seeds.
    #[test]
    fn every_caller_role_coordinate_is_authenticated_as_that_role() {
        for role in [CallerRole::Core, CallerRole::Claims, CallerRole::Trading] {
            let authenticated_as = match caller_coordinate(role) {
                // The caller coordinates are themselves the third Registry
                // entry, which `authenticate_releases` requests as Trading.
                CallerCoordinateV3::Caller => ExecutionRoleV1::Trading,
                // Or they are pinned equal to a coordinate the route always
                // authenticates, as the role that entry is requested under.
                CallerCoordinateV3::Core => ExecutionRoleV1::Core,
                CallerCoordinateV3::Claims => ExecutionRoleV1::Claims,
            };
            assert_eq!(
                authenticated_as,
                execution_role(role),
                "the authority for {role:?} is derived under a program \
                 authenticated as {authenticated_as:?}, not as its own role"
            );
        }
    }

    /// Only Trading brings an external program to the caller coordinates.
    ///
    /// This is the fact that makes the third Registry entry conditional. If a
    /// future role were given [`CallerCoordinateV3::Caller`] without also being
    /// requested as its own role, the assertion above would fail; if it were
    /// given a pin, this one records that it brings no new program.
    #[test]
    fn only_a_trading_caller_carries_a_program_of_its_own() {
        assert_eq!(
            caller_coordinate(CallerRole::Trading),
            CallerCoordinateV3::Caller
        );
        assert_eq!(
            caller_coordinate(CallerRole::Core),
            CallerCoordinateV3::Core
        );
        assert_eq!(
            caller_coordinate(CallerRole::Claims),
            CallerCoordinateV3::Claims
        );
    }

    /// One width-three plan with a single Position moving a single
    /// coordinate, and the aggregate moving with it.
    ///
    /// One Position because the table requires strictly increasing owners, so
    /// a two-row plan at one coordinate can never name the same owner twice --
    /// and the positive control this gate needs is exactly "the escrow, and
    /// nobody else, moved the failure column".
    fn one_position_plan_bytes(
        owner: [u8; 32],
        outcome: u32,
        direction: DeltaDirectionV3,
    ) -> Vec<u8> {
        let positions = [SignedDeltaPositionV3::new(owner, 4).expect("position")];
        let rows = [PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: 0,
                outcome,
                delta: delta(direction, 5),
            },
            1,
            3,
        )
        .expect("row")];
        let mut aggregates = [
            delta(DeltaDirectionV3::Neutral, 0),
            delta(DeltaDirectionV3::Neutral, 0),
            delta(DeltaDirectionV3::Neutral, 0),
        ];
        aggregates[outcome as usize] = delta(direction, 5);
        let mut bytes = vec![0; plan_bytes(3, 1, 1).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 3,
                claim_count: 3,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("encode");
        bytes
    }

    const ESCROW_OWNER: [u8; 32] = [0x5e; 32];
    const STRANGER_OWNER: [u8; 32] = [0x77; 32];
    /// Coordinate 2 of a width-three Market: `refunding_failure_index(3)`.
    const FAILURE: u32 = 2;

    /// POSITIVE CONTROL FIRST. Without it the refusals below prove nothing: a
    /// gate that refused every plan would pass them all.
    ///
    /// The escrow's own failure column moving is ADMITTED, which is what a
    /// refunding merge's burn and a failure settlement's retirement look like
    /// on this route.
    #[test]
    fn the_escrows_own_failure_column_may_move() {
        let bytes = one_position_plan_bytes(ESCROW_OWNER, FAILURE, DeltaDirectionV3::Credit);
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        assert!(plan_touches_failure_coordinate_v1(plan, FAILURE).expect("scan"));
        admit_failure_coordinate_owners_v1(plan, FAILURE, ESCROW_OWNER)
            .expect("the escrow's own column may move");
    }

    /// A refunding Market's failure claims CREDITED to a stranger refuse.
    ///
    /// Decision 0025's hazard in bytes, on the one waist the complete-set gate
    /// does not cover: claims worth nothing on a refunding basis, in somebody's
    /// hands, and therefore sellable to a reader of a claim balance.
    #[test]
    fn a_strangers_failure_column_refuses_failure_escrow() {
        let bytes = one_position_plan_bytes(STRANGER_OWNER, FAILURE, DeltaDirectionV3::Credit);
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        assert!(plan_touches_failure_coordinate_v1(plan, FAILURE).expect("scan"));
        assert_eq!(
            admit_failure_coordinate_owners_v1(plan, FAILURE, ESCROW_OWNER).unwrap_err(),
            ProgramError::Custom(crate::ClaimsSbfError::FailureEscrow as u32),
        );
    }

    /// A stranger GIVING UP a failure column is admitted, and that is what
    /// keeps every market founded before the seating able to close.
    ///
    /// Cohort-16 founds refunding RECORDS whose failure column is still the
    /// founder's, because the seating rides cohort-17. Their settlement and
    /// their retirement both debit that column from a Position that is not the
    /// escrow. A gate that refused debits would have frozen it, and a market
    /// that cannot retire leaks rent forever.
    #[test]
    fn a_stranger_giving_up_a_failure_column_is_admitted() {
        let bytes = one_position_plan_bytes(STRANGER_OWNER, FAILURE, DeltaDirectionV3::Debit);
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        assert!(plan_touches_failure_coordinate_v1(plan, FAILURE).expect("scan"));
        admit_failure_coordinate_owners_v1(plan, FAILURE, ESCROW_OWNER)
            .expect("a debit takes worthless claims out of a stranger's hands");
    }

    /// The gate is INERT off the failure coordinate, and that is the COST
    /// claim: a plan touching no failure coordinate never reaches the basis
    /// decode or the escrow derivation, so every ordinary fill and every
    /// ordinary redemption pays this rule nothing at all.
    #[test]
    fn a_plan_that_touches_no_failure_coordinate_is_never_scanned_for_owners() {
        let bytes = one_position_plan_bytes(STRANGER_OWNER, 0, DeltaDirectionV3::Credit);
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        assert!(!plan_touches_failure_coordinate_v1(plan, FAILURE).expect("scan"));
        // The SAME plan refuses when the coordinate it moves IS the failure
        // one, which is what makes the assertion above load-bearing rather
        // than a tautology about a plan that could never refuse at all.
        assert_eq!(
            admit_failure_coordinate_owners_v1(plan, 0, ESCROW_OWNER).unwrap_err(),
            ProgramError::Custom(crate::ClaimsSbfError::FailureEscrow as u32),
        );
    }

    fn delta(direction: DeltaDirectionV3, magnitude: u64) -> SignedDeltaV3 {
        SignedDeltaV3::new(direction, magnitude).expect("delta")
    }

    fn plan_fixture() -> Vec<u8> {
        let positions = [
            SignedDeltaPositionV3::new([7; 32], 4).expect("a"),
            SignedDeltaPositionV3::new([8; 32], 9).expect("b"),
        ];
        let rows = [
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 0,
                    outcome: 1,
                    delta: delta(DeltaDirectionV3::Debit, u64::MAX),
                },
                2,
                2,
            )
            .expect("debit"),
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 1,
                    outcome: 1,
                    delta: delta(DeltaDirectionV3::Credit, u64::MAX),
                },
                2,
                2,
            )
            .expect("credit"),
        ];
        let aggregates = [
            delta(DeltaDirectionV3::Neutral, 0),
            delta(DeltaDirectionV3::Neutral, 0),
        ];
        let mut bytes = vec![0; plan_bytes(2, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 3,
                claim_count: 2,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("encode");
        bytes
    }

    fn aggregate_credit_plan_bytes(quantity: u64) -> Vec<u8> {
        let positions = [SignedDeltaPositionV3::new([7; 32], 4).expect("destination")];
        let aggregates = [delta(DeltaDirectionV3::Credit, quantity)];
        let rows = [PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: 0,
                outcome: 0,
                delta: delta(DeltaDirectionV3::Credit, quantity),
            },
            1,
            1,
        )
        .expect("complete-set credit")];
        let mut bytes = vec![0; plan_bytes(1, 1, 1).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 3,
                claim_count: 1,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("encode complete-set credit");
        bytes
    }

    #[test]
    fn aggregate_credit_cap_boundary_excess_and_overflow_preflight_without_mutation() {
        let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + SCALAR_BYTES];
        put_u64(&mut market, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 7)
            .expect("outstanding supply");
        let before = market.clone();
        let boundary_bytes = aggregate_credit_plan_bytes(3);
        let boundary = SignedDeltaPlanV3::decode(&boundary_bytes).expect("boundary plan");
        assert_eq!(admit_principal_growth(boundary, &market, 10), Ok(()));
        assert_eq!(market, before);

        let excess_bytes = aggregate_credit_plan_bytes(4);
        let excess = SignedDeltaPlanV3::decode(&excess_bytes).expect("excess plan");
        assert_eq!(
            admit_principal_growth(excess, &market, 10),
            Err(SignedDeltaSbfErrorV3::PrincipalCapacity.into())
        );
        assert_eq!(market, before, "cap refusal cannot mutate aggregate bytes");

        put_u64(
            &mut market,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            u64::MAX,
        )
        .expect("maximum outstanding supply");
        let overflow_before = market.clone();
        let overflow_bytes = aggregate_credit_plan_bytes(1);
        let overflow = SignedDeltaPlanV3::decode(&overflow_bytes).expect("overflow plan");
        assert_eq!(
            admit_principal_growth(overflow, &market, u64::MAX),
            Err(SignedDeltaSbfErrorV3::PrincipalCapacity.into())
        );
        assert_eq!(market, overflow_before);
    }

    #[test]
    fn candidate_application_is_atomic_and_full_range() {
        let bytes = plan_fixture();
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16];
        let mut positions = vec![
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
        ];
        put_u64(
            positions.get_mut(0).expect("a"),
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8,
            u64::MAX,
        )
        .expect("balance");
        apply_deltas(plan, &mut market, &mut positions).expect("transfer");
        assert_eq!(
            read_u64(
                positions.first().expect("a"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8
            ),
            Ok(0)
        );
        assert_eq!(
            read_u64(
                positions.get(1).expect("b"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8
            ),
            Ok(u64::MAX)
        );
        let before = positions.clone();
        assert_eq!(
            apply_deltas(plan, &mut market, &mut positions),
            Err(SignedDeltaSbfErrorV3::Candidate.into())
        );
        assert_eq!(
            positions, before,
            "refusal does not partially mutate another coordinate"
        );
    }

    #[test]
    fn dispatch_magic_is_exact() {
        let bytes = plan_fixture();
        assert_eq!(
            bytes.get(..SIGNED_DELTA_PLAN_MAGIC_V3.len()),
            Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice())
        );
    }
}
