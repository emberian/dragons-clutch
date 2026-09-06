//! Pure wallet-terminal payout INPUT derivation — stage one, callable from a
//! browser.
//!
//! Stage two — the payout manifest — was extracted into
//! `dclutch-wallet-terminal-payout-operator` so the browser could run the same
//! derivation instead of reimplementing it. Stage one, which produces the input
//! that stage consumes, stayed a CLI command inside
//! `tools/local-validator/bootstrap/successor/src/terminal_lifecycle.rs`: the
//! last command standing between a stranger and a redemption.
//!
//! Its impurity was two file reads, an RPC, and a cluster-origin policy. None
//! of that is authority. What the derivation actually needs is:
//!
//! - the **six protocol coordinates** — the Core, Claims, Custody, Registry and
//!   Resolution program ids and the release-set id ([`ProtocolCoordinatesV1`]);
//! - a **routing table** of which addresses to observe and the record digests
//!   that address them ([`TerminalRoutingTableV1`]) — an address book, never
//!   authority: every row it names is re-derived here and re-authenticated
//!   against finalized state by the caller's own observations;
//! - the caller's **request** ([`TerminalPayoutRequestV1`]);
//! - and **two rounds of observed accounts**, which this crate names and never
//!   reads.
//!
//! # Three pure phases, two rounds
//!
//! 1. [`terminal_payout_round_one_addresses_v1`] — the two addresses round one
//!    observes. The Claims aggregate is a PDA of the Market under the plan's
//!    Claims program, so it is derivable before any read and shares the Market's
//!    round rather than costing one of its own.
//! 2. [`route_terminal_payout_frame_v1`] — those two observations become the
//!    routed frame, and the frame names the addresses round two observes.
//! 3. [`complete_terminal_payout_input_v1`] — those observations plus the claim
//!    index and quantity become the exact payout input.
//!
//! Nothing here reads a file, opens a socket, holds a key, or decides which
//! cluster a caller may talk to.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use crate::ObservedAccount;
use crate::wallet_terminal_payout::{
    Error, Result, hex,
    wire::{
        FinalizedSnapshotV1, INPUT_FORMAT, LookupTableRequirementV1, PlanInputV1,
        ProgramSelectorsV1, RecordSelectorsV1, SelectedInputV1, build_report,
    },
};
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_market::CoreState;
use sha2::{Digest, Sha256};
use solana_program::{hash::hashv, pubkey::Pubkey};
use spl_associated_token_account_interface::{
    address::get_associated_token_address_with_program_id,
    program::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
};

pub mod address_book;

const PARENT_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/wallet-terminal-parent-context/v1";

/// The six protocol coordinates the derivation takes from a deployment.
///
/// Enumerated rather than assumed: every `plan.*` access in the producer was
/// one of these six. A browser holds five from its own deployment table and
/// reads the sixth — the release set — out of the Market's Core state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolCoordinatesV1 {
    /// Registry program id — the record layer every raw-record PDA is under.
    pub registry: Pubkey,
    /// Core program id — the Market's own owner.
    pub core: Pubkey,
    /// Claims program id — the aggregate, Position and custody-replay owner.
    pub claims: Pubkey,
    /// Custody program id — the replay, authority and hoard owner.
    pub custody: Pubkey,
    /// Resolution program id — the terminal certificate's owner.
    pub resolution: Pubkey,
    /// The activated release set this Market selected.
    pub release_set: [u8; 32],
}

/// One published record's routing hint: the content digest that addresses it,
/// and the address the publisher recorded for it.
///
/// Carrying both is what lets [`route_terminal_payout_frame_v1`] check the
/// claimed address against the canonical raw-record PDA the digest derives. A
/// table that names an address its own digest does not derive is refused by
/// name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutedRecordV1 {
    /// SHA-256 of the published record body.
    pub digest: [u8; 32],
    /// The raw-record address the publisher recorded.
    pub address: Pubkey,
}

/// The nine published records the payout frame authenticates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRecordRoutingV1 {
    /// Realm record.
    pub realm: RoutedRecordV1,
    /// Product Runtime V2 root record.
    pub product: RoutedRecordV1,
    /// Result-domain record.
    pub result_domain: RoutedRecordV1,
    /// Portfolio record.
    pub portfolio: RoutedRecordV1,
    /// Linked graded-basis record.
    pub product_basis: RoutedRecordV1,
    /// Native composition descriptor record.
    pub composition_descriptor: RoutedRecordV1,
    /// Native composition graph record.
    pub composition_graph: RoutedRecordV1,
    /// Native composition translation record.
    pub composition_translation: RoutedRecordV1,
    /// Native composition exposure record.
    pub composition_exposure: RoutedRecordV1,
}

/// Which addresses to observe, and the digests that address them.
///
/// AN ADDRESS BOOK, NEVER AUTHORITY. The CLI projects this out of one sealed
/// `dclutch-successor-campaign-report-v1`; the campaign emitter and that
/// report's parser deliberately live together and are not moved here. What
/// crosses the boundary is the projection the derivation actually reads, which
/// is exactly this — so no client grows a second list of the report's
/// execution, transaction or account fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRoutingTableV1 {
    /// The Market the founding recorded, checked against the requested one.
    pub founding_market: Pubkey,
    /// The Realm's collateral mint.
    pub collateral_mint: Pubkey,
    /// The token program that owns that mint.
    pub token_program: Pubkey,
    /// The nine published records.
    pub records: TerminalRecordRoutingV1,
}

