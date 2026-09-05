//! Lowering of Structured V2 actions onto the adopted Rational child wire.
//!
//! Decision 0011 §3b took Option A: Structured reaches the chain through the
//! **existing** Rational child ABI, so nothing here introduces a request magic,
//! a `ReceiptKindV3` variant, or a line of program code.  This module is the
//! whole of Structured's wire adoption, and it is host-side by construction.
//!
//! # What this module owns, and what it deliberately does not
//!
//! It owns the three mappings the ruling created and the one refusal that makes
//! them safe to use:
//!
//! 1. **Kind → style.** [`structured_child_token_style_v2`] maps each
//!    [`StructuredHotTokenKindV2`] to its Rational
//!    [`TokenEffectStyleV2`], and REFUSES the two closure kinds by name,
//!    because they are not `TokenEffectStyleV2` members at all -- they live on
//!    the lifecycle wire ([`structured_child_lifecycle_action_v2`]).
//! 2. **Action → wire.** [`structured_child_wire_v2`] says which of the two
//!    child wires an action reaches, and refuses to pretend that Structured's
//!    `TerminalRedeem` is Rational's `RedeemTerminal`.  It is not; see
//!    [`StructuredChildWireV2::TerminalTwoPhase`].
//! 3. **Effect ORDER.** [`structured_child_effect_order_v2`] emits the exact
//!    cursor order the callee will reconstruct.  `Issue` INVERTS relative to
//!    this crate's own plan order, and that is not negotiable: the callee
//!    indexes the asset row from the cursor.
//!
//! And the refusal: [`bind_structured_child_descriptor_v2`] joins the supplied
//! Rational descriptor coordinates to the Structured terms they claim to
//! represent.  Under Option A the on-chain authority is keyed only by
//! `descriptor_id`, so the chain cannot tell one family's descriptor from
//! another's -- it does not need to, because a descriptor names its own
//! Market, Product instance and coefficients and the Claims program refuses
//! any mismatch.  What the chain cannot do is stop THIS crate from encoding a
//! request against somebody else's descriptor and discovering it at execution
//! time.  That join is this module's job, and it is the same role decision
//! 0011 §5 keeps `hot_v2.rs` for: the operator's adversary, not the chain's.
//!
//! It does not derive a PDA, sign, or submit, for the reason the crate root
//! gives: `find_program_address` belongs to the physical adapter.

use dclutch_claims::rational::{
    ABSENT_REVISION, ASSET_BYTES_V3, AssetV2, CallerRoleV2, REQUEST_STRUCTURED_HEADER_BYTES_V3,
    RepresentationActionV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
    TokenEffectStyleV2,
};
use dclutch_claims::rational_lifecycle::LifecycleActionV2;
use dclutch_claims::structured::{StructuredActionV2, StructuredHotTokenKindV2};
use dclutch_claims::structured_kernel::{ShardMovementV2, StructuredTermsV2};

use crate::{Error, Result};

