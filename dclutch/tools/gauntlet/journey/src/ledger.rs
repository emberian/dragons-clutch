//! The journey's conservation ledger.
//!
//! One object, threaded through every stage, that re-reads the whole economic
//! state from the chain at each stage boundary and evaluates the same fixed set
//! of laws over it. It is deliberately NOT a set of per-step spot checks: a
//! spot check answers "did this transaction do what it said," and a market can
//! pass every one of those while leaking atoms across the seams between them.
//!
//! ## The laws
//!
//! **L1 collateral closure.** The sum of every collateral-mint token account
//! the ledger tracks equals the Mint's own `supply`. This is the law that
//! catches an escape: an atom that moved to an account nobody registered makes
//! the tracked sum fall short of a supply that did not change.
//!
//! **L2 Hoard movement is declared.** Between boundaries the Hoard's balance may
//! change only by the amount the stage DECLARED it would change. This is the
//! law L1 cannot state: collateral moving from the Hoard into a wallet the
//! ledger already tracks leaves L1 perfectly balanced, and that movement is
//! exactly what an undetected leak of principal looks like. Stating it as
//! `hoard + Σ wallets == supply` instead — the obvious phrasing — would have
//! been L1's arithmetic rewritten, unable to fail on its own, and a law that
//! cannot fail independently is decoration.
//!
//! **L3 supply-vector agreement.** For every outcome `i`, the sum over all
//! tracked Positions of `balance(i)` equals the Claims aggregate's `supply(i)`.
//! The aggregate is the market's own account of what it owes; the Positions are
//! who it owes it to. A transfer that credits without debiting breaks this even
//! when both accounts individually look well-formed.
//!
//! **L4 full collateralisation.** `hoard >= max_i supply(i) * unit`. dClutch's
//! whole premise is that no outcome can be under-funded, so the worst outcome
//! is the one that has to be covered. `unit` is read from the Registry's own
//! published `ProductBasisV3.payout_scale`, not from the Hoard divided by the
//! outstanding supply — that phrasing would make L4 assert that the founding
//! equals itself.
//!
//! **L5 stage delta.** Between consecutive observations the change in tracked
//! collateral must equal the change the stage DECLARED. A stage that moves
//! atoms says how many before it runs; a stage that says zero and moves any is
//! a failure even if L1 still balances afterwards, because L1 balances for a
//! transfer between two tracked accounts.
//!
//! **L6 rent conservation.** Lamports that leave a closed protocol account
//! arrive at the declared beneficiary. Rent is the one value in the system that
//! is not collateral and still must not evaporate.
//!
//! **L7 lamport accounting.** The fee payer's lamports move by exactly the fees
//! its own transactions paid, plus whatever landed in an account this ledger
//! watches. This is the law the other six cannot state, and the trading stages
//! are what forced it: L1..L5 are about collateral ATOMS and say nothing about
//! the lamport side of a fill, and L6 only fires when a watched account CLOSES,
//! so a route that quietly debited whoever submitted it — or that placed rent
//! into an account nobody named — passes all six. Stated as
//! `payer_delta + fees + watched_growth == 0`, it is the general form of the
//! per-transaction check the rent-sweep stage already made by hand for one
//! route ("the fee payer moved by exactly the fee and nothing else"), and it is
//! exactly the "debit == credit + fee" claim a fill has to satisfy.
//!
//! A law that cannot be evaluated at a given boundary — no aggregate yet, no
//! Hoard yet — is recorded as `Inapplicable` with the reason. It is never
//! silently skipped: a law that quietly stops applying is how a conservation
//! argument rots.

use std::collections::BTreeMap;

use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_custody_contract::{CompartmentV1, CustodyVaultSeedsV1};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase};
use dclutch_token_svm::{MINT_BYTES, Mint, TokenAccount};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, rpc::Rpc};

/// What a stage claims about the lamports it moved, for L7.
///
/// `fees_lamports` is never a prediction: it is summed off the stage's own
/// transaction evidence, so L7 compares the chain against what the chain
/// charged rather than against a number this campaign chose.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct LamportClaimV1 {
    /// Fees the stage's own transactions paid, summed from their evidence.
    pub(crate) fees_lamports: u64,
    /// Lamports the stage placed in accounts this ledger does not watch.
    ///
    /// There is exactly one legitimate source of these and it is named in
    /// `unwatched_note`: an address lookup table's address is derived from the
    /// slot that created it, so it cannot be registered from before it exists
    /// the way every other account this journey creates is. The number is read
    /// off the tables themselves, not chosen — and a nonzero value with an
    /// empty note VIOLATES L7 rather than excusing it, because "declare
    /// whatever makes it balance" is the failure this term could otherwise
    /// become.
    pub(crate) unwatched_lamports: u64,
    pub(crate) unwatched_note: String,
    /// Why L7 cannot be evaluated at this boundary, when it cannot. A stage
    /// that cannot account for its lamports says so; it never stays silent.
    pub(crate) inapplicable: Option<String>,
}

/// What a stage claims it moved, per compartment class.
///
/// L8 used to accept silence and DERIVE a claim from the two numbers a stage
/// already stated. That derivation was sound only for a two-class census --
/// `unclassified = tracked - hoard` -- so every richer market got
/// `inapplicable`, and the eight-class table the law exists to produce could
/// never fill. Worse, the only method that could have filled it,
/// `declare_class_deltas`, had no caller in the tree and carried
/// `#[allow(dead_code)]`: L8 was a law whose input nothing supplied.
///
/// So the claim is now a REQUIRED argument of [`ConservationLedgerV1::observe`].
/// A stage cannot forget to state one, because it cannot compile without
/// stating one, and what the table reports is what a stage DECLARED rather than
/// what arithmetic re-derived from its other declarations.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ClassClaimV1 {
    /// class -> signed atoms. A class ABSENT from this map is a claim of
    /// ZERO, which is the strong statement, not a gap.
    pub(crate) deltas: BTreeMap<String, i128>,
    /// Why L8 cannot be evaluated at this boundary, when it cannot -- exactly
    /// as `LamportClaimV1::inapplicable` does for L7. A stage that cannot
    /// account for its classes says so; it never stays silent.
    pub(crate) inapplicable: Option<String>,
}

impl ClassClaimV1 {
    /// Every compartment class moved by zero.
    ///
    /// The strongest claim a stage can make, and the right one for every stage
    /// whose transfers are between two accounts of the same class: it fails if
    /// a single atom crosses a compartment boundary.
    pub(crate) fn unchanged() -> Self {
        Self {
            deltas: BTreeMap::new(),
            inapplicable: None,
        }
    }

    /// A stage that moved exactly these classes by exactly these amounts.
    /// Every class not named is claimed to have moved zero.
    #[allow(dead_code)]
    pub(crate) fn moves(deltas: BTreeMap<String, i128>) -> Self {
        Self {
            deltas,
            inapplicable: None,
        }
    }

    /// [`Self::moves`] over labels that came from outside this ledger — a
    /// command line, a config, another process.
    ///
    /// A declaration is a claim about a class this census can OBSERVE, and a
    /// label outside [`class_labels`] names nothing observable: it would sit in
    /// the map as a row L8 compares against an absent census entry, so it would
    /// hold at zero and fail at anything else, which is a law testing a typo
    /// rather than a chain. A misspelled compartment would additionally leave
    /// the compartment it MEANT claimed at zero, so a real movement there would
    /// red L8 in a place the declarer never named. It refuses by name instead.
    #[allow(dead_code)]
    pub(crate) fn declaring(deltas: BTreeMap<String, i128>) -> Result<Self> {
        let known = class_labels();
        let unknown: Vec<&str> = deltas
            .keys()
            .map(String::as_str)
            .filter(|label| !known.contains(label))
            .collect();
        if !unknown.is_empty() {
            return Err(Error::new(format!(
                "declared class delta names {} class(es) this census does not report ({}); it \
                 reports exactly {}",
                unknown.len(),
                unknown.join(", "),
                known.join(", ")
            )));
        }
        Ok(Self::moves(deltas))
    }

    /// A stage whose per-class movement this ledger does not account for, and
    /// why.
    pub(crate) fn inapplicable(reason: impl Into<String>) -> Self {
        Self {
            deltas: BTreeMap::new(),
            inapplicable: Some(reason.into()),
        }
    }
}

impl LamportClaimV1 {
    /// A stage whose lamport movement this ledger does not account for, and why.
    pub(crate) fn inapplicable(reason: impl Into<String>) -> Self {
        Self {
            fees_lamports: 0,
            unwatched_lamports: 0,
            unwatched_note: String::new(),
            inapplicable: Some(reason.into()),
        }
    }

    /// A stage that paid exactly these fees and placed everything else in
    /// accounts the ledger watches.
    pub(crate) fn fees(fees_lamports: u64) -> Self {
        Self {
            fees_lamports,
            unwatched_lamports: 0,
            unwatched_note: String::new(),
            inapplicable: None,
        }
    }

