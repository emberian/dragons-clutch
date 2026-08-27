//! Onchain-safe Fractional V2 execution candidate for common Trading Hot.

use dclutch_claims_svm::{
    CallerRole,
    frame_spec_v1::SignedDeltaFrameSpecV3,
    signed_delta_v3::{
        DeltaDirectionV3, SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3, SignedDeltaPlanV3,
        SignedDeltaReceiptCommitmentV3, SignedDeltaReceiptV3,
    },
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TerminalSettlementReceiptV3,
        TerminalSettlementRequestV3,
    },
};
use dclutch_fractional_claim_kernel::{FractionalExposureTermsV2, divide_exposure_shards_v2};
use sha2::{Digest, Sha256};

use crate::{
    FRACTIONAL_ROOT_BYTES_V1, FractionalExposureActionV2, FractionalExposureRequestV2,
    FractionalRootInputV1, FractionalRootV1,
};

/// Measured/account-profile bound inherited from Fractional V2 terms.
pub const FRACTIONAL_HOT_MAX_TOKEN_EFFECTS_V2: usize = 256;
/// Canonical absent Claims route width.
pub const FRACTIONAL_HOT_NO_ROUTE_ACCOUNTS_V2: u16 = 0;

/// Stable refusal from V2 candidate preparation or postcondition validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalHotErrorV2 {
    /// Request, terms, root, or immutable identity differed.
    IdentityMismatch,
    /// An account coordinate or key was absent, aliased, or noncanonical.
    AccountMismatch,
    /// Token effect count, order, action, amount, or pre/post state differed.
    TokenMismatch,
    /// Claims child kind, packet, frame, or receipt differed.
    ClaimsMismatch,
    /// Root revision or exact candidate bytes differed.
    RootMismatch,
    /// Checked integer arithmetic overflowed or underflowed.
    Arithmetic,
}

/// Result alias for the V2 Hot contract.
pub type Result<T> = core::result::Result<T, FractionalHotErrorV2>;

/// Exact account identity and coordinate in the authenticated AccountProfile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotAccountRefV2 {
    coordinate: u16,
    key: [u8; 32],
}

impl FractionalHotAccountRefV2 {
    /// Construct one nonzero account identity at a concrete profile coordinate.
    pub fn new(coordinate: u16, key: [u8; 32]) -> Result<Self> {
        if is_zero(key) {
            return Err(FractionalHotErrorV2::AccountMismatch);
        }
        Ok(Self { coordinate, key })
    }
    /// AccountProfile coordinate.
    pub const fn coordinate(self) -> u16 {
        self.coordinate
    }
    /// Exact authenticated account identity.
    pub const fn key(self) -> [u8; 32] {
        self.key
    }
}

/// Exact Token-2022 effect selected by one V2 action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalHotTokenKindV2 {
    /// Mint exact denominator-scaled shard atoms.
    Mint,
    /// Transfer raw same-Mint shard atoms.
    Transfer,
    /// Permissioned burn of an exact denominator multiple.
    Burn,
    /// Close one zero-supply terms-ordered Mint during retirement.
    CloseMint,
}

/// One fixed Token-2022 effect and its independently observed pre/post state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotTokenEffectV2 {
    /// Exact effect kind.
    pub kind: FractionalHotTokenKindV2,
    /// Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
    /// Terms-selected Token program.
    pub token_program: FractionalHotAccountRefV2,
    /// Terms-selected shard Mint.
    pub mint: FractionalHotAccountRefV2,
    /// Source Token account when active.
    pub source: Option<FractionalHotAccountRefV2>,
    /// Destination Token account or RentCredit when active.
    pub destination: Option<FractionalHotAccountRefV2>,
    /// Exact signing authority: root for Mint/Burn/Close, owner for Transfer.
    pub authority: FractionalHotAccountRefV2,
    /// Exact raw shard atoms; zero only for CloseMint.
    pub amount: u64,
    /// Mint supply before and after the effect.
    pub pre_supply: u64,
    /// Mint supply after the effect.
    pub post_supply: u64,
    /// Source amount before the effect, zero when absent.
    pub pre_source: u64,
    /// Source amount after the effect, zero when absent.
    pub post_source: u64,
    /// Destination amount before the effect, zero when absent.
    pub pre_destination: u64,
    /// Destination amount after the effect, zero when absent.
    pub post_destination: u64,
}

