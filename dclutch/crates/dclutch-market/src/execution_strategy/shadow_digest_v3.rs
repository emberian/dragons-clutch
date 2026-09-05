//! Canonical transcripts compared by Trading and stateless V3 accelerators.
//!
//! These digests deliberately encode only already authenticated observations,
//! interpreted register/effect outputs, and invocation coordinates. They do
//! not authenticate accounts or artifacts and grant no state or CPI authority.

use core::convert::TryFrom;

use dclutch_core_contract::ContentId;
use dclutch_sha256_adapter::digestv;

#[cfg(feature = "alloc")]
extern crate alloc;

/// Domain for complete family request bytes.
pub const FAMILY_REQUEST_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-family-request:v3";
/// Domain for AccountProfile-ordered runtime observations.
pub const RUNTIME_OBSERVATION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-runtime-observations:v3";
/// Domain for one interpreted candidate register bank.
pub const CANDIDATE_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-candidate:v3";
/// Domain for one interpreted effect projection.
pub const EFFECT_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-effect:v4-ordered-receipts";
/// Domain for one route's exact ordered prior-receipt dependency list.
pub const RECEIPT_DEPENDENCIES_DIGEST_DOMAIN_V4: &[u8] = b"dclutch:shadow-receipt-dependencies:v4";
/// Domain for one selected action invocation.
pub const INVOCATION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:shadow-invocation:v3";
/// Domain for one accelerator invocation's caller-authority seed.
pub const ACCELERATOR_CALLER_AUTHORITY_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch:accelerator-caller-authority:v1";

/// Stable refusal from canonical transcript construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowDigestErrorV3 {
    /// A slice length or coordinate exceeded its exact encoded `u32` width.
    CountOverflow,
    /// A route used an unknown role or route-kind tag.
    InvalidRouteTag,
    /// A route's optional item/witness presence grammar was noncanonical.
    InvalidRoutePresence,
    /// A supposedly read-only accelerator observation retained caller privileges.
    PrivilegedRuntimeObservation,
    /// SHA-256 produced the reserved all-zero content identity.
    ZeroDigest,
}

/// Result alias for canonical V3 transcript construction.
pub type Result<T> = core::result::Result<T, ShadowDigestErrorV3>;

/// Which accelerator disposition an authority is minted for.
///
/// Not data: each call site is one of the two dispositions and names its own,
/// so the two families cannot derive one address. `resolve_execution_candidate_v2`
/// selects exactly one disposition per action, so a shared address would not
/// have been a shared authority either — this makes the separation structural
/// rather than an argument that has to stay true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AcceleratorCallerKindV1 {
    /// The admitted-AOT route: `AdmittedAcceleratorRequestV2`, one authority per
    /// output invocation.
    Admitted = 0,
    /// The Shadow comparison route: `ShadowRequestV3`, exactly one authority.
    Shadow = 1,
}

/// The `role_request_digest` seed of one accelerator invocation's caller authority.
///
/// # A caller authority's address is a function of the signed instruction alone
///
/// Both routes used to seed this with the digest of the request they were about
/// to send, and a request carries the register bank. An AccountProfile
/// declaring `TrustedEnvironmentV2::CurrentSlot` puts `Clock::get().slot` in
/// that bank — General's seven window-gated actions do — so the digest, and
/// every `find_program_address` over it, differed in every slot. A
/// caller-authority PDA has to be NAMED in the top-level account list, which is
/// fixed when the transaction is signed, so those addresses were valid for
/// exactly one slot and no caller could deliver into them.
///
/// The tree already stated the law:
/// `the_window_gated_actions_declare_the_current_slot_in_their_bank` says
/// "Anything outside the executing instruction that has to STATE that bank is
/// therefore valid for exactly one slot, which no caller can deliver into", and
/// that sentence deleted the input scratch-page transport. These addresses
/// survived the cut because it reasoned about page ACCOUNTS and this is a
/// page-less ADDRESS. See
/// `docs/design/GENERAL_CALLER_AUTHORITY_SLOT_BINDING_2026_09_03.md`.
///
/// The preimage is now the digest of the SIGNED top-level family request —
/// [`family_request_digest_v3`] of the exact `DCLTHOT3` payload, which both
/// invocation contexts already carry and every reader already re-derives — plus
/// the disposition and the invocation ordinal, the only coordinate that varies
/// between the invocations of one execution. All three are computable by any
/// caller that can build the transaction at all, and none is a
/// trusted-environment observation.
///
/// # What it gives up, and what still covers that
///
/// The authority is no longer bound to the exact request BYTES, only to the
/// family request that determines them. Everything else in those bytes is
/// derived by Trading from authenticated artifacts and chain state inside the
/// same instruction, and the callee re-derives it: the accelerators' own frame
/// checks and `require_admitted_bank_matches_frame_v3` cover the bank, and each
/// acknowledgement still names the digest of the request it answered, so a
/// reply to another request is still refused. The authority stops being a
/// second, redundant statement of what Trading just computed and becomes a
/// statement of what the caller asked for.
pub fn accelerator_caller_authority_digest_v1(
    kind: AcceleratorCallerKindV1,
    parent_request_digest: ContentId,
    invocation_index: u32,
) -> Result<ContentId> {
    ContentId::new(digestv(&[
        ACCELERATOR_CALLER_AUTHORITY_DIGEST_DOMAIN_V1,
        &[kind as u8],
        parent_request_digest.as_bytes(),
        &invocation_index.to_le_bytes(),
    ]))
    .map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

/// One exact read-only runtime observation in AccountProfile logical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowRuntimeObservationV3<'a> {
    /// Account public key.
    pub key: [u8; 32],
    /// Account owner program.
    pub owner: [u8; 32],
    /// Observed lamports.
    pub lamports: u64,
    /// Exact observed account bytes.
    pub data: &'a [u8],
    /// Must be false: the accelerator receives no signer privilege for runtime state.
    pub signer: bool,
    /// Must be false: the accelerator receives runtime state read-only.
    pub writable: bool,
    /// Whether the top-level account is executable.
    pub executable: bool,
}

