# Decision-0012 devnet Upgrade and open-market dryplan

Date: 2026-08-29

This is a key-free execution map, not an acceptance, deployment, or claim that
the next release is ready. No current candidate ELF is pinned here. The next
payload authority must be a new `dclutch-checked-upgrade-gate-v1` emitted by
`tools/release/checked-release-candidate.sh` after the complete thirteen-link
build and frame pass.

## Fixed substrate

The seven permanent Program and ProgramData identities exactly match
`DEPLOY_1.md`, `DEVNET_ITERATION_2.md`, and the operational table in
`successor/src/upgrade.rs`. Registry and Rent carry forward. Custody,
Resolution, Claims, Trading, and Core Upgrade in that order. The retained
authority stays
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`.

The DEPLOY-1 ProgramData accounts hold 31,772,309,520 lamports. That historical
number is an orientation fact only. A fresh one-context finalized capture must
replace it as the arithmetic prestate immediately before the first write.

There is no contradiction among the deployment record, decision 0012, and the
current executor identity table. The old slots in the deployment record are
expected to advance for the five Upgrade roles. Registry and Rent must retain
their exact slots and full Loader state through the CarryForward closure.

The static template pins all seven old deployment slots, live ELF byte lengths
and SHA-256 digests, ProgramData account-data digests, and exact ProgramData
lamports from the two deployment records. A fresh capture that differs is an
unexpected pre-write state and refuses; this plan cannot adopt an unexplained
prior Upgrade as its baseline.

## Inputs that must be fresh

1. A checked release gate and its separately recorded SHA-256. The five
   mutable role links must bind their exact ELF and
   `dclutch-sbf-link-provenance-v1` descriptor.
2. One finalized `devnet-permanent-substrate-capture-v1` result over all
   fourteen Loader accounts and the exclusive fee payer.
3. One key-free baseline per Upgrade role, using the checked raw ELF length.
4. Five new public persistent-Buffer identities. Private key material and
   keypair contents remain outside this plan.
5. A deployment-set journal carrying the two-role CarryForward pin and the
   five role baselines, receipts, and dumps.

The offline assembler at `tools/release/devnet_upgrade_dryplan/dryplan.py`
joins and rejects substitutions across items 1 through 4. It never authorizes
a mutation. The successor capture, set-audit, preflight, Extend, and Upgrade
commands remain the semantic and operational owners.

## Ordered mutation and recovery

Run Custody, Resolution, Claims, Trading, and Core sequentially. For a role
whose checked ELF is wider than its allocation, run checked Extend, prove its
exact ProgramData rent top-up and fee, and recapture the role baseline. Then
create and authenticate one persistent Buffer, submit one exact Loader Upgrade,
and prove the advanced slot, unchanged authority/link, exact live payload,
unchanged post-Extend ProgramData lamports, exact Buffer refund, and exact
finalized fee.

Each ambiguous boundary resumes from its durable packet and signature. A
present signature is poll-only. Only an expired, history-null signature with
the entire prestate unchanged admits a replacement packet. An unattributed
account change refuses. No permanent program is recycled or closed.

The exact durable rent after all five roles is:

```text
fresh pre-write ProgramData lamports + sum(five exact extension top-ups)
```

Buffer rent is sequential transient exposure and returns at Upgrade. Exact
fees come from authenticated Buffer history and Loader receipts; they are not
estimated here. Hoard principal is not part of the envelope.

## Release, market, activity, and site suffix

After the five Upgrade receipts are complete:

1. Reobserve the complete seven-role set and capture the unchanged
   Registry/Rent closure plus all five updated ProgramData bodies.
2. Prepare and publish the checked deployment set and initialize its release
   profile.
3. Activate Core, Claims, Trading, Resolution, and Custody in that exact order,
   one transaction per role.
4. Run the actual candidate's founding frame diagnostic and complete packet
   census, then preflight `DCLTGMF2` and `DCLTPCB2`.
5. Create, found, and open the first public devnet market.
6. Run bounded multiwallet activity, reconcile every finalized transaction and
   wallet ledger, and preserve the exact activity closure.
7. Update the public site with the real Market and activity artifacts, trigger
   the manual GitHub Pages workflow, and perform cold-browser acceptance.

The frontend and activity service must read authenticated onchain state and
finalized receipts. They are untrusted projections and do not replace the
release, Market, or transaction authorities.

### Corrected Activity-v3 funding authority

The semantic owner is
`tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json`, exact
SHA-256
`a018a4012ffe981482e67e24958cfefcc4f2ff296802ac02626b6d425e61aacd`.
The offline dryplan rehashes and strict-decodes that fixture and emits only a
derived join. It does not maintain another editable funding truth.

The old flagship fixture remains scenario-only and is refused as an
Activity-v3 input. Its Direct, redeem, and retirement operations do not claim
executable mutations, and its four 50,000,000-lamport participant transfers
exceed its 150,000,000-lamport deployer bankroll before fees.

The replacement devnet-only manifest must bind this exact ten-wallet plan:

- deployer is the campaign payer and receives 360,000,000 lamports initially;
- collateral Mint, collateral wallet, founding beneficiary, founding
  projection witness, and founding source funder are five distinct fresh
  campaign wallets with zero prefunding;
- ash, birch, cobalt, and dahlia each receive exactly 50,000,000 lamports after
  the founding adapter finalizes;
- founding founder and substituted founder remain two distinct literal public
  keys, not disposable wallets;
- the authorization caps participant transfers at 200,000,000 lamports,
  post-init funding fees at 10,000,000 lamports, total spend at 210,000,000
  lamports, and activity-adapter fees at a separate 10,000,000 lamports;
- this leaves at least 150,000,000 lamports in the deployer before lifecycle
  fees, and reconciliation records actual debits under both separate ceilings;
- the 50-basis-point-per-side fee uses denominator 10,000 and the scenario's
  named integer rounding boundary.

The lifecycle is one founding, four participant admissions, four Direct
trades, one resolution, fourteen exhaustive redeems, and one retirement in
canonical predecessor order. All twenty-five rows must be
`mutationExpected: true` and name accepted caller schemas. No manifest may
convert an adapter gap into evidence. The canonical scenario envelope,
Activity-v3 manifest, initial funding closure, post-init funding plan digest,
live authorization, and final reconciliation are still required before the
activity service can send.