/// Canonical optional Claims child selected by the action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalHotClaimsEffectV2<'a> {
    /// Token-only, terminalization, or retirement action.
    None,
    /// Canonical K-width SignedDelta transfer for Wrap/WholeUnwrap.
    SignedDelta {
        /// Selected Claims program coordinate and identity.
        claims_program: FractionalHotAccountRefV2,
        /// First coordinate of the exact contiguous Claims frame.
        route_base: u16,
        /// Exact canonical SignedDelta packet.
        packet: &'a [u8],
    },
    /// Family-neutral terminal settlement owned by Claims.
    Terminal {
        /// Selected Claims program coordinate and identity.
        claims_program: FractionalHotAccountRefV2,
        /// First coordinate of the exact contiguous 35-account Claims frame.
        route_base: u16,
        /// Exact typed terminal request; payout is deliberately absent.
        request: &'a TerminalSettlementRequestV3,
    },
}

/// Optional lifecycle-Rent close coordinates for zero-supply retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotRentCloseV2 {
    /// Selected Rent program.
    pub rent_program: FractionalHotAccountRefV2,
    /// Root-bound lifecycle RentCredit.
    pub rent_credit: FractionalHotAccountRefV2,
    /// First coordinate of the exact Rent close frame.
    pub route_base: u16,
    /// Core-authenticated producer-subtree postresource digest.
    pub post_resource_digest: [u8; 32],
}

/// All chain-derived inputs required to prepare one nonforgeable candidate.
#[derive(Clone, Copy, Debug)]
pub struct FractionalHotCandidateInputV2<'a> {
    /// Exact hostile-decoded V2 request.
    pub request: FractionalExposureRequestV2,
    /// Exact authenticated immutable V2 terms.
    pub terms: FractionalExposureTermsV2<'a>,
    /// Exact authenticated root bytes.
    pub root_bytes: &'a [u8],
    /// Root account identity and AccountProfile coordinate.
    pub root: FractionalHotAccountRefV2,
    /// Ordered Token effects; length is action-derived and at most 256.
    pub token_effects: &'a [FractionalHotTokenEffectV2],
    /// Optional canonical Claims child.
    pub claims: FractionalHotClaimsEffectV2<'a>,
    /// Lifecycle-Rent close only for ZeroSupplyRetire.
    pub rent_close: Option<FractionalHotRentCloseV2>,
}

/// Opaque onchain-safe candidate consumed by common Trading Hot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotCandidateV2<'a> {
    request: FractionalExposureRequestV2,
    root: FractionalHotAccountRefV2,
    token_effects: &'a [FractionalHotTokenEffectV2],
    claims: FractionalHotClaimsEffectV2<'a>,
    rent_close: Option<FractionalHotRentCloseV2>,
    pre_revision: u64,
    post_revision: Option<u64>,
    root_candidate: Option<[u8; FRACTIONAL_ROOT_BYTES_V1]>,
}

impl<'a> FractionalHotCandidateV2<'a> {
    /// Prepare and fully validate one bounded candidate.
    pub fn prepare(input: FractionalHotCandidateInputV2<'a>) -> Result<Self> {
        let request = input
            .request
            .bind_terms(input.terms)
            .map_err(|_| FractionalHotErrorV2::IdentityMismatch)?;
        let root =
            FractionalRootV1::decode(input.root_bytes).ok_or(FractionalHotErrorV2::RootMismatch)?;
        if root.input().terms != input.terms.terms_id()
            || root.input().market != input.terms.market()
            || root.input().revision != request.input().expected_revision
            || input.token_effects.len() > FRACTIONAL_HOT_MAX_TOKEN_EFFECTS_V2
        {
            return Err(FractionalHotErrorV2::IdentityMismatch);
        }
        validate_token_effects(input, request)?;
        validate_claims_effect(input, request)?;
        let retiring = request.action() == FractionalExposureActionV2::ZeroSupplyRetire;
        if retiring != input.rent_close.is_some() {
            return Err(FractionalHotErrorV2::RootMismatch);
        }
        if let Some(close) = input.rent_close
            && is_zero(close.post_resource_digest)
        {
            return Err(FractionalHotErrorV2::RootMismatch);
        }
        let (post_revision, root_candidate) = if retiring {
            (None, None)
        } else {
            let revision = root
                .input()
                .revision
                .checked_add(1)
                .ok_or(FractionalHotErrorV2::Arithmetic)?;
            let candidate = FractionalRootV1::new(FractionalRootInputV1 {
                revision,
                ..root.input()
            })
            .ok_or(FractionalHotErrorV2::RootMismatch)?
            .to_bytes();
            (Some(revision), Some(candidate))
        };
        Ok(Self {
            request,
            root: input.root,
            token_effects: input.token_effects,
            claims: input.claims,
            rent_close: input.rent_close,
            pre_revision: root.input().revision,
            post_revision,
            root_candidate,
        })
    }

