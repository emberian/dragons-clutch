//! Safe executable reference for the registered terminal projection.
//!
//! This is validator code, not an on-chain implementation. It independently
//! evaluates the physical `u64` projection emitted by the Lean corpus and
//! deliberately mutates caller-owned state only after every gate succeeds.

/// Registered terminal action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    /// Maker-authorized cancellation.
    Cancel,
    /// Permissionless expiry after the signed window.
    Expire,
}

impl Action {
    /// Decode the Lean-owned action tag.
    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Cancel),
            1 => Some(Self::Expire),
            _ => None,
        }
    }
}

/// Physical projection sufficient to decide and apply a terminal transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct State {
    /// Open 0, filled 1, cancelled 2, or expired 3.
    pub(crate) phase: u8,
    /// Retained unfilled quantity.
    pub(crate) remaining: u64,
    /// Exact signed maximum.
    pub(crate) maximum: u64,
    /// Registration-local replay sequence.
    pub(crate) sequence: u64,
    /// Last signed valid slot.
    pub(crate) valid_through: u64,
    /// Persisted authenticated maker coordinate.
    pub(crate) maker: u64,
}

/// Untrusted adapter observations surrounding one terminal request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Request {
    /// Cancellation or expiry.
    pub(crate) action: Action,
    /// Current Clock slot for expiry; ignored by cancellation.
    pub(crate) slot: u64,
    /// Caller-observed local sequence.
    pub(crate) expected_sequence: u64,
    /// Signing maker coordinate for cancellation; ignored by expiry.
    pub(crate) actor_maker: u64,
}

fn valid(state: State) -> bool {
    state.remaining <= state.maximum
        && match state.phase {
            0 => state.remaining > 0,
            1 => state.remaining == 0,
            2 | 3 => true,
            _ => false,
        }
}

/// Apply the terminal transition atomically.
///
/// `false` guarantees that `state` remains byte-for-byte unchanged.
pub(crate) fn apply(state: &mut State, request: Request) -> bool {
    let before = *state;
    let action_allowed = match request.action {
        Action::Cancel => request.actor_maker == before.maker,
        Action::Expire => request.slot > before.valid_through,
    };
    let Some(sequence) = before.sequence.checked_add(1) else {
        return false;
    };
    if !valid(before)
        || before.phase != 0
        || before.sequence != request.expected_sequence
        || !action_allowed
    {
        return false;
    }
    *state = State {
        phase: match request.action {
            Action::Cancel => 2,
            Action::Expire => 3,
        },
        sequence,
        ..before
    };
    true
}

/// Strictly classify the exact claim-owner terminal instruction.
pub(crate) fn decode_claim(input: &[u8]) -> Option<(Action, u64)> {
    let header: [u8; 8] = input.get(..8)?.try_into().ok()?;
    let action = match &header {
        b"DCRC\x01\0\0\0" => Action::Cancel,
        b"DCRE\x01\0\0\0" => Action::Expire,
        _ => return None,
    };
    if input.len() != 16 {
        return None;
    }
    let sequence = u64::from_le_bytes(input.get(8..16)?.try_into().ok()?);
    Some((action, sequence))
}
