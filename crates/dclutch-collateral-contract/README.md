# dClutch collateral wire contract

This crate owns the exact, SDK-free instruction bytes and account-role frames
for the first collateral lifecycle. It does not own SVM addresses, hashing,
rent calculation, CPI, token state, or Market economics.

The contracts intentionally use semantic instruction families instead of the
historical Dragon's Clutch action table. Provider-specific resolution data is
outside this crate. Market founding authenticates immutable Product-Instance,
ClaimBasis, CapacityProfile, resolution-policy, and capability-manifest
accounts committed by the canonical Market identity. The explicit Product
records are founding inputs rather than universal mutable Market children.
Market identity commits an occurrence-specific Product Instance which links
reusable Terms, Occurrence, ClaimBasis, and CapacityProfile content without
making each record universal mutable runtime state.

`FoundMarketAndFundV1` carries only the immutable Market identity and exact
outcome count. It carries no funding amounts. The authenticated capability
manifest must uniquely select the `RequiredAtFounding` entry whose `config_id`
equals the Market's `resolution_policy_id`. The current Pyth adapter derives
exact Fund rent, provider reimbursement, and positive success bounty from that
entry after validating the specialized one-shot Fund profile. Manifest funding
is the sole authority; sponsor-authored instruction values cannot override it.

The categorical width `2..=16` is a **provisional measured-profile bound**
shared with the current Realm and Market contracts. Its lifting path is a new
adapter release using paged or dynamically selected fixed-layout kernels; it
is not a mathematical limit on dClutch products.

The program-owned collateral-custody root persists the Vault-opening sponsor as
the one rent-refund recipient. Empty Vault retirement returns both Vault and
custody-root lamports exactly there; it never strands them in Market state or
pays an unauthenticated closer. Vault tokens are never a rent source, and
surplus token sweeping transfers only `vault.amount - Market.hoard`.
