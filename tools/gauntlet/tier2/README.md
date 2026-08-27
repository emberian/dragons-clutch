# Tier 2 — the Claims and Custody family fast lane

Tier 2 executes the Claims and Custody family routes against the **real
first-party ELFs** and the runtime's **real SPL Token and Token-2022 programs**,
under `solana-program-test` rather than a validator. Chain state — the Core
Market, the Registry activation cache, the Product Runtime V3 record graph, the
LiabilityBasisV2 aggregate, the Realm record, the token accounts, the lifecycle
RentCredit — is **installed directly**. There is no Hot execution and no open
Market in the loop, which is precisely why these routes are reachable here and
are not reachable in tier 1.

Every observation is recorded under its own campaign name (`claims-programtest`,
`custody-programtest`) and the census report prints that name next to the route.
A route whose only evidence came from this lane says so on its own row.

## What this lane evidences

- The real ELF refuses and accepts exactly what its taxonomy says. Every named
  refusal in `*-bindings.json` is compared by the census against the
  `custom program error: 0x…` the runtime itself logged.
- Rollback is byte-exact. Each campaign compares whole `Account` values —
  lamports, owner and data — before and after a refused transaction. The census
  does not model this; the campaigns assert it directly.
- Compute stays under the real 1,400,000-unit maximum with the real 32 KB heap.
  Both are gates here, not adjustable diagnostics: `set_compute_max_units` is
  called with exactly the runtime maximum and nothing requests a heap frame.
- Real token effects. The Custody campaign runs twice, once against
  `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` and once against
  `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`, and a witness requires the
  chain's own logs to show that program invoked.

## What this lane does NOT evidence, and tier 1 must

Stated plainly, because `TIERS.md`'s fast-lane bar demands it:

1. **Deployment.** The lane INSTALLS Loader-v3 ProgramData accounts with the
   documented layout instead of deploying and revoking. It therefore proves
   nothing about `SetAuthority(Some -> None)`, about ProgramData deployment
   slots, or about what a real deploy writes. Tier 1 owns that.
2. **Packet serialisation.** ProgramTest never submits a packet, so a frame that
   exceeds Solana's 1,232-byte legacy maximum survives this lane untouched. The
   Claims campaigns route their 26- and 22- and 15-account frames over a real
   Address Lookup Table as v0 messages, which is the shape a validator would
   need, but the byte budget itself is unproven here. Found31 was exactly this
   defect and it survived every fixture test.
3. **Fee, rent-collection and slot economics.** Warping slots is not the passage
   of time, and nothing here pays a real fee market.
4. **Concurrency.** One transaction at a time, no forks, no replay of a real
   ledger.

Compute figures printed by these campaigns vary by a few thousand units between
runs: PDA bump-seed search cost depends on the addresses, and some addresses are
per-run. Treat a tier-2 CU number as an order-of-magnitude measurement with the
1,400,000 gate on top of it, not as a pinned constant.

## Layout

```
tier2/
  campaign.sh            build the ELFs, run the campaigns, merge the evidence
  claims-bindings.json   census campaign `claims-programtest`
  custody-bindings.json  census campaign `custody-programtest`
  witnesses.json         asserted witnesses, each naming its provenance
  check-witnesses.sh     evaluates them against the merged evidence
```

The campaigns themselves live with the programs they exercise:

| campaign document | source |
|---|---|
| `custody-ordinary-legacy`, `custody-ordinary-token-2022` | `programs/dclutch-custody-sbf/tests/program_test.rs` |
| `claims-protocol-position` | `programs/dclutch-claims-sbf/program-test/protocol-position/tests/lifecycle.rs` |
| `claims-sparse-chain` | `programs/dclutch-claims-sbf/program-test/sparse-chain/tests/sparse_chain.rs` |

A campaign emits its document only when `DCLUTCH_CAMPAIGN_EVIDENCE_DIR` is set,
through the shared `tools/gauntlet/programtest/evidence.rs` module, which every
campaign includes by `#[path]` so a standalone program-test workspace needs no
manifest or lockfile change to participate. Run any campaign on its own with its
`run-program-test.sh`; run the lane with `tools/gauntlet/run.sh --mode fast`.

## Adding a campaign to this tier

1. Include `tools/gauntlet/programtest/evidence.rs`, open a `CampaignEvidence`
   with a name nothing else uses, declare the census program labels with the
   addresses the campaign actually gave those programs, record every submitted
   transaction, and `finish()`.
2. Add the manifest and test target to `CAMPAIGNS` in `campaign.sh`, and the
   document name to the census campaign group it belongs to. A document in no
   group is a hard error, not a silent skip.
3. Bind every transaction in the group's bindings file. An unbound label fails
   the census, and so does a binding that matched nothing.
4. A refusal raised by a program the census does not enumerate — a test-only
   caller that deliberately refuses after the child committed — uses
   `unnamed_refusal`, which checks the code against the chain but credits it to
   no first-party taxonomy. Naming a first-party refusal that happens to share
   the number is the mirror failure this tier exists to avoid.