/// Largest Product outcome width one Structured child request can execute.
///
/// This is not a Structured choice and not a capacity measurement.  One
/// RequestProfile V1 artifact is bounded at `REQUEST_PROFILE_MAX_BYTES_V1=1312`
/// bytes as `32 + operations * 24`, so it admits at most `(1312-32)/24 = 53`
/// operations; the canonical projection of a `RepresentationRequestV2` costs
/// `29 + 8K`.  `K = 3` is therefore 53 operations and 1,304 bytes -- eight
/// bytes of slack, which is not a fourth operation -- and `K = 4` is 61
/// operations and 1,496 bytes.
///
/// **This constant is a MIRROR, not the author.** The one derived author is
/// `dclutch_bearer_v2_operator::open_structured_v3::RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3`,
/// which computes this value from the profile bound and the `29`/`8` operation
/// counts instead of restating it. This crate cannot derive it too: the
/// operation counts live in `dclutch-bearer-v2-operator`, which links
/// `solana-program`, and this crate's root deliberately promises no Solana SDK
/// types (see its `Cargo.toml`, where bearer is a DEV dependency for exactly
/// that reason). The two are pinned equal by
/// `the_two_ceilings_are_one_number_and_neither_crate_restates_it`. Lifting the
/// counts into a Solana-free contract crate so both sides derive is the
/// outstanding fix; until then this literal is guarded by that test alone.
///
/// Because the wire additionally requires `asset_count == outcome_count` for
/// the two Structured actions, this is a bound on the **Product outcome
/// width**, not on how many coordinates carry a nonzero coefficient.
///
/// **Both walls moved on 2026-09-02 and this bound did not, on purpose.** The
/// note here said the packet was the binding wall and capped full-width
/// issuance at `K = 2`, one coordinate BELOW this ceiling, so raising the
/// RequestProfile bound alone would admit descriptors that could be published
/// and denominated but never issued. Physical ABI v3 was the lift it named:
/// commit-don't-inline plus an action-conditional header take a full-width
/// K = 3 request from 968 bytes to 576, which moves the measured Claims-direct
/// frame from 1,397 to 1,005 and the artifact ceiling from 3 to 6.
///
/// **Provisional above 3, and this literal is the measured half.** The
/// arithmetic says K = 5 fits the Claims-direct frame (1,149 bytes, 1,161 with
/// the house builder's unconditional `set_compute_unit_price`) and K = 6 misses
/// by one byte; the Trading common-Hot route caps at K = 3 (1,197). None of
/// that above K = 3 has been EXECUTED -- the campaign has only ever driven
/// K = 3 -- and a bound admitted on arithmetic is how a descriptor gets
/// published that nothing can issue. LIFTING PLAN: re-run
/// `tools/gauntlet/claims-rational-representation-v2` at K = 4 and K = 5,
/// re-pin the extents, and raise this to the lower of the measured packet
/// ceiling and `RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3`.
///
/// `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 = 257` is a capacity-profile
/// measurement and has no executable meaning; do not size against it.
pub const STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2: u32 = 3;

/// Which child wire one Structured action reaches, under decision 0011 §3b.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredChildWireV2 {
    /// One `RepresentationRequestV2` carries the whole action.
    Representation(RepresentationActionV2),
    /// One `LifecycleRequestV2` carries the whole action.
    Lifecycle,
    /// The action is NOT one child request.
    ///
    /// Structured's `TerminalRedeem` builds exactly `Unwrap`'s Token effects
    /// (burn receipts, release the basket from custody) and then settles the
    /// released shards for collateral.  Rational's `RedeemTerminal` burns the
    /// shards of ONE selected outcome and pays that outcome's collateral, and
    /// the wire refuses it unless `asset_count == 1`.  So the lowering is one
    /// `UnwrapStructured` to release the basket, followed by one
    /// `RedeemTerminal` per coordinate whose released shards yield a whole
    /// claim -- up to `K + 1` child requests, not one transaction.
    TerminalTwoPhase,
}

/// Route one Structured action to its child wire.
pub const fn structured_child_wire_v2(action: StructuredActionV2) -> StructuredChildWireV2 {
    match action {
        StructuredActionV2::Issue => {
            StructuredChildWireV2::Representation(RepresentationActionV2::IssueStructured)
        }
        StructuredActionV2::Unwrap => {
            StructuredChildWireV2::Representation(RepresentationActionV2::UnwrapStructured)
        }
        StructuredActionV2::TerminalRedeem => StructuredChildWireV2::TerminalTwoPhase,
        StructuredActionV2::ZeroSupplyRetire => StructuredChildWireV2::Lifecycle,
    }
}

/// Map one Structured Token kind onto the Rational Token effect style.
///
/// The two closure kinds refuse with [`Error::ChildWire`]: they are not
/// `TokenEffectStyleV2` members, and asking for one is a lowering mistake that
/// must be caught here rather than produce a plausible wrong style.  Use
/// [`structured_child_lifecycle_action_v2`] for them.
pub const fn structured_child_token_style_v2(
    kind: StructuredHotTokenKindV2,
) -> Result<TokenEffectStyleV2> {
    match kind {
        StructuredHotTokenKindV2::MintReceipts => Ok(TokenEffectStyleV2::MintReceipt),
        StructuredHotTokenKindV2::BurnReceipts => Ok(TokenEffectStyleV2::BurnReceipt),
        StructuredHotTokenKindV2::LockShards => Ok(TokenEffectStyleV2::TransferShardToStructured),
        StructuredHotTokenKindV2::ReleaseShards => {
            Ok(TokenEffectStyleV2::TransferShardFromStructured)
        }
        StructuredHotTokenKindV2::CloseCustody | StructuredHotTokenKindV2::CloseReceiptMint => {
            Err(Error::ChildWire)
        }
    }
}