    /// Exact accepted action.
    pub const fn action(self) -> FractionalExposureActionV2 {
        self.request.action()
    }
    /// Exact accepted request.
    pub const fn request(self) -> FractionalExposureRequestV2 {
        self.request
    }
    /// Exact root account reference.
    pub const fn root(self) -> FractionalHotAccountRefV2 {
        self.root
    }
    /// Ordered bounded Token effects.
    pub const fn token_effects(self) -> &'a [FractionalHotTokenEffectV2] {
        self.token_effects
    }
    /// Optional canonical Claims child.
    pub const fn claims(self) -> FractionalHotClaimsEffectV2<'a> {
        self.claims
    }
    /// Optional lifecycle-Rent close coordinates.
    pub const fn rent_close(self) -> Option<FractionalHotRentCloseV2> {
        self.rent_close
    }
    /// Pre-revision authenticated from root bytes.
    pub const fn pre_revision(self) -> u64 {
        self.pre_revision
    }
    /// Post-revision, absent only when retirement closes the root.
    pub const fn post_revision(self) -> Option<u64> {
        self.post_revision
    }
    /// Copy exact root candidate bytes for commit-last execution.
    pub const fn root_candidate_bytes(self) -> Option<[u8; FRACTIONAL_ROOT_BYTES_V1]> {
        self.root_candidate
    }

    /// Recheck exact Token-owned post observations after every Token CPI.
    pub fn validate_token_poststate(self, observed: &[FractionalHotTokenPostV2]) -> Result<()> {
        if observed.len() != self.token_effects.len() {
            return Err(FractionalHotErrorV2::TokenMismatch);
        }
        for (effect, actual) in self.token_effects.iter().zip(observed) {
            if actual.representation_coordinate != effect.representation_coordinate
                || actual.mint != effect.mint.key()
                || actual.supply != effect.post_supply
                || actual.source_amount != effect.post_source
                || actual.destination_amount != effect.post_destination
            {
                return Err(FractionalHotErrorV2::TokenMismatch);
            }
        }
        Ok(())
    }

    /// Validate one SignedDelta receipt against exact packet/table/postresource commitments.
    pub fn validate_signed_delta_receipt(
        self,
        receipt_bytes: &[u8],
        post_resource_digest: [u8; 32],
    ) -> Result<()> {
        let FractionalHotClaimsEffectV2::SignedDelta {
            claims_program,
            packet,
            ..
        } = self.claims
        else {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        };
        let plan =
            SignedDeltaPlanV3::decode(packet).map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
        let packet_digest: [u8; 32] = Sha256::digest(packet).into();
        let (positions, aggregates, deltas) = plan.table_bytes();
        let table_digest = digestv(&[
            SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
            positions,
            aggregates,
            deltas,
        ]);
        let commitment = SignedDeltaReceiptCommitmentV3::new(
            packet_digest,
            table_digest,
            claims_program.key(),
            post_resource_digest,
        )
        .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
        SignedDeltaReceiptV3::decode(receipt_bytes)
            .and_then(|receipt| receipt.validate_commitment(plan, commitment))
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)
    }

    /// Validate the generic Claims terminal receipt and action-selected payout shape.
    pub fn validate_terminal_receipt(self, receipt_bytes: &[u8]) -> Result<()> {
        let FractionalHotClaimsEffectV2::Terminal { request, .. } = self.claims else {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        };
        let receipt = TerminalSettlementReceiptV3::decode(receipt_bytes)
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
        let evidence = receipt.evidence();
        let request_digest: [u8; 32] = Sha256::digest(request.to_bytes()).into();
        if receipt.request() != *request
            || evidence.request_digest != request_digest
            || (self.action() == FractionalExposureActionV2::TerminalRedeem && evidence.payout == 0)
            || (self.action() == FractionalExposureActionV2::TerminalZeroBurn
                && evidence.payout != 0)
        {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        }
        Ok(())
    }

    /// Require the exact root candidate or exact terminal close.
    pub fn validate_root_poststate(self, actual: Option<&[u8]>) -> Result<()> {
        match (self.root_candidate, actual) {
            (Some(expected), Some(actual)) if actual == expected => Ok(()),
            (None, None) => Ok(()),
            _ => Err(FractionalHotErrorV2::RootMismatch),
        }
    }
}

