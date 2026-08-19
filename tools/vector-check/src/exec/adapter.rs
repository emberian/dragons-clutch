//! `programs/solana-reference` (surface S5), the offline transition adapter.
//!
//! The state form is the decoded account plane, not its bytes: a vector names
//! account *fields*, and this executor encodes them with the frozen layout
//! codecs before calling `apply`.  That keeps the vector language-neutral, as
//! §6 requires, and keeps the byte layout owned by `solana-layout` alone.

use clutch_kernel::{PayoutSet, PayoutVector, MAX_OUTCOMES, MAX_PAYOUTS};
use clutch_solana_layout::{
    account_len, Hash32, HoardAccount, Intent, MarketAccount, PositionAccount, SupplyLedgerAccount,
};
use clutch_solana_reference::{
    AccountMetadata, ActorMetadata, Error, ExpectedBindings, ExternalAccount, KernelAccount,
    ReplayAccount, StateBytes, TransitionMetadata, TransitionOutput, EXTERNAL_ACCOUNT_LEN,
    KERNEL_ACCOUNT_LEN, MAX_REQUEST_LEN, REPLAY_ACCOUNT_LEN,
};

use super::*;
use crate::json::Value;
use crate::taxonomy::{Observed, Refusal};

// The reference request framing is `const` and private inside
// `programs/solana-reference` (`REQUEST_TAG`, `REFERENCE_VERSION`,
// `ACTION_LAYOUT`, `ACTION_RESOLVE`, `ACTION_REDEEM_INTERNAL`).  This lane may
// not edit that crate, so the four bytes are restated here and every request
// this executor builds is round-tripped through the crate's own public
// `Request::decode` before it is used.  A restatement that drifted would fail
// to decode rather than silently encode a different action.
const REQUEST_TAG: u8 = 0xd1;
const REFERENCE_VERSION: u8 = 1;
const ACTION_LAYOUT: u8 = 0;
const ACTION_RESOLVE: u8 = 1;
const ACTION_REDEEM_INTERNAL: u8 = 2;

/// S5's variant to taxonomy-code map.  §2.4 maps 22 of the 34 variants; the
/// remainder come from the extension block of `TAXONOMY.json`.
pub fn code(error: Error) -> Refusal {
    let (code, variant) = match error {
        Error::Layout(inner) => {
            return Refusal::new(inner.code(), "layout", format!("Layout({inner:?})"))
        }
        Error::Kernel(inner) => {
            let inner = super::kernel::code(inner);
            return Refusal::new(inner.code, "kernel", format!("Kernel({})", inner.variant));
        }
        Error::Window(inner) => {
            let inner = super::accumulator::window_code(inner);
            return Refusal::new(
                inner.code,
                "accumulator",
                format!("Window({})", inner.variant),
            );
        }
        Error::Resolution(inner) => {
            let (code, variant) = resolution_code(inner);
            return Refusal::new(code, "reference-adapter", format!("Resolution({variant})"));
        }
        Error::WrongLength => (2010, "WrongLength"),
        Error::WrongTag => (2030, "WrongTag"),
        Error::WrongVersion => (2031, "WrongVersion"),
        Error::NonCanonical => (2020, "NonCanonical"),
        Error::Arithmetic => (1000, "Arithmetic"),
        Error::WrongProgramOwner => (4004, "WrongProgramOwner"),
        Error::AccountAlias => (4006, "AccountAlias"),
        Error::WrongAccountKey => (4005, "WrongAccountKey"),
        Error::NotWritable => (4007, "NotWritable"),
        Error::MissingSignature => (4001, "MissingSignature"),
        Error::UnauthorizedActor => (4002, "UnauthorizedActor"),
        Error::AuthorizationUnavailable => (4003, "AuthorizationUnavailable"),
        Error::ResolutionEvidenceUnavailable => (6001, "ResolutionEvidenceUnavailable"),
        Error::TermsBindingMismatch => (4024, "TermsBindingMismatch"),
        Error::PayoutSetMismatch => (4025, "PayoutSetMismatch"),
        Error::ResolutionBindingMismatch => (4026, "ResolutionBindingMismatch"),
        Error::ResolutionAlreadyRecorded => (7003, "ResolutionAlreadyRecorded"),
        Error::ResolutionNotRecorded => (3003, "ResolutionNotRecorded"),
        Error::PayoutIndexMismatch => (4027, "PayoutIndexMismatch"),
        Error::ImmutableAccountWritable => (4028, "ImmutableAccountWritable"),
        Error::UnexpectedEvidence => (2077, "UnexpectedEvidence"),
        Error::WindowIdentityUnavailable => (4029, "WindowIdentityUnavailable"),
        Error::CollateralPolicyNotFrozen => (4030, "CollateralPolicyNotFrozen"),
        Error::WrongBump => (4008, "WrongBump"),
        Error::MismatchedState => (4011, "MismatchedState"),
        Error::AggregateClosureMismatch => (5011, "AggregateClosureMismatch"),
        Error::NonEmptyInitialization => (5012, "NonEmptyInitialization"),
        Error::Replay => (7001, "Replay"),
        Error::UnsupportedIntent => (9001, "UnsupportedIntent"),
        Error::CollateralCap => (8003, "CollateralCap"),
    };
    Refusal::new(code, "reference-adapter", variant)
}

