# Decision-0012 devnet Upgrade dryplan

This tool creates and checks an offline plan for updating the five mutable
execution programs behind Dragon's Clutch's permanent devnet addresses. It
also carries the two unchanged infrastructure programs and the ordered path
from the updated deployment set to an open market, live activity, and the
public site.

It cannot read RPC, open a keypair, sign, submit, deploy, fund, or publish. A
dryplan always says `mutationPermitted: false`. The existing successor commands
remain the only operator for captures, Buffer writes, Loader instructions,
release activation, founding, and activity.

## Permanent set

The operational identity owner is
`PERMANENT_DEVNET_UPGRADE_TARGETS_V1` in `successor/src/upgrade.rs`. This tool
independently refuses any planning input that does not match it exactly:

| order | role | disposition | Program | ProgramData |
|---:|---|---|---|---|
| 0 | Registry | CarryForward | `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj` | `ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz` |
| 1 | Rent | CarryForward | `DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3` | `78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy` |
| 2 | Custody | Upgrade | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | `EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf` |
| 3 | Resolution | Upgrade | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | `2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f` |
| 4 | Claims | Upgrade | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | `4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j` |
| 5 | Trading | Upgrade | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | `AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn` |
| 6 | Core | Upgrade | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | `AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN` |

The retained public authority is
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`. The tool accepts only the
canonical devnet endpoint and genesis hash.

The current pre-write payload pins are:

| role | slot | live ELF bytes | ProgramData lamports | live ELF SHA-256 |
|---|---:|---:|---:|---|
| Registry | 489100383 | 207,072 | 1,442,425,200 | `e1f4a20f0fefb60ad8f809f153c4403363d298d5eb11b88e29abe404048ac6e1` |
| Rent | 489100242 | 137,608 | 958,955,760 | `3b857b2236522c29e17b7d73cf27df6e6028fd8298a52df386753638f915ff79` |
| Custody | 489100460 | 360,328 | 2,509,086,960 | `d171cf742391dcc6ff152171657187d6a62538f38cedc9ce048af457b16746f1` |
| Resolution | 489100560 | 588,336 | 4,096,022,640 | `03842494bc1604b7f4806962157f93529056848f51499a4e0de771d1b8ab1fbf` |
| Claims | 489100803 | 1,010,496 | 7,034,256,240 | `51967830f17ab6ebad074fbaf178482c027910bc9d14a8ade070e17004b84b8a` |
| Trading | 489100942 | 1,325,848 | 9,229,106,160 | `7facb8e58e45843f46b9d3d572ced5e45507bfcbfb2250e865b5427baa1b9d3c` |
| Core | 489100672 | 934,088 | 6,502,456,560 | `e0cc7109da7a7b2b94cfa5a0f00a63c40ce44519f7d0186b6c1fbfe39b68f0ee` |

The generated template also pins each full ProgramData account-data digest.
Assembly refuses unless the fresh pre-write capture still matches every slot,
payload length/digest, ProgramData digest, retained authority, and lamport
balance. An unexpected prior Upgrade therefore stops as a state change rather
than becoming this plan's implicit baseline.

## Make a template

Use an absolute new output path:

```sh
python3 tools/release/devnet_upgrade_dryplan/dryplan.py template \
  --output /absolute/work/upgrade-dryplan-template.json

python3 tools/release/devnet_upgrade_dryplan/dryplan.py verify \
  --plan /absolute/work/upgrade-dryplan-template.json \
  --require-stage template
```

The template names every missing authoritative input. It does not pin an old
ELF as a future target.

## Assemble a captured plan

After a new checked release exists and the read-only captures in
`docs/guides/devnet-release-capture.md` have run, create an inputs document:

```json
{
  "schema": "dclutch-decision-0012-devnet-upgrade-inputs-v1",
  "checkedReleaseGate": {
    "canonicalPath": "/absolute/release/CHECKED_UPGRADE_GATE.json",
    "sha256": "64_lowercase_hex"
  },
  "permanentSubstrateCapture": {
    "canonicalPath": "/absolute/evidence/permanent-substrate-snapshot.json",
    "sha256": "64_lowercase_hex"
  },
  "baselines": [
    {
      "role": "custody",
      "canonicalPath": "/absolute/evidence/custody-baseline.json",
      "sha256": "64_lowercase_hex"
    }
  ],
  "feePayer": "PUBLIC_ADDRESS_ONLY",
  "buffers": [
    {"role": "custody", "publicKey": "NEW_PUBLIC_BUFFER_ADDRESS"}
  ]
}
```

Supply all five baseline and Buffer rows in this exact order: Custody,
Resolution, Claims, Trading, Core. A Buffer must be new, unique, and distinct
from the permanent set, retained authority, and fee payer. No keypair path or
key bytes belong in this document.

```sh
python3 tools/release/devnet_upgrade_dryplan/dryplan.py assemble \
  --inputs /absolute/work/upgrade-inputs.json \
  --output /absolute/work/upgrade-dryplan-captured.json

python3 tools/release/devnet_upgrade_dryplan/dryplan.py verify \
  --plan /absolute/work/upgrade-dryplan-captured.json \
  --require-stage captured