/// Token-owned facts re-read immediately after one effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotTokenPostV2 {
    /// Claims representation coordinate.
    pub representation_coordinate: u32,
    /// Exact terms-selected Mint.
    pub mint: [u8; 32],
    /// Exact post supply.
    pub supply: u64,
    /// Exact post source amount, zero when absent.
    pub source_amount: u64,
    /// Exact post destination amount, zero when absent.
    pub destination_amount: u64,
}

fn validate_token_effects(
    input: FractionalHotCandidateInputV2<'_>,
    request: FractionalExposureRequestV2,
) -> Result<()> {
    let fields = request.input();
    let expected_count = if request.action() == FractionalExposureActionV2::Terminalize {
        0
    } else if request.action() == FractionalExposureActionV2::ZeroSupplyRetire {
        usize::try_from(input.terms.representation_width())
            .map_err(|_| FractionalHotErrorV2::Arithmetic)?
    } else {
        1
    };
    if input.token_effects.len() != expected_count {
        return Err(FractionalHotErrorV2::TokenMismatch);
    }
    let mut prior_mint_coordinate = None;
    let mut canonical_token_program = None;
    for (index, effect) in input.token_effects.iter().copied().enumerate() {
        validate_token_account_plan(input.root, effect)?;
        if let Some(expected) = canonical_token_program {
            if effect.token_program != expected {
                return Err(FractionalHotErrorV2::AccountMismatch);
            }
        } else {
            canonical_token_program = Some(effect.token_program);
        }
        if let Some(prior) = prior_mint_coordinate
            && effect.mint.coordinate() <= prior
        {
            return Err(FractionalHotErrorV2::AccountMismatch);
        }
        prior_mint_coordinate = Some(effect.mint.coordinate());
        if effect.token_program.key() != input.terms.token_program()
            || effect.mint.key()
                != input
                    .terms
                    .shard_mint(effect.representation_coordinate)
                    .map_err(|_| FractionalHotErrorV2::TokenMismatch)?
        {
            return Err(FractionalHotErrorV2::TokenMismatch);
        }
        if request.action() == FractionalExposureActionV2::ZeroSupplyRetire {
            let coordinate = u32::try_from(index).map_err(|_| FractionalHotErrorV2::Arithmetic)?;
            if effect.kind != FractionalHotTokenKindV2::CloseMint
                || effect.representation_coordinate != coordinate
                || effect.authority != input.root
                || effect.source.is_some()
                || effect.destination != input.rent_close.map(|close| close.rent_credit)
                || effect.amount != 0
                || effect.pre_supply != 0
                || effect.post_supply != 0
                || [
                    effect.pre_source,
                    effect.post_source,
                    effect.pre_destination,
                    effect.post_destination,
                ]
                .into_iter()
                .any(|value| value != 0)
            {
                return Err(FractionalHotErrorV2::TokenMismatch);
            }
            continue;
        }
        if effect.representation_coordinate != fields.representation_coordinate {
            return Err(FractionalHotErrorV2::TokenMismatch);
        }
        let expected_kind = match request.action() {
            FractionalExposureActionV2::Wrap => FractionalHotTokenKindV2::Mint,
            FractionalExposureActionV2::Transfer => FractionalHotTokenKindV2::Transfer,
            FractionalExposureActionV2::WholeUnwrap
            | FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn => FractionalHotTokenKindV2::Burn,
            _ => return Err(FractionalHotErrorV2::TokenMismatch),
        };
        let amount = match expected_kind {
            FractionalHotTokenKindV2::Mint => fields
                .quantity
                .checked_mul(input.terms.denominator())
                .ok_or(FractionalHotErrorV2::Arithmetic)?,
            FractionalHotTokenKindV2::Transfer => fields.quantity,
            FractionalHotTokenKindV2::Burn => {
                divide_exposure_shards_v2(
                    input.terms,
                    fields.representation_coordinate,
                    fields.quantity,
                )
                .map_err(|_| FractionalHotErrorV2::TokenMismatch)?
                .consumed
                .shard_atoms
            }
            FractionalHotTokenKindV2::CloseMint => 0,
        };
        if effect.kind != expected_kind || effect.amount != amount {
            return Err(FractionalHotErrorV2::TokenMismatch);
        }
        validate_one_token_transition(input.root, fields, effect, amount)?;
    }
    Ok(())
}