fn resolution_code(refusal: clutch_solana_reference::ResolutionRefusal) -> (u32, &'static str) {
    use clutch_solana_reference::ResolutionRefusal as R;
    match refusal {
        R::TermsDigestMismatch => (4024, "TermsDigestMismatch"),
        R::TermsMalformed => (2078, "TermsMalformed"),
        R::PartitionMalformed => (2079, "PartitionMalformed"),
        R::WindowDomainMismatch(inner) => {
            let inner = super::accumulator::window_code(inner);
            (inner.code, "WindowDomainMismatch")
        }
        R::StatisticUnsupported => (9002, "StatisticUnsupported"),
        R::AmbiguousInterval => (6004, "AmbiguousInterval"),
        R::NoAcceptedCoverage => (6002, "NoAcceptedCoverage"),
        R::AmbiguousDenominator => (1005, "AmbiguousDenominator"),
        R::PayoutIndexOutOfRange => (2061, "PayoutIndexOutOfRange"),
        R::MarketNotActive => (3001, "MarketNotActive"),
        R::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
        /* TermsAccount v3 (derived-basis) classes.  No pinned vector reaches
         * them yet, so the pinned TAXONOMY.json does not carry these rows;
         * the codes below are the family-consistent proposals for the next
         * spine revision, and VER-8 will refuse them (loudly, not silently)
         * if a vector starts producing one before the taxonomy lane lands
         * the rows. */
        R::BasisMalformed => (2080, "BasisMalformed"),
        R::WeightDerivationOverflow => (1006, "WeightDerivationOverflow"),
        R::ValueOutOfRange => (6008, "ValueOutOfRange"),
        R::NonPointEvidence => (6009, "NonPointEvidence"),
        R::DerivedVectorUnrepresentable => (9008, "DerivedVectorUnrepresentable"),
        R::WrongResolutionMode => (9009, "WrongResolutionMode"),
    }
}

fn hash(value: &Value) -> Result<Hash32, String> {
    Ok(Hash32::from_bytes(read_hash32(value)?))
}

fn account_metadata(value: &Value, program: Hash32) -> Result<AccountMetadata, String> {
    Ok(AccountMetadata {
        key: hash(field(value, "key")?)?,
        owner_program: match value.get("owner_program") {
            Some(entry) => hash(entry)?,
            None => program,
        },
        writable: match value.get("writable") {
            Some(entry) => entry.as_bool()?,
            None => true,
        },
    })
}

