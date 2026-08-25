# General capability configuration V2

This narrow safe `no_std`/`no_alloc` crate owns the 232-byte immutable General
configuration consumed by successor adapters. Its layout and exact fixture are
generated from `DClutchSemantics.GeneralConfigAbi`.

V2 uses the former trailing 32-byte region for a nonzero
`quote_surplus_beneficiary`: the token-owner authority that must own any
operational destination account receiving settlement quote surplus. It is not
a candidate field, token-account identity, fee destination, or lamport
RentCredit beneficiary. The Market capability manifest authenticates the hash
of the complete config bytes.

The config also commits the nonzero `selection_policy_id`. The immutable policy
record owns the interpreted criterion list and mandatory candidate-ID tie-break;
the config selects that record, and a batch cursor may not select its objective.

The configured order bound counts distinct globally grouped order identities,
not execution fragments. The streamed verifier persists that count across page
boundaries, so splitting one order across pages neither changes quote rounding
nor consumes another configured order slot.

The same Lean ABI owns a 128-byte `GeneralRootV2`. The root persists only its
canonical Core Market key, immutable generation, authenticated `config_id`,
lifecycle, revision, next-batch sequence, and live-batch count. It deliberately
does not copy the config's beneficiary/capacities or Core's RentCredit. General
root activation classifies a precreated PDA's lamports into exact Rent, displaced
prepaid Rent, and unsolicited surplus; the latter two route to the authenticated
Core Market's immutable RentCredit.
