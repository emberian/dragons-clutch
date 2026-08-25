# Capability manifest contract

The Market's immutable `capability_manifest_id` is the only capability
authority. It must be the composing hash policy's content identity for the
exact canonical manifest bytes. Indexes, bit sets, UI labels, and cached
feature summaries are untrusted projections and cannot grant a capability.

Each entry uses content identities for its kind, implementation release,
configuration, capacity profile, child schema, and child derivation policy.
There is deliberately no global capability enum or permanent bit position.
Entries are strictly ordered by kind identity, so one manifest cannot select
two competing releases for the same kind.

## Reusable templates

`CapabilityTemplateV1` is a distinct `DCLTCTP1` content preimage, never a
Market manifest and never capability authority. It preserves the exact
profile-1 entry width and every manifest fact, but byte 194—reserved zero in a
realized manifest—selects how configuration is obtained. Static entries carry
their ordinary nonzero config ID and selector zero. An occurrence-resolution
entry carries selector one and a canonical all-zero config slot. Bytes 195–199
remain zero. Manifest bytes cannot decode as a template, and template bytes
cannot decode as a manifest.

Projection substitutes one authenticated occurrence Source-material ID into
the selected config slot, clears the template selector with the manifest's
ordinary reserved bytes, and preserves entry count, indices, kind ordering,
dependencies, releases, capacity, child derivation, activation, deadlines,
and funding byte-for-byte. The result is an ordinary canonical `DCLTCAP1`
manifest. Projection requires exactly one occurrence-resolution selector and
requires it to be `RequiredAtFounding`; missing, repeated, lazy, static-ID
collision, and substituted realized manifests refuse. Encoding prevalidates
the complete order and dependency DAG before mutating caller storage.

The capability crate owns exact projection but deliberately owns no hash
algorithm. A composing Series release streams the canonical 16-byte realized
manifest header and each projected 528-byte entry, in index order, into its
named content-hash boundary. For Source-material identity `s`, the resulting
manifest identity `m`—not the reusable template identity—is placed in the
Market identity. Found then authenticates the actual manifest bytes under `m`
and selects the unique founding entry with config `s`.

## Provisional artifact profile

Artifact profile 1 admits at most 16 entries and at most 16 dependencies per
entry. These are provisional encoding and measured SVM-frame bounds, not
mathematical or product limits. Dependency references are canonical entry
indices, not a global bit mask. A future profile lifts either bound by defining
a new profile/schema decoder and wider entry layout. Existing profile-1 bytes
and content identities remain valid; a Market that needs the wider profile is
founded with that profile's manifest identity rather than mutating an existing
Market.

## Funding boundary

`FundingQuoteV1` is 304 bytes of immutable manifest content. Each Rent,
Creation, Work, Provider, Bounty, Liquidity, and Service compartment contains
an explicit asset class and amount. Zero has exactly one representation:
`NotApplicable(0)`. A positive compartment is either `NativeLamports` or
`RealmCollateral`. Rent and Creation are mathematical/SVM compartments and may
only be native lamports; the other five are capability-selected. Capability
profiles narrow that choice further: for example, General admits native Work,
Bounty, and Service but no Realm collateral, while Dealer admits Realm
Liquidity and Service and requires its unused generic compartments to be N/A.

The quote and both halves of the 320-byte `FundingStateV1` carry independent
checked native-lamport and Realm-collateral totals. There is deliberately no
cross-asset `total_principal`, addition, comparison, exchange rate, or unit
conversion. Remaining plus released must reproduce every quoted compartment
with the same asset class and must conserve each asset total separately.

Any nonzero Realm collateral requires one immutable binding containing the
Realm content identity, collateral-release identity, token-program key, mint,
and refund token beneficiary. The binding is absent—and its 160-byte region is
canonical zero—when Realm collateral is zero. DREGG is not named or special.
The selected mint comes only through the Market's immutable Realm authority.

Physical custody is also two-dimensional. The program-owned funding-state PDA
holds exactly its current Rent minimum plus remaining native lamports. An
optional canonical Realm token vault holds remaining collateral atomic units;
its authority is a separate canonical capability-funding authority PDA. The
adapter authenticates state/vault ownership, derives all three addresses,
obtains current Rent, and constructs `FundingCustodyObservationV1`. Ordinary
construction, readiness, activation, and releases require exact equality and
therefore refuse both underfunding and donations. A token vault additionally
must match the quote's Realm, release, program, mint, authority, and vault PDA.