    /// A stage that also rent-funded routing tables, whose addresses depend on
    /// the slot that created them and so cannot be watched in advance.
    pub(crate) fn with_unwatched(mut self, lamports: u64, note: impl Into<String>) -> Self {
        self.unwatched_lamports = lamports;
        self.unwatched_note = note.into();
        self
    }
}

/// One law's outcome at one stage boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct VerdictV1 {
    /// `L1`..`L7`.
    pub(crate) law: String,
    /// `holds`, `violated`, or `inapplicable`.
    pub(crate) status: String,
    /// The arithmetic, or the reason the law does not apply here.
    pub(crate) detail: String,
}

impl VerdictV1 {
    fn holds(law: &str, detail: String) -> Self {
        Self {
            law: law.into(),
            status: "holds".into(),
            detail,
        }
    }

    fn violated(law: &str, detail: String) -> Self {
        Self {
            law: law.into(),
            status: "violated".into(),
            detail,
        }
    }

    fn inapplicable(law: &str, detail: &str) -> Self {
        Self {
            law: law.into(),
            status: "inapplicable".into(),
            detail: detail.into(),
        }
    }

    fn failed(&self) -> bool {
        self.status == "violated"
    }
}

/// Every physical Custody compartment, with the label L8 reports it under.
///
/// All nine, deliberately. A per-class law that omitted `HoardPrincipal`
/// because L2 already watches it would leave the same hole in mirror image:
/// atoms could cross from the Hoard into a class L8 does not name, and the two
/// laws would each see a movement the other was supposed to cover.
const COMPARTMENTS: [(CompartmentV1, &str); 9] = [
    (CompartmentV1::None, "None"),
    (CompartmentV1::External, "External"),
    (CompartmentV1::Settlement, "Settlement"),
    (CompartmentV1::HoardPrincipal, "HoardPrincipal"),
    (CompartmentV1::TradingPrincipal, "TradingPrincipal"),
    (CompartmentV1::FeeVault, "FeeVault"),
    (CompartmentV1::LivenessVault, "LivenessVault"),
    (CompartmentV1::SeriesEscrow, "SeriesEscrow"),
    (CompartmentV1::RecoveryReserve, "RecoveryReserve"),
];

/// The class of a tracked collateral account that is not a Custody vault under
/// any namespace this ledger can derive — an ordinary wallet, or a vault under
/// a context the journey has not admitted.
///
/// It is a CLASS, not a bucket to hide in: L8 holds it to a declared delta like
/// every other, so atoms moving into or out of it must still be stated.
const UNCLASSIFIED: &str = "unclassified";

/// Every class label a census can report: one per physical compartment, plus
/// [`UNCLASSIFIED`].
///
/// This is the vocabulary a per-class declaration has to spell, and it is
/// derived from [`COMPARTMENTS`] rather than written a second time, so a
/// compartment renamed there is renamed here. [`ClassClaimV1::declaring`]
/// checks declared labels against it; nothing else may respell them.
#[allow(dead_code)]
pub(crate) fn class_labels() -> Vec<&'static str> {
    COMPARTMENTS
        .iter()
        .map(|(_, label)| *label)
        .chain(std::iter::once(UNCLASSIFIED))
        .collect()
}

/// The exact state of one account the ledger tracks.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AccountStateV1 {
    pub(crate) address: String,
    pub(crate) exists: bool,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
}

/// One complete census of the economic state, plus every law evaluated on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ObservationV1 {
    /// The stage boundary this census was taken at.
    pub(crate) stage: String,
    /// The finalized slot the census was read at.
    pub(crate) slot: u64,
    /// Collateral atoms the stage said it would move in or out of the tracked
    /// set, signed.
    pub(crate) declared_collateral_delta: i128,
    /// Collateral atoms the stage said it would move in or out of the Hoard,
    /// signed. Zero is the strong claim, not the absent one.
    pub(crate) declared_hoard_delta: i128,
    /// What the stage claims about the lamports it moved.
    pub(crate) lamports: LamportClaimV1,
    /// The fee payer's lamports at this census.
    pub(crate) payer_lamports: u64,
    pub(crate) mint_supply: u64,
    /// label -> raw atoms, for every tracked collateral token account.
    pub(crate) token_atoms: BTreeMap<String, u64>,
    /// compartment class -> raw atoms, summed by ADDRESS so an account tracked
    /// under several labels is counted once. This is L8's census.
    pub(crate) class_atoms: BTreeMap<String, u64>,
    /// What the stage claims it moved per class, signed. `None` means the stage
    /// has not stated a per-class claim at all, which makes L8 inapplicable
    /// rather than green: a stage that has not accounted for its classes says
    /// so, exactly as `LamportClaimV1::inapplicable` does for L7.
    pub(crate) declared_class_deltas: ClassClaimV1,
    pub(crate) tracked_collateral: u64,
    pub(crate) hoard_atoms: u64,
    pub(crate) outcome_count: u32,
    /// The Claims aggregate's own liability supply, one entry per outcome.
    pub(crate) aggregate_supply: Vec<u64>,
    /// label -> that Position's claim balances, one entry per outcome.
    pub(crate) position_balances: BTreeMap<String, Vec<u64>>,
    /// The sum over every tracked Position, one entry per outcome.
    pub(crate) position_totals: Vec<u64>,
    /// Every tracked account's exact state, protocol accounts included.
    pub(crate) accounts: BTreeMap<String, AccountStateV1>,
    /// The bound Market's Core lifecycle phase at this census, lowercased.
    /// `None` means no Market was bound, which is every census taken before
    /// this field existed -- hence the `serde` default, so a `--prior` chain
    /// across the change reloads without a schema break.
    #[serde(default)]
    pub(crate) market_phase: Option<String>,
    pub(crate) verdicts: Vec<VerdictV1>,
}

/// What the ledger watches, and the laws it evaluates over it.
pub(crate) struct ConservationLedgerV1 {
    mint: Pubkey,
    hoard: Option<Pubkey>,
    aggregate: Option<Pubkey>,
    /// The Core Market whose LIFECYCLE PHASE decides which laws still apply.
    ///
    /// Deliberately NOT in `watched`: the phase is a fact about a law's
    /// applicability, not a balance L7 differences, and adding an account to
    /// the aperture makes L7 inapplicable at the boundary that admits it. A
    /// ledger given no Market keeps the behaviour it always had.
    market: Option<Pubkey>,
    /// Collateral atoms one claim of one outcome is worth.
    claim_unit_atoms: u64,
    /// The key that pays for every journey-owned transaction. L7 is stated
    /// about this account because it is the only one every stage debits.
    payer: Pubkey,
    token_accounts: BTreeMap<String, Pubkey>,
    positions: BTreeMap<String, Pubkey>,
    watched: BTreeMap<String, Pubkey>,
    /// Vault address -> compartment class, DERIVED from
    /// [`CustodyVaultSeedsV1`], never declared by a caller. The compartment tag
    /// is one of the PDA's own seeds, so an account's class is a fact about its
    /// address: a mislabelled account cannot satisfy L8, and a law that read a
    /// self-declared class would be a guard whose two sides move together.
    vault_classes: BTreeMap<Pubkey, String>,
    observations: Vec<ObservationV1>,
}

impl ConservationLedgerV1 {
    /// Start a ledger over one collateral Mint.
    ///
    /// The Hoard, the aggregate and the claim unit are not known until the
    /// founding commits, so they are admitted later. Until then the laws that
    /// need them record themselves inapplicable rather than vanishing.
    pub(crate) fn new(mint: Pubkey, payer: Pubkey) -> Self {
        Self {
            mint,
            hoard: None,
            aggregate: None,
            market: None,
            claim_unit_atoms: 0,
            payer,
            token_accounts: BTreeMap::new(),
            positions: BTreeMap::new(),
            watched: BTreeMap::new(),
            vault_classes: BTreeMap::new(),
            observations: Vec::new(),
        }
    }

    /// Register a collateral token account. Every one must be named: an
    /// unnamed account is exactly what L1 exists to catch.
    pub(crate) fn track_token_account(&mut self, label: &str, address: Pubkey) {
        self.token_accounts.insert(label.into(), address);
        self.watched.insert(label.into(), address);
    }

    /// Register a Claims Position whose balances participate in L3.
    pub(crate) fn track_position(&mut self, label: &str, address: Pubkey) {
        self.positions.insert(label.into(), address);
        self.watched.insert(label.into(), address);
    }

    /// Watch an account for its exact final state without giving it a role in
    /// the collateral or supply laws — rent credits, replays, permits.
    pub(crate) fn watch(&mut self, label: &str, address: Pubkey) {
        self.watched.insert(label.into(), address);
    }

    /// Derive every Custody vault address under one namespace and record the
    /// compartment each one IS.
    ///
    /// One call per `(market, release_set, context)` the journey can name from
    /// authenticated chain state. Nine `find_program_address` derivations per
    /// call, done once here rather than per census. A vault under a context the
    /// journey has not admitted stays [`UNCLASSIFIED`], which is a class L8
    /// holds to a declared delta like any other — never a hiding place.
    pub(crate) fn admit_custody_namespace(
        &mut self,
        custody_program: Pubkey,
        market: [u8; 32],
        release_set: [u8; 32],
        context: [u8; 32],
    ) {
        for (compartment, label) in COMPARTMENTS {
            let vault = Pubkey::find_program_address(
                &CustodyVaultSeedsV1::new(market, release_set, context, compartment).as_slices(),
                &custody_program,
            )
            .0;
            self.vault_classes.insert(vault, label.to_string());
        }
    }