fn validate_token_account_plan(
    root: FractionalHotAccountRefV2,
    effect: FractionalHotTokenEffectV2,
) -> Result<()> {
    let accounts = [
        Some(effect.token_program),
        Some(effect.mint),
        effect.source,
        effect.destination,
        Some(effect.authority),
    ];
    for (left_index, left) in accounts.iter().enumerate() {
        let Some(left) = left else { continue };
        for right in accounts.iter().skip(left_index + 1).flatten() {
            if left.coordinate() == right.coordinate() || left.key() == right.key() {
                return Err(FractionalHotErrorV2::AccountMismatch);
            }
        }
    }
    if effect.authority.key() == root.key() && effect.authority != root {
        return Err(FractionalHotErrorV2::AccountMismatch);
    }
    Ok(())
}

fn validate_one_token_transition(
    root: FractionalHotAccountRefV2,
    fields: crate::FractionalExposureRequestInputV2,
    effect: FractionalHotTokenEffectV2,
    amount: u64,
) -> Result<()> {
    let checked_add = |value: u64| {
        value
            .checked_add(amount)
            .ok_or(FractionalHotErrorV2::Arithmetic)
    };
    let checked_sub = |value: u64| {
        value
            .checked_sub(amount)
            .ok_or(FractionalHotErrorV2::TokenMismatch)
    };
    match effect.kind {
        FractionalHotTokenKindV2::Mint => {
            if effect.authority != root
                || effect.source.is_some()
                || effect.destination.map(FractionalHotAccountRefV2::key)
                    != Some(fields.destination_token_account)
                || effect.post_supply != checked_add(effect.pre_supply)?
                || effect.pre_source != 0
                || effect.post_source != 0
                || effect.post_destination != checked_add(effect.pre_destination)?
            {
                return Err(FractionalHotErrorV2::TokenMismatch);
            }
        }
        FractionalHotTokenKindV2::Transfer => {
            if effect.authority.key() != fields.owner
                || effect.source.map(FractionalHotAccountRefV2::key)
                    != Some(fields.source_token_account)
                || effect.destination.map(FractionalHotAccountRefV2::key)
                    != Some(fields.destination_token_account)
                || effect.post_supply != effect.pre_supply
                || effect.post_source != checked_sub(effect.pre_source)?
                || effect.post_destination != checked_add(effect.pre_destination)?
            {
                return Err(FractionalHotErrorV2::TokenMismatch);
            }
        }
        FractionalHotTokenKindV2::Burn => {
            if effect.authority != root
                || effect.source.map(FractionalHotAccountRefV2::key)
                    != Some(fields.source_token_account)
                || effect.destination.is_some()
                || effect.post_supply != checked_sub(effect.pre_supply)?
                || effect.post_source != checked_sub(effect.pre_source)?
                || effect.pre_destination != 0
                || effect.post_destination != 0
            {
                return Err(FractionalHotErrorV2::TokenMismatch);
            }
        }
        FractionalHotTokenKindV2::CloseMint => {
            return Err(FractionalHotErrorV2::TokenMismatch);
        }
    }
    Ok(())
}

fn validate_claims_effect(
    input: FractionalHotCandidateInputV2<'_>,
    request: FractionalExposureRequestV2,
) -> Result<()> {
    match (request.action(), input.claims) {
        (
            FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap,
            FractionalHotClaimsEffectV2::SignedDelta {
                route_base, packet, ..
            },
        ) => validate_signed_delta(input, request, route_base, packet),
        (
            FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn,
            FractionalHotClaimsEffectV2::Terminal {
                claims_program,
                route_base,
                request: terminal,
            },
        ) => validate_terminal(input, request, claims_program, route_base, *terminal),
        (
            FractionalExposureActionV2::Transfer
            | FractionalExposureActionV2::Terminalize
            | FractionalExposureActionV2::ZeroSupplyRetire,
            FractionalHotClaimsEffectV2::None,
        ) => Ok(()),
        _ => Err(FractionalHotErrorV2::ClaimsMismatch),
    }
}

