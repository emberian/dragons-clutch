//! Target-independent identity slots shared by registered Direct execution.

/// Native signer identity selected by both registered creation actions.
///
/// The host-only registered artifact producer authors this slot, while the
/// onchain artifact authenticator consumes it. Keeping the slot here prevents
/// either target from restating the persisted profile geometry.
pub const REGISTERED_IDENTITY_NATIVE_SIGNER_V4: usize = 1;