    /// L8's census: how many atoms each compartment class holds.
    ///
    /// `tracked` is what the labelled collateral accounts hold, already summed
    /// BY ADDRESS so an account watched under several labels counts once.
    ///
    /// The table is then widened to EVERY compartment the admitted namespace
    /// names, read through `atoms_of`, because a table only as wide as
    /// `token_accounts` cannot do what L8 claims to do. A FeeVault or a
    /// LivenessVault that exists on chain and that no stage happened to track
    /// contributed no row at all, so L8 could not hold it to a declared delta —
    /// the law's own doc comment says atoms crossing into "a class L8 does not
    /// name" is the hole it exists to close, and a narrow census is exactly
    /// that hole. A compartment that has never been created reads as ZERO
    /// atoms, which is an observation about a compartment and not an absence.
    fn class_census(
        &self,
        tracked: &BTreeMap<Pubkey, u64>,
        mut atoms_of: impl FnMut(&Pubkey) -> Result<Option<u64>>,
    ) -> Result<BTreeMap<String, u64>> {
        let mut atoms_by_address = tracked.clone();
        for address in self.vault_classes.keys() {
            if atoms_by_address.contains_key(address) {
                continue;
            }
            atoms_by_address.insert(*address, atoms_of(address)?.unwrap_or(0));
        }
        let mut class_atoms: BTreeMap<String, u64> = BTreeMap::new();
        for (address, atoms) in &atoms_by_address {
            let class = self.class_of(address);
            let total = class_atoms.entry(class).or_insert(0);
            *total = total.checked_add(*atoms).ok_or_else(|| {
                Error::new("a compartment class overflowed u64, which no real supply can")
            })?;
        }
        Ok(class_atoms)
    }

    /// The compartment an address IS, by derivation.
    fn class_of(&self, address: &Pubkey) -> String {
        self.vault_classes
            .get(address)
            .cloned()
            .unwrap_or_else(|| UNCLASSIFIED.to_string())
    }

    /// Bind the Core Market whose phase decides which laws still apply.
    ///
    /// L4 is a PRE-TERMINAL invariant, and a Market that has resolved and paid
    /// violates it by construction: settlement moves the whole Hoard out to the
    /// winners and the aggregate's supply vector is untouched, so "Hoard >=
    /// worst outcome" is false about a Market that owes nothing. Without this
    /// binding the census had no way to say that, and cohort-14b's post-payout
    /// L4 read VIOLATED at a boundary where the protocol had done exactly what
    /// it is supposed to do.
    pub(crate) fn track_market(&mut self, address: Pubkey) {
        self.market = Some(address);
    }

    /// Admit the founding's Hoard, aggregate, and per-claim collateral unit.
    pub(crate) fn admit_founding(
        &mut self,
        hoard: Pubkey,
        aggregate: Pubkey,
        claim_unit_atoms: u64,
    ) {
        self.hoard = Some(hoard);
        self.aggregate = Some(aggregate);
        self.claim_unit_atoms = claim_unit_atoms;
        self.token_accounts.insert("hoard".into(), hoard);
        self.watched.insert("hoard".into(), hoard);
        self.watched.insert("claims_aggregate".into(), aggregate);
    }

    /// Take a census at a stage boundary and evaluate every law over it.
    ///
    /// `declared_collateral_delta` is what the stage said it would move; L5
    /// checks the chain against that claim rather than against zero. `classes`
    /// is the same statement per compartment, and it is REQUIRED: L8 spent its
    /// whole existence `inapplicable` because the method that supplied it had
    /// no caller, and a law whose input is optional is a law that reports
    /// nothing.
    pub(crate) fn observe(
        &mut self,
        rpc: &mut Rpc,
        stage: &str,
        declared_collateral_delta: i128,
        declared_hoard_delta: i128,
        lamports: LamportClaimV1,
        classes: ClassClaimV1,
    ) -> Result<()> {
        let payer_lamports = rpc
            .account(self.payer)?
            .map(|account| account.lamports)
            .unwrap_or(0);
        let slot = rpc.finalized_slot()?;
        let mint_account = rpc.required_account(self.mint, "collateral Mint")?;
        let mint_bytes = mint_account
            .data
            .get(..MINT_BYTES)
            .ok_or_else(|| Error::new("collateral Mint is narrower than the base layout"))?;
        let mint = Mint::parse(mint_bytes)
            .map_err(|error| Error::new(format!("collateral Mint: {error:?}")))?;

        let mut token_atoms = BTreeMap::new();
        let mut tracked_collateral: u64 = 0;
        for (label, address) in &self.token_accounts {
            let atoms = match rpc.account(*address)? {
                None => 0,
                Some(account) => {
                    // `parse_base_or_immutable_owner`, not `parse`, and the
                    // reason is that this ledger has to be able to watch the
                    // account a payout PAYS. Under Token-2022 the ATA program
                    // ALWAYS adds `ImmutableOwner`, so a wallet's associated
                    // token account is 170 bytes and the strict parser refuses
                    // it `InvalidLength` -- measured on cohort-14b, where the
                    // 500,000,000-atom founder ATA
                    // `DsQSGKPbmJcZ89xts1Jgs1P5fprmX64fomqGFsQM1kmU` could not
                    // be bound and the post-payout L1 therefore read VIOLATED
                    // for a shortfall that was really an unwatchable account.
                    // This is the SAME admission the chain and the operator
                    // already share, from the same author: `token_svm`'s
                    // `profile.rs` and `wallet_terminal_payout_v3.rs` both call
                    // it, so the ledger now admits exactly the destinations the
                    // protocol admits -- no more, and no fewer.
                    let state = TokenAccount::parse_base_or_immutable_owner(&account.data)
                        .map_err(|error| {
                            Error::new(format!("{label} is not a token account: {error:?}"))
                        })?;
                    if state.mint != self.mint.to_bytes() {
                        return Err(Error::new(format!(
                            "{label} holds a different mint than the ledger tracks"
                        )));
                    }
                    state.amount
                }
            };
            tracked_collateral = tracked_collateral.checked_add(atoms).ok_or_else(|| {
                Error::new("tracked collateral overflowed u64, which no real supply can")
            })?;
            token_atoms.insert(label.clone(), atoms);
        }
        // L8's census. Summed by ADDRESS, for the same reason L7 is: the ledger
        // legitimately watches one account under several labels, and summing per
        // label would count a class twice.
        let mut atoms_by_address: BTreeMap<Pubkey, u64> = BTreeMap::new();
        for (label, address) in &self.token_accounts {
            if let Some(atoms) = token_atoms.get(label) {
                atoms_by_address.insert(*address, *atoms);
            }
        }
        let mint_key = self.mint.to_bytes();
        let class_atoms = self.class_census(&atoms_by_address, |address| {
            let Some(account) = rpc.account(*address)? else {
                return Ok(None);
            };
            // Same admission as the tracked-token loop above: a compartment
            // that is an associated token account is 170 bytes and is still a
            // compartment.
            let state = TokenAccount::parse_base_or_immutable_owner(&account.data).map_err(|error| {
                Error::new(format!(
                    "the Custody vault at {address} is not a token account: {error:?}. Its \
                     address derives from this Market's own compartment seeds, so something \
                     else is sitting at a compartment address"
                ))
            })?;
            if state.mint != mint_key {
                return Err(Error::new(format!(
                    "the Custody vault at {address} holds a different mint than the ledger \
                     tracks; a compartment of this Market's collateral namespace cannot hold \
                     another Market's collateral"
                )));
            }
            Ok(Some(state.amount))
        })?;

        let hoard_atoms = self
            .hoard
            .and_then(|_| token_atoms.get("hoard").copied())
            .unwrap_or(0);

        let (outcome_count, aggregate_supply) = match self.aggregate {
            None => (0, Vec::new()),
            Some(address) => {
                let account = rpc.required_account(address, "Claims aggregate")?;
                let view = LiabilityBasisMarketViewV2::decode(&account.data)
                    .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
                let count = view.claim_count;
                let mut supply = Vec::with_capacity(count as usize);
                for index in 0..count {
                    supply.push(view.supply(&account.data, index).map_err(|error| {
                        Error::new(format!("aggregate supply {index}: {error:?}"))
                    })?);
                }
                (count, supply)
            }
        };

        let mut position_balances = BTreeMap::new();
        let mut position_totals = vec![0_u64; outcome_count as usize];
        for (label, address) in &self.positions {
            let Some(account) = rpc.account(*address)? else {
                position_balances.insert(label.clone(), Vec::new());
                continue;
            };
            let view = LiabilityBasisPositionViewV2::decode(&account.data)
                .map_err(|error| Error::new(format!("{label} Position: {error:?}")))?;
            // A Position narrower or wider than the aggregate would make L3 sum
            // over a partial vector and still balance. Refuse instead: the law
            // is only meaningful over one common outcome width.
            if view.claim_count != outcome_count {
                return Err(Error::new(format!(
                    "{label} carries {} outcomes and the Claims aggregate owes over {outcome_count}; \
                     a conservation law cannot be evaluated across two widths",
                    view.claim_count
                )));
            }
            let mut balances = Vec::with_capacity(view.claim_count as usize);
            for index in 0..view.claim_count {
                let balance = view
                    .balance(&account.data, index)
                    .map_err(|error| Error::new(format!("{label} balance {index}: {error:?}")))?;
                balances.push(balance);
                if let Some(total) = position_totals.get_mut(index as usize) {
                    *total = total.checked_add(balance).ok_or_else(|| {
                        Error::new("Position totals overflowed u64, which no real supply can")
                    })?;
                }
            }
            position_balances.insert(label.clone(), balances);
        }

        let mut accounts = BTreeMap::new();
        for (label, address) in &self.watched {
            let state = match rpc.account(*address)? {
                None => AccountStateV1 {
                    address: address.to_string(),
                    exists: false,
                    owner: String::new(),
                    lamports: 0,
                    data_len: 0,
                },
                Some(account) => AccountStateV1 {
                    address: address.to_string(),
                    exists: true,
                    owner: account.owner.to_string(),
                    lamports: account.lamports,
                    data_len: account.data.len(),
                },
            };
            accounts.insert(label.clone(), state);
        }

        // The Market's own phase, read off the chain rather than declared.
        // A caller that could ASSERT "this Market is Terminal" would be a
        // caller that can switch a conservation law off by saying so.
        let market_phase = match self.market {
            None => None,
            Some(address) => {
                let account = rpc.required_account(address, "Core Market")?;
                let state = CoreState::decode(&account.data)
                    .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
                Some(
                    match state.phase {
                        CorePhase::Founding => "founding",
                        CorePhase::Open => "open",
                        CorePhase::Terminal => "terminal",
                        CorePhase::Retiring => "retiring",
                        CorePhase::Retired => "retired",
                    }
                    .to_owned(),
                )
            }
        };

        let mut observation = ObservationV1 {
            stage: stage.into(),
            slot,
            declared_collateral_delta,
            declared_hoard_delta,
            lamports,
            payer_lamports,
            mint_supply: mint.supply,
            token_atoms,
            declared_class_deltas: classes,
            class_atoms,
            tracked_collateral,
            hoard_atoms,
            outcome_count,
            aggregate_supply,
            position_balances,
            position_totals,
            accounts,
            market_phase,
            verdicts: Vec::new(),
        };
        observation.verdicts = self.evaluate(&observation);
        self.observations.push(observation);
        Ok(())
    }