pub struct AdapterExecutor {
    state: TransitionOutput,
    metadata: TransitionMetadata,
    bindings: ExpectedBindings,
    outcome_count: usize,
    last_payout: u64,
}

impl AdapterExecutor {
    pub fn open(constructed_by: &str, value: &Value) -> Result<Self, String> {
        if constructed_by != "raw-fields" {
            return Err(
                "adapter.reference-transition/v1 states are not reachable through the crate's own constructors, so §3.3 requires constructed_by \"raw-fields\""
                    .into(),
            );
        }
        let bindings_value = field(value, "bindings")?;
        let program = hash(field(bindings_value, "program_id")?)?;
        let bindings = ExpectedBindings {
            program_id: program,
            market: hash(field(bindings_value, "market")?)?,
            hoard: hash(field(bindings_value, "hoard")?)?,
            position: hash(field(bindings_value, "position")?)?,
            kernel: hash(field(bindings_value, "kernel")?)?,
            external: hash(field(bindings_value, "external")?)?,
            replay: hash(field(bindings_value, "replay")?)?,
            supply: hash(field(bindings_value, "supply")?)?,
            market_bump: small_field(bindings_value, "market_bump")? as u8,
            hoard_bump: small_field(bindings_value, "hoard_bump")? as u8,
            position_bump: small_field(bindings_value, "position_bump")? as u8,
            external_bump: small_field(bindings_value, "external_bump")? as u8,
            replay_bump: small_field(bindings_value, "replay_bump")? as u8,
            supply_bump: small_field(bindings_value, "supply_bump")? as u8,
        };

        let metadata_value = field(value, "metadata")?;
        let actor_value = field(metadata_value, "actor")?;
        let key_meta = |role: &str, key: Hash32| -> Result<AccountMetadata, String> {
            match metadata_value.get(role) {
                Some(entry) => account_metadata(entry, program),
                None => Ok(AccountMetadata {
                    key,
                    owner_program: program,
                    writable: true,
                }),
            }
        };
        let metadata = TransitionMetadata {
            market: key_meta("market", bindings.market)?,
            hoard: key_meta("hoard", bindings.hoard)?,
            position: key_meta("position", bindings.position)?,
            kernel: key_meta("kernel", bindings.kernel)?,
            external: key_meta("external", bindings.external)?,
            replay: key_meta("replay", bindings.replay)?,
            supply: key_meta("supply", bindings.supply)?,
            actor: ActorMetadata {
                key: hash(field(actor_value, "key")?)?,
                signer: field(actor_value, "signer")?.as_bool()?,
            },
        };

        let accounts = field(value, "accounts")?;
        let market_value = field(accounts, "market")?;
        let outcome_count = small_field(market_value, "outcome_count")? as usize;

        let market_id = hash(field(market_value, "market")?)?;
        let realm = hash(field(market_value, "realm")?)?;
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        let outcome_items = field(market_value, "outcomes")?.as_array()?;
        if outcome_items.len() != outcome_count {
            return Err("ARR-1: market.outcomes must be exactly `outcome_count` entries".into());
        }
        for (index, item) in outcome_items.iter().enumerate() {
            outcomes[index] = hash(item)?;
        }
        let market = MarketAccount {
            market: market_id,
            realm,
            profile: hash(field(market_value, "profile")?)?,
            terms: hash(field(market_value, "terms")?)?,
            outcome_count: outcome_count as u8,
            lifecycle: match str_field(market_value, "lifecycle")? {
                "active" => 0,
                "resolved" => 1,
                other => return Err(format!("ENUM-1: unknown lifecycle {other:?}")),
            },
            stored_bump: small_field(market_value, "stored_bump")? as u8,
            hoard_bump: small_field(market_value, "hoard_bump")? as u8,
            outcomes,
            feed: clutch_solana_layout::FeedId::from_bytes(read_hash32(field(
                market_value,
                "feed",
            )?)?),
            collateral_cap: u64_field(market_value, "collateral_cap")?,
            created_slot: u64_field(market_value, "created_slot")?,
            reserved: Hash32::ZERO,
        };

        let hoard_value = field(accounts, "hoard")?;
        let hoard = HoardAccount {
            market: market_id,
            realm,
            authority: hash(field(hoard_value, "authority")?)?,
            collateral_atoms: u64_field(hoard_value, "collateral_atoms")?,
            stored_bump: small_field(hoard_value, "stored_bump")? as u8,
            flags: small_field(hoard_value, "flags")? as u8,
        };

        let position_value = field(accounts, "position")?;
        let owner = hash(field(position_value, "owner")?)?;
        let position = PositionAccount {
            market: market_id,
            owner,
            generation: u64_field(position_value, "generation")?,
            internal: read_prefix(field(position_value, "internal")?, outcome_count)?,
            cash_atoms: u64_field(position_value, "cash_atoms")?,
            reserved_cash_atoms: u64_field(position_value, "reserved_cash_atoms")?,
            stored_bump: small_field(position_value, "stored_bump")? as u8,
            close_state: match str_field(position_value, "close_state")? {
                "open" => 0,
                "closing" => 1,
                other => return Err(format!("ENUM-1: unknown close state {other:?}")),
            },
        };

        let kernel_value = field(accounts, "kernel")?;
        let payouts_value = field(kernel_value, "payouts")?;
        let payout_count = small_field(payouts_value, "count")? as u8;
        let payout_items = field(payouts_value, "vectors")?.as_array()?;
        if payout_items.len() != usize::from(payout_count) {
            return Err("ARR-1: kernel.payouts.vectors must be exactly `count` entries".into());
        }
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        for (index, item) in payout_items.iter().enumerate() {
            vectors[index] = PayoutVector::new(
                u64_field(item, "denominator")?,
                read_prefix(field(item, "weights")?, outcome_count)?,
            );
        }
        let kernel = KernelAccount {
            market: market_id,
            phase: match str_field(kernel_value, "phase")? {
                "active" => 0,
                "resolved" => 1,
                other => return Err(format!("ENUM-1: unknown kernel phase {other:?}")),
            },
            basis_mode: match str_field(kernel_value, "basis_mode")? {
                "finite-preset" => clutch_kernel::BasisMode::FinitePreset,
                "derived-basis" => clutch_kernel::BasisMode::DerivedBasis,
                other => return Err(format!("ENUM-1: unknown kernel basis mode {other:?}")),
            },
            resolved_payout: small_field(kernel_value, "resolved_payout")? as u8,
            payouts: PayoutSet::new(
                payout_count,
                small_field(payouts_value, "outcomes")? as u8,
                vectors,
            ),
            total_supply: read_prefix(field(kernel_value, "total_supply")?, outcome_count)?,
        };

        let external_value = field(accounts, "external")?;
        let external = ExternalAccount {
            market: market_id,
            owner,
            position_generation: u64_field(external_value, "position_generation")?,
            balances: read_prefix(field(external_value, "balances")?, outcome_count)?,
            stored_bump: small_field(external_value, "stored_bump")? as u8,
            flags: small_field(external_value, "flags")? as u8,
        };

        let replay_value = field(accounts, "replay")?;
        let replay = ReplayAccount {
            market: market_id,
            owner,
            position_generation: u64_field(replay_value, "position_generation")?,
            sequence: u64_field(replay_value, "sequence")?,
            stored_bump: small_field(replay_value, "stored_bump")? as u8,
            flags: small_field(replay_value, "flags")? as u8,
        };

        let supply_value = field(accounts, "supply")?;
        let supply = SupplyLedgerAccount {
            market: market_id,
            realm,
            generation: u64_field(supply_value, "generation")?,
            outcome_count: outcome_count as u8,
            internal_supply: read_prefix(field(supply_value, "internal_supply")?, outcome_count)?,
            external_supply: read_prefix(field(supply_value, "external_supply")?, outcome_count)?,
            stored_bump: small_field(supply_value, "stored_bump")? as u8,
            flags: small_field(supply_value, "flags")? as u8,
        };

        let mut state = TransitionOutput {
            market: [0; account_len::MARKET],
            hoard: [0; account_len::HOARD],
            position: [0; account_len::POSITION],
            kernel: [0; KERNEL_ACCOUNT_LEN],
            external: [0; EXTERNAL_ACCOUNT_LEN],
            replay: [0; REPLAY_ACCOUNT_LEN],
            supply: [0; account_len::SUPPLY_LEDGER],
            resolution: None,
            redemption_payout: 0,
        };
        let enc = |what: &str, result: Result<usize, clutch_solana_layout::CodecError>| {
            result
                .map(|_| ())
                .map_err(|error| format!("{what} does not encode: {error:?}"))
        };
        enc("market", market.encode(&mut state.market))?;
        enc("hoard", hoard.encode(&mut state.hoard))?;
        enc("position", position.encode(&mut state.position))?;
        enc("supply", supply.encode(&mut state.supply))?;
        kernel
            .encode(&mut state.kernel)
            .map_err(|error| format!("kernel does not encode: {error:?}"))?;
        external
            .encode(&mut state.external)
            .map_err(|error| format!("external does not encode: {error:?}"))?;
        replay
            .encode(&mut state.replay)
            .map_err(|error| format!("replay does not encode: {error:?}"))?;

        Ok(Self {
            state,
            metadata,
            bindings,
            outcome_count,
            last_payout: 0,
        })
    }