fn validate_signed_delta(
    input: FractionalHotCandidateInputV2<'_>,
    request: FractionalExposureRequestV2,
    route_base: u16,
    packet: &[u8],
) -> Result<()> {
    let frame = SignedDeltaFrameSpecV3::new(2)
        .and_then(SignedDeltaFrameSpecV3::account_count)
        .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
    route_base
        .checked_add(frame)
        .ok_or(FractionalHotErrorV2::AccountMismatch)?;
    let plan =
        SignedDeltaPlanV3::decode(packet).map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
    let request_digest: [u8; 32] = Sha256::digest(
        request
            .to_bytes()
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?,
    )
    .into();
    if plan.caller_role() != CallerRole::Trading
        || plan.release_set() != input.terms.release_set()
        || plan.market() != input.terms.market()
        || plan.request_id() != request_digest
        || plan.product_record_digest() != input.terms.product_record()
        || plan.semantic_basis_id() != input.terms.representation_basis()
        || plan.linked_basis_record_digest() != input.terms.product_basis()
        || plan.claim_count() != input.terms.representation_width()
        || plan.position_count() != 2
        || plan.position_delta_count() != 2
    {
        return Err(FractionalHotErrorV2::ClaimsMismatch);
    }
    for coordinate in 0..plan.claim_count() {
        if plan
            .aggregate_delta(coordinate)
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?
            .direction()
            != DeltaDirectionV3::Neutral
        {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        }
    }
    let fields = request.input();
    let whole = if request.action() == FractionalExposureActionV2::Wrap {
        fields.quantity
    } else {
        divide_exposure_shards_v2(
            input.terms,
            fields.representation_coordinate,
            fields.quantity,
        )
        .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?
        .whole_claims
    };
    let mut saw_root = false;
    let mut saw_owner = false;
    for index in 0..2 {
        let position = plan
            .position(index)
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
        let row = plan
            .position_delta(index)
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
        if row.position_index() != index
            || row.outcome() != fields.representation_coordinate
            || row.delta().magnitude() != whole
        {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        }
        if position.owner() == input.root.key() {
            saw_root = true;
            let expected = if request.action() == FractionalExposureActionV2::Wrap {
                DeltaDirectionV3::Credit
            } else {
                DeltaDirectionV3::Debit
            };
            if row.delta().direction() != expected {
                return Err(FractionalHotErrorV2::ClaimsMismatch);
            }
        } else if position.owner() == fields.owner {
            saw_owner = true;
            let expected = if request.action() == FractionalExposureActionV2::Wrap {
                DeltaDirectionV3::Debit
            } else {
                DeltaDirectionV3::Credit
            };
            if row.delta().direction() != expected {
                return Err(FractionalHotErrorV2::ClaimsMismatch);
            }
        } else {
            return Err(FractionalHotErrorV2::ClaimsMismatch);
        }
    }
    if !saw_root || !saw_owner {
        return Err(FractionalHotErrorV2::ClaimsMismatch);
    }
    Ok(())
}

fn validate_terminal(
    input: FractionalHotCandidateInputV2<'_>,
    request: FractionalExposureRequestV2,
    claims_program: FractionalHotAccountRefV2,
    route_base: u16,
    terminal: TerminalSettlementRequestV3,
) -> Result<()> {
    route_base
        .checked_add(
            u16::try_from(TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3)
                .map_err(|_| FractionalHotErrorV2::Arithmetic)?,
        )
        .ok_or(FractionalHotErrorV2::AccountMismatch)?;
    let fields = request.input();
    let parent: [u8; 32] = Sha256::digest(
        request
            .to_bytes()
            .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?,
    )
    .into();
    let terminal = terminal.input();
    let division = divide_exposure_shards_v2(
        input.terms,
        fields.representation_coordinate,
        fields.quantity,
    )
    .map_err(|_| FractionalHotErrorV2::ClaimsMismatch)?;
    if terminal.caller_role != CallerRole::Trading
        || terminal.release_set != input.terms.release_set()
        || terminal.market != input.terms.market()
        || terminal.parent_context != parent
        || terminal.product_record_digest != input.terms.product_record()
        || terminal.exposure_id != input.terms.exposure_id()
        || terminal.terminal_record_digest != fields.terminal_digest
        || terminal.owner != input.root.key()
        || terminal.recipient_owner != fields.owner
        || terminal.claims_program != claims_program.key()
        || terminal.token_program != input.terms.token_program()
        || terminal.semantic_basis_id != input.terms.representation_basis()
        || terminal.linked_basis_record_digest != input.terms.product_basis()
        || terminal.quantity != division.whole_claims
        || terminal.claim_index != fields.representation_coordinate
    {
        return Err(FractionalHotErrorV2::ClaimsMismatch);
    }
    Ok(())
}

fn digestv(parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
