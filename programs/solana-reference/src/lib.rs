#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Offline reference adapter joining hostile-byte layouts to the pure kernel.
//!
//! This is deliberately not a Solana program. It has no entrypoint, account
//! runtime, PDA derivation, CPI, token implementation, clock, signatures, or
//! transaction atomicity. Callers provide explicit account metadata and fixed
//! account bytes. The adapter authenticates the facts it can represent, refuses
//! the facts it cannot, runs [`clutch_kernel`] on local copies, and returns exact
//! post-state bytes.
//!
//! The extra kernel, external-balance, and replay accounts are reference-only
//! state needed to expose the missing semantic seams. Their layouts are not a
//! deployment ABI.

use clutch_kernel::{
    Error as KernelError, MarketState, PayoutSet, PayoutVector, Phase, Position,
    MAX_OUTCOMES as KERNEL_MAX_OUTCOMES, MAX_PAYOUTS,
};
use clutch_solana_layout::{
    account_len, canonical_market_id, CodecError, Hash32, HoardAccount, Intent, MarketAccount,
    PositionAccount, ProfileAccount, RealmAccount, MAX_OUTCOMES,
};

const KERNEL_TAG: u8 = 0x41;
const EXTERNAL_TAG: u8 = 0x42;
const REPLAY_TAG: u8 = 0x43;
const REQUEST_TAG: u8 = 0xd1;
const REFERENCE_VERSION: u8 = 1;
const ACTION_LAYOUT: u8 = 0;
const ACTION_RESOLVE: u8 = 1;
const ACTION_REDEEM_INTERNAL: u8 = 2;
const PAYOUT_VECTOR_BYTES: usize = 8 + (8 * MAX_OUTCOMES);

/// Exact length of the reference-only kernel account.
pub const KERNEL_ACCOUNT_LEN: usize =
    2 + 32 + 1 + 1 + 1 + 1 + (8 * MAX_OUTCOMES) + (MAX_PAYOUTS * PAYOUT_VECTOR_BYTES);
/// Exact length of the reference-only external-balance account.
pub const EXTERNAL_ACCOUNT_LEN: usize = 2 + 32 + 32 + 8 + (8 * MAX_OUTCOMES) + 1 + 1;
/// Exact length of the reference-only replay account.
pub const REPLAY_ACCOUNT_LEN: usize = 2 + 32 + 32 + 8 + 8 + 1 + 1;
/// Largest request accepted by this reference adapter.
pub const MAX_REQUEST_LEN: usize = 2 + 8 + 1 + 2 + clutch_solana_layout::MAX_INTENT_BYTES;

/// Errors from metadata checks, codecs, or pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A frozen layout codec rejected hostile bytes.
    Layout(CodecError),
    /// The pure semantic kernel rejected a state or transition.
    Kernel(KernelError),
    /// A reference-only account or request had the wrong exact length.
    WrongLength,
    /// A reference-only discriminator was wrong.
    WrongTag,
    /// A reference-only version was unsupported.
    WrongVersion,
    /// A reference-only enum, flag, or padding field was invalid.
    NonCanonical,
    /// A checked arithmetic operation overflowed or underflowed.
    Arithmetic,
    /// A supplied account was not owned by the expected program identity.
    WrongProgramOwner,
    /// Two logical account roles shared one key.
    AccountAlias,
    /// An account key did not match the trusted binding supplied by the caller.
    WrongAccountKey,
    /// A state account required for a transition was not writable.
    NotWritable,
    /// The actor did not present a signature assertion.
    MissingSignature,
    /// The signed actor was not authorized for the requested action.
    UnauthorizedActor,
    /// No authority policy exists for this action, so it fails closed.
    AuthorizationUnavailable,
    /// No typed maturity, sealed-window, source, terms, and payout evidence exists.
    ResolutionEvidenceUnavailable,
    /// A stored bump differed from the separately supplied expected bump.
    WrongBump,
    /// Account identities, generations, phases, or immutable fields disagreed.
    MismatchedState,
    /// The closed reference model's one position did not equal aggregate supply.
    AggregateClosureMismatch,
    /// Market initialization contained pre-existing claims or a closing position.
    NonEmptyInitialization,
    /// A request sequence was stale, skipped, or exhausted.
    Replay,
    /// The operation is outside this deliberately small reference subset.
    UnsupportedIntent,
    /// The market collateral cap would be exceeded.
    CollateralCap,
}

impl From<CodecError> for Error {
    fn from(value: CodecError) -> Self {
        Self::Layout(value)
    }
}

impl From<KernelError> for Error {
    fn from(value: KernelError) -> Self {
        Self::Kernel(value)
    }
}

/// Result returned by this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Runtime metadata asserted for one account by a future adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetadata {
    /// Account key.
    pub key: Hash32,
    /// Runtime-reported owner program.
    pub owner_program: Hash32,
    /// Whether the instruction declared the account writable.
    pub writable: bool,
}

/// Runtime metadata asserted for a transaction signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorMetadata {
    /// Signer account key.
    pub key: Hash32,
    /// Whether the runtime authenticated a signature for this actor.
    pub signer: bool,
}

/// Metadata for every state role in a reference transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionMetadata {
    /// Frozen market account metadata.
    pub market: AccountMetadata,
    /// Collateral hoard account metadata.
    pub hoard: AccountMetadata,
    /// Owner position account metadata.
    pub position: AccountMetadata,
    /// Reference kernel-state account metadata.
    pub kernel: AccountMetadata,
    /// Reference external-balance account metadata.
    pub external: AccountMetadata,
    /// Reference replay account metadata.
    pub replay: AccountMetadata,
    /// Authenticated action actor.
    pub actor: ActorMetadata,
}