    fn request(&self, value: &Value) -> Result<Vec<u8>, String> {
        let sequence = u64_field(value, "sequence")?;
        let mut out = vec![0u8; MAX_REQUEST_LEN];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        match str_field(value, "action")? {
            "layout" => {
                let intent = self.intent(field(value, "intent")?)?;
                let mut bytes = [0u8; clutch_solana_layout::MAX_INTENT_BYTES];
                let len = intent
                    .encode(&mut bytes)
                    .map_err(|error| format!("intent does not encode: {error:?}"))?;
                out[10] = ACTION_LAYOUT;
                out[11..13].copy_from_slice(&(len as u16).to_le_bytes());
                out[13..13 + len].copy_from_slice(&bytes[..len]);
                out.truncate(13 + len);
            }
            "resolve" => {
                out[10] = ACTION_RESOLVE;
                out[11] = small_field(value, "payout_index")? as u8;
                out.truncate(12);
            }
            "redeem_internal" => {
                out[10] = ACTION_REDEEM_INTERNAL;
                out[11] = small_field(value, "outcome")? as u8;
                out[12..20].copy_from_slice(&u64_field(value, "quantity")?.to_le_bytes());
                out.truncate(20);
            }
            other => return Err(format!("ENUM-1: unknown request action {other:?}")),
        }
        // The framing restatement above is checked against the owning crate.
        let decoded = clutch_solana_reference::Request::decode(&out)
            .map_err(|error| format!("the restated request framing does not decode: {error:?}"))?;
        if decoded.sequence != sequence {
            return Err("the restated request framing lost the sequence".into());
        }
        Ok(out)
    }

