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
//! A law that cannot be evaluated at a given boundary — no aggregate yet, no
//! Hoard yet — is recorded as `Inapplicable` with the reason. It is never
//! silently skipped: a law that quietly stops applying is how a conservation
//! argument rots.

use std::collections::BTreeMap;

use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_token_svm::{MINT_BYTES, Mint, TokenAccount};
use serde::Serialize;
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, rpc::Rpc};

/// One law's outcome at one stage boundary.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct VerdictV1 {
    /// `L1`..`L6`.
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

/// The exact state of one account the ledger tracks.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AccountStateV1 {
    pub(crate) address: String,
    pub(crate) exists: bool,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
}

/// One complete census of the economic state, plus every law evaluated on it.
#[derive(Clone, Debug, Serialize)]
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
    pub(crate) mint_supply: u64,
    /// label -> raw atoms, for every tracked collateral token account.
    pub(crate) token_atoms: BTreeMap<String, u64>,
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
    pub(crate) verdicts: Vec<VerdictV1>,
}

/// What the ledger watches, and the laws it evaluates over it.
pub(crate) struct ConservationLedgerV1 {
    mint: Pubkey,
    hoard: Option<Pubkey>,
    aggregate: Option<Pubkey>,
    /// Collateral atoms one claim of one outcome is worth.
    claim_unit_atoms: u64,
    token_accounts: BTreeMap<String, Pubkey>,
    positions: BTreeMap<String, Pubkey>,
    watched: BTreeMap<String, Pubkey>,
    observations: Vec<ObservationV1>,
}

impl ConservationLedgerV1 {
    /// Start a ledger over one collateral Mint.
    ///
    /// The Hoard, the aggregate and the claim unit are not known until the
    /// founding commits, so they are admitted later. Until then the laws that
    /// need them record themselves inapplicable rather than vanishing.
    pub(crate) fn new(mint: Pubkey) -> Self {
        Self {
            mint,
            hoard: None,
            aggregate: None,
            claim_unit_atoms: 0,
            token_accounts: BTreeMap::new(),
            positions: BTreeMap::new(),
            watched: BTreeMap::new(),
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
    /// checks the chain against that claim rather than against zero.
    pub(crate) fn observe(
        &mut self,
        rpc: &mut Rpc,
        stage: &str,
        declared_collateral_delta: i128,
        declared_hoard_delta: i128,
    ) -> Result<()> {
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
                    let state = TokenAccount::parse(&account.data).map_err(|error| {
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

        let mut observation = ObservationV1 {
            stage: stage.into(),
            slot,
            declared_collateral_delta,
            declared_hoard_delta,
            mint_supply: mint.supply,
            token_atoms,
            tracked_collateral,
            hoard_atoms,
            outcome_count,
            aggregate_supply,
            position_balances,
            position_totals,
            accounts,
            verdicts: Vec::new(),
        };
        observation.verdicts = self.evaluate(&observation);
        self.observations.push(observation);
        Ok(())
    }

    fn evaluate(&self, now: &ObservationV1) -> Vec<VerdictV1> {
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

        // L4 full collateralisation.
        verdicts.push(match (self.aggregate, now.aggregate_supply.iter().max()) {
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

        verdicts
    }

    /// Every census taken, in order.
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