/// Trusted account bindings a real SVM adapter must derive rather than accept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedBindings {
    /// Program identity expected to own all state accounts.
    pub program_id: Hash32,
    /// Expected market account key.
    pub market: Hash32,
    /// Expected hoard account key.
    pub hoard: Hash32,
    /// Expected position account key.
    pub position: Hash32,
    /// Expected reference kernel account key.
    pub kernel: Hash32,
    /// Expected reference external account key.
    pub external: Hash32,
    /// Expected reference replay account key.
    pub replay: Hash32,
    /// Expected market PDA bump.
    pub market_bump: u8,
    /// Expected hoard PDA bump.
    pub hoard_bump: u8,
    /// Expected position PDA bump.
    pub position_bump: u8,
    /// Expected reference external-account bump.
    pub external_bump: u8,
    /// Expected reference replay-account bump.
    pub replay_bump: u8,
}

/// Immutable byte slices consumed by one reference transition.
#[derive(Clone, Copy, Debug)]
pub struct StateBytes<'a> {
    /// Market layout bytes.
    pub market: &'a [u8],
    /// Hoard layout bytes.
    pub hoard: &'a [u8],
    /// Position layout bytes.
    pub position: &'a [u8],
    /// Reference kernel-state bytes.
    pub kernel: &'a [u8],
    /// Reference external-balance bytes.
    pub external: &'a [u8],
    /// Reference replay bytes.
    pub replay: &'a [u8],
}

/// Exact post-state bytes returned only after every check succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionOutput {
    /// Market account post-state.
    pub market: [u8; account_len::MARKET],
    /// Hoard account post-state.
    pub hoard: [u8; account_len::HOARD],
    /// Position account post-state.
    pub position: [u8; account_len::POSITION],
    /// Reference kernel-state post-state.
    pub kernel: [u8; KERNEL_ACCOUNT_LEN],
    /// Reference external-balance post-state.
    pub external: [u8; EXTERNAL_ACCOUNT_LEN],
    /// Reference replay post-state.
    pub replay: [u8; REPLAY_ACCOUNT_LEN],
    /// Reserved redemption receipt; always zero while resolution evidence is unavailable.
    pub redemption_payout: u64,
}

/// Kernel-only facts not present in the frozen Solana layout prototype.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Kernel phase: zero active, one resolved.
    pub phase: u8,
    /// Selected payout index after resolution.
    pub resolved_payout: u8,
    /// Immutable finite payout set.
    pub payouts: PayoutSet,
    /// Aggregate internal plus external supply by outcome.
    pub total_supply: [u64; MAX_OUTCOMES],
}

/// Reference shadow for claims materialized outside the internal ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Associated owner identity.
    pub owner: Hash32,
    /// Position generation this shadow belongs to.
    pub position_generation: u64,
    /// External claim balances by outcome.
    pub balances: [u64; MAX_OUTCOMES],
    /// Stored bump checked against caller-supplied trusted derivation.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

/// Reference replay sequence, namespaced by position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Associated owner identity.
    pub owner: Hash32,
    /// Position generation this sequence belongs to.
    pub position_generation: u64,
    /// Exact next request sequence.
    pub sequence: u64,
    /// Stored bump checked against caller-supplied trusted derivation.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

/// A decoded reference request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// Exact replay sequence.
    pub sequence: u64,
    /// Requested semantic action.
    pub action: Action,
}