    fn intent(&self, value: &Value) -> Result<Intent, String> {
        Ok(match str_field(value, "kind")? {
            "split" => Intent::Split {
                market: hash(field(value, "market")?)?,
                owner: hash(field(value, "owner")?)?,
                quantity: u64_field(value, "quantity")?,
            },
            "merge" => Intent::Merge {
                market: hash(field(value, "market")?)?,
                owner: hash(field(value, "owner")?)?,
                quantity: u64_field(value, "quantity")?,
            },
            "materialize" => Intent::Materialize {
                market: hash(field(value, "market")?)?,
                owner: hash(field(value, "owner")?)?,
                destination: hash(field(value, "destination")?)?,
                outcome: small_field(value, "outcome")? as u8,
                quantity: u64_field(value, "quantity")?,
            },
            "dematerialize" => Intent::Dematerialize {
                market: hash(field(value, "market")?)?,
                owner: hash(field(value, "owner")?)?,
                source: hash(field(value, "source")?)?,
                outcome: small_field(value, "outcome")? as u8,
                quantity: u64_field(value, "quantity")?,
            },
            other => return Err(format!("ENUM-1: unsupported intent kind {other:?}")),
        })
    }

    fn state_bytes(&self) -> StateBytes<'_> {
        StateBytes {
            market: &self.state.market,
            hoard: &self.state.hoard,
            position: &self.state.position,
            kernel: &self.state.kernel,
            external: &self.state.external,
            replay: &self.state.replay,
            supply: &self.state.supply,
        }
    }
}

