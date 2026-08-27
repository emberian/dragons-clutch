# The first browser-built transaction executes against a live chain — 2026-08-27

The FE-TRADER lane's proof: `apps/dclutch-web` built a state-mutating dClutch
transaction in a real Chromium, had a Wallet Standard wallet sign it, submitted
it through `lib/rpc.ts`'s one `sendTransaction` seam, and confirmed it against
the chain — the first time any browser-built transaction has executed against
any dClutch chain.

**Evidence level.** Local-validator execution. Not devnet, not mainnet, not an
official deployment or frontend. The signing key is the journey campaign's
seeded founder — a TEST-ONLY determinism switch the producer refuses off
loopback (`tools/local-validator/bootstrap/successor/src/seed.rs`). Nothing was
published or funded anywhere but a fresh localhost ledger.

## The transaction

The redeem flow's opening move: **Claims-role Custody replay creation**
(`DCLCCR01`, ADR-0008 §7) for a Market that had been founded, traded around,
resolved by real Pyth (Wormhole-verified VAA through the captured receiver
ELF), and moved to Retiring — all by the journey campaign
(`tools/gauntlet/journey/run-journey.sh`) at commit `7e33ecfbe5ec`, on per-run
ports beside two other lanes' validators.

ADR-0008 §7.3 obliges every redemption builder — "including the browser's" —
to open with this instruction when the Claims-role replay is absent. The
browser's builder (`lib/claimsCustodyReplay.ts`) mirrors the route's own
`expected_request_v1` byte for byte from a new generated ABI module
(`lib/generated/claimsCustodyReplayV1.ts`, `npm run abi:claims-replay`,
verifier wired into `npm test`): the Claims caller-authority PDA is seeded by
the SHA-256 of the derived 672-byte Custody request, so one differently-derived
field would derive a different authority and refuse on chain.

## The loop the browser executed

Driven by `tools/gauntlet/frontend/drive-execution.mjs` (real headless
Chromium; the wallet is a minimal Wallet Standard wallet injected by the
harness — the page sees only the standard interface, so every byte crosses the
same seam a Talisman user's bytes would cross). Per-step screenshots and the
machine transcript: see the recorded-run table below.

1. **Connect** — the wallet registers through the Wallet Standard registry;
   the page's own discovery validates and lists it; connecting fills the owner.
2. **Read** — `/portfolio` derives the Claims aggregate and Position, decodes
   the founder's balances at one finalized floor, and shows the winning claim
   with its exact redeemable atoms on the resolved Market.
3. **Plan** — the redeem flow derives the complete replay-creation plan from
   the aggregate's own persisted namespace: replay PDA, caller authority,
   activation cache, realm record pair, exact rent, one legacy packet.
4. **Execute** — the wallet signs; the page submits through
   `SolanaRpcClient.sendRawTransaction`; confirmation is the POSTCONDITION
   (the replay decoding at its derived address at finalized commitment),
   polled beside the signature status.
5. **Verify independently** — the harness re-reads the replay over raw
   JSON-RPC with offsets cited to `dclutch-custody-contract/src/generated.rs`
   (no `apps/` import): magic `DCLCUSS1`, caller role 1 (Claims), the Market,
   `next_revision` 1, and `rent_refund` equal to the wallet that signed.
6. **Watch resolution** — `/markets/:address` renders the Retiring phase, the
   terminal receipt, the winning claim, and — for the first time — a DERIVED
   Hoard figure, because the aggregate's `custody_context` now tells the truth
   (ADR-0008 §7.6's re-founding condition is met by any post-fix founding).
7. **The honest trading verdict** — the new trade panel asked the chain about
   trading this Market and rendered the chain's own answer: this Market's
   manifest lists three entries and none is the Direct successor kind, so
   Direct trading was never part of its founding. A named reason, not a
   greyed-out button.

## The recorded run

| Coordinate | Value |
|---|---|
| Journey run | `/private/tmp/dclutch-fetrader-journey2/runs/20260827T214302Z-7e33ecfbe5ec-h2` (seed phrase `dclutch/fe-trader/evidence-seed/v3`), resumed on `127.0.0.1:23890` |
| Market | `BxPN7kzQEjtiaVgciwq4nKjcWzYKdkg9q63eYuvgdgKT` · Retiring · winner claim 2 · terminal receipt written |
| Wallet (signer, payer, rent refund) | `5HLEhFSRXiuMMqYsMbLcCPajaDH19Q5PTCgZ1qHvcgp5` |
| Redeemable shown | 500,000,000 atoms of the winning claim |
| Signature | `3Uy2Z9nCLfiXwgWC1BXpN1DfTDm2DjavaQYi7M8QqaVvmYuaSYKkW3JUf1aKwPQojNdE8yptohamuKkzbYn3scFW` |
| Created replay | `42oipYoza83GVhJH89wWYWyuqEqt7uFMVEs6YbbeBDvT` · `DCLCUSS1` · caller role 1 (Claims) · next revision 1 |
| Measured | slot 5648 · fee 5,000 lamports · 145,138 CU (inside the Custody-CPI family band; the browser sets an explicit 500k ceiling) |
| Owner lamport delta | −2,900,360 (the 2,895,360 replay rent, refundable, plus the fee) — rendered by `/activity` from the node's own history |

Machine transcript (per-step facts, written incrementally):
`docs/evidence/first-browser-execution-2026-08-27/transcript.json`.
Screenshots: `/private/tmp/dclutch-fetrader-evidence3/*.png` (five steps,
full-page; not committed — the transcript and the chain state are the record).

Two earlier passes of the same drive executed the same instruction against two
earlier journey ledgers (seed phrases `journey/campaign-seed/v1` and
`fe-trader/evidence-seed/v1`); both chains hold their created Claims-role
replays with `rent_refund` bound to the signing wallet. The recorded pass is
the third, end to end in one browser session.

## What this does and does not prove

- **Proves**: the browser's builders produce bytes a live chain accepts; the
  Wallet Standard integration signs real transactions end to end; the one RPC
  submission seam works; the redeem flow's step one is a product, not a form.
- **Does not prove**: a browser trade (three named protocol walls stand —
  wallet-side Position admission, geometry-parametric Direct artifacts, and
  the 1,268 > 1,232 packet decision — all rendered as named reasons in the
  product), or the payout leg (`claims/terminal_settlement_v3` admits caller
  role Core or Trading only; ADR-0008 §7.6). Those are protocol gaps this
  frontend states; it cannot route around them.

## Controls

- Web suite green for every FE-TRADER surface; `eslint` clean; `npm run build`
  completes; `abi:claims-replay:verify` wired into `npm test` alongside its
  siblings. (Three failing test files in the same tree belong to another
  lane's in-flight explorer/SBOM work and are boarded, not mine to green.)
- The drive harness imports nothing from `apps/`; its independent decode cites
  Rust offsets, not the browser's.
- Validators ran on per-run ports (23890 block); the shared 20890 slot and
  other lanes' validators were never touched.