/// One exact read-only runtime observation whose two identities are BORROWED
/// where they already sit.
///
/// [`ShadowRuntimeObservationV3`] OWNS its key and owner, which is right for a
/// host emitter decoding a wire and wrong for an on-chain caller: both
/// identities are already addressable in the account frame for the whole
/// invocation, so a bank of the owning form is ninety-six bytes per coordinate
/// of pure copy on an allocator that never frees.
/// `AccountObservationV1::key_bytes` borrows for exactly this reason and says
/// so.
///
/// Measured 2026-09-02 on the Dealer post-trade partial equity Remove
/// (`accepted.rs`, seventy-four runtime coordinates): the owning bank was
/// **7,104 bytes**, dead the instant the digest returned, on a route whose peak
/// stood **136 bytes** over its 65,536-byte grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRuntimeObservationV3<'a> {
    /// Account public key, borrowed where the caller already holds it.
    pub key: &'a [u8; 32],
    /// Account owner program, borrowed where the caller already holds it.
    pub owner: &'a [u8; 32],
    /// Observed lamports.
    pub lamports: u64,
    /// Exact observed account bytes.
    pub data: &'a [u8],
    /// Must be false: the accelerator receives no signer privilege for runtime state.
    pub signer: bool,
    /// Must be false: the accelerator receives runtime state read-only.
    pub writable: bool,
    /// Whether the top-level account is executable.
    pub executable: bool,
}

/// Canonical child role tag in the Shadow effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ShadowRouteRoleV3 {
    /// Market Core.
    Core = 0,
    /// Claims.
    Claims = 1,
    /// Resolution.
    Resolution = 3,
    /// Custody.
    Custody = 4,
}

/// Canonical invocation geometry tag in the Shadow effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ShadowRouteKindV3 {
    /// One fixed request/account frame.
    Once = 0,
    /// One fixed prefix plus the complete affine item tail.
    AffineOnce = 1,
    /// One invocation for one canonical item.
    Each = 2,
}

/// Exact adapter-resolved child route committed by the effect transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowResolvedRouteV3 {
    /// Authenticated child role.
    pub role: ShadowRouteRoleV3,
    /// Resolved invocation geometry.
    pub kind: ShadowRouteKindV3,
    /// Item ordinal, present only for [`ShadowRouteKindV3::Each`].
    pub item: Option<u32>,
    /// Fixed account-frame start.
    pub fixed_account_start: u16,
    /// Fixed account-frame count.
    pub fixed_account_count: u16,
    /// First expanded item-account coordinate.
    pub item_account_start: u32,
    /// Accounts in one repeated item subframe.
    pub item_account_count: u16,
    /// Distance between repeated item subframes.
    pub item_account_stride: u16,
    /// Number of repeated item subframes.
    pub repeated_item_count: u32,
    /// Offset in the projected request bank.
    pub request_offset: u32,
    /// Exact request bytes before any authenticated borrowed witness.
    pub request_len: u32,
    /// Exact optional `(offset, length)` in the complete family request.
    pub borrowed_witness: Option<(u32, u32)>,
    /// Optional exact resolved prior receipt appended after the request/witness.
    pub receipt_dependency: Option<ShadowReceiptDependencyV3>,
    /// Number of receipts in the exact ordered dependency list.
    pub receipt_dependency_count: u16,
    /// Digest of every resolved dependency in declared append order; zero only
    /// when the count is zero.
    pub receipt_dependencies_digest: [u8; 32],
}

/// Exact prior receipt selected for one resolved Shadow route invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowReceiptDependencyV3 {
    /// Expected release-selected producer role.
    pub producer_role: ShadowRouteRoleV3,
    /// Strictly earlier producer route ordinal.
    pub producer_route: u16,
    /// Producer invocation: zero for fixed/affine, same item for each-item.
    pub producer_invocation: u32,
    /// Exact raw producer return-data width.
    pub expected_receipt_bytes: u16,
}

impl ShadowResolvedRouteV3 {
    fn validate(self) -> Result<()> {
        let item_is_canonical = match self.kind {
            ShadowRouteKindV3::Each => self.item.is_some(),
            ShadowRouteKindV3::Once | ShadowRouteKindV3::AffineOnce => self.item.is_none(),
        };
        if !item_is_canonical {
            return Err(ShadowDigestErrorV3::InvalidRoutePresence);
        }
        let dependency_presence = match self.receipt_dependency_count {
            0 => self.receipt_dependency.is_none() && self.receipt_dependencies_digest == [0; 32],
            1 => self.receipt_dependency.is_some() && self.receipt_dependencies_digest != [0; 32],
            _ => self.receipt_dependency.is_none() && self.receipt_dependencies_digest != [0; 32],
        };
        if !dependency_presence
            || self
                .receipt_dependency
                .is_some_and(|dependency| dependency.expected_receipt_bytes == 0)
        {
            return Err(ShadowDigestErrorV3::InvalidRoutePresence);
        }
        Ok(())
    }
}

/// Digest one exact ordered resolved dependency list for the Shadow effect
/// transcript. Reordering or substituting any role, route, invocation, or
/// width changes the commitment.
pub fn receipt_dependencies_digest_v4(
    dependencies: &[ShadowReceiptDependencyV3],
) -> Result<[u8; 32]> {
    if dependencies.is_empty()
        || dependencies
            .iter()
            .any(|entry| entry.expected_receipt_bytes == 0)
    {
        return Err(ShadowDigestErrorV3::InvalidRoutePresence);
    }
    if dependencies.len() > MAX_RECEIPT_DEPENDENCIES_V4 {
        return Err(ShadowDigestErrorV3::CountOverflow);
    }
    let count =
        u16::try_from(dependencies.len()).map_err(|_| ShadowDigestErrorV3::CountOverflow)?;
    // Every dependency field is a fixed-width scalar, so the whole list is one
    // contiguous run and the preimage is three slices regardless of the count.
    let mut tail = [0_u8; RECEIPT_DEPENDENCY_BYTES_V4 * MAX_RECEIPT_DEPENDENCIES_V4];
    for (index, dependency) in dependencies.iter().enumerate() {
        let mut entry = [0_u8; RECEIPT_DEPENDENCY_BYTES_V4];
        put(&mut entry, 0, &[dependency.producer_role as u8])?;
        put(&mut entry, 1, &dependency.producer_route.to_le_bytes())?;
        put(&mut entry, 3, &dependency.producer_invocation.to_le_bytes())?;
        put(
            &mut entry,
            7,
            &dependency.expected_receipt_bytes.to_le_bytes(),
        )?;
        put(&mut tail, index * RECEIPT_DEPENDENCY_BYTES_V4, &entry)?;
    }
    let written = dependencies.len() * RECEIPT_DEPENDENCY_BYTES_V4;
    let digest = digestv(&[
        RECEIPT_DEPENDENCIES_DIGEST_DOMAIN_V4,
        &[0_u8],
        &count.to_le_bytes(),
        tail.get(..written)
            .ok_or(ShadowDigestErrorV3::CountOverflow)?,
    ]);
    if digest == [0; 32] {
        Err(ShadowDigestErrorV3::ZeroDigest)
    } else {
        Ok(digest)
    }
}