    /// Every law over one census. Visible to the whole binary because the
    /// `ledger-census` command is a second caller of these laws and has to be
    /// able to prove one of them RED over a census it constructs, without a
    /// cluster.
    pub(crate) fn evaluate(&self, now: &ObservationV1) -> Vec<VerdictV1> {
        let mut verdicts = Vec::new();

        // L1 collateral closure.
        verdicts.push(if now.tracked_collateral == now.mint_supply {
            VerdictV1::holds(
                "L1",
                format!(
                    "tracked {} atoms across {} accounts == Mint supply {}",
                    now.tracked_collateral,
                    now.token_atoms.len(),
                    now.mint_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L1",
                format!(
                    "tracked {} atoms across {} accounts != Mint supply {}; {} atoms are in accounts this ledger does not name",
                    now.tracked_collateral,
                    now.token_atoms.len(),
                    now.mint_supply,
                    now.mint_supply.abs_diff(now.tracked_collateral)
                ),
            )
        });

        // L2 Hoard movement is declared.
        verdicts.push(match (self.hoard, self.observations.last()) {
            (None, _) => {
                VerdictV1::inapplicable("L2", "no Hoard exists before the founding commits")
            }
            (Some(_), None) => {
                VerdictV1::inapplicable("L2", "the first census has no predecessor to move from")
            }
            (Some(_), Some(previous)) => {
                let observed = i128::from(now.hoard_atoms) - i128::from(previous.hoard_atoms);
                if observed == now.declared_hoard_delta {
                    VerdictV1::holds(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}`, exactly as declared; it holds {}",
                            previous.stage, now.hoard_atoms
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}` and the stage declared {}. \
                             L1 cannot see this: principal moving from the Hoard into a wallet this \
                             ledger already tracks leaves the total untouched.",
                            previous.stage, now.declared_hoard_delta
                        ),
                    )
                }
            }
        });

        // L3 supply-vector agreement.
        verdicts.push(if self.aggregate.is_none() {
            VerdictV1::inapplicable(
                "L3",
                "no Claims aggregate exists before the founding commits",
            )
        } else if now.position_totals == now.aggregate_supply {
            VerdictV1::holds(
                "L3",
                format!(
                    "{} Positions sum to the aggregate supply vector {:?}",
                    now.position_balances.len(),
                    now.aggregate_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L3",
                format!(
                    "Positions sum to {:?} but the aggregate owes {:?}",
                    now.position_totals, now.aggregate_supply
                ),
            )
        });

        // L4 full collateralisation -- a PRE-TERMINAL invariant, and it says so.
        //
        // The law reads "the Hoard covers the worst outcome". That is a claim
        // about a Market that still owes: while it is Open the Hoard is the
        // collateral behind every outstanding claim, and a shortfall is a real
        // defect. Terminal settlement is the act that DISCHARGES the liability
        // -- it pays the winning outcome out of the Hoard and leaves the
        // aggregate's supply vector standing as the record of what was owed --
        // so "Hoard 0 < worst outcome 500000000" is not a Market that broke,
        // it is a Market that paid. Cohort-14b read that VIOLATED at its
        // post-payout boundary and the reading was the law's, not the chain's.
        //
        // So the law RETIRES rather than relaxing: it is declared inapplicable
        // by name, on a phase read off the Market account, and it goes on
        // holding or violating for every phase before Terminal. What remains
        // watching a paid Market is L1 (every atom is in a named account), L3
        // (Positions sum to the aggregate) and L7 (lamports close) -- none of
        // which weakens here.
        verdicts.push(match now.market_phase.as_deref() {
            Some(phase @ ("terminal" | "retiring" | "retired")) => VerdictV1::inapplicable(
                "L4",
                &format!(
                    "the Market is {phase}: settlement DISCHARGED the liability this law is \
                     stated about, so `Hoard {} >= worst outcome {} x unit {}` is a question \
                     about a Market that still owes and this one does not. L4 is a \
                     PRE-TERMINAL invariant and retires by name here rather than reading \
                     VIOLATED against a protocol that did exactly what it should. L1, L3 and \
                     L7 go on watching this boundary unweakened.",
                    now.hoard_atoms,
                    now.aggregate_supply.iter().max().copied().unwrap_or(0),
                    self.claim_unit_atoms
                ),
            ),
            _ => match (self.aggregate, now.aggregate_supply.iter().max()) {
                (None, _) | (_, None) => VerdictV1::inapplicable(
                    "L4",
                    "no outstanding liability exists before the founding commits",
                ),
                (Some(_), Some(worst)) => match worst.checked_mul(self.claim_unit_atoms) {
                    None => VerdictV1::violated(
                        "L4",
                        format!(
                            "worst outcome {worst} claims at {} atoms each overflows u64",
                            self.claim_unit_atoms
                        ),
                    ),
                    Some(required) if now.hoard_atoms >= required => VerdictV1::holds(
                        "L4",
                        format!(
                            "Hoard {} >= worst outcome {worst} x unit {} = {required}",
                            now.hoard_atoms, self.claim_unit_atoms
                        ),
                    ),
                    Some(required) => VerdictV1::violated(
                        "L4",
                        format!(
                            "Hoard {} < worst outcome {worst} x unit {} = {required}; the Market is under-collateralised",
                            now.hoard_atoms, self.claim_unit_atoms
                        ),
                    ),
                },
            },
        });

        // L5 stage delta.
        verdicts.push(match self.observations.last() {
            None => VerdictV1::inapplicable("L5", "the first census has no predecessor"),
            Some(previous) => {
                let observed =
                    i128::from(now.tracked_collateral) - i128::from(previous.tracked_collateral);
                if observed == now.declared_collateral_delta {
                    VerdictV1::holds(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`, exactly as declared",
                            previous.stage
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`; the stage declared {}",
                            previous.stage, now.declared_collateral_delta
                        ),
                    )
                }
            }
        });