/// One caller's payout request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalPayoutRequestV1 {
    /// The Market being redeemed against.
    pub market: Pubkey,
    /// The wallet that owns the Claims Position.
    pub owner: Pubkey,
    /// The token account the proceeds are paid into.
    pub recipient: Pubkey,
    /// Which winning claim index to redeem.
    pub claim_index: u32,
    /// How many atoms, or the whole authenticated balance when absent.
    pub quantity: Option<u64>,
}

/// What live Core's accepted terminal receipt means.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPayoutReceiptMeaningV1 {
    /// Every admitted Core writer persists the exact Resolution certificate
    /// account key. Market-family interpretation belongs to the authenticated
    /// certificate body, never to this identity.
    ResolutionCertificate(Pubkey),
}

impl TerminalPayoutReceiptMeaningV1 {
    /// A reader-facing name for this meaning.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ResolutionCertificate(_) => "Resolution certificate",
        }
    }
}

/// The routed frame: a payout input that selects accounts but not yet a
/// quantity, and the addresses it authenticates.
#[derive(Clone)]
pub struct RoutedTerminalFrameV1 {
    input: PlanInputV1,
    selected: SelectedInputV1,
}

impl RoutedTerminalFrameV1 {
    /// Every address round two observes, in the derivation's own order.
    ///
    /// The caller reads exactly this list, at the floor round one established.
    /// Handing back the derivation's own addresses — rather than letting a
    /// client assemble them — is what keeps a second routing implementation
    /// from existing.
    pub fn addresses(&self) -> Vec<Pubkey> {
        self.selected.addresses()
    }

    /// The Claims Position this payout debits.
    pub fn position(&self) -> Pubkey {
        self.selected.position
    }

    /// The routed input before quantity and parent context are filled in.
    pub fn routed_input(&self) -> &PlanInputV1 {
        &self.input
    }
}

/// The payout input, and what the live receipt it authenticated means.
#[derive(Clone, Debug)]
pub struct CompletedTerminalPayoutInputV1 {
    /// The exact `dclutch-wallet-terminal-payout-plan-input-v1` stage two takes.
    pub input: PlanInputV1,
    /// The meaning of the terminal receipt live Core carries.
    pub receipt_meaning: TerminalPayoutReceiptMeaningV1,
}

/// The Claims aggregate address for one Market.
///
/// Decision 0008 §1: the Claims aggregate is the SOLE persisted owner of a
/// Market's Custody namespace, and no route may re-guess it. It is a PDA of the
/// Market under the Claims program, so it is derivable before any read — which
/// is why the Custody context costs no round of its own.
pub fn claims_aggregate_address_v1(claims: Pubkey, market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()], &claims).0
}

/// The conventional destination for a payout: the owner's associated token
/// account for the collateral mint.
///
/// A DEFAULT, NOT A RULE. The protocol takes any token account the owner
/// controls and this crate changes nothing about that; the CLI still names one
/// with `--recipient` and a browser caller that names one overrides this. What
/// it removes is the last thing a stranger had to know before redeeming, and
/// it removes it by the standard convention rather than by a new one.
///
/// The associated-token-account program is pinned BY CONSTANT NAME from the
/// interface crate that declares it, and the address comes from that crate's
/// own derivation rather than from seeds written down here.
pub fn associated_token_account_v1(
    owner: Pubkey,
    collateral_mint: Pubkey,
    token_program: Pubkey,
) -> Pubkey {
    get_associated_token_address_with_program_id(&owner, &collateral_mint, &token_program)
}

/// The associated-token-account program this crate derives under.
///
/// Exposed so a client can state which program the default came from instead of
/// writing its id down.
pub fn associated_token_account_program_v1() -> Pubkey {
    ASSOCIATED_TOKEN_PROGRAM_ID
}

/// The release set this Market selected, read from the Market itself.
///
/// The sixth coordinate. A browser's deployment table names five program ids
/// and no release set — the release set is the MARKET's choice, not the
/// deployment's — so a caller that holds only a deployment reads it here, from
/// round one, before it can state the coordinates the rest of the derivation
/// takes. A caller that already pins one (the CLI does, from its plan) supplies
/// it instead and keeps the two-source check in
/// [`decode_routed_market_v1`].
pub fn market_release_set_v1(
    core: Pubkey,
    market: Pubkey,
    round_one: &FinalizedSnapshotV1,
) -> Result<[u8; 32]> {
    let account = round_one.required(market, "Core Market")?;
    if account.owner != core || account.executable {
        return Err(Error::new(format!(
            "the Core Market at {market} is owned by {}, not the deployment's Core program {core}",
            account.owner
        )));
    }
    let state = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    if state.identity.market_id.to_bytes() != market.to_bytes() {
        return Err(Error::new(
            "the Core Market names another market as its own identity",
        ));
    }
    Ok(state.identity.selected_release_set.to_bytes())
}

/// PHASE ONE — the two addresses round one observes.
///
/// Pure. Given the routing table and the request, this authenticates the
/// table's Market against the requested one and names the Core Market and the
/// Claims aggregate. Nothing else is knowable before those two are read.
pub fn terminal_payout_round_one_addresses_v1(
    coordinates: &ProtocolCoordinatesV1,
    routing: &TerminalRoutingTableV1,
    request: &TerminalPayoutRequestV1,
) -> Result<[Pubkey; 2]> {
    if routing.founding_market != request.market {
        return Err(Error::new(
            "terminal Market differed from exact founding campaign evidence",
        ));
    }
    Ok([
        request.market,
        claims_aggregate_address_v1(coordinates.claims, request.market),
    ])
}