The seed domains are distinct:

- `dclutch/cap-funding/v1` derives the program-owned funding state from Market,
  generation, manifest entry, config, and capability release;
- `dclutch/cap-fund-auth/v1` derives its token-signing authority from the
  funding-state key; and
- `dclutch/cap-fund-vault/v1` derives the optional token vault from authority,
  token program, and mint.

`activate` releases only exact native Rent and Creation amounts. A subsequent
compartment release returns a typed transfer plan, so a Realm amount cannot be
executed as lamports or vice versa. The capability-specific adapter still owns
the beneficiary rule for nonterminal Work/Provider/Bounty/Liquidity/Service
movement and must persist state only with the successful physical transfer.

Terminal or abandonment close returns a complete `FundingClosePlanV1` rather
than a mixed total. Remaining native funding, state/vault Rent, and unsolicited
lamport donations route to the authenticated immutable Market RentCredit.
Remaining Realm collateral and unsolicited same-mint token donations route to
the quote's immutable token beneficiary. Donations are explicitly classified
as gifts to those refund destinations, never protocol revenue. The plan covers
every observed lamport and token atom, so neither an emptied vault nor its Rent
can become stranded.

Hoard collateral and expected future fees are unrepresentable here. Hoard is
never a funding source, beneficiary, fee, or close residual.

`RequiredAtFounding` entries must activate before Market opening.
`PrepaidLazy` entries may remain pending through opening, but their exact typed
custody—including native creation/rent and any Realm vault—is present from
founding and activation must occur no later than the committed slot deadline.

Market founding selects resolution funding by immutable meaning, not by a
caller-supplied amount or a conventional manifest position. The authenticated
manifest must contain exactly one `RequiredAtFounding` entry whose `config_id`
equals the Market identity's `resolution_policy_id`. The total no-allocation
selector returns that entry together with its canonical index and refuses both
missing and ambiguous matches; manifest order is never a tie breaker.

The current one-shot Pyth resolution Fund is a specialized native-only adapter
profile. At its adapter boundary, the selected entry's quote must contain:

- Fund-account rent equal to the authenticated Rent calculation;
- provider reimbursement committed by the manifest; and
- a positive resolution-success bounty.

Creation, Work, Liquidity, and Service must be N/A. Rent, Provider, and Bounty
must be native lamports, and the Realm binding must be absent because the Fund
has no token vault. Provider and bounty values are derived from the immutable
quote. They do not appear in the founding instruction, and neither Realm
collateral nor future fees may replace them.

## Market-opening readiness

`MarketOpeningReadinessV1` is a transient direct Market child, not an opaque
caller attestation and not another economic ledger. Its 128-byte exact record
binds Market key, generation, manifest content identity, exact manifest entry
count, canonical next entry index, and the sponsor rent-refund identity.
There is no stored Ready status: it is derived only when `next_entry_index ==
entry_count`.

The adapter authenticates the manifest content hash, derives the child using
`MARKET_OPENING_READINESS_PDA_DOMAIN`, and starts it with the Market child
count. That canonical domain is `dclutch/open-readiness/v1` (25 bytes), below
the chain-derived 32-byte maximum for one PDA seed component; adapters must not
hash or rewrite it. Each advance must name exactly the next manifest index and
supplies the
actual canonical `FundingStateV1`, typed physical custody observation, and
current slot. The kernel calls `validate_market_open`; required pending entries,
lazy deadline expiry, wrong Realm/mint/program/authority/vault, missing vault,
donations, underfunding, replay, skips, and reordering refuse before readiness
changes. Funding state remains the sole owner of every amount, released
compartment, and activation fact; the manifest quote remains immutable truth.

After an advance and before Open, the SBF adapter must seal capability
operations: while the Market is Founding, no capability operation may release
principal. At Open, SBF must include the immutable `CapabilityManifest` account,
authenticate its content identity from the Market root, decode its exact
canonical bytes, and pass that actual manifest to `require_ready_for_open` for
the exact Market/generation/manifest. It then atomically consumes and
rent-refunds readiness while creating custody, keeping the direct-child count
coherent. Malformed, noncanonical, wrong-identity, or wrong-count manifest
bytes must refuse before custody creation. This is an adapter transition
contract only; this crate does not implement SBF or duplicate custody facts.