/// Actions supported by the offline reference adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// A frozen layout intent; only a strict subset can transition state.
    Layout(Intent),
    /// Wire request for resolution; execution currently refuses fail-closed.
    Resolve {
        /// Payout-vector index.
        payout_index: u8,
    },
    /// Wire request for redemption; execution currently refuses fail-closed.
    RedeemInternal {
        /// Outcome index.
        outcome: u8,
        /// Claim atoms to redeem exactly.
        quantity: u64,
    },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], expected: usize, tag: u8) -> Result<Self> {
        if bytes.len() != expected {
            return Err(Error::WrongLength);
        }
        if bytes[0] != tag {
            return Err(Error::WrongTag);
        }
        if bytes[1] != REFERENCE_VERSION {
            return Err(Error::WrongVersion);
        }
        Ok(Self { bytes, at: 2 })
    }
    fn raw<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::WrongLength)?;
        if end > self.bytes.len() {
            return Err(Error::WrongLength);
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.at..end]);
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.raw::<1>()?[0])
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.raw::<8>()?))
    }
    fn hash(&mut self) -> Result<Hash32> {
        Ok(Hash32::from_bytes(self.raw::<32>()?))
    }
    fn done(self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

struct Writer<'a> {
    bytes: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(bytes: &'a mut [u8], tag: u8) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(Error::WrongLength);
        }
        bytes[0] = tag;
        bytes[1] = REFERENCE_VERSION;
        Ok(Self { bytes, at: 2 })
    }
    fn raw(&mut self, value: &[u8]) -> Result<()> {
        let end = self.at.checked_add(value.len()).ok_or(Error::WrongLength)?;
        if end > self.bytes.len() {
            return Err(Error::WrongLength);
        }
        self.bytes[self.at..end].copy_from_slice(value);
        self.at = end;
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<()> {
        self.raw(&[value])
    }
    fn u64(&mut self, value: u64) -> Result<()> {
        self.raw(&value.to_le_bytes())
    }
    fn hash(&mut self, value: Hash32) -> Result<()> {
        self.raw(&value.bytes())
    }
    fn done(self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

impl KernelAccount {
    /// Encode the exact reference-only kernel account layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != KERNEL_ACCOUNT_LEN {
            return Err(Error::WrongLength);
        }
        let mut writer = Writer::new(out, KERNEL_TAG)?;
        writer.hash(self.market)?;
        writer.u8(self.phase)?;
        writer.u8(self.resolved_payout)?;
        writer.u8(self.payouts.count)?;
        writer.u8(self.payouts.outcomes)?;
        for amount in self.total_supply {
            writer.u64(amount)?;
        }
        for vector in self.payouts.vectors {
            writer.u64(vector.denominator)?;
            for weight in vector.weights {
                writer.u64(weight)?;
            }
        }
        writer.done()?;
        Ok(KERNEL_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only kernel account layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes, KERNEL_ACCOUNT_LEN, KERNEL_TAG)?;
        let market = reader.hash()?;
        let phase = reader.u8()?;
        let resolved_payout = reader.u8()?;
        let count = reader.u8()?;
        let outcomes = reader.u8()?;
        let mut total_supply = [0; MAX_OUTCOMES];
        for amount in &mut total_supply {
            *amount = reader.u64()?;
        }
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        for vector in &mut vectors {
            let denominator = reader.u64()?;
            let mut weights = [0; MAX_OUTCOMES];
            for weight in &mut weights {
                *weight = reader.u64()?;
            }
            *vector = PayoutVector::new(denominator, weights);
        }
        reader.done()?;
        let value = Self {
            market,
            phase,
            resolved_payout,
            payouts: PayoutSet::new(count, outcomes, vectors),
            total_supply,
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        if self.market == Hash32::ZERO || self.phase > 1 {
            return Err(Error::NonCanonical);
        }
        self.payouts.validate()?;
        if self.phase == 0 && self.resolved_payout != 0 {
            return Err(Error::NonCanonical);
        }
        if self.phase == 1 && self.resolved_payout >= self.payouts.count {
            return Err(Error::NonCanonical);
        }
        let count = usize::from(self.payouts.outcomes);
        if self.total_supply[count..].iter().any(|amount| *amount != 0) {
            return Err(Error::NonCanonical);
        }
        Ok(())
    }
}

impl ExternalAccount {
    /// Encode the exact reference-only external-balance layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != EXTERNAL_ACCOUNT_LEN || self.flags != 0 {
            return Err(Error::NonCanonical);
        }
        let mut writer = Writer::new(out, EXTERNAL_TAG)?;
        writer.hash(self.market)?;
        writer.hash(self.owner)?;
        writer.u64(self.position_generation)?;
        for balance in self.balances {
            writer.u64(balance)?;
        }
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.done()?;
        Ok(EXTERNAL_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only external-balance layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes, EXTERNAL_ACCOUNT_LEN, EXTERNAL_TAG)?;
        let market = reader.hash()?;
        let owner = reader.hash()?;
        let position_generation = reader.u64()?;
        let mut balances = [0; MAX_OUTCOMES];
        for balance in &mut balances {
            *balance = reader.u64()?;
        }
        let value = Self {
            market,
            owner,
            position_generation,
            balances,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.done()?;
        if value.market == Hash32::ZERO || value.owner == Hash32::ZERO || value.flags != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

impl ReplayAccount {
    /// Encode the exact reference-only replay layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != REPLAY_ACCOUNT_LEN || self.flags != 0 {
            return Err(Error::NonCanonical);
        }
        let mut writer = Writer::new(out, REPLAY_TAG)?;
        writer.hash(self.market)?;
        writer.hash(self.owner)?;
        writer.u64(self.position_generation)?;
        writer.u64(self.sequence)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.done()?;
        Ok(REPLAY_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only replay layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes, REPLAY_ACCOUNT_LEN, REPLAY_TAG)?;
        let value = Self {
            market: reader.hash()?,
            owner: reader.hash()?,
            position_generation: reader.u64()?,
            sequence: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.done()?;
        if value.market == Hash32::ZERO || value.owner == Hash32::ZERO || value.flags != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

impl Request {
    /// Decode a strict replay envelope and, where applicable, a frozen layout intent.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 11 || bytes.len() > MAX_REQUEST_LEN {
            return Err(Error::WrongLength);
        }
        if bytes[0] != REQUEST_TAG {
            return Err(Error::WrongTag);
        }
        if bytes[1] != REFERENCE_VERSION {
            return Err(Error::WrongVersion);
        }
        let sequence = u64::from_le_bytes(bytes[2..10].try_into().map_err(|_| Error::WrongLength)?);
        let action = match bytes[10] {
            ACTION_LAYOUT => {
                if bytes.len() < 13 {
                    return Err(Error::WrongLength);
                }
                let len = usize::from(u16::from_le_bytes(
                    bytes[11..13].try_into().map_err(|_| Error::WrongLength)?,
                ));
                if len > clutch_solana_layout::MAX_INTENT_BYTES || bytes.len() != 13 + len {
                    return Err(Error::WrongLength);
                }
                Action::Layout(Intent::decode(&bytes[13..])?)
            }
            ACTION_RESOLVE => {
                if bytes.len() != 12 {
                    return Err(Error::WrongLength);
                }
                Action::Resolve {
                    payout_index: bytes[11],
                }
            }
            ACTION_REDEEM_INTERNAL => {
                if bytes.len() != 20 {
                    return Err(Error::WrongLength);
                }
                Action::RedeemInternal {
                    outcome: bytes[11],
                    quantity: u64::from_le_bytes(
                        bytes[12..20].try_into().map_err(|_| Error::WrongLength)?,
                    ),
                }
            }
            _ => return Err(Error::NonCanonical),
        };
        Ok(Self { sequence, action })
    }
}

/// Validate an already encoded initial market against a create intent and kernel payout set.
///
/// This proves only local byte/identity coherence. It deliberately does not
/// authorize creation; [`apply`] refuses `CreateMarket` until an SVM authority
/// model exists.
pub fn validate_market_init(
    realm_bytes: &[u8],
    profile_bytes: &[u8],
    state: StateBytes<'_>,
    create_intent_bytes: &[u8],
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<()> {
    validate_metadata(metadata, bindings, false)?;
    let realm = RealmAccount::decode(realm_bytes)?;
    let profile = ProfileAccount::decode(profile_bytes)?;
    let market = MarketAccount::decode(state.market)?;
    let hoard = HoardAccount::decode(state.hoard)?;
    let position = PositionAccount::decode(state.position)?;
    let kernel = KernelAccount::decode(state.kernel)?;
    let external = ExternalAccount::decode(state.external)?;
    let replay = ReplayAccount::decode(state.replay)?;
    validate_links(
        &market, &hoard, &position, &kernel, &external, &replay, bindings,
    )?;
    let intent = Intent::decode(create_intent_bytes)?;
    let (intent_realm, intent_profile, nonce, outcomes, terms, feed) = match intent {
        Intent::CreateMarket {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        } => (realm, profile, market_nonce, outcome_count, terms, feed),
        _ => return Err(Error::UnsupportedIntent),
    };
    let expected_market = canonical_market_id(intent_realm, intent_profile, nonce);
    if realm.realm != intent_realm
        || realm.profile != intent_profile
        || profile.profile != intent_profile
        || profile.realm != intent_realm
        || realm.profile_version != profile.version
        || usize::from(realm.max_outcomes) != MAX_OUTCOMES
        || outcomes > realm.max_outcomes
        || market.market != expected_market
        || market.realm != intent_realm
        || market.profile != intent_profile
        || market.outcome_count != outcomes
        || market.terms != terms
        || market.feed != feed
        || market.lifecycle != 0
        || hoard.market != market.market
        || hoard.realm != market.realm
        || position.market != market.market
        || kernel.market != market.market
        || kernel.phase != 0
        || external.market != market.market
        || external.owner != position.owner
        || external.position_generation != position.generation
        || replay.market != market.market
        || replay.owner != position.owner
        || replay.position_generation != position.generation
    {
        return Err(Error::MismatchedState);
    }
    if hoard.collateral_atoms != 0
        || position.close_state != 0
        || position.internal.iter().any(|amount| *amount != 0)
        || position.cash_atoms != 0
        || position.reserved_cash_atoms != 0
        || kernel.total_supply.iter().any(|amount| *amount != 0)
        || external.balances.iter().any(|amount| *amount != 0)
        || replay.sequence != 0
    {
        return Err(Error::NonEmptyInitialization);
    }
    let pure = kernel_market(&market, &hoard, &kernel)?;
    pure.check_invariants()?;
    validate_padding(&market, &position, &kernel, &external)?;
    validate_aggregate_closure(&market, &position, &kernel, &external)?;
    Ok(())
}

/// Apply one strict request to local copies and return exact post-state bytes.
///
/// No caller-provided output is mutated on error. The returned state is an
/// offline transition witness, not evidence of SVM execution or token movement.
pub fn apply(
    request_bytes: &[u8],
    state: StateBytes<'_>,
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<TransitionOutput> {
    validate_metadata(metadata, bindings, true)?;
    let request = Request::decode(request_bytes)?;
    let market = MarketAccount::decode(state.market)?;
    let mut hoard = HoardAccount::decode(state.hoard)?;
    let mut position_account = PositionAccount::decode(state.position)?;
    let mut kernel_account = KernelAccount::decode(state.kernel)?;
    let mut external = ExternalAccount::decode(state.external)?;
    let mut replay = ReplayAccount::decode(state.replay)?;
    validate_links(
        &market,
        &hoard,
        &position_account,
        &kernel_account,
        &external,
        &replay,
        bindings,
    )?;
    validate_padding(&market, &position_account, &kernel_account, &external)?;
    validate_aggregate_closure(&market, &position_account, &kernel_account, &external)?;
    if request.sequence != replay.sequence {
        return Err(Error::Replay);
    }
    let next_sequence = replay.sequence.checked_add(1).ok_or(Error::Replay)?;
    let mut pure_market = kernel_market(&market, &hoard, &kernel_account)?;
    let mut pure_position = Position {
        internal: position_account.internal,
        external: external.balances,
    };
    let payout = match request.action {
        Action::Layout(intent) => match intent {
            Intent::Split {
                market: intent_market,
                owner,
                quantity,
            } => {
                authorize_owner(metadata.actor, position_account.owner)?;
                require_intent_binding(intent_market, owner, &market, &position_account)?;
                if market.lifecycle != 0 || position_account.close_state != 0 {
                    return Err(Error::MismatchedState);
                }
                let next_collateral = hoard
                    .collateral_atoms
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                if next_collateral > market.collateral_cap {
                    return Err(Error::CollateralCap);
                }
                position_account.cash_atoms = position_account
                    .cash_atoms
                    .checked_sub(quantity)
                    .ok_or(Error::Arithmetic)?;
                pure_market.split(&mut pure_position, quantity)?;
                0
            }
            Intent::Materialize {
                market: intent_market,
                owner,
                destination,
                outcome,
                quantity,
            } => {
                authorize_owner(metadata.actor, position_account.owner)?;
                require_intent_binding(intent_market, owner, &market, &position_account)?;
                if destination != metadata.external.key {
                    return Err(Error::WrongAccountKey);
                }
                pure_market.materialize(&mut pure_position, outcome, quantity)?;
                0
            }
            Intent::Dematerialize {
                market: intent_market,
                owner,
                source,
                outcome,
                quantity,
            } => {
                authorize_owner(metadata.actor, position_account.owner)?;
                require_intent_binding(intent_market, owner, &market, &position_account)?;
                if source != metadata.external.key {
                    return Err(Error::WrongAccountKey);
                }
                pure_market.dematerialize(&mut pure_position, outcome, quantity)?;
                0
            }
            Intent::CreateMarket { .. } => return Err(Error::AuthorizationUnavailable),
            _ => return Err(Error::UnsupportedIntent),
        },
        Action::Resolve { .. } | Action::RedeemInternal { .. } => {
            return Err(Error::ResolutionEvidenceUnavailable);
        }
    };
    hoard.collateral_atoms = pure_market.collateral;
    position_account.internal = pure_position.internal;
    external.balances = pure_position.external;
    kernel_account.phase = match pure_market.phase {
        Phase::Active => 0,
        Phase::Resolved => 1,
    };
    kernel_account.resolved_payout = pure_market.resolved_payout;
    kernel_account.total_supply = pure_market.total_supply;
    replay.sequence = next_sequence;
    validate_aggregate_closure(&market, &position_account, &kernel_account, &external)?;
    encode_output(
        market,
        hoard,
        position_account,
        kernel_account,
        external,
        replay,
        payout,
    )
}

fn kernel_market(
    market: &MarketAccount,
    hoard: &HoardAccount,
    kernel: &KernelAccount,
) -> Result<MarketState> {
    if usize::from(market.outcome_count) > KERNEL_MAX_OUTCOMES
        || kernel.payouts.outcomes != market.outcome_count
    {
        return Err(Error::MismatchedState);
    }
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(Error::NonCanonical),
    };
    let pure = MarketState {
        outcomes: market.outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        collateral: hoard.collateral_atoms,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    pure.check_invariants()?;
    Ok(pure)
}

fn validate_metadata(
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
    writable: bool,
) -> Result<()> {
    let accounts = [
        metadata.market,
        metadata.hoard,
        metadata.position,
        metadata.kernel,
        metadata.external,
        metadata.replay,
    ];
    let expected = [
        bindings.market,
        bindings.hoard,
        bindings.position,
        bindings.kernel,
        bindings.external,
        bindings.replay,
    ];
    for (index, account) in accounts.iter().enumerate() {
        if account.owner_program != bindings.program_id {
            return Err(Error::WrongProgramOwner);
        }
        if account.key != expected[index] {
            return Err(Error::WrongAccountKey);
        }
        if writable && !account.writable {
            return Err(Error::NotWritable);
        }
        if account.key == metadata.actor.key {
            return Err(Error::AccountAlias);
        }
        for other in &accounts[index + 1..] {
            if account.key == other.key {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

fn validate_links(
    market: &MarketAccount,
    hoard: &HoardAccount,
    position: &PositionAccount,
    kernel: &KernelAccount,
    external: &ExternalAccount,
    replay: &ReplayAccount,
    bindings: &ExpectedBindings,
) -> Result<()> {
    if market.stored_bump != bindings.market_bump
        || market.hoard_bump != bindings.hoard_bump
        || hoard.stored_bump != bindings.hoard_bump
        || position.stored_bump != bindings.position_bump
        || external.stored_bump != bindings.external_bump
        || replay.stored_bump != bindings.replay_bump
    {
        return Err(Error::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != position.market
        || market.market != kernel.market
        || market.market != external.market
        || market.market != replay.market
        || position.owner != external.owner
        || position.owner != replay.owner
        || position.generation != external.position_generation
        || position.generation != replay.position_generation
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(Error::MismatchedState);
    }
    Ok(())
}

fn validate_padding(
    market: &MarketAccount,
    position: &PositionAccount,
    kernel: &KernelAccount,
    external: &ExternalAccount,
) -> Result<()> {
    let count = usize::from(market.outcome_count);
    if position.internal[count..].iter().any(|amount| *amount != 0)
        || kernel.total_supply[count..]
            .iter()
            .any(|amount| *amount != 0)
        || external.balances[count..].iter().any(|amount| *amount != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn validate_aggregate_closure(
    market: &MarketAccount,
    position: &PositionAccount,
    kernel: &KernelAccount,
    external: &ExternalAccount,
) -> Result<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(market.outcome_count) {
        let local = position.internal[outcome]
            .checked_add(external.balances[outcome])
            .ok_or(Error::Arithmetic)?;
        if local != kernel.total_supply[outcome] {
            return Err(Error::AggregateClosureMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

fn authorize(actor: ActorMetadata, expected: Hash32) -> Result<()> {
    if !actor.signer {
        return Err(Error::MissingSignature);
    }
    if actor.key != expected {
        return Err(Error::UnauthorizedActor);
    }
    Ok(())
}

fn authorize_owner(actor: ActorMetadata, owner: Hash32) -> Result<()> {
    authorize(actor, owner)
}

fn require_intent_binding(
    intent_market: Hash32,
    owner: Hash32,
    market: &MarketAccount,
    position: &PositionAccount,
) -> Result<()> {
    if intent_market != market.market || owner != position.owner {
        return Err(Error::MismatchedState);
    }
    Ok(())
}

fn encode_output(
    market: MarketAccount,
    hoard: HoardAccount,
    position: PositionAccount,
    kernel: KernelAccount,
    external: ExternalAccount,
    replay: ReplayAccount,
    redemption_payout: u64,
) -> Result<TransitionOutput> {
    let mut output = TransitionOutput {
        market: [0; account_len::MARKET],
        hoard: [0; account_len::HOARD],
        position: [0; account_len::POSITION],
        kernel: [0; KERNEL_ACCOUNT_LEN],
        external: [0; EXTERNAL_ACCOUNT_LEN],
        replay: [0; REPLAY_ACCOUNT_LEN],
        redemption_payout,
    };
    market.encode(&mut output.market)?;
    hoard.encode(&mut output.hoard)?;
    position.encode(&mut output.position)?;
    kernel.encode(&mut output.kernel)?;
    external.encode(&mut output.external)?;
    replay.encode(&mut output.replay)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use clutch_solana_layout::{
        canonical_outcome_id, canonical_profile_hash, canonical_realm_id, FeedId,
    };

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn payout_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut left = [0; MAX_OUTCOMES];
        left[0] = 1;
        vectors[0] = PayoutVector::new(1, left);
        let mut right = [0; MAX_OUTCOMES];
        right[1] = 1;
        vectors[1] = PayoutVector::new(1, right);
        PayoutSet::new(2, 2, vectors)
    }

    struct Fixture {
        state: TransitionOutput,
        metadata: TransitionMetadata,
        bindings: ExpectedBindings,
        realm: [u8; account_len::REALM],
        profile: [u8; account_len::PROFILE],
        create: [u8; 139],
    }

    fn fixture() -> Fixture {
        let profile_hash = canonical_profile_hash(b"fixture-profile");
        let realm_hash = canonical_realm_id(profile_hash, 7);
        let market_id = canonical_market_id(realm_hash, profile_hash, 9);
        let owner = h(31);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market_id, 0);
        outcomes[1] = canonical_outcome_id(market_id, 1);
        let market = MarketAccount {
            market: market_id,
            realm: realm_hash,
            profile: profile_hash,
            terms: h(8),
            outcome_count: 2,
            lifecycle: 0,
            stored_bump: 3,
            hoard_bump: 4,
            outcomes,
            feed: FeedId::from_bytes([9; 32]),
            collateral_cap: 1_000,
            created_slot: 55,
            reserved: Hash32::ZERO,
        };
        let hoard = HoardAccount {
            market: market_id,
            realm: realm_hash,
            authority: h(10),
            collateral_atoms: 0,
            stored_bump: 4,
            flags: 0,
        };
        let position = PositionAccount {
            market: market_id,
            owner,
            generation: 2,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 100,
            reserved_cash_atoms: 7,
            stored_bump: 5,
            close_state: 0,
        };
        let kernel = KernelAccount {
            market: market_id,
            phase: 0,
            resolved_payout: 0,
            payouts: payout_set(),
            total_supply: [0; MAX_OUTCOMES],
        };
        let external = ExternalAccount {
            market: market_id,
            owner,
            position_generation: 2,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 6,
            flags: 0,
        };
        let replay = ReplayAccount {
            market: market_id,
            owner,
            position_generation: 2,
            sequence: 0,
            stored_bump: 7,
            flags: 0,
        };
        let mut state = TransitionOutput {
            market: [0; account_len::MARKET],
            hoard: [0; account_len::HOARD],
            position: [0; account_len::POSITION],
            kernel: [0; KERNEL_ACCOUNT_LEN],
            external: [0; EXTERNAL_ACCOUNT_LEN],
            replay: [0; REPLAY_ACCOUNT_LEN],
            redemption_payout: 0,
        };
        market.encode(&mut state.market).unwrap();
        hoard.encode(&mut state.hoard).unwrap();
        position.encode(&mut state.position).unwrap();
        kernel.encode(&mut state.kernel).unwrap();
        external.encode(&mut state.external).unwrap();
        replay.encode(&mut state.replay).unwrap();
        let program = h(50);
        let keys = [h(51), h(52), h(53), h(54), h(55), h(56)];
        let am = |key| AccountMetadata {
            key,
            owner_program: program,
            writable: true,
        };
        let metadata = TransitionMetadata {
            market: am(keys[0]),
            hoard: am(keys[1]),
            position: am(keys[2]),
            kernel: am(keys[3]),
            external: am(keys[4]),
            replay: am(keys[5]),
            actor: ActorMetadata {
                key: owner,
                signer: true,
            },
        };
        let bindings = ExpectedBindings {
            program_id: program,
            market: keys[0],
            hoard: keys[1],
            position: keys[2],
            kernel: keys[3],
            external: keys[4],
            replay: keys[5],
            market_bump: 3,
            hoard_bump: 4,
            position_bump: 5,
            external_bump: 6,
            replay_bump: 7,
        };
        let realm = RealmAccount {
            realm: realm_hash,
            profile: profile_hash,
            max_outcomes: 16,
            profile_version: 2,
            stored_bump: 2,
            flags: 0,
        };
        let profile = ProfileAccount {
            profile: profile_hash,
            realm: realm_hash,
            version: 2,
            flags: 0,
            collateral_policy_digest: Hash32::ZERO,
        };
        let mut realm_bytes = [0; account_len::REALM];
        let mut profile_bytes = [0; account_len::PROFILE];
        realm.encode(&mut realm_bytes).unwrap();
        profile.encode(&mut profile_bytes).unwrap();
        let create_intent = Intent::CreateMarket {
            realm: realm_hash,
            profile: profile_hash,
            market_nonce: 9,
            outcome_count: 2,
            terms: h(8),
            feed: h(9),
        };
        let mut create = [0; 139];
        assert_eq!(create_intent.encode(&mut create), Ok(139));
        Fixture {
            state,
            metadata,
            bindings,
            realm: realm_bytes,
            profile: profile_bytes,
            create,
        }
    }

    fn state_bytes(state: &TransitionOutput) -> StateBytes<'_> {
        StateBytes {
            market: &state.market,
            hoard: &state.hoard,
            position: &state.position,
            kernel: &state.kernel,
            external: &state.external,
            replay: &state.replay,
        }
    }

    fn clear_init_cash(state: &mut TransitionOutput) {
        let mut position = PositionAccount::decode(&state.position).unwrap();
        position.cash_atoms = 0;
        position.reserved_cash_atoms = 0;
        position.encode(&mut state.position).unwrap();
    }

    fn layout_request(sequence: u64, intent: Intent) -> [u8; MAX_REQUEST_LEN] {
        let mut intent_bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
        let len = intent.encode(&mut intent_bytes).unwrap();
        let mut out = [0; MAX_REQUEST_LEN];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_LAYOUT;
        out[11..13].copy_from_slice(&(len as u16).to_le_bytes());
        out[13..13 + len].copy_from_slice(&intent_bytes[..len]);
        out
    }

    fn layout_request_len(request: &[u8; MAX_REQUEST_LEN]) -> usize {
        13 + usize::from(u16::from_le_bytes([request[11], request[12]]))
    }

    fn resolve_request(sequence: u64, payout: u8) -> [u8; 12] {
        let mut out = [0; 12];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_RESOLVE;
        out[11] = payout;
        out
    }

    fn redeem_request(sequence: u64, outcome: u8, quantity: u64) -> [u8; 20] {
        let mut out = [0; 20];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_REDEEM_INTERNAL;
        out[11] = outcome;
        out[12..20].copy_from_slice(&quantity.to_le_bytes());
        out
    }

    #[test]
    fn initialized_market_validation_runs_kernel_invariants() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Ok(())
        );
    }

    #[test]
    fn initialized_market_refuses_preexisting_position_claims() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        let mut hoard = HoardAccount::decode(&f.state.hoard).unwrap();
        hoard.collateral_atoms = 1;
        hoard.encode(&mut f.state.hoard).unwrap();
        let mut position = PositionAccount::decode(&f.state.position).unwrap();
        position.internal[0] = 1;
        position.internal[1] = 1;
        position.encode(&mut f.state.position).unwrap();
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        kernel.total_supply[0] = 1;
        kernel.total_supply[1] = 1;
        kernel.encode(&mut f.state.kernel).unwrap();
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::NonEmptyInitialization)
        );
    }

    #[test]
    fn forged_position_cannot_materialize_claims_absent_from_aggregate() {
        let mut f = fixture();
        let mut position = PositionAccount::decode(&f.state.position).unwrap();
        position.internal[0] = 1;
        position.encode(&mut f.state.position).unwrap();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let request = layout_request(
            0,
            Intent::Materialize {
                market,
                owner: position.owner,
                destination: f.metadata.external.key,
                outcome: 0,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::AggregateClosureMismatch)
        );
        assert_eq!(
            KernelAccount::decode(&f.state.kernel).unwrap().total_supply[0],
            0
        );
        assert_eq!(
            ExternalAccount::decode(&f.state.external).unwrap().balances[0],
            0
        );
    }

    #[test]
    fn split_has_exact_full_account_pre_and_post_vectors() {
        let f = fixture();
        let market_id = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            0,
            Intent::Split {
                market: market_id,
                owner,
                quantity: 11,
            },
        );
        let output = apply(
            &request[..layout_request_len(&request)],
            state_bytes(&f.state),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();

        let expected_market = f.state.market;
        let mut expected_hoard = f.state.hoard;
        expected_hoard[98..106].copy_from_slice(&11_u64.to_le_bytes());
        let mut expected_position = f.state.position;
        expected_position[74..82].copy_from_slice(&11_u64.to_le_bytes());
        expected_position[82..90].copy_from_slice(&11_u64.to_le_bytes());
        expected_position[202..210].copy_from_slice(&89_u64.to_le_bytes());
        let mut expected_kernel = f.state.kernel;
        expected_kernel[38..46].copy_from_slice(&11_u64.to_le_bytes());
        expected_kernel[46..54].copy_from_slice(&11_u64.to_le_bytes());
        let expected_external = f.state.external;
        let mut expected_replay = f.state.replay;
        expected_replay[74..82].copy_from_slice(&1_u64.to_le_bytes());

        assert_eq!(output.market, expected_market);
        assert_eq!(output.hoard, expected_hoard);
        assert_eq!(output.position, expected_position);
        assert_eq!(output.kernel, expected_kernel);
        assert_eq!(output.external, expected_external);
        assert_eq!(output.replay, expected_replay);
        assert_eq!(output.redemption_payout, 0);
    }

    #[test]
    fn materialize_and_dematerialize_are_supply_neutral() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let split = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 20,
            },
        );
        let split_state = apply(
            &split[..layout_request_len(&split)],
            state_bytes(&f.state),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let materialize = layout_request(
            1,
            Intent::Materialize {
                market,
                owner,
                destination: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        );
        let materialized = apply(
            &materialize[..layout_request_len(&materialize)],
            state_bytes(&split_state),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let kernel_before = KernelAccount::decode(&split_state.kernel).unwrap();
        let kernel_after = KernelAccount::decode(&materialized.kernel).unwrap();
        assert_eq!(kernel_after.total_supply, kernel_before.total_supply);
        assert_eq!(
            PositionAccount::decode(&materialized.position)
                .unwrap()
                .internal[1],
            13
        );
        assert_eq!(
            ExternalAccount::decode(&materialized.external)
                .unwrap()
                .balances[1],
            7
        );

        let dematerialize = layout_request(
            2,
            Intent::Dematerialize {
                market,
                owner,
                source: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        );
        let restored = apply(
            &dematerialize[..layout_request_len(&dematerialize)],
            state_bytes(&materialized),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(
            PositionAccount::decode(&restored.position)
                .unwrap()
                .internal[1],
            20
        );
        assert_eq!(
            ExternalAccount::decode(&restored.external)
                .unwrap()
                .balances[1],
            0
        );
    }

    #[test]
    fn bounded_closed_traces_preserve_position_aggregate_equality() {
        let mut quantity = 1_u64;
        while quantity <= 16 {
            let f = fixture();
            let market = MarketAccount::decode(&f.state.market).unwrap().market;
            let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
            let split = layout_request(
                0,
                Intent::Split {
                    market,
                    owner,
                    quantity,
                },
            );
            let split_state = apply(
                &split[..layout_request_len(&split)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            )
            .unwrap();
            let moved = quantity / 2;
            let state = if moved == 0 {
                split_state
            } else {
                let materialize = layout_request(
                    1,
                    Intent::Materialize {
                        market,
                        owner,
                        destination: f.metadata.external.key,
                        outcome: 0,
                        quantity: moved,
                    },
                );
                apply(
                    &materialize[..layout_request_len(&materialize)],
                    state_bytes(&split_state),
                    &f.metadata,
                    &f.bindings,
                )
                .unwrap()
            };
            let position = PositionAccount::decode(&state.position).unwrap();
            let external = ExternalAccount::decode(&state.external).unwrap();
            let kernel = KernelAccount::decode(&state.kernel).unwrap();
            let mut outcome = 0_usize;
            while outcome < 2 {
                assert_eq!(
                    position.internal[outcome] + external.balances[outcome],
                    kernel.total_supply[outcome]
                );
                outcome += 1;
            }
            quantity += 1;
        }
    }

    #[test]
    fn signer_cannot_bypass_missing_resolution_evidence() {
        let mut f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let split = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 15,
            },
        );
        let split_state = apply(
            &split[..layout_request_len(&split)],
            state_bytes(&f.state),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        f.metadata.actor = ActorMetadata {
            key: h(60),
            signer: true,
        };
        assert_eq!(
            apply(
                &resolve_request(1, 1),
                state_bytes(&split_state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );

        let mut forged_resolved = split_state;
        let mut market_account = MarketAccount::decode(&forged_resolved.market).unwrap();
        market_account.lifecycle = 1;
        market_account.encode(&mut forged_resolved.market).unwrap();
        let mut kernel = KernelAccount::decode(&forged_resolved.kernel).unwrap();
        kernel.phase = 1;
        kernel.resolved_payout = 1;
        kernel.encode(&mut forged_resolved.kernel).unwrap();
        f.metadata.actor = ActorMetadata {
            key: owner,
            signer: true,
        };
        assert_eq!(
            apply(
                &redeem_request(1, 1, 15),
                state_bytes(&forged_resolved),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );
    }

    #[test]
    fn aliases_versions_owners_bumps_and_replays_fail_closed() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        let request = &request[..layout_request_len(&request)];

        let mut alias = f.metadata;
        alias.hoard.key = alias.market.key;
        assert_eq!(
            apply(request, state_bytes(&f.state), &alias, &f.bindings),
            Err(Error::AccountAlias)
        );

        let mut versioned = f.state.market;
        versioned[1] = 2;
        let state = StateBytes {
            market: &versioned,
            ..state_bytes(&f.state)
        };
        assert_eq!(
            apply(request, state, &f.metadata, &f.bindings),
            Err(Error::Layout(CodecError::WrongVersion))
        );

        let mut wrong_owner = f.metadata;
        wrong_owner.kernel.owner_program = h(99);
        assert_eq!(
            apply(request, state_bytes(&f.state), &wrong_owner, &f.bindings),
            Err(Error::WrongProgramOwner)
        );

        let mut wrong_bump = f.bindings;
        wrong_bump.position_bump ^= 1;
        assert_eq!(
            apply(request, state_bytes(&f.state), &f.metadata, &wrong_bump),
            Err(Error::WrongBump)
        );

        let first = apply(request, state_bytes(&f.state), &f.metadata, &f.bindings).unwrap();
        assert_eq!(
            apply(request, state_bytes(&first), &f.metadata, &f.bindings),
            Err(Error::Replay)
        );
    }

    #[test]
    fn replay_and_arithmetic_overflow_refuse_without_output() {
        let mut f = fixture();
        let mut replay = ReplayAccount::decode(&f.state.replay).unwrap();
        replay.sequence = u64::MAX;
        replay.encode(&mut f.state.replay).unwrap();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            u64::MAX,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Replay)
        );

        let overflow = fixture();
        let mut hoard = HoardAccount::decode(&overflow.state.hoard).unwrap();
        hoard.collateral_atoms = u64::MAX;
        let mut hoard_bytes = overflow.state.hoard;
        hoard.encode(&mut hoard_bytes).unwrap();
        let mut position = PositionAccount::decode(&overflow.state.position).unwrap();
        position.internal[0] = u64::MAX;
        position.internal[1] = u64::MAX;
        let mut position_bytes = overflow.state.position;
        position.encode(&mut position_bytes).unwrap();
        let mut kernel = KernelAccount::decode(&overflow.state.kernel).unwrap();
        kernel.total_supply[0] = u64::MAX;
        kernel.total_supply[1] = u64::MAX;
        let mut kernel_bytes = overflow.state.kernel;
        kernel.encode(&mut kernel_bytes).unwrap();
        let overflow_state = StateBytes {
            hoard: &hoard_bytes,
            position: &position_bytes,
            kernel: &kernel_bytes,
            ..state_bytes(&overflow.state)
        };
        let overflow_request = layout_request(
            0,
            Intent::Split {
                market: MarketAccount::decode(overflow_state.market).unwrap().market,
                owner: position.owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &overflow_request[..layout_request_len(&overflow_request)],
                overflow_state,
                &overflow.metadata,
                &overflow.bindings,
            ),
            Err(Error::Arithmetic)
        );
    }

    #[test]
    fn unsupported_layout_intents_and_unsigned_owner_refuse() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let merge = layout_request(
            0,
            Intent::Merge {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &merge[..layout_request_len(&merge)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::UnsupportedIntent)
        );
        let mut unsigned = f.metadata;
        unsigned.actor.signer = false;
        let split = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &split[..layout_request_len(&split)],
                state_bytes(&f.state),
                &unsigned,
                &f.bindings,
            ),
            Err(Error::MissingSignature)
        );

        let wrong_owner = layout_request(
            0,
            Intent::Split {
                market,
                owner: h(98),
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &wrong_owner[..layout_request_len(&wrong_owner)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::MismatchedState)
        );
    }
}