        // L6 rent conservation.
        verdicts.push(match self.observations.last() {
            None => VerdictV1::inapplicable("L6", "the first census has no predecessor"),
            Some(previous) => {
                let vanished: Vec<String> = previous
                    .accounts
                    .iter()
                    .filter_map(|(label, before)| {
                        let after = now.accounts.get(label)?;
                        (before.exists && !after.exists && before.lamports > 0)
                            .then(|| format!("{label} ({} lamports)", before.lamports))
                    })
                    .collect();
                if vanished.is_empty() {
                    VerdictV1::holds("L6", "no watched account closed at this boundary".into())
                } else {
                    // A closure is not a violation; an unaccounted one is. The
                    // stage that closes an account states where the rent went,
                    // and the journey watches that destination, so the check is
                    // that SOMETHING watched grew by at least the closed rent.
                    let closed: u64 = previous
                        .accounts
                        .iter()
                        .filter_map(|(label, before)| {
                            let after = now.accounts.get(label)?;
                            (before.exists && !after.exists).then_some(before.lamports)
                        })
                        .sum();
                    let gained: u64 = now
                        .accounts
                        .iter()
                        .filter_map(|(label, after)| {
                            let before = previous.accounts.get(label)?;
                            after.lamports.checked_sub(before.lamports)
                        })
                        .sum();
                    if gained >= closed {
                        VerdictV1::holds(
                            "L6",
                            format!(
                                "closed {} lamports across {}; watched accounts gained {gained}",
                                closed,
                                vanished.join(", ")
                            ),
                        )
                    } else {
                        VerdictV1::violated(
                            "L6",
                            format!(
                                "closed {closed} lamports across {}; watched accounts gained only {gained}, so the rent went somewhere this journey does not watch",
                                vanished.join(", ")
                            ),
                        )
                    }
                }
            }
        });

        // L7 lamport accounting.
        // L8 per-class conservation.
        verdicts.push(self.evaluate_classes(now));

        verdicts.push(self.evaluate_lamports(now));