/// Map one Structured closure kind onto the Rational lifecycle action.
///
/// The four supply kinds refuse with [`Error::ChildWire`] for the mirror
/// reason: they move atoms and belong on the representation wire.
pub const fn structured_child_lifecycle_action_v2(
    kind: StructuredHotTokenKindV2,
) -> Result<LifecycleActionV2> {
    match kind {
        StructuredHotTokenKindV2::CloseCustody => Ok(LifecycleActionV2::RetireCoordinate),
        StructuredHotTokenKindV2::CloseReceiptMint => Ok(LifecycleActionV2::RetireReceipt),
        StructuredHotTokenKindV2::MintReceipts
        | StructuredHotTokenKindV2::BurnReceipts
        | StructuredHotTokenKindV2::LockShards
        | StructuredHotTokenKindV2::ReleaseShards => Err(Error::ChildWire),
    }
}

/// One entry of the exact effect order the callee reconstructs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredChildEffectSlotV2 {
    /// Position in the callee's effect iteration.
    pub cursor: u32,
    /// Style the callee will emit at this cursor.
    pub style: TokenEffectStyleV2,
    /// Asset row the callee reads, or `None` for the receipt effect.
    ///
    /// The callee derives this from the cursor itself -- `cursor` when issuing,
    /// `cursor - 1` when unwrapping -- so it is reported rather than chosen.
    pub asset_row: Option<u32>,
}

/// Emit the exact ordered effect slots for one Structured representation action.
///
/// **`Issue` inverts this crate's own plan order.**
/// `build_supply_effects` pushes the receipt effect first for both actions;
/// the callee emits `IssueStructured` as `TransferShardToStructured` at cursors
/// `0..K-1` and `MintReceipt` LAST at cursor `K`.  `UnwrapStructured` agrees
/// (`BurnReceipt` first, transfers at `1..K`).  The ordering is not a
/// preference: the callee indexes the asset row from the cursor, so a reordered
/// effect reads the wrong coordinate.
///
/// The strictly ascending shard sweep is preserved for free, because the cursor
/// IS the row index.
pub fn structured_child_effect_order_v2(
    action: RepresentationActionV2,
    outcome_count: u32,
) -> Result<Vec<StructuredChildEffectSlotV2>> {
    if outcome_count == 0 || outcome_count > STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 {
        return Err(Error::ChildWidth);
    }
    let mut slots = Vec::new();
    match action {
        RepresentationActionV2::IssueStructured => {
            let mut row = 0_u32;
            while row < outcome_count {
                slots.push(StructuredChildEffectSlotV2 {
                    cursor: row,
                    style: TokenEffectStyleV2::TransferShardToStructured,
                    asset_row: Some(row),
                });
                row = row.checked_add(1).ok_or(Error::ChildWidth)?;
            }
            slots.push(StructuredChildEffectSlotV2 {
                cursor: outcome_count,
                style: TokenEffectStyleV2::MintReceipt,
                asset_row: None,
            });
        }
        RepresentationActionV2::UnwrapStructured => {
            slots.push(StructuredChildEffectSlotV2 {
                cursor: 0,
                style: TokenEffectStyleV2::BurnReceipt,
                asset_row: None,
            });
            let mut row = 0_u32;
            while row < outcome_count {
                let cursor = row.checked_add(1).ok_or(Error::ChildWidth)?;
                slots.push(StructuredChildEffectSlotV2 {
                    cursor,
                    style: TokenEffectStyleV2::TransferShardFromStructured,
                    asset_row: Some(row),
                });
                row = cursor;
            }
        }
        RepresentationActionV2::Denominate
        | RepresentationActionV2::Reconstitute
        | RepresentationActionV2::RedeemTerminal => return Err(Error::ChildWire),
    }
    Ok(slots)
}

