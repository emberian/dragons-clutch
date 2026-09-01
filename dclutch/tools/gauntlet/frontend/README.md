# Frontend witness

Checks that `apps/dclutch-web` tells the truth about a chain that exists.

A screenshot of the browser is not evidence — it checks a decoder against
itself. These programs decode the same finalized accounts a **second** time,
from byte offsets transcribed out of the first-party Rust, and grade what a real
Chromium actually painted against that second decode.

Nothing here imports anything from `apps/`.

## What each program is

| Program | Does |
|---|---|
| `resume-validator.sh` | brings a finished campaign's ledger back up as a live chain |
| `chain-witness.mjs` | the independent decoder: raw JSON-RPC, offsets cited to their Rust owner |
| `expect.mjs` | reads the chain and writes the expectation set (`--fixture-out` also dumps raw bytes) |
| `drive.mjs` | drives the real browser and harvests every rendered label/value pair |
| `compare.mjs` | grades rendered against chain; exits nonzero on any mismatch |
| `campaign-checked-release.sh` | builds checked-release evidence bound to a campaign's deployed artifacts |
| `loader-prediction.mjs` | constructed Loader V3 bytes vs the deployed accounts |
| `ungate.mjs` | exercises the activation un-gate: honest, tampered, and wrong-artifact |
| `pages-nav-check.mjs` | clicks every link in the assembled Pages artifact against a host-shaped server; also hard-loads a Market permalink and an unknown path |

Playwright is deliberately **not** a repository dependency — this is evidence
tooling, not shipped code. Pass `--playwright /abs/path/to/playwright/index.mjs`
or set `PLAYWRIGHT_MODULE`.

## The whole pass

```sh
# 1. Run a supported named family campaign in your OWN work root, or use its
#    completed work root. `run.sh --mode full` is unavailable at HEAD and
#    refuses before creating one. The campaign's README names its own runner.
RUN="<the completed family campaign run directory>"

# 2. resume the ledger on a port of your own.
tools/gauntlet/frontend/resume-validator.sh "$RUN/ledger" 21890 &

# 3. serve the app
( cd apps/dclutch-web && npm run dev -- --port 3111 )

# 4. decode the chain independently, drive the browser, grade the two
node tools/gauntlet/frontend/expect.mjs --endpoint http://127.0.0.1:21890/ \
  --run "$RUN" --out-dir /private/tmp/<yours>/witness \
  --fixture-out apps/dclutch-web/fixtures/live-open-market.json
node tools/gauntlet/frontend/drive.mjs --base-url http://127.0.0.1:3111 \
  --endpoint http://127.0.0.1:21890/ --core <core> --registry <registry> \
  --claims <claims> --market <market> --owner <founder> \
  --out-dir /private/tmp/<yours>/shots
node tools/gauntlet/frontend/compare.mjs \
  --expected /private/tmp/<yours>/witness/expected.json \
  --rendered /private/tmp/<yours>/shots/rendered.json \
  --out-dir /private/tmp/<yours>/witness

# 5. the release side
tools/gauntlet/frontend/campaign-checked-release.sh --run "$RUN"
tools/gauntlet/frontend/campaign-checked-release.sh --run "$RUN" \
  --work /private/tmp/<yours>/tampered --tamper-role custody
node tools/gauntlet/frontend/loader-prediction.mjs --run "$RUN" --endpoint http://127.0.0.1:21890/
node tools/gauntlet/frontend/ungate.mjs --base-url http://127.0.0.1:3111 \
  --endpoint http://127.0.0.1:21890/ --run "$RUN" \
  --tampered-work /private/tmp/<yours>/tampered --out-dir /private/tmp/<yours>/ungate
```

Programs, the Market and the founder come out of `$RUN/plan.json` and
`$RUN/evidence.json`; `expect.mjs` prints them all.

## Reading the results

`compare.mjs` exits zero only when every expected fact was rendered with the
same value. `loader-prediction.mjs` exits zero only when every constructed
Loader account is byte-identical to the deployed one — it is **expected** to
report Core's ProgramData as differing whenever the campaign revoked Core's
upgrade authority, and `docs/evidence/FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md`
explains why that is a semantic gap rather than a defect.

`ungate.mjs` asserts nothing. The gate is a refusal: a run reporting it stayed
closed is a result. What would be a failure is a gate that **opened** on
evidence the chain does not support, or a refusal that named nothing.

## What this produces

Local-validator evidence. Not devnet, not mainnet, not a deployment, and not
grounds for calling any address or frontend official.