/// Bytes one receipt dependency contributes to the dependency-list preimage.
const RECEIPT_DEPENDENCY_BYTES_V4: usize = 1 + 2 + 4 + 2;

/// Largest ordered receipt-dependency list this transcript will commit to.
///
/// PROVISIONAL, measured-profile. The widest dependency list any shipped
/// artifact declares is two (`SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3`); the
/// decoder's own ceiling is the `u16` the effect artifact carries. This bound
/// exists because the runtime hashes a slice list rather than a stream, so the
/// preimage needs a declared width. Lifting plan: it rises with the widest
/// shipped artifact, and the arithmetic is nine bytes per dependency -- raising
/// it to 512 would still be under five kilobytes.
pub const MAX_RECEIPT_DEPENDENCIES_V4: usize = 32;

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(bytes.len())
                    .ok_or(ShadowDigestErrorV3::CountOverflow)?,
        )
        .ok_or(ShadowDigestErrorV3::CountOverflow)?
        .copy_from_slice(bytes);
    Ok(())
}

/// Complete interpreted effect projection before physical CPI or mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowEffectProjectionV3<'a> {
    /// Product-authoritative runtime tail count.
    pub tail_count: u32,
    /// Candidate lamports in AccountProfile logical order.
    pub output_lamports: &'a [u64],
    /// Exact projected request bank.
    pub request_bank: &'a [u8],
    /// Enabled, resolved child routes in canonical route/invocation order.
    pub routes: &'a [ShadowResolvedRouteV3],
}

/// Coordinates selecting one exact top-level invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowInvocationContextV3 {
    /// Current immutable release set.
    pub release_set: ContentId,
    /// Current logical Market.
    pub market: ContentId,
    /// Current Trading-owned root account.
    pub root: ContentId,
    /// Action-selected CapabilityProgramV3 content identity.
    pub capability_program: ContentId,
    /// Selected action value from CapabilityProgramSetV1.
    pub selected_action: u32,
    /// Digest of the exact complete family request.
    pub family_request_digest: ContentId,
    /// Digest of the exact root prestate.
    pub root_prestate_digest: ContentId,
}

/// Digest the exact complete family request.
///
/// The request bytes are the largest thing on this path and they are borrowed,
/// not copied: the preimage is four slices over memory that already exists.
pub fn family_request_digest_v3(bytes: &[u8]) -> Result<ContentId> {
    let length = u32::try_from(bytes.len()).map_err(|_| ShadowDigestErrorV3::CountOverflow)?;
    ContentId::new(digestv(&[
        FAMILY_REQUEST_DIGEST_DOMAIN_V3,
        &[0_u8],
        &length.to_le_bytes(),
        bytes,
    ]))
    .map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

// ---------------------------------------------------------------------------
// The one-shot preimage builders.
//
// SHA-256 is a pure function of its preimage, but WHICH implementation runs is
// worth ~104.75 CU per byte against ~0.5 for the runtime's `sol_sha256`
// syscall. The syscall is one-shot over a slice list -- there is no resumable
// state, and `solana_sha256_hasher::Hasher` is software even on chain -- so a
// streaming caller has to restate its preimage as the slice list it always
// was. For these three the slice list is not a constant: it grows with the
// observation count, the register widths, and the route count, and the widest
// admissible shapes (256 runtime accounts, 512 scalars, route_count x
// tail_count routes) do not fit an SBF 4,096-byte frame.
//
// So the scratch is the CALLER'S. That is the whole API change: the caller
// sizes two buffers from the counts it already holds, and on the hot path it
// takes them from the scratch region it already opens for this phase. The
// preimages below are byte-for-byte what the streaming versions absorbed --
// every digest in the tree is unchanged -- and nothing here allocates.
// ---------------------------------------------------------------------------

/// Refusal added by the one-shot builders.
///
/// Kept out of [`ShadowDigestErrorV3`]'s existing variants because it is a
/// caller-side sizing mistake, not a malformed transcript.
const fn scratch_error() -> ShadowDigestErrorV3 {
    ShadowDigestErrorV3::CountOverflow
}

/// Write `bytes` at `at`, returning the next offset.
fn absorb(buffer: &mut [u8], at: usize, bytes: &[u8]) -> Result<usize> {
    let end = at.checked_add(bytes.len()).ok_or_else(scratch_error)?;
    buffer
        .get_mut(at..end)
        .ok_or_else(scratch_error)?
        .copy_from_slice(bytes);
    Ok(end)
}

/// Append one slice to the caller's list, returning the next index.
fn push_slice<'p>(slices: &mut [&'p [u8]], at: usize, slice: &'p [u8]) -> Result<usize> {
    *slices.get_mut(at).ok_or_else(scratch_error)? = slice;
    at.checked_add(1).ok_or_else(scratch_error)
}

/// Borrow `[at, at + len)` of an already-filled scratch buffer.
fn scratch_span(buffer: &[u8], at: usize, len: usize) -> Result<&[u8]> {
    let end = at.checked_add(len).ok_or_else(scratch_error)?;
    buffer.get(at..end).ok_or_else(scratch_error)
}

fn count_bytes(count: usize) -> Result<[u8; 4]> {
    Ok(u32::try_from(count)
        .map_err(|_| ShadowDigestErrorV3::CountOverflow)?
        .to_le_bytes())
}

/// Fixed scratch bytes one runtime observation contributes.
///
/// `lamports || signer || writable || executable || 0 || data_len`, which is
/// exactly the run of scalars between the observation's two borrowed 32-byte
/// identities and its borrowed data.
pub const RUNTIME_OBSERVATION_SCALAR_BYTES_V3: usize = 16;

/// Scratch bytes [`runtime_observations_digest_in_v3`] needs for `count`.
#[must_use]
pub const fn runtime_observations_scratch_bytes_v3(count: usize) -> usize {
    // The domain's trailing zero and the observation count, then one scalar
    // run per observation.
    5 + count * RUNTIME_OBSERVATION_SCALAR_BYTES_V3
}

/// Slice-list entries [`runtime_observations_digest_in_v3`] needs for `count`.
#[must_use]
pub const fn runtime_observations_scratch_slices_v3(count: usize) -> usize {
    // domain, header, then key/owner/scalars/data per observation.
    2 + count * 4
}