/// Rational descriptor coordinates the physical adapter observed.
///
/// Every field here is a fact the Claims program will independently re-derive
/// or re-check.  They are named rather than derived because PDA derivation is
/// the adapter's boundary, and they are JOINED to the Structured terms by
/// [`bind_structured_child_descriptor_v2`] before any of them reaches a wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredChildDescriptorV2 {
    /// Content identity of the finalized Rational descriptor record.
    ///
    /// Under Option A this is the ONLY per-representation identity the chain
    /// has: the representation authority, the shard Mints, the Structured
    /// custody accounts, the Claims custody owners and the replay record are
    /// all keyed by it.  Structured's own `terms_id` names no account.
    pub descriptor_id: [u8; 32],
    /// Finalized composition EXPOSURE bundle the descriptor selects.
    ///
    /// **The Rational wire calls this field `graph_id`, and it is not the
    /// source graph.**  `RepresentationDescriptorV2::graph_id()` is the value
    /// the Claims adapter hands to `CompositionExposureBundleV3::decode` as
    /// `RecordAdmissionV3::selected_id` under
    /// `COMPOSITION_EXPOSURE_SCHEMA_ID_V3`
    /// (`rational-representation-v2-operator/src/lib.rs:558-576`), and
    /// `RepresentationDescriptorV2::authenticate_exposure` then requires it to
    /// equal `exposure.bundle_id()`
    /// (`rational-representation-v2-kernel/src/lib.rs:902`).  The descriptor's
    /// own ENCODER already names it correctly:
    /// `RepresentationDescriptorInputV3::exposure_id`.
    ///
    /// So it joins `StructuredTermsV2::shard_exposure`, never
    /// `StructuredTermsV2::graph_id` -- the terms carry BOTH, and their decoder
    /// proves they differ (`require_distinct_identities` lists both in the
    /// pairwise-distinct set), exactly as the shard layer carries both
    /// `FractionalExposureTermsV2::exposure_id` and `::graph_id`.  It is named
    /// `exposure_id` here because the shared name is what made the first
    /// version of this join compare the wrong record.
    ///
    /// The source graph identity reaches the descriptor transitively -- the
    /// exposure bundle names it, and the chain re-checks it -- so it is joined
    /// where the exposure record is read, not here.
    pub exposure_id: [u8; 32],
    /// Claims-owned authority: `(RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    /// descriptor_id)`.
    ///
    /// This is what replaces the Structured root as receipt Mint authority,
    /// and it takes TWO Token-2022 roles at once -- Mint authority for
    /// `MintReceipt` and permissioned-burn authority for `BurnReceipt`.
    pub representation_authority: [u8; 32],
    /// Receipt Mint address the descriptor persists.
    pub receipt_mint: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Execution release set.
    pub release_set: [u8; 32],
    /// Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Product outcome width; equals the descriptor's coefficient count.
    pub outcome_count: u32,
    /// Shard atoms backing one native claim atom.
    pub denominator: u64,
}

/// Refuse a Rational descriptor that does not represent these Structured terms.
///
/// This is the cross-family substitution refusal, and it is exact about where
/// it lives.  On chain, a descriptor is inseparable from its own Market,
/// Product instance and coefficients, so a request naming descriptor `D`
/// reaches `D`'s objects and nothing else -- there is no authority to bring
/// from elsewhere.  What remains possible is a HOST mistake: encoding a
/// Structured action against a descriptor belonging to another representation,
/// which would fail at execution after the transaction was built and signed.
/// This join turns that into a local refusal.
///
/// Both directions are covered by the same equality, and both are tested: a
/// Rational-context descriptor driving Structured terms refuses, and Structured
/// terms driving another representation's descriptor refuses.
///
/// The record-identity join is
/// [`StructuredChildDescriptorV2::exposure_id`] against
/// [`StructuredTermsV2::shard_exposure`]; see that field's documentation for
/// why the Rational wire's own name for it is misleading and what the chain
/// actually does with the value.
pub fn bind_structured_child_descriptor_v2(
    terms: StructuredTermsV2<'_>,
    descriptor: StructuredChildDescriptorV2,
) -> Result<()> {
    for identity in [
        descriptor.descriptor_id,
        descriptor.exposure_id,
        descriptor.representation_authority,
        descriptor.receipt_mint,
        descriptor.market,
        descriptor.release_set,
        descriptor.token_program,
    ] {
        if identity == [0; 32] {
            return Err(Error::ChildIdentity);
        }
    }
    if descriptor.market != terms.market()
        || descriptor.release_set != terms.release_set()
        || descriptor.token_program != terms.token_program()
        || descriptor.receipt_mint != terms.receipt_mint()
        || descriptor.exposure_id != terms.shard_exposure()
        || descriptor.outcome_count != terms.representation_width()
        || descriptor.denominator != terms.denominator()
    {
        return Err(Error::ChildIdentity);
    }
    // The two identities are different records and the terms decoder proves it,
    // so a descriptor whose exposure slot carries the SOURCE GRAPH identity is
    // the exact mistake this join exists to catch rather than an equal value
    // reached another way.
    if descriptor.exposure_id == terms.graph_id() {
        return Err(Error::ChildIdentity);
    }
    // The Structured root is not the receipt Mint authority any more, so the
    // adopted authority must not be allowed to alias any coordinate it is
    // supposed to be independent of.
    if descriptor.representation_authority == descriptor.receipt_mint
        || descriptor.representation_authority == descriptor.descriptor_id
        || descriptor.representation_authority == descriptor.market
    {
        return Err(Error::ChildIdentity);
    }
    if descriptor.outcome_count == 0
        || descriptor.outcome_count > STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2
    {
        return Err(Error::ChildWidth);
    }
    Ok(())
}