impl Executor for AdapterExecutor {
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String> {
        match op {
            "apply" => {
                let request = self.request(field(args, "request")?)?;
                let result = clutch_solana_reference::apply(
                    &request,
                    self.state_bytes(),
                    &self.metadata,
                    &self.bindings,
                );
                match result {
                    Ok(output) => {
                        self.last_payout = output.redemption_payout;
                        self.state = output;
                        Ok(Observed::Ok(obj(vec![(
                            "redemption_payout",
                            dec(u128::from(self.last_payout)),
                        )])))
                    }
                    // The adapter's contract is that no caller-provided output
                    // is mutated on error; `self.state` is left untouched here,
                    // so `post_state_on_error: "unchanged"` is a real check.
                    Err(error) => Ok(Observed::Error(code(error))),
                }
            }
            other => Err(format!("solana-reference has no operation {other:?}")),
        }
    }

    fn render_state(&self) -> Value {
        let count = self.outcome_count;
        let market = MarketAccount::decode(&self.state.market).expect("post-state market decodes");
        let hoard = HoardAccount::decode(&self.state.hoard).expect("post-state hoard decodes");
        let position =
            PositionAccount::decode(&self.state.position).expect("post-state position decodes");
        let kernel = KernelAccount::decode(&self.state.kernel).expect("post-state kernel decodes");
        let external =
            ExternalAccount::decode(&self.state.external).expect("post-state external decodes");
        let replay = ReplayAccount::decode(&self.state.replay).expect("post-state replay decodes");
        let supply =
            SupplyLedgerAccount::decode(&self.state.supply).expect("post-state supply decodes");
        obj(vec![
            (
                "market",
                obj(vec![(
                    "lifecycle",
                    Value::Str(
                        if market.lifecycle == 0 {
                            "active"
                        } else {
                            "resolved"
                        }
                        .to_string(),
                    ),
                )]),
            ),
            (
                "hoard",
                obj(vec![(
                    "collateral_atoms",
                    dec(u128::from(hoard.collateral_atoms)),
                )]),
            ),
            (
                "position",
                obj(vec![
                    ("internal", prefix(&position.internal, count)),
                    ("cash_atoms", dec(u128::from(position.cash_atoms))),
                ]),
            ),
            (
                "kernel",
                obj(vec![
                    (
                        "phase",
                        Value::Str(
                            if kernel.phase == 0 {
                                "active"
                            } else {
                                "resolved"
                            }
                            .to_string(),
                        ),
                    ),
                    ("total_supply", prefix(&kernel.total_supply, count)),
                ]),
            ),
            (
                "external",
                obj(vec![("balances", prefix(&external.balances, count))]),
            ),
            (
                "replay",
                obj(vec![("sequence", dec(u128::from(replay.sequence)))]),
            ),
            (
                "supply",
                obj(vec![
                    ("internal_supply", prefix(&supply.internal_supply, count)),
                    ("external_supply", prefix(&supply.external_supply, count)),
                ]),
            ),
        ])
    }
}