```

Assembly authenticates the exact gate digest and requires its thirteen-link
shape. It rehashes the five mutable role ELFs, each role's
`dclutch-sbf-link-provenance-v1` descriptor, and the descriptor's referenced
build/frame files. The checked gate's existing validator remains responsible
for complete thirteen-link admission. The assembler also authenticates the permanent-substrate
capture's domain-separated digest, exact Loader pairs, slots, authority,
widths, and lamport totals. Each baseline must rejoin that same pre-write
ProgramData digest and slot, the checked raw ELF width, and exact extension
rent arithmetic.

## Arithmetic and execution order

All amounts are integer lamports.

- Buffer data is `raw ELF bytes + 37`. Its rent is a transient debit and the
  Loader Upgrade refunds it to the exact payer spill account.
- Target ProgramData data is `max(current data bytes, max(raw ELF bytes,
  current live ELF bytes) + 45)`.
- If that target is wider, Extend is a separate transaction. Its durable rent
  increase is the fresh baseline's exact rent top-up. Recapture the baseline
  after Extend before creating the Buffer.
- An ordinary Upgrade's net debit is the authenticated Buffer upload fees plus
  the finalized Upgrade fee. ProgramData lamports do not move during Upgrade.
- The parked ProgramData balance after the iteration is the pre-write capture
  total plus exact extension top-ups. It is program rent, not a fee, reserve,
  bounty, treasury balance, or hoard principal.
- Peak transient exposure is per sequential role, not the sum of five Buffer
  rents. Exact Buffer rent and transaction fees arrive only from captures and
  finalized receipts, so the template deliberately does not guess them.

For each Upgrade role, finish any required Extend and new baseline, then run
preflight, create and write one persistent Buffer, submit the Loader Upgrade,
capture the finalized poststate, and publish the exact dump and receipt. Finish
all five in Custody-to-Core order. Then recapture Registry/Rent and the five
updated ProgramData bodies, prepare and publish the checked deployment set,
initialize the profile, and activate Core, Claims, Trading, Resolution, and
Custody one transaction at a time.

Before opening a market, rerun the founding frame and packet census over the
actual candidate. Preflight and run the current `DCLTGMF2`/`DCLTPCB2` route,
then run the bounded multiwallet activity harness, reconcile finalized
activity, update the site manifest, trigger the manual Pages workflow, and
perform cold-browser acceptance against finalized devnet.

The Activity-v3 funding authority in the dryplan is deliberately different
from the old scenario-only flagship fixture. It uses ten wallets: the deployer
campaign payer starts with 360,000,000 lamports; five distinct campaign signer
roles start at zero; and ash, birch, cobalt, and dahlia each receive 50,000,000
lamports after founding. The bounded authorization sets the four participant
transfers at 200,000,000 lamports, a distinct 10,000,000-lamport post-init fee
ceiling, a 210,000,000-lamport total-spend ceiling, and a separate
10,000,000-lamport activity-adapter fee ceiling. This guarantees at least
150,000,000 lamports remain in the deployer before lifecycle fees.

The corrected scenario has twenty-five ordered mutations: one founding, four
participant admissions, four Direct trades, one resolution, fourteen
exhaustive redeems, and one retirement. Every operation must have an accepted
caller and `mutationExpected: true`. The old
`tools/devnet-scenarios/fixtures/flagship.json` remains refused for Activity-v3:
its Direct, redeem, and retirement rows are nonmutating gaps, and its
150,000,000-lamport deployer bankroll cannot fund 200,000,000 lamports of
participant transfers before fees. A later canonical scenario and manifest
must bind the real Market, checked release, caller completion schemas, funding
closures, and both distinct fee ceilings; this dryplan does not silently edit
or reinterpret the old fixture.

The semantic owner of these amounts and the economic operation ensemble is
`tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json`, pinned by
the dryplan at SHA-256
`a018a4012ffe981482e67e24958cfefcc4f2ff296802ac02626b6d425e61aacd`.
The builder rehashes and strict-decodes that file every time it creates or
validates a plan. The dryplan's Activity section is an authenticated derived
summary, not a second funding fixture.

## Crash rules

The dryplan preserves the successor Upgrade journals' restart rules:

- each role uses one persistent Buffer identity and one exact writer lease;
- the unsigned message and signed packet are durable before the one send with
  `maxRetries=0`;
- a present signature is poll-only, including after its blockhash expires;
- a replacement packet is allowed only after finalized height passes the
  recorded limit, history says the exact signature is absent, and the whole
  prestate is unchanged;
- account movement without the exact finalized transaction never proves
  success;
- a Program is never recycled, ProgramData is never closed, and the retained
  authority never changes in this iteration.

## Focused tests

```sh
cd tools/release/devnet_upgrade_dryplan
python3 -m unittest -v test_dryplan.py
```

The hostiles cover mainnet substitution, permanent-address substitution,
release/provenance substitution, SBF diagnostic admission, capture authority
and lamport-total drift, baseline slot/digest/rent drift, duplicate Buffers,
and dryplan mutation or state-digest forgery.
