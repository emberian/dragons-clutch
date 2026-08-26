# dClutch Bearer V2 operator

This crate derives the generic Rational Representation V2 Claims PDAs and
constructs the canonical shared `RepresentationRequestV2` for Denominate,
Reconstitute, and RedeemTerminal. It defines no Bearer instruction wire and no
offchain authority.

Associated holder accounts are a convenience, not an identity restriction.
Callers may select another transferable Token account; the Claims adapter must
authenticate the exact account Mint and owner from chain state.
