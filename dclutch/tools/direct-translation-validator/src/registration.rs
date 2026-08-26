//! Safe executable reference for the registered-creation projection.
//!
//! This is validator code, not an on-chain implementation. It evaluates the
//! physical `u64` projection emitted by Lean and commits replay/registration
//! state only after every admission gate succeeds.

/// Persisted registered-intent projection produced by creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredState {
    /// Open is the only creation phase and has physical tag zero.
    pub(crate) phase: u8,
    /// Initial remaining quantity.
    pub(crate) remaining: u64,
    /// Registration-local replay sequence, initially zero.
    pub(crate) sequence: u64,
    /// Market identity coordinate used by the finite corpus.
    pub(crate) market: u64,
    /// Immutable Market generation.
    pub(crate) generation: u64,
    /// Authenticated maker identity coordinate.
    pub(crate) maker: u64,
    /// Globally consumed maker nonce.
    pub(crate) nonce: u64,
}

/// Registration coordinate before or after the attempted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegistrationSlot {
    /// The PDA-relevant registration coordinate is unoccupied.
    Vacant,
    /// A prior value occupies the coordinate.
    Occupied(RegisteredState),
}

/// Mutable projection atomically owned by registered creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Store {
    /// Global maker/Market/generation replay nonce.
    pub(crate) next_nonce: u64,
    /// Exact maker/Market/generation/nonce registration coordinate.
    pub(crate) registration: RegistrationSlot,
}

/// Untrusted observations needed by the semantic creation gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Request {
    /// Market phase: founding 0, open 1, resolved 2, retiring 3, retired 4.
    pub(crate) market_phase: u8,
    /// Current Clock slot.
    pub(crate) slot: u64,
    /// Market identity coordinate used by the finite corpus.
    pub(crate) market: u64,
    /// Immutable Market generation.
    pub(crate) generation: u64,
    /// Authenticated maker identity coordinate.
    pub(crate) maker: u64,
    /// Requested globally consumed maker nonce.
    pub(crate) nonce: u64,
    /// First signed valid slot.
    pub(crate) valid_from: u64,
    /// Last signed valid slot.
    pub(crate) valid_through: u64,
    /// Positive signed maximum fill.
    pub(crate) maximum: u64,
    /// Product-owned outcome coordinate.
    pub(crate) outcome: u64,
    /// Product outcome count.
    pub(crate) outcome_count: u64,
    /// Maker-signed fee rate.
    pub(crate) intent_fee: u64,
    /// Market-owned fee rate.
    pub(crate) policy_fee: u64,
}

const OCCUPIED_SENTINEL: RegisteredState = RegisteredState {
    phase: 3,
    remaining: 17,
    sequence: 19,
    market: 23,
    generation: 29,
    maker: 31,
    nonce: 37,
};

/// Construct the exact pre-state represented by the Lean `vacant` observation.
pub(crate) fn store(next_nonce: u64, vacant: bool) -> Store {
    Store {
        next_nonce,
        registration: if vacant {
            RegistrationSlot::Vacant
        } else {
            RegistrationSlot::Occupied(OCCUPIED_SENTINEL)
        },
    }
}

/// Apply registered creation atomically.
///
/// `false` guarantees that replay and registration remain exactly unchanged.
pub(crate) fn apply(store: &mut Store, request: Request) -> bool {
    let before = *store;
    let Some(next_nonce) = before.next_nonce.checked_add(1) else {
        return false;
    };
    if before.registration != RegistrationSlot::Vacant
        || request.market_phase != 1
        || request.valid_from > request.valid_through
        || request.slot > request.valid_through
        || request.maximum == 0
        || request.outcome >= request.outcome_count
        || request.intent_fee != request.policy_fee
        || request.nonce != before.next_nonce
    {
        return false;
    }
    *store = Store {
        next_nonce,
        registration: RegistrationSlot::Occupied(RegisteredState {
            phase: 0,
            remaining: request.maximum,
            sequence: 0,
            market: request.market,
            generation: request.generation,
            maker: request.maker,
            nonce: request.nonce,
        }),
    };
    true
}