/// One coordinate's observed Rational account quadruple and balances.
///
/// Every coordinate needs one of these, INCLUDING a zero-coefficient
/// coordinate: the wire refuses `asset_count != outcome_count`, so an inert row
/// still needs its accounts materialized and still executes a zero-amount
/// transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredChildCoordinateV2 {
    /// `(RATIONAL_SHARD_MINT_SEED_V2, descriptor_id, outcome_le)`.
    pub shard_mint: [u8; 32],
    /// Actor's shard Token account.
    pub actor_shard_account: [u8; 32],
    /// `(RATIONAL_STRUCTURED_CUSTODY_SEED_V2, descriptor_id, outcome_le)`.
    pub structured_custody_account: [u8; 32],
    /// `ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor_id, outcome)`.
    pub claims_custody_owner: [u8; 32],
    /// Observed Token Mint supply before execution.
    pub expected_shard_supply: u64,
    /// Observed actor shard balance before execution.
    pub expected_actor_shards: u64,
}

/// Actor-side coordinates one Structured child request carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredChildActorV2 {
    /// Token holder, and the Claims Position owner.
    pub actor: [u8; 32],
    /// Actor's receipt Token account.
    pub receipt_account: [u8; 32],
    /// Upstream packet digest, used as the caller-authority context seed.
    pub parent_context: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Exact Structured replay revision expected before execution.
    pub expected_representation_revision: u64,
    /// Observed receipt Mint supply before execution.
    pub expected_receipt_supply: u64,
}

/// Exact encoded byte width of one Structured child representation request.
pub fn structured_child_request_bytes_v2(outcome_count: u32) -> Result<usize> {
    if outcome_count == 0 || outcome_count > STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 {
        return Err(Error::ChildWidth);
    }
    usize::try_from(outcome_count)
        .ok()
        .and_then(|width| width.checked_mul(ASSET_BYTES_V3))
        .and_then(|assets| assets.checked_add(REQUEST_STRUCTURED_HEADER_BYTES_V3))
        .ok_or(Error::ChildWidth)
}