        verdicts
    }

    /// L8: every compartment class moved by exactly the amount its stage
    /// declared.
    ///
    /// This is L2's argument made for all nine classes instead of one. L1 and
    /// L5 are stated over a single total, so they balance for any transfer
    /// between two tracked accounts; L2 closes that for the Hoard alone. A
    /// transfer between any other pair of compartments passes every one of
    /// L1..L7, which is the cross-subsidy C-10 exists to forbid.
    ///
    /// The class is DERIVED from the vault's own PDA seeds, so this law cannot
    /// be satisfied by relabelling an account.
    fn evaluate_classes(&self, now: &ObservationV1) -> VerdictV1 {
        if let Some(reason) = &now.declared_class_deltas.inapplicable {
            return VerdictV1::inapplicable("L8", reason);
        }
        let declared = &now.declared_class_deltas.deltas;
        let Some(previous) = self.observations.last() else {
            return VerdictV1::inapplicable("L8", "the first census has no predecessor");
        };
        let mut classes: Vec<&str> = previous
            .class_atoms
            .keys()
            .chain(now.class_atoms.keys())
            .map(String::as_str)
            .collect();
        classes.sort_unstable();
        classes.dedup();
        let mut breaches = Vec::new();
        let mut held = Vec::new();
        for class in classes {
            let before = i128::from(previous.class_atoms.get(class).copied().unwrap_or(0));
            let after = i128::from(now.class_atoms.get(class).copied().unwrap_or(0));
            let observed = after - before;
            let expected = declared.get(class).copied().unwrap_or(0);
            if observed == expected {
                held.push(format!("{class} {observed:+}"));
            } else {
                breaches.push(format!(
                    "{class} moved {observed:+} atoms and the stage declared {expected:+}"
                ));
            }
        }
        if breaches.is_empty() {
            VerdictV1::holds(
                "L8",
                format!(
                    "every compartment moved exactly as declared since `{}`: {}",
                    previous.stage,
                    held.join(", ")
                ),
            )
        } else {
            VerdictV1::violated(
                "L8",
                format!(
                    "{}. L1 and L5 cannot see this: a transfer between two tracked \
                     compartments leaves the single total untouched, and L2 covers only the Hoard.",
                    breaches.join("; ")
                ),
            )
        }
    }

    /// L7: the payer's lamports moved by exactly its fees plus what landed in
    /// accounts this ledger watches.
    ///
    /// The growth term is summed only over labels present at BOTH boundaries.
    /// A label the stage introduced has no predecessor balance to subtract, and
    /// silently treating its whole balance as growth would let a stage admit an
    /// account into the ledger and its own leak in the same breath. So a
    /// boundary that introduced a watched label reports `inapplicable` and
    /// NAMES the labels, rather than reporting a green it did not earn.
    fn evaluate_lamports(&self, now: &ObservationV1) -> VerdictV1 {
        if let Some(reason) = &now.lamports.inapplicable {
            return VerdictV1::inapplicable("L7", reason);
        }
        let Some(previous) = self.observations.last() else {
            return VerdictV1::inapplicable("L7", "the first census has no predecessor");
        };
        let introduced: Vec<&str> = now
            .accounts
            .keys()
            .filter(|label| !previous.accounts.contains_key(*label))
            .map(String::as_str)
            .collect();
        if !introduced.is_empty() {
            return VerdictV1::inapplicable(
                "L7",
                &format!(
                    "this boundary admitted {} account(s) the previous census did not watch ({}), \
                     so their balances have no predecessor to difference against; L7 resumes at \
                     the next boundary",
                    introduced.len(),
                    introduced.join(", ")
                ),
            );
        }
        // Sum by ADDRESS, not by label. The ledger legitimately watches one
        // account under several names -- the discovery loop names accounts by
        // the founding's own evidence keys, and several of those keys point at
        // one account: the Market's rent beneficiary IS the founding's
        // lifecycle credit, the Found31 Market is the `market` record, the
        // normal and projected Custody replays are the same realized account,
        // and the fee payer is also a credit's refund wallet. Summing per label
        // counted every change to those FOUR addresses twice, and the first run
        // that closed accounts and refunded rent turned that into three L7
        // violations whose residuals were exactly the doubled amounts.
        //
        // The payer is then excluded outright rather than subtracted back out:
        // it appears under two labels here, so subtracting "the payer's label"
        // removed one copy and left the other.
        let payer = self.payer.to_string();
        let mut before_by_address: BTreeMap<&str, u64> = BTreeMap::new();
        let mut after_by_address: BTreeMap<&str, u64> = BTreeMap::new();
        for (label, after) in &now.accounts {
            if after.address == payer {
                continue;
            }
            let Some(before) = previous.accounts.get(label) else {
                continue;
            };
            before_by_address.insert(before.address.as_str(), before.lamports);
            after_by_address.insert(after.address.as_str(), after.lamports);
        }
        let mut watched_growth: i128 = 0;
        for (address, after) in &after_by_address {
            let before = before_by_address.get(address).copied().unwrap_or(0);
            watched_growth += i128::from(*after) - i128::from(before);
        }
        let payer_delta = i128::from(now.payer_lamports) - i128::from(previous.payer_lamports);

        if now.lamports.unwatched_lamports > 0 && now.lamports.unwatched_note.trim().is_empty() {
            return VerdictV1::violated(
                "L7",
                format!(
                    "the stage declared {} lamports placed outside the watched set and did not say \
                     where; an undescribed declaration is a hole with a number in it",
                    now.lamports.unwatched_lamports
                ),
            );
        }
        let residual = payer_delta
            + i128::from(now.lamports.fees_lamports)
            + watched_growth
            + i128::from(now.lamports.unwatched_lamports);
        if residual == 0 {
            VerdictV1::holds(
                "L7",
                format!(
                    "the payer moved {payer_delta} lamports since `{}`, its transactions paid {} in \
                     fees, watched accounts gained {}, and {} went to {}; debit == credit + fee",
                    previous.stage,
                    now.lamports.fees_lamports,
                    watched_growth,
                    now.lamports.unwatched_lamports,
                    if now.lamports.unwatched_note.is_empty() {
                        "nothing unwatched"
                    } else {
                        now.lamports.unwatched_note.as_str()
                    }
                ),
            )
        } else {
            VerdictV1::violated(
                "L7",
                format!(
                    "the payer moved {payer_delta} lamports since `{}` and its transactions paid {} \
                     in fees, but watched accounts gained {}; {residual} lamports are unaccounted \
                     for. L1..L6 cannot see this: they are stated about collateral atoms and about \
                     accounts that CLOSE, and nothing closed here.",
                    previous.stage, now.lamports.fees_lamports, watched_growth
                ),
            )
        }
    }

    /// Every census taken, in order.
    /// Reload observations a previous invocation of an EXTERNAL census took,
    /// so the delta laws (L2, L5, L7's fee history) evaluate across process
    /// boundaries. The chain stays the authority for every ABSOLUTE law; what
    /// this restores is only what a prior census DECLARED and read.
    ///
    /// Used by the successor driver's `ledger-census`; the journey itself
    /// never restores — its boundaries all live in one process.
    #[allow(dead_code)]
    pub(crate) fn restore_observations(&mut self, observations: Vec<ObservationV1>) {
        self.observations = observations;
    }

    pub(crate) fn observations(&self) -> &[ObservationV1] {
        &self.observations
    }

    /// Every violated law across the whole journey, named by stage.
    pub(crate) fn violations(&self) -> Vec<String> {
        self.observations
            .iter()
            .flat_map(|observation| {
                observation
                    .verdicts
                    .iter()
                    .filter(|verdict| verdict.failed())
                    .map(move |verdict| {
                        format!("{}: {} {}", observation.stage, verdict.law, verdict.detail)
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUSTODY: u8 = 0xc5;
    const MARKET: [u8; 32] = [0xc6; 32];
    const RELEASE: [u8; 32] = [0xc7; 32];
    const CONTEXT: [u8; 32] = [0xc8; 32];

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    /// The real vault address for one compartment, derived exactly as Custody
    /// derives it. The tests therefore exercise the classifier, not a label.
    fn vault(compartment: CompartmentV1) -> Pubkey {
        Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(MARKET, RELEASE, CONTEXT, compartment).as_slices(),
            &key(CUSTODY),
        )
        .0
    }

    fn account(address: Pubkey, lamports: u64) -> AccountStateV1 {
        AccountStateV1 {
            address: address.to_string(),
            exists: true,
            owner: key(0xb0).to_string(),
            lamports,
            data_len: 165,
        }
    }

    fn new_ledger() -> ConservationLedgerV1 {
        let mut ledger = ConservationLedgerV1::new(key(0xc1), key(0xd1));
        ledger.admit_custody_namespace(key(CUSTODY), MARKET, RELEASE, CONTEXT);
        ledger.admit_founding(vault(CompartmentV1::HoardPrincipal), key(0xa4), 10);
        ledger.track_token_account(
            "trading_principal_vault",
            vault(CompartmentV1::TradingPrincipal),
        );
        ledger.track_token_account("fee_vault", vault(CompartmentV1::FeeVault));
        ledger
    }

    /// A census in which the market holds `hoard` principal, `trading` in a
    /// TradingPrincipal vault and `fee` in a FeeVault, against one outcome of
    /// `supply` claims worth ten atoms each. `class_atoms` is built through the
    /// ledger's own derivation, so a wrong classifier fails these tests.
    fn census(
        ledger: &ConservationLedgerV1,
        stage: &str,
        hoard: u64,
        trading: u64,
        fee: u64,
        supply: u64,
        declared_class_deltas: ClassClaimV1,
    ) -> ObservationV1 {
        let tracked = hoard + trading + fee;
        let balances = [
            (vault(CompartmentV1::HoardPrincipal), "hoard", hoard),
            (
                vault(CompartmentV1::TradingPrincipal),
                "trading_principal_vault",
                trading,
            ),
            (vault(CompartmentV1::FeeVault), "fee_vault", fee),
        ];
        let mut token_atoms = BTreeMap::new();
        let mut class_atoms: BTreeMap<String, u64> = BTreeMap::new();
        let mut accounts = BTreeMap::new();
        for (address, label, atoms) in balances {
            token_atoms.insert(label.to_string(), atoms);
            *class_atoms.entry(ledger.class_of(&address)).or_insert(0) += atoms;
            accounts.insert(label.to_string(), account(address, 2_039_280));
        }
        ObservationV1 {
            stage: stage.into(),
            slot: 1,
            declared_collateral_delta: 0,
            declared_hoard_delta: 0,
            lamports: LamportClaimV1::inapplicable("this census is synthetic"),
            payer_lamports: 1_000_000,
            mint_supply: tracked,
            token_atoms,
            class_atoms,
            declared_class_deltas,
            tracked_collateral: tracked,
            hoard_atoms: hoard,
            outcome_count: 1,
            aggregate_supply: vec![supply],
            position_balances: BTreeMap::from([("holder".into(), vec![supply])]),
            position_totals: vec![supply],
            accounts,
            // Synthetic censuses bind no Market, so the phase-gated laws behave
            // exactly as they did before the binding existed.
            market_phase: None,
            verdicts: Vec::new(),
        }
    }

    /// A stage that declares exactly these class movements. An empty slice is
    /// `unchanged()`: every class claimed to have moved zero.
    fn moved(entries: &[(&str, i128)]) -> ClassClaimV1 {
        ClassClaimV1::moves(
            entries
                .iter()
                .map(|(class, delta)| ((*class).to_string(), *delta))
                .collect(),
        )
    }

    /// A stage that says it cannot account for its classes, and why.
    fn undeclared() -> ClassClaimV1 {
        ClassClaimV1::inapplicable("this synthetic census states no per-class claim")
    }

    fn verdict<'a>(verdicts: &'a [VerdictV1], law: &str) -> &'a VerdictV1 {
        verdicts
            .iter()
            .find(|value| value.law == law)
            .unwrap_or_else(|| panic!("{law} must be evaluated"))
    }

    // ---------- the Direct fill boundary ----------
    //
    // L1 and L3 have never been evaluated across a fill. The one Direct fill
    // this substrate has ever taken was read back by a hand-written script, not
    // by this ledger, so the two laws most exposed to a fill -- collateral
    // closure and supply-vector agreement -- have only ever been exercised over
    // foundings, admissions and vault transfers. These fake one.

    /// The seller's and the venue fee's Direct token PDAs, and the buyer's own
    /// token account. None of the three is a Custody vault, so all three
    /// classify `unclassified`: a fill moves atoms only within the External
    /// compartment and touches no Custody class at all.
    fn fill_ledger() -> ConservationLedgerV1 {
        let mut ledger = ConservationLedgerV1::new(key(0xc1), key(0xd1));
        ledger.admit_custody_namespace(key(CUSTODY), MARKET, RELEASE, CONTEXT);
        ledger.admit_founding(vault(CompartmentV1::HoardPrincipal), key(0xa4), 10);
        ledger.track_token_account("buyer_token", key(0x51));
        ledger.track_token_account("direct_seller_token", key(0x52));
        ledger.track_token_account("direct_venue_fee_token", key(0x53));
        ledger.track_position("seller_position", key(0x61));
        ledger.track_position("buyer_position", key(0x62));
        ledger
    }

    const FILL_HOARD_ATOMS: u64 = 10_000;

    /// One census of a two-outcome market mid-trade.
    ///
    /// `aggregate` is passed rather than summed from the Positions, so L3 holds
    /// only because the fill actually conserved the supply vector. Summing it
    /// would make L3 an identity and it would stop being able to fail.
    fn fill_census(
        ledger: &ConservationLedgerV1,
        stage: &str,
        buyer_token: u64,
        seller_direct: u64,
        venue_fee: u64,
        seller_claims: [u64; 2],
        buyer_claims: [u64; 2],
        aggregate: [u64; 2],
        omit_seller_direct_token: bool,
        omit_buyer_position: bool,
    ) -> ObservationV1 {
        let balances = [
            (
                vault(CompartmentV1::HoardPrincipal),
                "hoard",
                FILL_HOARD_ATOMS,
            ),
            (key(0x51), "buyer_token", buyer_token),
            (key(0x52), "direct_seller_token", seller_direct),
            (key(0x53), "direct_venue_fee_token", venue_fee),
        ];
        let mut token_atoms = BTreeMap::new();
        let mut class_atoms: BTreeMap<String, u64> = BTreeMap::new();
        let mut accounts = BTreeMap::new();
        let mut tracked = 0;
        for (address, label, atoms) in balances {
            if omit_seller_direct_token && label == "direct_seller_token" {
                continue;
            }
            token_atoms.insert(label.to_string(), atoms);
            *class_atoms.entry(ledger.class_of(&address)).or_insert(0) += atoms;
            accounts.insert(label.to_string(), account(address, 2_039_280));
            tracked += atoms;
        }
        // The Mint is the whole supply whether or not this census names every
        // account holding it. That asymmetry is the point of L1.
        let mint_supply = FILL_HOARD_ATOMS + buyer_token + seller_direct + venue_fee;
        let mut position_balances =
            BTreeMap::from([("seller_position".to_string(), seller_claims.to_vec())]);
        let mut position_totals = seller_claims.to_vec();
        if !omit_buyer_position {
            position_balances.insert("buyer_position".into(), buyer_claims.to_vec());
            for (total, claim) in position_totals.iter_mut().zip(buyer_claims) {
                *total += claim;
            }
        }
        accounts.insert("seller_position".into(), account(key(0x61), 1_823_904));
        accounts.insert("buyer_position".into(), account(key(0x62), 1_823_904));
        ObservationV1 {
            stage: stage.into(),
            slot: 1,
            // A fill is External to External between two accounts this ledger
            // tracks, and the Hoard is not a party: the Direct path never
            // touches it. Zero is the STRONG claim here, not the absent one.
            declared_collateral_delta: 0,
            declared_hoard_delta: 0,
            lamports: LamportClaimV1::inapplicable("this synthetic census states no lamport claim"),
            payer_lamports: 1_000_000,
            mint_supply,
            token_atoms,
            class_atoms,
            declared_class_deltas: ClassClaimV1::unchanged(),
            tracked_collateral: tracked,
            hoard_atoms: FILL_HOARD_ATOMS,
            outcome_count: 2,
            aggregate_supply: aggregate.to_vec(),
            position_balances,
            position_totals,
            accounts,
            // Synthetic censuses bind no Market, so the phase-gated laws behave
            // exactly as they did before the binding existed.
            market_phase: None,
            verdicts: Vec::new(),
        }
    }

    fn before_fill(ledger: &ConservationLedgerV1) -> ObservationV1 {
        fill_census(
            ledger,
            "before-fill",
            100,
            0,
            0,
            [500, 500],
            [0, 0],
            [500, 500],
            false,
            false,
        )
    }

    /// The seller sold 100 claims of outcome 0 for 100 atoms at a fee that
    /// floored to zero, which is the only shape the deployed release can fill.
    fn after_fill(
        ledger: &ConservationLedgerV1,
        omit_seller_direct_token: bool,
        omit_buyer_position: bool,
    ) -> ObservationV1 {
        fill_census(
            ledger,
            "after-fill",
            0,
            100,
            0,
            [400, 500],
            [100, 0],
            [500, 500],
            omit_seller_direct_token,
            omit_buyer_position,
        )
    }

    /// Every law holds across a fill, and each is asserted by NAME rather than
    /// in aggregate, so a change to one is caught here instead of quietly
    /// altering what a fill is compared against.
    /// L4 is a PRE-TERMINAL invariant, and the phase that retires it is read
    /// off the chain rather than declared.
    ///
    /// Cohort-14b's post-payout boundary read `VIOLATED L4: Hoard 0 < worst
    /// outcome 500000000` about a Market that had RESOLVED, settled and paid
    /// 500,000,000 atoms into the founder's associated token account. Nothing
    /// was wrong with the Market; the law was asked a question about a Market
    /// that still owes, and that one did not.
    ///
    /// The three assertions that matter are separate on purpose. An
    /// under-collateralised OPEN market must still go red -- retiring the law
    /// for every phase would delete it. A market with no phase bound at all
    /// must behave exactly as it did before this binding existed, because
    /// every census taken before today has `market_phase: None`. And the
    /// terminal verdict must be INAPPLICABLE, never `holds`: a law that cannot
    /// be evaluated has not been satisfied.
    #[test]
    fn l4_retires_at_terminal_and_at_no_earlier_phase() {
        let ledger = new_ledger();
        // Hoard 0 against one outcome owing 10 claims x 10 atoms = 100.
        let broke = |phase: Option<&str>| {
            let mut observation = census(&ledger, "boundary", 0, 0, 0, 10, undeclared());
            observation.market_phase = phase.map(ToOwned::to_owned);
            ledger.evaluate(&observation)
        };
        assert_eq!(
            verdict(&broke(None), "L4").status,
            "violated",
            "a census that binds no Market keeps the behaviour it always had"
        );
        assert_eq!(
            verdict(&broke(Some("open")), "L4").status,
            "violated",
            "an OPEN market whose Hoard cannot cover its worst outcome is still a defect"
        );
        assert_eq!(
            verdict(&broke(Some("founding")), "L4").status,
            "violated",
            "a founding market is not exempt either"
        );
        for phase in ["terminal", "retiring", "retired"] {
            let verdicts = broke(Some(phase));
            let l4 = verdict(&verdicts, "L4");
            assert_eq!(
                l4.status, "inapplicable",
                "L4 retires by name at phase {phase}, and never reports a pass it did not earn"
            );
            assert!(
                l4.detail.contains(phase) && l4.detail.contains("PRE-TERMINAL"),
                "the verdict must name the phase and the reason: {}",
                l4.detail
            );
            // The other laws do not follow it out. A paid Market is still
            // watched by everything that can still be evaluated.
            for law in ["L1", "L3"] {
                assert_ne!(
                    verdict(&verdicts, law).status,
                    "inapplicable",
                    "{law} must go on watching a terminal Market"
                );
            }
        }
    }

    #[test]
    fn every_law_holds_across_a_direct_fill_boundary() {
        let mut ledger = fill_ledger();
        ledger.restore_observations(vec![before_fill(&fill_ledger())]);
        let verdicts = ledger.evaluate(&after_fill(&fill_ledger(), false, false));

        for law in ["L1", "L2", "L3", "L4", "L5", "L6", "L8"] {
            assert_eq!(
                verdict(&verdicts, law).status,
                "holds",
                "{law}: {}",
                verdict(&verdicts, law).detail
            );
        }
        // A fill moves collateral only within the External compartment, so the
        // strongest per-class claim there is -- every class moved zero -- is
        // the true one, and L8 earns its green rather than sitting out.
        assert_eq!(verdict(&verdicts, "L7").status, "inapplicable");
    }

    /// L1 RED when the seller's Direct token PDA is not named.
    ///
    /// This is the census binding `simulator.py` grew on 2026-09-02, stated as
    /// a law rather than as a configuration note: the atoms are exactly where
    /// the trade put them, and a census that does not name the destination
    /// reports a shortfall of precisely the traded amount.
    #[test]
    fn an_unnamed_direct_token_destination_shows_up_as_the_traded_atoms_missing() {
        let mut ledger = fill_ledger();
        ledger.restore_observations(vec![before_fill(&fill_ledger())]);
        let verdicts = ledger.evaluate(&after_fill(&fill_ledger(), true, false));

        let l1 = verdict(&verdicts, "L1");
        assert_eq!(l1.status, "violated");
        assert!(
            l1.detail
                .contains("100 atoms are in accounts this ledger does not name"),
            "{}",
            l1.detail
        );
        // And it is L1 alone: the supply vector is untouched by a token account
        // going unnamed, so a reader is not sent hunting through the claims.
        assert_eq!(verdict(&verdicts, "L3").status, "holds");
    }

    /// L3 RED when the buyer's Position is not tracked.
    ///
    /// The claims left the seller and arrived somewhere. A census that names
    /// only the seller sees the departure and not the arrival, and says so.
    #[test]
    fn an_untracked_buyer_position_shows_up_as_the_supply_vector_falling_short() {
        let mut ledger = fill_ledger();
        ledger.restore_observations(vec![before_fill(&fill_ledger())]);
        let verdicts = ledger.evaluate(&after_fill(&fill_ledger(), false, true));

        let l3 = verdict(&verdicts, "L3");
        assert_eq!(l3.status, "violated");
        assert!(l3.detail.contains("400"), "{}", l3.detail);
        // L1 is untouched: the collateral is all named, only the claims are not.
        assert_eq!(verdict(&verdicts, "L1").status, "holds");
    }

    /// A fill that declared a Hoard movement it did not make is caught, which
    /// is the guard against copying a founding's declaration onto a trade.
    #[test]
    fn a_fill_that_declares_a_hoard_delta_it_did_not_move_is_refused() {
        let mut ledger = fill_ledger();
        ledger.restore_observations(vec![before_fill(&fill_ledger())]);
        let mut after = after_fill(&fill_ledger(), false, false);
        after.declared_hoard_delta = -100;
        let verdicts = ledger.evaluate(&after);
        assert_eq!(verdict(&verdicts, "L2").status, "violated");
        assert_eq!(verdict(&verdicts, "L1").status, "holds");
    }

    /// The classifier is a derivation, not a label: every one of the nine
    /// compartments resolves to its own class, and an address that is not a
    /// vault under an admitted namespace is `unclassified` rather than being
    /// folded into one.
    #[test]
    fn every_compartment_is_recovered_from_its_own_pda_seeds() {
        let ledger = new_ledger();
        for (compartment, label) in COMPARTMENTS {
            assert_eq!(ledger.class_of(&vault(compartment)), label);
        }
        assert_eq!(ledger.class_of(&key(0xee)), UNCLASSIFIED);
        // A vault under a context the ledger has not admitted is not silently
        // classified as one it has.
        let foreign = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(MARKET, RELEASE, [0xfe; 32], CompartmentV1::FeeVault)
                .as_slices(),
            &key(CUSTODY),
        )
        .0;
        assert_eq!(ledger.class_of(&foreign), UNCLASSIFIED);
    }

    /// THE DEFECT, unchanged: L1..L7 are blind to a cross-class transfer.
    ///
    /// This is the control for L8, and it is asserted law by law rather than in
    /// aggregate, so a future change that alters one of the seven is caught here
    /// rather than quietly rewriting what L8 is compared against.
    #[test]
    fn the_original_seven_laws_hold_while_atoms_cross_from_trading_principal_to_the_fee_vault() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let after = census(&new_ledger(), "after", 1_000, 0, 500, 100, undeclared());
        let verdicts = ledger.evaluate(&after);

        for law in ["L1", "L2", "L3", "L4", "L5", "L6"] {
            assert_eq!(verdict(&verdicts, law).status, "holds", "{law}");
        }
        assert_eq!(verdict(&verdicts, "L7").status, "inapplicable");
        // And L8 does not invent a green it did not earn. The stage here has
        // said it cannot account for its classes, so L8 reports THAT REASON
        // back verbatim rather than one of its own -- and in particular does
        // not report `holds` on a boundary where two compartments moved.
        assert_eq!(verdict(&verdicts, "L8").status, "inapplicable");
        assert_eq!(
            verdict(&verdicts, "L8").detail,
            "this synthetic census states no per-class claim"
        );
    }

    /// L8 RED on exactly the transfer the seven laws could not see.
    #[test]
    fn l8_catches_the_cross_class_transfer_the_other_laws_cannot_see() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        // The stage claims it moved nothing at all.
        let after = census(&new_ledger(), "after", 1_000, 0, 500, 100, moved(&[]));
        let verdicts = ledger.evaluate(&after);

        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("TradingPrincipal moved -500"));
        assert!(l8.detail.contains("FeeVault moved +500"));
        // The other seven are unchanged by L8's arrival.
        for law in ["L1", "L2", "L3", "L4", "L5", "L6"] {
            assert_eq!(verdict(&verdicts, law).status, "holds", "{law}");
        }
    }

    /// L8 GREEN when the stage states the movement it actually made.
    #[test]
    fn l8_admits_the_same_transfer_once_the_stage_declares_it() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            500,
            100,
            moved(&[("TradingPrincipal", -500), ("FeeVault", 500)]),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "holds");
    }

    /// THE TABLE'S WIDTH IS THE NAMESPACE, NOT THE STAGE'S BOOKKEEPING.
    ///
    /// The journey tracks exactly one collateral vault -- the Hoard -- so under
    /// a census confined to `token_accounts` L8's table had two rows and could
    /// never have more, whatever any stage declared. Eight compartments existed
    /// as derivable addresses and contributed nothing, which is precisely the
    /// "class L8 does not name" its own doc comment says it exists to close.
    ///
    /// This asserts the fix at the seam `observe` uses: every compartment the
    /// admitted namespace names is READ, a compartment that has never been
    /// created reads as zero rather than as an absence, and an account that is
    /// not a Custody vault at all still lands in `unclassified`.
    #[test]
    fn the_class_census_reads_every_admitted_compartment_not_only_the_tracked_ones() {
        let mut ledger = ConservationLedgerV1::new(key(0xc1), key(0xd1));
        ledger.admit_custody_namespace(key(CUSTODY), MARKET, RELEASE, CONTEXT);
        // The journey's real shape: the Hoard is tracked, and an ordinary
        // holder wallet that is not a vault under any namespace.
        let wallet = key(0xee);
        let tracked = BTreeMap::from([
            (vault(CompartmentV1::HoardPrincipal), 1_000_u64),
            (wallet, 250),
        ]);

        // Only one compartment has ever been created on this chain, and it is
        // NOT the one the stage tracks.
        let live_fee_vault = vault(CompartmentV1::FeeVault);
        let mut read = Vec::new();
        let census = ledger
            .class_census(&tracked, |address| {
                read.push(*address);
                Ok(if *address == live_fee_vault {
                    Some(42)
                } else {
                    None
                })
            })
            .expect("the census reads every admitted compartment");

        // Eight compartments were read: all nine the namespace names, minus the
        // Hoard, which the stage already tracked and which is therefore never
        // re-read.
        assert_eq!(read.len(), COMPARTMENTS.len() - 1);
        assert!(!read.contains(&vault(CompartmentV1::HoardPrincipal)));
        assert!(!read.contains(&wallet));

        // And the table is as wide as the namespace: a row for every one of the
        // nine compartments plus `unclassified` for the wallet.
        assert_eq!(census.len(), COMPARTMENTS.len() + 1);
        assert_eq!(census.get("HoardPrincipal"), Some(&1_000));
        assert_eq!(census.get("FeeVault"), Some(&42));
        assert_eq!(census.get(UNCLASSIFIED), Some(&250));
        // A compartment nobody has created is ZERO, and it is present. Under
        // the old census it was simply not there, so no declaration could be
        // checked against it.
        assert_eq!(census.get("LivenessVault"), Some(&0));
        assert_eq!(census.get("RecoveryReserve"), Some(&0));
    }

    /// And L8 then holds those wider rows to what the stage declared: the
    /// FeeVault the journey never tracked moves, the stage declared
    /// `unchanged()`, and the law goes RED. Before the widening this movement
    /// produced no row and therefore no verdict.
    #[test]
    fn a_compartment_no_stage_tracks_is_still_held_to_the_declaration() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            0,
            0,
            100,
            undeclared(),
        )]);
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            42,
            100,
            ClassClaimV1::unchanged(),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("FeeVault moved +42"));
        // L5 sees only that the tracked total grew; it cannot say where.
        assert_eq!(verdict(&verdicts, "L5").status, "violated");
        assert!(!verdict(&verdicts, "L5").detail.contains("FeeVault"));
    }

    /// The journey's own shape, stated rather than derived.
    ///
    /// Every journey stage moves collateral between accounts of the SAME class
    /// -- wallet to wallet, or nothing at all -- so each one declares
    /// `unchanged()`, and L8 reports a row per compartment the namespace names.
    /// The previous design derived `{HoardPrincipal, unclassified}` from the
    /// two numbers the stage already stated, which meant L8's table could never
    /// be wider than the arithmetic that produced it and could never disagree
    /// with L2 and L5. This asserts the opposite property: the table is the
    /// DECLARATION checked against the chain.
    #[test]
    fn a_stage_that_declares_unchanged_gets_a_row_for_every_class_it_censused() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            250,
            100,
            undeclared(),
        )]);
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            500,
            250,
            100,
            ClassClaimV1::unchanged(),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "holds");
        // Three compartments censused, three rows, every one of them +0 -- and
        // the row exists because the stage declared it, not because a
        // subtraction produced it.
        for class in ["HoardPrincipal", "TradingPrincipal", "FeeVault"] {
            assert!(
                l8.detail.contains(&format!("{class} +0")),
                "L8 must report {class}: {}",
                l8.detail
            );
        }
    }

    /// A stage that cannot account for its classes SAYS SO, and L8 repeats its
    /// reason rather than inventing one.
    ///
    /// `relayed-vertical` links this file by `#[path]` and admits no Custody
    /// namespace at all, so it cannot classify even its own Hoard. Under the
    /// old design the ledger inferred that from an empty `vault_classes` and
    /// reported a reason of its own devising; now the campaign states it, which
    /// is the same discipline `LamportClaimV1::inapplicable` already imposes
    /// for L7.
    #[test]
    fn a_stage_that_cannot_account_for_its_classes_states_the_reason_itself() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            500,
            100,
            ClassClaimV1::inapplicable("the stage refused part way through"),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        assert_eq!(l8.status, "inapplicable");
        assert_eq!(l8.detail, "the stage refused part way through");
    }

    /// THE TRIPWIRE, and it is now strictly stronger than the derivation was.
    ///
    /// A compartment moving that the stage did not name used to be
    /// UNDERDETERMINED: the derivation refused to guess a split and L8 reported
    /// `inapplicable`, so the movement produced no verdict at all. With the
    /// claim required, an unnamed class is a claim of ZERO, so the same
    /// movement is a VIOLATION. Silence stopped being a way to avoid the law.
    #[test]
    fn a_compartment_the_stage_did_not_name_is_a_violation_and_no_longer_a_shrug() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        // The stage names the Hoard, honestly, and says nothing about the two
        // compartments that actually moved.
        let after = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            500,
            100,
            moved(&[("HoardPrincipal", 0)]),
        );
        let verdicts = ledger.evaluate(&after);
        let l8 = verdict(&verdicts, "L8");
        std::println!("L8 {} -- {}", l8.status, l8.detail);
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("TradingPrincipal moved -500"));
        assert!(l8.detail.contains("FeeVault moved +500"));
    }

    /// A compartment that VANISHES between boundaries is caught the same way a
    /// compartment that appears is: it is a class in the union of the two
    /// censuses, so it is held to whatever the stage declared for it.
    #[test]
    fn a_compartment_that_vanishes_is_held_to_its_declaration_too() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let drained = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            0,
            100,
            ClassClaimV1::unchanged(),
        );
        let verdicts = ledger.evaluate(&drained);
        let l8 = verdict(&verdicts, "L8");
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("TradingPrincipal moved -500"));

        // And it is admitted the moment the stage states it -- 500 atoms left
        // the tracked set entirely, which L5 sees and L8 attributes.
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let declared = census(
            &new_ledger(),
            "after",
            1_000,
            0,
            0,
            100,
            moved(&[("TradingPrincipal", -500)]),
        );
        assert_eq!(verdict(&ledger.evaluate(&declared), "L8").status, "holds");
    }

    /// L8 covers the Hoard too. A law that omitted it because L2 exists would
    /// leave the same hole in mirror image.
    #[test]
    fn l8_covers_the_hoard_and_does_not_defer_to_l2() {
        let mut ledger = new_ledger();
        ledger.restore_observations(vec![census(
            &new_ledger(),
            "before",
            1_000,
            500,
            0,
            100,
            undeclared(),
        )]);
        let after = census(&new_ledger(), "after", 500, 500, 500, 50, moved(&[]));
        let verdicts = ledger.evaluate(&after);
        assert_eq!(verdict(&verdicts, "L2").status, "violated");
        let l8 = verdict(&verdicts, "L8");
        assert_eq!(l8.status, "violated");
        assert!(l8.detail.contains("HoardPrincipal moved -500"));
    }
}
