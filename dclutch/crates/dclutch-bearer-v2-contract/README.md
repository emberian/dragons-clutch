# dClutch Bearer V2 contract

Bearer V2 is not another claims ledger. It is the exact basis-vector subset of
an immutable `RepresentationDescriptorV2`: one selected Product outcome has
coefficient equal to the descriptor denominator and every other coefficient is
zero.

The crate authenticates the descriptor's graph digest, graph/root identity,
Market, release, Token program, authority, runtime width, and denominator, then
delegates physical composition to the canonical Rational Representation V2
request and plan. Token remains the only holder/supply owner; Claims remains the
only native/materialized quantity owner.

The offchain operator derives generic Rational V2 asset identities. The Claims
SBF adapter must rederive and authenticate those addresses before CPI; operator
derivation is never treated as authority.