/// Encode one Structured `Issue` or `Unwrap` onto the Rational child wire.
///
/// `movements` is the plan's FULL-WIDTH movement table, including inert
/// zero-coefficient rows, because that is exactly the width the wire demands.
/// The coefficient and the required post-custody come from the movements and
/// the terms, so a caller cannot supply an amount the kernel did not derive;
/// the callee independently recomputes `K_i = c_i * S` anyway.
///
/// The returned bytes decode as a `RepresentationRequestV2` and are validated
/// by its own `encode_into`, so every action-shape rule (`asset_count ==
/// outcome_count`, `selected_outcome == u32::MAX`, receipt account present,
/// Realm and collateral recipient ABSENT, the revision quartet) is enforced by
/// the wire's own author rather than restated here.
pub fn encode_structured_child_representation_v2(
    action: StructuredActionV2,
    terms: StructuredTermsV2<'_>,
    descriptor: StructuredChildDescriptorV2,
    actor: StructuredChildActorV2,
    coordinates: &[StructuredChildCoordinateV2],
    movements: &[ShardMovementV2],
    receipt_atoms: u64,
) -> Result<Vec<u8>> {
    bind_structured_child_descriptor_v2(terms, descriptor)?;
    let representation = match structured_child_wire_v2(action) {
        StructuredChildWireV2::Representation(value) => value,
        StructuredChildWireV2::Lifecycle | StructuredChildWireV2::TerminalTwoPhase => {
            return Err(Error::ChildWire);
        }
    };
    let width = descriptor.outcome_count;
    let expected = usize::try_from(width).map_err(|_| Error::ChildWidth)?;
    if coordinates.len() != expected || movements.len() != expected {
        return Err(Error::ChildWidth);
    }
    if receipt_atoms == 0 {
        return Err(Error::ChildWire);
    }

    let mut assets = Vec::new();
    let mut coordinate = 0_u32;
    while coordinate < width {
        let index = usize::try_from(coordinate).map_err(|_| Error::ChildWidth)?;
        let observed = *coordinates.get(index).ok_or(Error::ChildWidth)?;
        let movement = *movements.get(index).ok_or(Error::ChildWidth)?;
        if movement.representation_coordinate != coordinate
            || movement.shard_mint != observed.shard_mint
        {
            return Err(Error::ChildIdentity);
        }
        // Structured custody before the action, recovered from the plan: the
        // movement names the required custody AFTER it, plus the named surplus
        // that no plan ever moves.
        let post_backing = movement
            .post_required_custody
            .checked_add(movement.surplus_shard_custody)
            .ok_or(Error::ChildWidth)?;
        let expected_structured_shards = match representation {
            RepresentationActionV2::IssueStructured => post_backing
                .checked_sub(movement.shard_atoms)
                .ok_or(Error::ChildIdentity)?,
            RepresentationActionV2::UnwrapStructured => post_backing
                .checked_add(movement.shard_atoms)
                .ok_or(Error::ChildWidth)?,
            _ => return Err(Error::ChildWire),
        };
        let asset = AssetV2 {
            shard_mint: observed.shard_mint,
            actor_shard_account: observed.actor_shard_account,
            structured_custody_account: observed.structured_custody_account,
            claims_custody_owner: observed.claims_custody_owner,
            coefficient: terms.coefficient(coordinate).map_err(|_| Error::Terms)?,
            expected_shard_supply: observed.expected_shard_supply,
            expected_actor_shards: observed.expected_actor_shards,
            expected_structured_shards,
        };
        let mut row = [0_u8; ASSET_BYTES_V3];
        asset.encode_into(&mut row).map_err(|_| Error::ChildWire)?;
        assets.extend_from_slice(&row);
        coordinate = coordinate.checked_add(1).ok_or(Error::ChildWidth)?;
    }

    let header = RepresentationRequestHeaderV2 {
        action: representation,
        caller_role: CallerRoleV2::Trading,
        release_set: descriptor.release_set,
        market: descriptor.market,
        // The wire's `graph_id` field carries the composition EXPOSURE bundle
        // identity; the callee compares it to `descriptor.graph_id()`, which is
        // the same record. The translation is stated here because it is the one
        // place the two names meet.
        graph_id: descriptor.exposure_id,
        descriptor_id: descriptor.descriptor_id,
        parent_context: actor.parent_context,
        actor: actor.actor,
        receipt_mint: descriptor.receipt_mint,
        receipt_account: actor.receipt_account,
        representation_authority: descriptor.representation_authority,
        token_program: descriptor.token_program,
        realm: [0; 32],
        collateral_recipient: [0; 32],
        expected_representation_revision: actor.expected_representation_revision,
        expected_claims_market_revision: ABSENT_REVISION,
        expected_actor_position_revision: ABSENT_REVISION,
        expected_custody_position_revision: ABSENT_REVISION,
        expected_custody_replay_revision: ABSENT_REVISION,
        generation: actor.generation,
        quantity: receipt_atoms,
        denominator: descriptor.denominator,
        expected_receipt_supply: actor.expected_receipt_supply,
        outcome_count: width,
        selected_outcome: u32::MAX,
        asset_count: width,
    };
    let request = RepresentationRequestV2::new(header, &assets).map_err(|_| Error::ChildWire)?;
    let mut output = vec![0_u8; structured_child_request_bytes_v2(width)?];
    request
        .encode_into(&mut output)
        .map_err(|_| Error::ChildWire)?;
    Ok(output)
}