/// PHASE TWO — the routed frame, from round one's two observations.
///
/// Pure. The Market's own state supplies the terminal receipt and the routing
/// authentication; the aggregate supplies the Custody namespace, read from the
/// account that owns it rather than taken from a document that could name
/// another Market's.
pub fn route_terminal_payout_frame_v1(
    coordinates: &ProtocolCoordinatesV1,
    routing: &TerminalRoutingTableV1,
    request: &TerminalPayoutRequestV1,
    round_one: &FinalizedSnapshotV1,
) -> Result<RoutedTerminalFrameV1> {
    let market_account = round_one.required(request.market, "Core Market")?;
    let live_market = decode_routed_market_v1(market_account, coordinates)?;
    let terminal_receipt = live_market
        .terminal_receipt
        .ok_or_else(|| Error::new("Core Market has no accepted terminal receipt"))?
        .to_bytes();
    let aggregate_key = claims_aggregate_address_v1(coordinates.claims, request.market);
    let aggregate_account = round_one.required(aggregate_key, "Claims aggregate")?;
    let custody_context =
        observed_custody_context_v1(aggregate_account, coordinates.claims, request.market)?;
    let input = routed_input_v1(
        coordinates,
        routing,
        request,
        terminal_receipt,
        custody_context,
    );
    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    authenticate_routing_hints_v1(&selected, routing)?;
    Ok(RoutedTerminalFrameV1 { input, selected })
}

/// PHASE THREE — the payout input, from round two's observations.
///
/// Pure. The quantity comes from the authenticated Position balance, the parent
/// context from the immutable request and the authenticated prestate, and the
/// whole graph is re-authenticated by stage two's own report builder before
/// this returns.
pub fn complete_terminal_payout_input_v1(
    frame: &RoutedTerminalFrameV1,
    round_two: &FinalizedSnapshotV1,
    request: &TerminalPayoutRequestV1,
) -> Result<CompletedTerminalPayoutInputV1> {
    let addresses = frame.addresses();
    let receipt_meaning =
        authenticate_core_terminal_receipt_meaning_v1(&frame.selected, round_two)?;

    let position_account = round_two.required(frame.selected.position, "Claims Position")?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("Claims Position: {error:?}")))?;
    let full_balance = position
        .balance(&position_account.data, request.claim_index)
        .map_err(|error| Error::new(format!("Claims Position balance: {error:?}")))?;
    let quantity = request.quantity.unwrap_or(full_balance);
    if quantity == 0 || quantity > full_balance {
        return Err(Error::new(format!(
            "payout quantity must be within 1..={full_balance} atoms at claim index {}",
            request.claim_index
        )));
    }
    let mut input = frame.input.clone();
    input.quantity = quantity.to_string();
    input.parent_context = hex(&stable_parent_context_v1(
        &frame.selected,
        round_two,
        quantity,
        request.claim_index,
    )?);

    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    if selected.addresses() != addresses {
        return Err(Error::new(
            "wallet payout selectors changed after authenticated quantity/context construction",
        ));
    }
    let _authenticated = build_report(&selected, round_two)?;
    Ok(CompletedTerminalPayoutInputV1 {
        input,
        receipt_meaning,
    })
}