/// Digest exact runtime observations in AccountProfile logical order.
///
/// Byte-for-byte the preimage the streaming version absorbed. `scratch` and
/// `slices` must be at least [`runtime_observations_scratch_bytes_v3`] and
/// [`runtime_observations_scratch_slices_v3`] of `observations.len()`.
#[inline(never)]
pub fn runtime_observations_digest_in_v3<'p>(
    observations: &'p [ShadowRuntimeObservationV3<'p>],
    scratch: &'p mut [u8],
    slices: &mut [&'p [u8]],
) -> Result<ContentId> {
    borrowed_runtime_observations_digest_in_v3(
        observations
            .iter()
            .map(|observation| BorrowedRuntimeObservationV3 {
                key: &observation.key,
                owner: &observation.owner,
                lamports: observation.lamports,
                data: observation.data,
                signer: observation.signer,
                writable: observation.writable,
                executable: observation.executable,
            }),
        scratch,
        slices,
    )
}

/// Digest exact runtime observations without materialising a bank of them.
///
/// The same preimage as [`runtime_observations_digest_in_v3`], byte for byte,
/// over observations produced one at a time. The iterator is walked TWICE --
/// once to absorb each observation's scalar run into `scratch`, once to push
/// the slice list -- so it is `Clone` rather than consumed, which a
/// `zip`/`map` over the caller's own slices satisfies for free and no
/// allocation satisfies at all.
///
/// `scratch` and `slices` must be at least
/// [`runtime_observations_scratch_bytes_v3`] and
/// [`runtime_observations_scratch_slices_v3`] of the observation count.
#[inline(never)]
pub fn borrowed_runtime_observations_digest_in_v3<'p, I>(
    observations: I,
    scratch: &'p mut [u8],
    slices: &mut [&'p [u8]],
) -> Result<ContentId>
where
    I: ExactSizeIterator<Item = BorrowedRuntimeObservationV3<'p>> + Clone,
{
    let mut at = absorb(scratch, 0, &[0_u8])?;
    at = absorb(scratch, at, &count_bytes(observations.len())?)?;
    let header_len = at;
    for observation in observations.clone() {
        if observation.signer || observation.writable {
            return Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation);
        }
        at = absorb(scratch, at, &observation.lamports.to_le_bytes())?;
        at = absorb(
            scratch,
            at,
            &[
                u8::from(observation.signer),
                u8::from(observation.writable),
                u8::from(observation.executable),
                0_u8,
            ],
        )?;
        at = absorb(scratch, at, &count_bytes(observation.data.len())?)?;
    }
    // The mutable borrow ends here: `scratch` becomes the shared borrow the
    // slice list needs, at the same lifetime, so the list can name spans of it.
    let scratch: &'p [u8] = scratch;
    let mut next = push_slice(slices, 0, RUNTIME_OBSERVATION_DIGEST_DOMAIN_V3)?;
    next = push_slice(slices, next, scratch_span(scratch, 0, header_len)?)?;
    let mut cursor = header_len;
    for observation in observations {
        next = push_slice(slices, next, observation.key.as_slice())?;
        next = push_slice(slices, next, observation.owner.as_slice())?;
        next = push_slice(
            slices,
            next,
            scratch_span(scratch, cursor, RUNTIME_OBSERVATION_SCALAR_BYTES_V3)?,
        )?;
        if !observation.data.is_empty() {
            next = push_slice(slices, next, observation.data)?;
        }
        cursor = cursor
            .checked_add(RUNTIME_OBSERVATION_SCALAR_BYTES_V3)
            .ok_or_else(scratch_error)?;
    }
    let preimage = slices.get(..next).ok_or_else(scratch_error)?;
    ContentId::new(digestv(preimage)).map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

/// Scratch bytes [`candidate_digest_in_v3`] needs for `scalars` of that width.
#[must_use]
pub const fn candidate_scratch_bytes_v3(scalar_count: usize) -> usize {
    // zero, tail_count, scalar count, identity count, then the scalar run.
    13 + scalar_count * 8
}

/// Slice-list entries [`candidate_digest_in_v3`] needs for that identity width.
#[must_use]
pub const fn candidate_scratch_slices_v3(identity_count: usize) -> usize {
    // domain, the contiguous header-and-scalars run, then one per identity.
    2 + identity_count
}