fn authenticate_core_terminal_receipt_meaning_v1(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<TerminalPayoutReceiptMeaningV1> {
    let market = CoreState::decode(&snapshot.required(selected.market, "Core Market")?.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let receipt = market
        .terminal_receipt
        .ok_or_else(|| Error::new("Core Market has no accepted terminal receipt"))?
        .to_bytes();
    if receipt != selected.terminal_certificate.to_bytes() {
        return Err(Error::new(
            "live Core terminal receipt differs from the projected payout identity",
        ));
    }
    Ok(TerminalPayoutReceiptMeaningV1::ResolutionCertificate(
        Pubkey::new_from_array(receipt),
    ))
}

/// Decode one Core Market and authenticate its owner, address, Registry and
/// release-set routing against the coordinates the caller deployed from.
pub fn decode_routed_market_v1(
    account: &ObservedAccount,
    coordinates: &ProtocolCoordinatesV1,
) -> Result<CoreState> {
    let market = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    if account.owner != coordinates.core
        || account.executable
        || account.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.identity.registry_program.to_bytes() != coordinates.registry.to_bytes()
        || market.identity.selected_release_set.to_bytes() != coordinates.release_set
    {
        return Err(Error::new(
            "Core Market owner/address/Registry/release-set routing authentication refused",
        ));
    }
    Ok(market)
}

/// Authenticate one Claims aggregate against a Market, and refuse the retiring
/// path while any claim still has supply.
///
/// # The two ways a supply can be nonzero, and why they need different words
///
/// An ORDINARY coordinate with outstanding supply means holders have not been
/// paid, and "produce and execute wallet terminal payouts first" is a true
/// instruction: somebody holds a key and can follow it.
///
/// A refunding Market's FAILURE coordinate is not that. Decision 0025 seats it
/// in an escrow Position whose owner is a program-derived address with no key,
/// no certificate pays it (`runtime_v3`'s `FailureCoordinateNotPayable` and the
/// failure arm's "nothing to the failure coordinate"), and the merge that could
/// move it wants a Hoard terminal settlement has already drained. Telling an
/// operator to produce a payout for it is instructing an act NO party can
/// perform, and that is what this function said until the addendum to decision
/// 0025 (2026-09-05) named the shape.
///
/// The burn shipped at `7d45d6ba3` (2026-09-06), and THIS FUNCTION NOW ADMITS
/// the seated residue rather than refusing it. That is not a weakened refusal:
/// the accusation "supply is outstanding" was never true of this coordinate,
/// and the act that discharges it is a step of the retirement itself -- the
/// checkpointed route's `prepare` packet burns the column inside the Claims
/// closure, with the escrow pair and the linked basis record in frame. What
/// this function refuses is everything that is NOT that: an ordinary
/// coordinate with supply, a failure column only partly in the escrow, an
/// escrow holding a tradeable claim beside the residue, and an unobserved
/// escrow -- because "the escrow does not hold it" and "nobody looked" are the
/// same number and different facts.
///
/// Whether the burn REACHES a given Market is a question about the release set
/// it was founded on, which decision 0012 forbids changing; this function reads
/// one aggregate and cannot answer it, and does not pretend to. A Market
/// founded before the burn is admitted here and refused on chain by name
/// (`ClaimsMarketClosureSbfErrorV1::Liability`, `0x5503`) when its retirement
/// reaches the closure's supply loop.
///
/// `escrow` is the observation of the derived failure-escrow Position, which
/// the caller finds with [`crate::failure_escrow_v1::failure_escrow_v1`] off
/// this same aggregate. It is optional because a categorical Market has no such
/// account; passing `None` where the failure column IS outstanding is refused
/// by name rather than read as absence.
pub fn authenticate_zero_claims_v1(
    account: &ObservedAccount,
    expected: Pubkey,
    claims: Pubkey,
    market: CoreState,
    custody_context: [u8; 32],
    escrow: Option<&ObservedAccount>,
) -> Result<()> {
    let aggregate = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if account.key != expected
        || account.owner != claims
        || account.executable
        || aggregate.logical_market != market.identity.market_id.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market.identity.registry_program.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.realm_id != market.identity.realm_id.to_bytes()
        || aggregate.custody_context != custody_context
        || aggregate.generation != market.identity.generation
    {
        return Err(Error::new(
            "Claims aggregate address/owner/Market/release/Product/Realm/custody/generation join refused",
        ));
    }
    let derived = crate::failure_escrow_v1::failure_escrow_v1(
        claims,
        aggregate.logical_market,
        account.key,
        aggregate.claim_count,
    )
    .ok();
    for claim_index in 0..aggregate.claim_count {
        let supply = aggregate
            .supply(&account.data, claim_index)
            .map_err(|error| Error::new(format!("Claims supply {claim_index}: {error:?}")))?;
        if supply == 0 {
            continue;
        }
        let seated = derived.is_some_and(|derived| derived.failure_selector == claim_index);
        if !seated {
            return Err(Error::new(format!(
                "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply}; produce and execute wallet terminal payouts first"
            )));
        }
        let derived = derived.expect("a seated coordinate has a derived escrow");
        authenticate_seated_failure_column_v1(&aggregate, derived, claim_index, supply, escrow)?;
    }
    Ok(())
}

/// Admit the seated residue, having first checked that it IS the residue rather
/// than a stranger's unpaid claim wearing the same index.
///
/// Every arm below is a DIFFERENT fact from "the closure will burn this", and
/// each one leaves an act some party can still perform. Only the last is the
/// residue, and only the last is admitted.
fn authenticate_seated_failure_column_v1(
    aggregate: &LiabilityBasisMarketViewV2,
    derived: crate::failure_escrow_v1::FailureEscrowV1,
    claim_index: u32,
    supply: u64,
    escrow: Option<&ObservedAccount>,
) -> Result<()> {
    let position = derived.position;
    let Some(escrow) = escrow else {
        return Err(Error::new(format!(
            "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply}, which is \
             this Market's failure coordinate, and the derived escrow Position {position} was not \
             observed. Observe it before reading this supply as either a seated residue or an \
             unpaid holder: the two are the same number and different facts"
        )));
    };
    if escrow.key != position {
        return Err(Error::new(format!(
            "the observation offered for the failure escrow is of {}, not the derived {position}",
            escrow.key
        )));
    }
    if escrow.data.is_empty() || escrow.lamports == 0 {
        return Err(Error::new(format!(
            "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply} and the \
             derived escrow Position {position} does not exist, so this Market did not seat its \
             failure column and the supply is in hands that can be paid; produce and execute \
             wallet terminal payouts first"
        )));
    }
    let held = escrow_native_v1(escrow, aggregate.claim_count)?;
    let outside = held
        .iter()
        .enumerate()
        .filter(|(index, balance)| **balance != 0 && *index != claim_index as usize)
        .map(|(index, balance)| format!("{index}={balance}"))
        .collect::<Vec<_>>();
    let seated = held.get(claim_index as usize).copied().unwrap_or(0);
    if seated != supply || !outside.is_empty() {
        let outside = if outside.is_empty() {
            "nothing".to_owned()
        } else {
            outside.join(", ")
        };
        return Err(Error::new(format!(
            "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply} and the \
             derived escrow {position} holds {seated} there and {outside} elsewhere, so this \
             column is NOT wholly the seated residue. Some of it is in hands that can be paid; \
             produce and execute wallet terminal payouts first"
        )));
    }
    // THE RESIDUE, ADMITTED. The whole failure column is in an escrow whose
    // owner is derived and keyless, nothing else is in it, and no ordinary
    // coordinate carries supply. Nothing about it is an unpaid holder, and the
    // act that discharges it belongs to the retirement, not to this preflight:
    // the checkpointed route's `prepare` burns it inside the Claims closure
    // with this Position, its admission and the Market's linked basis record in
    // frame (decision 0025, shape A, shipped at `7d45d6ba3`).
    Ok(())
}

/// Read one escrow Position's native vector at the aggregate's own width.
fn escrow_native_v1(escrow: &ObservedAccount, claim_count: u32) -> Result<Vec<u64>> {
    let view = LiabilityBasisPositionViewV2::decode(&escrow.data)
        .map_err(|error| Error::new(format!("failure escrow Position: {error:?}")))?;
    if view.claim_count != claim_count {
        return Err(Error::new(format!(
            "the failure escrow carries {} outcomes and the Claims aggregate owes over \
             {claim_count}; the two cannot be compared coordinate by coordinate",
            view.claim_count
        )));
    }
    (0..claim_count)
        .map(|index| {
            view.balance(&escrow.data, index)
                .map_err(|error| Error::new(format!("failure escrow balance {index}: {error:?}")))
        })
        .collect()
}

/// The Market's Custody namespace, read from the account that owns it.
///
/// The aggregate is addressed by derivation rather than by an evidence label,
/// so a substituted document cannot point this at another market's namespace;
/// the seed is the one every other `LiabilityBasisV2` consumer uses.
pub fn observed_custody_context_v1(
    account: &ObservedAccount,
    claims: Pubkey,
    market: Pubkey,
) -> Result<[u8; 32]> {
    let key = claims_aggregate_address_v1(claims, market);
    if account.key != key {
        return Err(Error::new(format!(
            "the observation offered for the Claims aggregate is of {}, not the derived {key}",
            account.key
        )));
    }
    if account.owner != claims || account.executable {
        return Err(Error::new(format!(
            "the Claims aggregate at {key} is owned by {}, not the deployment's Claims program {claims}",
            account.owner
        )));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if aggregate.logical_market != market.to_bytes() {
        return Err(Error::new(
            "the Claims aggregate names another logical market",
        ));
    }
    Ok(aggregate.custody_context)
}

fn routed_input_v1(
    coordinates: &ProtocolCoordinatesV1,
    routing: &TerminalRoutingTableV1,
    request: &TerminalPayoutRequestV1,
    terminal_receipt: [u8; 32],
    custody_context: [u8; 32],
) -> PlanInputV1 {
    let records = &routing.records;
    PlanInputV1 {
        format: INPUT_FORMAT.into(),
        market: request.market.to_string(),
        owner: request.owner.to_string(),
        recipient_owner: request.owner.to_string(),
        recipient: request.recipient.to_string(),
        collateral_mint: routing.collateral_mint.to_string(),
        token_program: routing.token_program.to_string(),
        // Quantity and parent context do not select accounts. They are filled
        // from the authenticated snapshot before this input is emitted.
        quantity: "1".into(),
        claim_index: request.claim_index,
        transfer_index: 0,
        parent_context: hex(&[1; 32]),
        custody_context: hex(&custody_context),
        release_set: hex(&coordinates.release_set),
        terminal_certificate: Pubkey::new_from_array(terminal_receipt).to_string(),
        lookup_table: None,
        programs: ProgramSelectorsV1 {
            registry: coordinates.registry.to_string(),
            core: coordinates.core.to_string(),
            claims: coordinates.claims.to_string(),
            custody: coordinates.custody.to_string(),
            resolution: coordinates.resolution.to_string(),
        },
        records: RecordSelectorsV1 {
            realm: hex(&records.realm.digest),
            product: hex(&records.product.digest),
            result_domain: hex(&records.result_domain.digest),
            portfolio: hex(&records.portfolio.digest),
            product_basis: hex(&records.product_basis.digest),
            composition_descriptor: hex(&records.composition_descriptor.digest),
            composition_graph: hex(&records.composition_graph.digest),
            composition_translation: hex(&records.composition_translation.digest),
            composition_exposure: hex(&records.composition_exposure.digest),
        },
    }
}

fn authenticate_routing_hints_v1(
    selected: &SelectedInputV1,
    routing: &TerminalRoutingTableV1,
) -> Result<()> {
    let records = &routing.records;
    let expected = [
        ("realm_record", records.realm, selected.realm.raw),
        ("product_record", records.product, selected.product.raw),
        (
            "result_domain_record",
            records.result_domain,
            selected.result_domain.raw,
        ),
        (
            "portfolio_record",
            records.portfolio,
            selected.portfolio.raw,
        ),
        (
            "linked_liability_basis_record",
            records.product_basis,
            selected.product_basis.raw,
        ),
        (
            "terminal_composition_descriptor_record",
            records.composition_descriptor,
            selected.composition_descriptor.raw,
        ),
        (
            "terminal_composition_graph_record",
            records.composition_graph,
            selected.composition_graph.raw,
        ),
        (
            "terminal_composition_translation_record",
            records.composition_translation,
            selected.composition_translation.raw,
        ),
        (
            "terminal_composition_exposure_record",
            records.composition_exposure,
            selected.composition_exposure.raw,
        ),
    ];
    for (label, routed, derived) in expected {
        if routed.address != derived {
            return Err(Error::new(format!(
                "persisted {label} address {} is not the canonical raw-record PDA {derived}",
                routed.address
            )));
        }
    }
    Ok(())
}

fn stable_parent_context_v1(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    quantity: u64,
    claim_index: u32,
) -> Result<[u8; 32]> {
    let market = snapshot.required(selected.market, "Core Market")?;
    let aggregate = snapshot.required(selected.aggregate, "Claims aggregate")?;
    let position = snapshot.required(selected.position, "Claims Position")?;
    let replay = snapshot.required(selected.custody_replay, "Claims Custody replay")?;
    let hoard = snapshot.required(selected.hoard, "Hoard token account")?;
    let recipient = snapshot.required(selected.recipient, "recipient token account")?;
    let market_digest = Sha256::digest(&market.data);
    let aggregate_digest = Sha256::digest(&aggregate.data);
    let position_digest = Sha256::digest(&position.data);
    let replay_digest = Sha256::digest(&replay.data);
    let hoard_digest = Sha256::digest(&hoard.data);
    let recipient_digest = Sha256::digest(&recipient.data);
    let quantity_bytes = quantity.to_le_bytes();
    let claim_index_bytes = claim_index.to_le_bytes();
    let transfer_index_bytes = 0_u16.to_le_bytes();
    let context = hashv(&[
        PARENT_CONTEXT_DOMAIN_V1,
        selected.market.as_ref(),
        selected.owner.as_ref(),
        selected.position.as_ref(),
        selected.recipient.as_ref(),
        &quantity_bytes,
        &claim_index_bytes,
        &transfer_index_bytes,
        &selected.release_set,
        selected.terminal_certificate.as_ref(),
        &market_digest,
        &aggregate_digest,
        &position_digest,
        &replay_digest,
        &hoard_digest,
        &recipient_digest,
    ])
    .to_bytes();
    if context == [0; 32] {
        return Err(Error::new("derived wallet payout parent context was zero"));
    }
    Ok(context)
}

#[cfg(test)]
mod tests {
    use crate::wallet_terminal_payout::{hex32, pubkey, wire::RecordPairV1};
    use crate::{Finality, Observation};
    use dclutch_claims::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        encode_liability_basis_market_into_v2, liability_basis_vector_width_v2,
    };
    use dclutch_market::{Identity, MarketIdentity, Phase, Readiness, StateBumpsV1};
    use solana_sdk_ids::system_program;

    use super::*;

    // ONE SHARED FIXTURE. `wire::tests::input()` is the payout operator's own,
    // reachable under `test-fixtures` rather than copied: a second copy would
    // drift, and stage one exists to produce exactly what stage two consumes.
    fn fixture() -> PlanInputV1 {
        let mut value = crate::wallet_terminal_payout::wire::tests::input();
        value.lookup_table = None;
        value
    }

    fn observation(slot: u64) -> Observation {
        Observation {
            slot,
            unix_timestamp: 1_700_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed(key: Pubkey, tag: u8, slot: u64) -> ObservedAccount {
        ObservedAccount {
            observation: observation(slot),
            key,
            owner: system_program::ID,
            lamports: 1,
            executable: false,
            data: vec![tag; 32],
        }
    }

    fn coordinates_from(input: &PlanInputV1) -> ProtocolCoordinatesV1 {
        ProtocolCoordinatesV1 {
            registry: pubkey(&input.programs.registry).unwrap(),
            core: pubkey(&input.programs.core).unwrap(),
            claims: pubkey(&input.programs.claims).unwrap(),
            custody: pubkey(&input.programs.custody).unwrap(),
            resolution: pubkey(&input.programs.resolution).unwrap(),
            release_set: hex32(&input.release_set).unwrap(),
        }
    }

    fn request_from(input: &PlanInputV1) -> TerminalPayoutRequestV1 {
        TerminalPayoutRequestV1 {
            market: pubkey(&input.market).unwrap(),
            owner: pubkey(&input.owner).unwrap(),
            recipient: pubkey(&input.recipient).unwrap(),
            claim_index: input.claim_index,
            quantity: Some(7),
        }
    }

    /// The routing table an honest publisher would have recorded: every address
    /// is the canonical raw-record PDA its own digest derives.
    fn routing_from(input: &PlanInputV1) -> TerminalRoutingTableV1 {
        let selected = SelectedInputV1::parse(input, LookupTableRequirementV1::Absent)
            .expect("fixture routes");
        let routed = |pair: RecordPairV1| RoutedRecordV1 {
            digest: pair.digest,
            address: pair.raw,
        };
        TerminalRoutingTableV1 {
            founding_market: pubkey(&input.market).unwrap(),
            collateral_mint: pubkey(&input.collateral_mint).unwrap(),
            token_program: pubkey(&input.token_program).unwrap(),
            records: TerminalRecordRoutingV1 {
                realm: routed(selected.realm),
                product: routed(selected.product),
                result_domain: routed(selected.result_domain),
                portfolio: routed(selected.portfolio),
                product_basis: routed(selected.product_basis),
                composition_descriptor: routed(selected.composition_descriptor),
                composition_graph: routed(selected.composition_graph),
                composition_translation: routed(selected.composition_translation),
                composition_exposure: routed(selected.composition_exposure),
            },
        }
    }

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    fn core_state(
        coordinates: &ProtocolCoordinatesV1,
        market: Pubkey,
        receipt: [u8; 32],
    ) -> CoreState {
        CoreState {
            phase: Phase::Terminal,
            readiness: Readiness::Consumed,
            terminal_winner: 1,
            identity: MarketIdentity {
                market_id: Identity::new(market.to_bytes()).expect("market identity"),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(6),
                selected_release_set: Identity::new(coordinates.release_set)
                    .expect("release set identity"),
                registry_program: Identity::new(coordinates.registry.to_bytes())
                    .expect("registry identity"),
                generation: 9,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: 1,
            rent_beneficiary: identity(10),
            terminal_receipt: Some(Identity::new(receipt).expect("receipt identity")),
            bumps: StateBumpsV1::UNRECORDED,
        }
    }

    fn market_account(
        coordinates: &ProtocolCoordinatesV1,
        market: Pubkey,
        receipt: [u8; 32],
        slot: u64,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: observation(slot),
            key: market,
            owner: coordinates.core,
            lamports: 1,
            executable: false,
            data: core_state(coordinates, market, receipt)
                .encode()
                .expect("Core Market encodes")
                .to_vec(),
        }
    }

    fn aggregate_account(
        coordinates: &ProtocolCoordinatesV1,
        market: Pubkey,
        custody_context: [u8; 32],
        slot: u64,
    ) -> ObservedAccount {
        let key = claims_aggregate_address_v1(coordinates.claims, market);
        let mut data =
            vec![
                0;
                liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 3)
                    .expect("aggregate width")
            ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: market.to_bytes(),
                release_set: coordinates.release_set,
                registry_program: coordinates.registry.to_bytes(),
                product_instance_id: [4; 32],
                basis_id: [12; 32],
                realm_id: [2; 32],
                custody_context,
                generation: 9,
            },
            &[0, 0, 0],
            &mut data,
        )
        .expect("aggregate encodes");
        ObservedAccount {
            observation: observation(slot),
            key,
            owner: coordinates.claims,
            lamports: 1,
            executable: false,
            data,
        }
    }

    fn round_one(
        coordinates: &ProtocolCoordinatesV1,
        input: &PlanInputV1,
        slot: u64,
    ) -> FinalizedSnapshotV1 {
        let market = pubkey(&input.market).unwrap();
        let receipt = pubkey(&input.terminal_certificate).unwrap().to_bytes();
        let custody_context = hex32(&input.custody_context).unwrap();
        let accounts = [
            market_account(coordinates, market, receipt, slot),
            aggregate_account(coordinates, market, custody_context, slot),
        ];
        FinalizedSnapshotV1 {
            observation: observation(slot),
            accounts: accounts
                .into_iter()
                .map(|account| (account.key, account))
                .collect(),
        }
    }

    /// PHASE ONE names two addresses and derives the second one.
    ///
    /// The Claims aggregate is a PDA of the Market under the deployment's
    /// Claims program, so it is knowable before any read. That is the whole
    /// reason this derivation takes two RPC rounds and not three.
    #[test]
    fn round_one_names_the_market_and_the_derived_aggregate() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let routing = routing_from(&input);
        let request = request_from(&input);
        let keys = terminal_payout_round_one_addresses_v1(&coordinates, &routing, &request)
            .expect("round one addresses");
        assert_eq!(keys[0], request.market);
        assert_eq!(
            keys[1],
            claims_aggregate_address_v1(coordinates.claims, request.market)
        );
    }

    #[test]
    fn a_routing_table_for_another_market_is_refused_before_any_read() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let mut routing = routing_from(&input);
        routing.founding_market = Pubkey::new_unique();
        let error =
            terminal_payout_round_one_addresses_v1(&coordinates, &routing, &request_from(&input))
                .expect_err("a cross-Market routing table must refuse");
        assert!(
            error
                .to_string()
                .contains("exact founding campaign evidence")
        );
    }

    /// PHASE TWO reproduces the artifact stage two consumes.
    ///
    /// The control this test is: the routed input is the SHARED FIXTURE, field
    /// for field, except the two coordinates phase three fills in. If phase two
    /// had grown its own opinion about any selector, this is where it would
    /// show.
    #[test]
    fn phase_two_reproduces_the_input_stage_two_consumes() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let routing = routing_from(&input);
        let request = request_from(&input);
        let frame = route_terminal_payout_frame_v1(
            &coordinates,
            &routing,
            &request,
            &round_one(&coordinates, &input, 100),
        )
        .expect("the frame routes");
        let routed = frame.routed_input();
        assert_eq!(routed.format, input.format);
        assert_eq!(routed.market, input.market);
        assert_eq!(routed.owner, input.owner);
        assert_eq!(routed.recipient_owner, input.recipient_owner);
        assert_eq!(routed.recipient, input.recipient);
        assert_eq!(routed.collateral_mint, input.collateral_mint);
        assert_eq!(routed.token_program, input.token_program);
        assert_eq!(routed.claim_index, input.claim_index);
        assert_eq!(routed.transfer_index, input.transfer_index);
        assert_eq!(routed.custody_context, input.custody_context);
        assert_eq!(routed.release_set, input.release_set);
        assert_eq!(routed.terminal_certificate, input.terminal_certificate);
        assert_eq!(routed.lookup_table, None);
        assert_eq!(routed.programs.registry, input.programs.registry);
        assert_eq!(routed.programs.core, input.programs.core);
        assert_eq!(routed.programs.claims, input.programs.claims);
        assert_eq!(routed.programs.custody, input.programs.custody);
        assert_eq!(routed.programs.resolution, input.programs.resolution);
        assert_eq!(routed.records.realm, input.records.realm);
        assert_eq!(routed.records.product, input.records.product);
        assert_eq!(routed.records.result_domain, input.records.result_domain);
        assert_eq!(routed.records.portfolio, input.records.portfolio);
        assert_eq!(routed.records.product_basis, input.records.product_basis);
        assert_eq!(
            routed.records.composition_descriptor,
            input.records.composition_descriptor
        );
        assert_eq!(
            routed.records.composition_graph,
            input.records.composition_graph
        );
        assert_eq!(
            routed.records.composition_translation,
            input.records.composition_translation
        );
        assert_eq!(
            routed.records.composition_exposure,
            input.records.composition_exposure
        );
        // Round two reads exactly what the frame names, and nothing assembles
        // that list anywhere else.
        assert!(frame.addresses().contains(&request.market));
        assert!(frame.addresses().contains(&frame.position()));
    }

    /// A routing table whose address is not its own digest's PDA is refused BY
    /// LABEL.
    ///
    /// This is the check that makes the table an address book rather than an
    /// authority: the digest derives the address, and a publisher that recorded
    /// another one is naming a record the payout would then authenticate in
    /// place of the real one.
    #[test]
    fn a_record_address_that_its_digest_does_not_derive_is_refused_by_name() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let request = request_from(&input);
        let snapshot = round_one(&coordinates, &input, 100);
        for (label, mutate) in [
            (
                "terminal_composition_exposure_record",
                (|table: &mut TerminalRoutingTableV1| {
                    table.records.composition_exposure.address = Pubkey::new_unique();
                }) as fn(&mut TerminalRoutingTableV1),
            ),
            ("realm_record", |table: &mut TerminalRoutingTableV1| {
                table.records.realm.address = Pubkey::new_unique();
            }),
        ] {
            let mut routing = routing_from(&input);
            mutate(&mut routing);
            let error = route_terminal_payout_frame_v1(&coordinates, &routing, &request, &snapshot)
                .map(|_| ())
                .expect_err("a substituted record address must refuse");
            assert!(
                error.to_string().contains(label)
                    && error.to_string().contains("canonical raw-record PDA"),
                "{label}: {error}"
            );
        }
    }

    /// The Custody namespace is read from the account that owns it.
    #[test]
    fn the_aggregate_must_be_the_derived_one_and_own_the_namespace() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let market = pubkey(&input.market).unwrap();
        let custody_context = hex32(&input.custody_context).unwrap();
        let account = aggregate_account(&coordinates, market, custody_context, 100);
        assert_eq!(
            observed_custody_context_v1(&account, coordinates.claims, market).expect("namespace"),
            custody_context
        );

        let mut foreign = account.clone();
        foreign.owner = Pubkey::new_unique();
        let error = observed_custody_context_v1(&foreign, coordinates.claims, market)
            .expect_err("a foreign owner must refuse");
        assert!(
            error
                .to_string()
                .contains("not the deployment's Claims program")
        );

        let mut mispaired = account.clone();
        mispaired.key = Pubkey::new_unique();
        let error = observed_custody_context_v1(&mispaired, coordinates.claims, market)
            .expect_err("an observation of another account must refuse");
        assert!(error.to_string().contains("not the derived"));
    }

    /// The Market's own routing is authenticated against the deployment.
    #[test]
    fn a_market_that_selects_another_registry_or_release_set_is_refused() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let market = pubkey(&input.market).unwrap();
        let receipt = pubkey(&input.terminal_certificate).unwrap().to_bytes();
        let account = market_account(&coordinates, market, receipt, 100);
        decode_routed_market_v1(&account, &coordinates).expect("the fixture Market routes");

        let mut other_registry = coordinates;
        other_registry.registry = Pubkey::new_unique();
        assert!(
            decode_routed_market_v1(&account, &other_registry)
                .expect_err("another Registry must refuse")
                .to_string()
                .contains("routing authentication refused")
        );

        let mut other_release = coordinates;
        other_release.release_set = [99; 32];
        assert!(decode_routed_market_v1(&account, &other_release).is_err());

        let mut other_core = coordinates;
        other_core.core = Pubkey::new_unique();
        assert!(decode_routed_market_v1(&account, &other_core).is_err());
    }

    /// The live Core receipt, not the projected one, is the terminal identity.
    #[test]
    fn a_projected_certificate_that_live_core_does_not_carry_is_refused() {
        let input = fixture();
        let coordinates = coordinates_from(&input);
        let market = pubkey(&input.market).unwrap();
        let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)
            .expect("fixture routes");
        let mut substituted = pubkey(&input.terminal_certificate).unwrap().to_bytes();
        substituted[0] ^= 1;
        let snapshot = FinalizedSnapshotV1 {
            observation: observation(100),
            accounts: [market_account(&coordinates, market, substituted, 100)]
                .into_iter()
                .map(|account| (account.key, account))
                .collect(),
        };
        let error = authenticate_core_terminal_receipt_meaning_v1(&selected, &snapshot)
            .expect_err("a receipt live Core does not carry must refuse");
        assert!(
            error
                .to_string()
                .contains("live Core terminal receipt differs")
        );
    }

    fn context_fixture(slot: u64) -> (SelectedInputV1, FinalizedSnapshotV1) {
        let value = fixture();
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Absent)
            .expect("selected input");
        let keys = [
            selected.market,
            selected.aggregate,
            selected.position,
            selected.custody_replay,
            selected.hoard,
            selected.recipient,
        ];
        let accounts = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, observed(key, u8::try_from(index + 1).unwrap(), slot)))
            .collect();
        (
            selected,
            FinalizedSnapshotV1 {
                observation: observation(slot),
                accounts,
            },
        )
    }

    #[test]
    fn retry_context_ignores_observation_slot_but_binds_request_and_prestate() {
        let (selected_a, snapshot_a) = context_fixture(100);
        let (mut selected_b, mut snapshot_b) = context_fixture(200);
        let first = stable_parent_context_v1(&selected_a, &snapshot_a, 7, 1).unwrap();
        let retry = stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap();
        assert_eq!(first, retry, "finalized slot is not caller entropy");

        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 6, 1).unwrap()
        );
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 0).unwrap()
        );
        selected_b.owner = Pubkey::new_unique();
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.owner = selected_a.owner;
        let mut substituted_certificate = selected_b.terminal_certificate.to_bytes();
        substituted_certificate[0] ^= 1;
        selected_b.terminal_certificate = Pubkey::new_from_array(substituted_certificate);
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.terminal_certificate = selected_a.terminal_certificate;
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        selected_b.recipient = Pubkey::new_unique();
        snapshot_b
            .accounts
            .insert(selected_b.recipient, observed(selected_b.recipient, 6, 200));
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
    }

    #[test]
    fn context_refuses_a_missing_authenticated_prestate() {
        let (selected, mut snapshot) = context_fixture(100);
        snapshot.accounts.remove(&selected.custody_replay);
        let error = stable_parent_context_v1(&selected, &snapshot, 7, 1)
            .expect_err("missing replay must refuse");
        assert!(error.to_string().contains("snapshot omitted"));
    }
}