/// Digest one complete interpreted scalar/identity candidate bank.
#[inline(never)]
pub fn candidate_digest_in_v3<'p>(
    tail_count: u32,
    scalars: &[u64],
    identities: &'p [[u8; 32]],
    scratch: &'p mut [u8],
    slices: &mut [&'p [u8]],
) -> Result<ContentId> {
    let mut at = absorb(scratch, 0, &[0_u8])?;
    at = absorb(scratch, at, &tail_count.to_le_bytes())?;
    at = absorb(scratch, at, &count_bytes(scalars.len())?)?;
    at = absorb(scratch, at, &count_bytes(identities.len())?)?;
    for scalar in scalars {
        at = absorb(scratch, at, &scalar.to_le_bytes())?;
    }
    let scratch: &'p [u8] = scratch;
    let mut next = push_slice(slices, 0, CANDIDATE_DIGEST_DOMAIN_V3)?;
    // The header and the whole scalar run are contiguous, so they are ONE
    // slice: the syscall charges per slice as well as per byte.
    next = push_slice(slices, next, scratch_span(scratch, 0, at)?)?;
    for identity in identities {
        next = push_slice(slices, next, identity.as_slice())?;
    }
    let preimage = slices.get(..next).ok_or_else(scratch_error)?;
    ContentId::new(digestv(preimage)).map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

/// Scratch bytes one resolved route contributes to the effect preimage.
pub const EFFECT_ROUTE_SCRATCH_BYTES_V3: usize = 84;

/// Scratch bytes [`effect_digest_in_v3`] needs for those widths.
#[must_use]
pub const fn effect_scratch_bytes_v3(lamport_count: usize, route_count: usize) -> usize {
    // zero, tail_count, lamport count, the lamport run, the request length;
    // then the route count and one fixed run per route.
    13 + lamport_count * 8 + 4 + route_count * EFFECT_ROUTE_SCRATCH_BYTES_V3
}

/// Slice-list entries [`effect_digest_in_v3`] always needs.
///
/// Four, whatever the widths: the domain, the header-and-lamports run, the
/// borrowed request bank, and the count-and-routes run.
pub const EFFECT_SCRATCH_SLICES_V3: usize = 4;

/// Digest one complete interpreted effect projection before physical writes.
#[inline(never)]
pub fn effect_digest_in_v3<'p>(
    projection: ShadowEffectProjectionV3<'p>,
    scratch: &'p mut [u8],
    slices: &mut [&'p [u8]],
) -> Result<ContentId> {
    let mut at = absorb(scratch, 0, &[0_u8])?;
    at = absorb(scratch, at, &projection.tail_count.to_le_bytes())?;
    at = absorb(scratch, at, &count_bytes(projection.output_lamports.len())?)?;
    for lamports in projection.output_lamports {
        at = absorb(scratch, at, &lamports.to_le_bytes())?;
    }
    at = absorb(scratch, at, &count_bytes(projection.request_bank.len())?)?;
    let header_len = at;
    at = absorb(scratch, at, &count_bytes(projection.routes.len())?)?;
    let routes_at = header_len;
    for route in projection.routes {
        route.validate()?;
        at = absorb(scratch, at, &[route.role as u8, route.kind as u8])?;
        match route.item {
            Some(item) => {
                at = absorb(scratch, at, &[1_u8])?;
                at = absorb(scratch, at, &item.to_le_bytes())?;
            }
            None => at = absorb(scratch, at, &[0_u8; 5])?,
        }
        at = absorb(scratch, at, &route.fixed_account_start.to_le_bytes())?;
        at = absorb(scratch, at, &route.fixed_account_count.to_le_bytes())?;
        at = absorb(scratch, at, &route.item_account_start.to_le_bytes())?;
        at = absorb(scratch, at, &route.item_account_count.to_le_bytes())?;
        at = absorb(scratch, at, &route.item_account_stride.to_le_bytes())?;
        at = absorb(scratch, at, &route.repeated_item_count.to_le_bytes())?;
        at = absorb(scratch, at, &route.request_offset.to_le_bytes())?;
        at = absorb(scratch, at, &route.request_len.to_le_bytes())?;
        match route.borrowed_witness {
            Some((offset, len)) => {
                at = absorb(scratch, at, &[1_u8])?;
                at = absorb(scratch, at, &offset.to_le_bytes())?;
                at = absorb(scratch, at, &len.to_le_bytes())?;
            }
            None => at = absorb(scratch, at, &[0_u8; 9])?,
        }
        match route.receipt_dependency {
            Some(dependency) => {
                at = absorb(scratch, at, &[1_u8, dependency.producer_role as u8])?;
                at = absorb(scratch, at, &dependency.producer_route.to_le_bytes())?;
                at = absorb(scratch, at, &dependency.producer_invocation.to_le_bytes())?;
                at = absorb(
                    scratch,
                    at,
                    &dependency.expected_receipt_bytes.to_le_bytes(),
                )?;
            }
            None => at = absorb(scratch, at, &[0_u8; 10])?,
        }
        at = absorb(scratch, at, &route.receipt_dependency_count.to_le_bytes())?;
        at = absorb(scratch, at, &route.receipt_dependencies_digest)?;
    }
    let routes_len = at.checked_sub(routes_at).ok_or_else(scratch_error)?;
    let scratch: &'p [u8] = scratch;
    let mut next = push_slice(slices, 0, EFFECT_DIGEST_DOMAIN_V3)?;
    next = push_slice(slices, next, scratch_span(scratch, 0, header_len)?)?;
    if !projection.request_bank.is_empty() {
        next = push_slice(slices, next, projection.request_bank)?;
    }
    next = push_slice(slices, next, scratch_span(scratch, routes_at, routes_len)?)?;
    let preimage = slices.get(..next).ok_or_else(scratch_error)?;
    ContentId::new(digestv(preimage)).map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

/// Digest exact runtime observations in AccountProfile logical order.
///
/// The allocating convenience form of [`runtime_observations_digest_in_v3`],
/// for callers with no scratch discipline of their own -- host emitters, the
/// accelerator boundaries, and the Series shadow evaluator.
///
/// **`trading-sbf` still takes this form at four of its five call sites, and
/// that is debt with a name, not a design.** An on-chain caller counting heap
/// should size the two buffers from counts it already holds. The fifth,
/// `hot_v3::runtime_transcript_digest_v3`, was migrated on 2026-09-02 to
/// [`borrowed_runtime_observations_digest_in_v3`] because the Dealer
/// post-trade partial equity Remove reached it 136 bytes over a 65,536-byte
/// grant -- so the earlier reading that "moving them buys no measured heap
/// today" was true only of the canonical Interpreted Direct bundle, which
/// takes none of these paths at all.
///
/// What that migration did NOT do is take the two buffers off the upward end.
/// It removed the bank of OWNED observations (7,104 bytes on that route); the
/// scratch run and the slice list are still `Vec`s, still dead the instant the
/// digest returns, and still charged for the rest of the invocation --
/// 5,960 bytes on the same route, named here as what is left. They cannot go
/// in the phase's scratch region as this doc once suggested: on that route the
/// region is already open and holds the observation bank, so moving bytes from
/// one end of one heap to the other moves the peak nowhere. Releasing them
/// needs a scratch end that can nest, which `HeapScratchRegionV1` refuses on
/// purpose.
#[cfg(feature = "alloc")]
#[inline(never)]
pub fn runtime_observations_digest_v3(
    observations: &[ShadowRuntimeObservationV3<'_>],
) -> Result<ContentId> {
    let mut scratch = alloc::vec![0_u8; runtime_observations_scratch_bytes_v3(observations.len())];
    let mut slices = alloc::vec![
        [].as_slice();
        runtime_observations_scratch_slices_v3(observations.len())
    ];
    runtime_observations_digest_in_v3(observations, &mut scratch, &mut slices)
}

/// Digest one complete interpreted scalar/identity candidate bank.
///
/// The allocating convenience form of [`candidate_digest_in_v3`].
#[cfg(feature = "alloc")]
#[inline(never)]
pub fn candidate_digest_v3(
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<ContentId> {
    let mut scratch = alloc::vec![0_u8; candidate_scratch_bytes_v3(scalars.len())];
    let mut slices = alloc::vec![[].as_slice(); candidate_scratch_slices_v3(identities.len())];
    candidate_digest_in_v3(tail_count, scalars, identities, &mut scratch, &mut slices)
}

/// Digest one complete interpreted effect projection before physical writes.
///
/// The allocating convenience form of [`effect_digest_in_v3`].
#[cfg(feature = "alloc")]
#[inline(never)]
pub fn effect_digest_v3(projection: ShadowEffectProjectionV3<'_>) -> Result<ContentId> {
    let mut scratch = alloc::vec![
        0_u8;
        effect_scratch_bytes_v3(projection.output_lamports.len(), projection.routes.len())
    ];
    let mut slices = alloc::vec![[].as_slice(); EFFECT_SCRATCH_SLICES_V3];
    effect_digest_in_v3(projection, &mut scratch, &mut slices)
}

/// Digest one action-selected invocation context.
///
/// Every field is fixed-width and every identity is borrowed where it sits.
pub fn invocation_context_digest_v3(context: ShadowInvocationContextV3) -> Result<ContentId> {
    let selected_action = context.selected_action.to_le_bytes();
    ContentId::new(digestv(&[
        INVOCATION_DIGEST_DOMAIN_V3,
        &[0_u8],
        context.release_set.as_bytes(),
        context.market.as_bytes(),
        context.root.as_bytes(),
        context.capability_program.as_bytes(),
        &selected_action,
        context.family_request_digest.as_bytes(),
        context.root_prestate_digest.as_bytes(),
    ]))
    .map_err(|_| ShadowDigestErrorV3::ZeroDigest)
}

#[cfg(test)]
mod tests {
    /// The caller-authority seed separates dispositions, requests and
    /// invocations, and nothing else.
    ///
    /// Its whole purpose is that the coordinates it DOES vary with are ones a
    /// caller holds before it signs, so the negative half — that it varies with
    /// neither the request bytes nor the register bank, because it never sees
    /// them — is a statement about the signature, not this function. What is
    /// checkable here is the positive half plus the domain: an undomained
    /// preimage would collide with any other 37-byte protocol preimage of that
    /// shape.
    #[test]
    fn the_caller_authority_seed_separates_disposition_request_and_invocation() {
        let parent = ContentId::new([41_u8; 32]).expect("parent");
        let first =
            accelerator_caller_authority_digest_v1(AcceleratorCallerKindV1::Admitted, parent, 0)
                .expect("chunk 0");
        let second =
            accelerator_caller_authority_digest_v1(AcceleratorCallerKindV1::Admitted, parent, 1)
                .expect("chunk 1");
        assert_ne!(first, second);
        assert_ne!(
            accelerator_caller_authority_digest_v1(
                AcceleratorCallerKindV1::Admitted,
                ContentId::new([42_u8; 32]).expect("other parent"),
                0,
            )
            .expect("other request"),
            first
        );
        assert_ne!(
            accelerator_caller_authority_digest_v1(AcceleratorCallerKindV1::Shadow, parent, 0)
                .expect("shadow"),
            first,
            "the two dispositions must not mint one address"
        );
        // The little-endian ordinal, not the big-endian one: a mismatch there
        // is an address neither side can name and a wall with no diagnostic.
        assert_eq!(
            first,
            ContentId::new(digestv(&[
                ACCELERATOR_CALLER_AUTHORITY_DIGEST_DOMAIN_V1,
                &[0_u8],
                &[41_u8; 32],
                &0_u32.to_le_bytes(),
            ]))
            .expect("preimage")
        );
        assert_ne!(
            second,
            ContentId::new(digestv(&[
                ACCELERATOR_CALLER_AUTHORITY_DIGEST_DOMAIN_V1,
                &[0_u8],
                &[41_u8; 32],
                &1_u32.to_be_bytes(),
            ]))
            .expect("big-endian ordinal")
        );
        assert_ne!(
            first,
            ContentId::new(digestv(&[&[0_u8], &[41_u8; 32], &0_u32.to_le_bytes()]))
                .expect("undomained")
        );
    }

    extern crate std;

    use super::*;

    extern crate alloc;

    use alloc::vec;
    use alloc::vec::Vec;

    use sha2::{Digest, Sha256};

    // ---- THE ORACLE -------------------------------------------------------
    // The streaming implementation, verbatim as it stood before the one-shot
    // builders replaced it, over a SOFTWARE SHA-256 that no shipped ELF links
    // any more. It exists so the differential test below compares two
    // independent constructions of the preimage. Comparing a one-shot builder
    // to its own allocating wrapper would pass no matter what either did.

    fn oracle_begin(domain: &[u8]) -> Sha256 {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update([0_u8]);
        hasher
    }

    fn oracle_count(hasher: &mut Sha256, count: usize) -> Result<()> {
        let count = u32::try_from(count).map_err(|_| ShadowDigestErrorV3::CountOverflow)?;
        hasher.update(count.to_le_bytes());
        Ok(())
    }

    fn oracle_bytes(hasher: &mut Sha256, bytes: &[u8]) -> Result<()> {
        oracle_count(hasher, bytes.len())?;
        hasher.update(bytes);
        Ok(())
    }

    fn oracle_finish(hasher: Sha256) -> Result<ContentId> {
        ContentId::new(hasher.finalize().into()).map_err(|_| ShadowDigestErrorV3::ZeroDigest)
    }

    fn oracle_runtime_observations(
        observations: &[ShadowRuntimeObservationV3<'_>],
    ) -> Result<ContentId> {
        let mut hasher = oracle_begin(RUNTIME_OBSERVATION_DIGEST_DOMAIN_V3);
        oracle_count(&mut hasher, observations.len())?;
        for observation in observations {
            if observation.signer || observation.writable {
                return Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation);
            }
            hasher.update(observation.key);
            hasher.update(observation.owner);
            hasher.update(observation.lamports.to_le_bytes());
            hasher.update([u8::from(observation.signer)]);
            hasher.update([u8::from(observation.writable)]);
            hasher.update([u8::from(observation.executable)]);
            hasher.update([0_u8]);
            oracle_bytes(&mut hasher, observation.data)?;
        }
        oracle_finish(hasher)
    }

    fn oracle_candidate(
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ContentId> {
        let mut hasher = oracle_begin(CANDIDATE_DIGEST_DOMAIN_V3);
        hasher.update(tail_count.to_le_bytes());
        oracle_count(&mut hasher, scalars.len())?;
        oracle_count(&mut hasher, identities.len())?;
        for scalar in scalars {
            hasher.update(scalar.to_le_bytes());
        }
        for identity in identities {
            hasher.update(identity);
        }
        oracle_finish(hasher)
    }

    fn oracle_effect(projection: ShadowEffectProjectionV3<'_>) -> Result<ContentId> {
        let mut hasher = oracle_begin(EFFECT_DIGEST_DOMAIN_V3);
        hasher.update(projection.tail_count.to_le_bytes());
        oracle_count(&mut hasher, projection.output_lamports.len())?;
        for lamports in projection.output_lamports {
            hasher.update(lamports.to_le_bytes());
        }
        oracle_bytes(&mut hasher, projection.request_bank)?;
        oracle_count(&mut hasher, projection.routes.len())?;
        for route in projection.routes {
            route.validate()?;
            hasher.update([route.role as u8]);
            hasher.update([route.kind as u8]);
            match route.item {
                Some(item) => {
                    hasher.update([1_u8]);
                    hasher.update(item.to_le_bytes());
                }
                None => hasher.update([0_u8; 5]),
            }
            hasher.update(route.fixed_account_start.to_le_bytes());
            hasher.update(route.fixed_account_count.to_le_bytes());
            hasher.update(route.item_account_start.to_le_bytes());
            hasher.update(route.item_account_count.to_le_bytes());
            hasher.update(route.item_account_stride.to_le_bytes());
            hasher.update(route.repeated_item_count.to_le_bytes());
            hasher.update(route.request_offset.to_le_bytes());
            hasher.update(route.request_len.to_le_bytes());
            match route.borrowed_witness {
                Some((offset, len)) => {
                    hasher.update([1_u8]);
                    hasher.update(offset.to_le_bytes());
                    hasher.update(len.to_le_bytes());
                }
                None => hasher.update([0_u8; 9]),
            }
            match route.receipt_dependency {
                Some(dependency) => {
                    hasher.update([1_u8]);
                    hasher.update([dependency.producer_role as u8]);
                    hasher.update(dependency.producer_route.to_le_bytes());
                    hasher.update(dependency.producer_invocation.to_le_bytes());
                    hasher.update(dependency.expected_receipt_bytes.to_le_bytes());
                }
                None => hasher.update([0_u8; 10]),
            }
            hasher.update(route.receipt_dependency_count.to_le_bytes());
            hasher.update(route.receipt_dependencies_digest);
        }
        oracle_finish(hasher)
    }
    // ---- end of the oracle ------------------------------------------------

    /// The one-shot builders must reproduce the streaming preimage EXACTLY.
    ///
    /// This is the only control that matters for the conversion: every digest
    /// in this tree that was ever committed by a host emitter, stored in a
    /// record, or compared by an accelerator was produced by the streaming
    /// versions. A one-shot builder that drew its slice boundaries correctly
    /// but mis-ordered one scalar would still be a perfectly good hash of a
    /// different preimage, and nothing else in the tree would notice until a
    /// stored transcript stopped matching.
    #[test]
    fn one_shot_builders_reproduce_the_streaming_digests() {
        let data_a = [7_u8; 40];
        let data_b: [u8; 0] = [];
        let data_c = [9_u8; 3];
        for observations in [
            Vec::new(),
            vec![ShadowRuntimeObservationV3 {
                key: [1; 32],
                owner: [2; 32],
                lamports: 0,
                data: &data_b,
                signer: false,
                writable: false,
                executable: false,
            }],
            vec![
                ShadowRuntimeObservationV3 {
                    key: [3; 32],
                    owner: [4; 32],
                    lamports: u64::MAX,
                    data: &data_a,
                    signer: false,
                    writable: false,
                    executable: true,
                },
                ShadowRuntimeObservationV3 {
                    key: [5; 32],
                    owner: [6; 32],
                    lamports: 1,
                    data: &data_c,
                    signer: false,
                    writable: false,
                    executable: false,
                },
                ShadowRuntimeObservationV3 {
                    key: [0; 32],
                    owner: [0; 32],
                    lamports: 0,
                    data: &data_b,
                    signer: false,
                    writable: false,
                    executable: false,
                },
            ],
        ] {
            let mut scratch = vec![0_u8; runtime_observations_scratch_bytes_v3(observations.len())];
            let mut slices =
                vec![[].as_slice(); runtime_observations_scratch_slices_v3(observations.len())];
            assert_eq!(
                runtime_observations_digest_in_v3(&observations, &mut scratch, &mut slices)
                    .expect("one-shot runtime observations"),
                oracle_runtime_observations(&observations).expect("oracle"),
                "runtime observation preimage diverged at {} observations",
                observations.len()
            );
        }

        for (tail, scalars, identities) in [
            (0_u32, Vec::new(), Vec::new()),
            (3, vec![0_u64, u64::MAX, 7], vec![[1_u8; 32]]),
            (
                u32::MAX,
                vec![1_u64; 9],
                vec![[0_u8; 32], [255_u8; 32], [17_u8; 32]],
            ),
        ] {
            let mut scratch = vec![0_u8; candidate_scratch_bytes_v3(scalars.len())];
            let mut slices = vec![[].as_slice(); candidate_scratch_slices_v3(identities.len())];
            assert_eq!(
                candidate_digest_in_v3(tail, &scalars, &identities, &mut scratch, &mut slices)
                    .expect("one-shot candidate"),
                oracle_candidate(tail, &scalars, &identities).expect("oracle"),
                "candidate preimage diverged"
            );
        }

        let bank = [11_u8; 71];
        let route_present = ShadowResolvedRouteV3 {
            role: ShadowRouteRoleV3::Claims,
            kind: ShadowRouteKindV3::Each,
            item: Some(4),
            fixed_account_start: 1,
            fixed_account_count: 2,
            item_account_start: 3,
            item_account_count: 4,
            item_account_stride: 5,
            repeated_item_count: 1,
            request_offset: 7,
            request_len: 8,
            borrowed_witness: Some((9, 10)),
            receipt_dependency: Some(ShadowReceiptDependencyV3 {
                producer_role: ShadowRouteRoleV3::Core,
                producer_route: 11,
                producer_invocation: 12,
                expected_receipt_bytes: 13,
            }),
            receipt_dependency_count: 1,
            receipt_dependencies_digest: [14; 32],
        };
        let route_absent = ShadowResolvedRouteV3 {
            role: ShadowRouteRoleV3::Custody,
            kind: ShadowRouteKindV3::Once,
            item: None,
            repeated_item_count: 0,
            item_account_count: 0,
            borrowed_witness: None,
            receipt_dependency: None,
            receipt_dependency_count: 0,
            receipt_dependencies_digest: [0; 32],
            ..route_present
        };
        let lamports = [1_u64, 0, u64::MAX];
        for (lamports, bank, routes) in [
            (&lamports[..0], &bank[..0], &[][..]),
            (&lamports[..], &bank[..], &[route_present][..]),
            (
                &lamports[..1],
                &bank[..5],
                &[route_absent, route_present, route_absent][..],
            ),
        ] {
            let projection = ShadowEffectProjectionV3 {
                tail_count: 2,
                output_lamports: lamports,
                request_bank: bank,
                routes,
            };
            let mut scratch = vec![0_u8; effect_scratch_bytes_v3(lamports.len(), routes.len())];
            let mut slices = vec![[].as_slice(); EFFECT_SCRATCH_SLICES_V3];
            assert_eq!(
                effect_digest_in_v3(projection, &mut scratch, &mut slices)
                    .expect("one-shot effect"),
                oracle_effect(projection).expect("oracle"),
                "effect preimage diverged over {} routes",
                routes.len()
            );
        }
    }

    /// The route scratch constant is the encoder's own width, not a guess.
    #[test]
    fn the_effect_route_scratch_width_is_what_the_encoder_writes() {
        let route = ShadowResolvedRouteV3 {
            role: ShadowRouteRoleV3::Claims,
            kind: ShadowRouteKindV3::Once,
            item: None,
            fixed_account_start: 0,
            fixed_account_count: 0,
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: 0,
            request_len: 0,
            borrowed_witness: None,
            receipt_dependency: None,
            receipt_dependency_count: 0,
            receipt_dependencies_digest: [0; 32],
        };
        let projection = ShadowEffectProjectionV3 {
            tail_count: 0,
            output_lamports: &[],
            request_bank: &[],
            routes: &[route],
        };
        // Exactly the computed size: one byte short must refuse.
        let exact = effect_scratch_bytes_v3(0, 1);
        let mut slices = vec![[].as_slice(); EFFECT_SCRATCH_SLICES_V3];
        let mut scratch = vec![0_u8; exact];
        assert!(effect_digest_in_v3(projection, &mut scratch, &mut slices).is_ok());
        let mut short = vec![0_u8; exact - 1];
        assert_eq!(
            effect_digest_in_v3(projection, &mut short, &mut slices),
            Err(ShadowDigestErrorV3::CountOverflow)
        );
    }

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero")
    }

    fn observation<'a>(key: u8, data: &'a [u8]) -> ShadowRuntimeObservationV3<'a> {
        ShadowRuntimeObservationV3 {
            key: [key; 32],
            owner: [9; 32],
            lamports: 10,
            data,
            signer: false,
            writable: false,
            executable: false,
        }
    }

    fn route() -> ShadowResolvedRouteV3 {
        ShadowResolvedRouteV3 {
            role: ShadowRouteRoleV3::Custody,
            kind: ShadowRouteKindV3::Each,
            item: Some(2),
            fixed_account_start: 4,
            fixed_account_count: 5,
            item_account_start: 6,
            item_account_count: 7,
            item_account_stride: 8,
            repeated_item_count: 1,
            request_offset: 9,
            request_len: 10,
            borrowed_witness: Some((11, 12)),
            receipt_dependency: Some(ShadowReceiptDependencyV3 {
                producer_role: ShadowRouteRoleV3::Claims,
                producer_route: 0,
                producer_invocation: 2,
                expected_receipt_bytes: 384,
            }),
            receipt_dependency_count: 1,
            receipt_dependencies_digest: receipt_dependencies_digest_v4(&[
                ShadowReceiptDependencyV3 {
                    producer_role: ShadowRouteRoleV3::Claims,
                    producer_route: 0,
                    producer_invocation: 2,
                    expected_receipt_bytes: 384,
                },
            ])
            .expect("dependency digest"),
        }
    }

    #[test]
    fn domains_and_runtime_order_are_distinct() {
        let first = observation(1, b"first");
        let second = observation(2, b"second");
        let ordered = runtime_observations_digest_v3(&[first, second]).expect("runtime");
        let swapped = runtime_observations_digest_v3(&[second, first]).expect("runtime");
        assert_ne!(ordered, swapped);
        assert_ne!(
            ordered,
            family_request_digest_v3(b"firstsecond").expect("family")
        );
        let substituted = observation(1, b"First");
        assert_ne!(
            runtime_observations_digest_v3(&[first]).expect("runtime"),
            runtime_observations_digest_v3(&[substituted]).expect("runtime")
        );
    }

    #[test]
    fn runtime_transcript_refuses_any_forwarded_privilege() {
        let mut privileged = observation(1, b"state");
        privileged.signer = true;
        assert_eq!(
            runtime_observations_digest_v3(&[privileged]),
            Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation)
        );
        privileged.signer = false;
        privileged.writable = true;
        assert_eq!(
            runtime_observations_digest_v3(&[privileged]),
            Err(ShadowDigestErrorV3::PrivilegedRuntimeObservation)
        );
    }

    #[test]
    fn candidate_binds_dimensions_order_and_tail() {
        let canonical = candidate_digest_v3(3, &[1, 2], &[[3; 32]]).expect("candidate");
        assert_ne!(
            canonical,
            candidate_digest_v3(4, &[1, 2], &[[3; 32]]).expect("candidate")
        );
        assert_ne!(
            canonical,
            candidate_digest_v3(3, &[2, 1], &[[3; 32]]).expect("candidate")
        );
    }

    #[test]
    fn effect_binds_banks_routes_and_presence() {
        let canonical = effect_digest_v3(ShadowEffectProjectionV3 {
            tail_count: 3,
            output_lamports: &[4, 5],
            request_bank: b"request",
            routes: &[route()],
        })
        .expect("effect");
        let mut changed = route();
        changed.request_len = 11;
        assert_ne!(
            canonical,
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[changed],
            })
            .expect("effect")
        );
        let mut changed_dependency = route();
        changed_dependency
            .receipt_dependency
            .as_mut()
            .expect("dependency")
            .producer_invocation = 1;
        assert_ne!(
            canonical,
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[changed_dependency],
            })
            .expect("changed dependency")
        );
        let mut zero_width = route();
        zero_width
            .receipt_dependency
            .as_mut()
            .expect("dependency")
            .expected_receipt_bytes = 0;
        assert_eq!(
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[zero_width],
            }),
            Err(ShadowDigestErrorV3::InvalidRoutePresence)
        );
        changed.kind = ShadowRouteKindV3::Once;
        assert_eq!(
            effect_digest_v3(ShadowEffectProjectionV3 {
                tail_count: 3,
                output_lamports: &[4, 5],
                request_bank: b"request",
                routes: &[changed],
            }),
            Err(ShadowDigestErrorV3::InvalidRoutePresence)
        );
    }

    #[test]
    fn invocation_binds_action_request_and_prestate() {
        let canonical = ShadowInvocationContextV3 {
            release_set: id(1),
            market: id(2),
            root: id(3),
            capability_program: id(4),
            selected_action: 5,
            family_request_digest: id(6),
            root_prestate_digest: id(7),
        };
        let first = invocation_context_digest_v3(canonical).expect("invocation");
        let mut changed = canonical;
        changed.selected_action = 6;
        assert_ne!(
            first,
            invocation_context_digest_v3(changed).expect("invocation")
        );
    }
}
