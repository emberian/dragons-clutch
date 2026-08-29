# Activity campaign staging — post-Open public path

Status: **preflight only.** This guide neither authorizes nor performs a
devnet mutation. It applies after cohort2 exists and the named Market is Open.
The campaign wrapper refuses its former `--trades` input because the terminal
client's `buy` and `sell` verbs are intentionally disabled; it must not pretend
that a disabled client can create participation.

The real caller sequence is progressive and each command owns its own durable
journal. A failed or ambiguous invocation is resumed by rerunning the same
caller with the same explicit paths, never by sending a replacement packet.

1. Inputs that must be present before public preflight:

   - exact HTTPS devnet RPC URL and
     `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` acknowledgement;
   - checked release `PLAN`, finalized founding campaign evidence, and the
     Open Market input, all absolute immutable paths;
   - one freshly generated cohort2 wallet public key and its explicit keypair
     path; an explicit fee-payer public key and keypair path; no default
     wallet is accepted;
   - an output path for the position-admission execution report, a minimum
     finalized slot, and, if principal is to be contributed, the explicit
     collateral owner/keypair/account and raw atom quantity;
   - after admission finalizes, the checked execution release, seller ticket,
     buyer ticket, and the exact private session produced from those inputs.

2. Preflight participant admission with the current public caller (omit
   `--execute`). Its output is the fsynced report named by `--output`; it
   contains the exact admission packet/rent/fee plan and must be finalized
   before it can become a Direct-session input.

   ```sh
   BOOT=/absolute/path/dclutch-local-successor-bootstrap
   "$BOOT" devnet-user-position-admission-v1 \
     --rpc-url "$RPC" --i-mean-devnet "$GENESIS" \
     --plan "$PLAN" --campaign-evidence "$FOUNDING_EVIDENCE" \
     --position-owner "$COHORT2_OWNER" --position-owner-keypair "$COHORT2_KEY" \
     --fee-payer "$FEE_PAYER" --fee-payer-keypair "$FEE_PAYER_KEY" \
     --minimum-finalized-slot "$OPEN_SLOT" --output "$ADMISSION_REPORT"
   ```

3. Only after the admission report is finalized, produce the checked Direct
   session with `tools/devnet-activity/prepare_direct_sessions_v1.py`. Its
   output directory contains `direct-trade-producer.json` at `finalized`,
   `direct-trade-public.json`, and `direct-trade-session.json`. The producer
   is key-free; the private session is the only accepted input to the next
   caller.

4. Preflight the actual public participation caller (again without
   `--execute`):

   ```sh
   "$BOOT" devnet-direct-trade-v1 \
     --rpc-url "$RPC" --i-mean-devnet "$GENESIS" \
     --session "$DIRECT_SESSION"
   ```

   Its output names the next durable ALT, seal, or Hot action. With later
   explicit authorization, repeat the *unchanged* command with `--execute`.
   It advances one action, writes its own submitted-before-send journal, and
   must be rerun until its caller-owned finalized completion says so.

5. Required public outputs for Explorer/reconciliation:

   - every finalized signature from the admission report and Direct caller
     journal, opened against the same devnet RPC in Explorer;
   - the cohort wallet and its chain-derived Position/admission account;
   - the finalized Direct completion report, exact fee and collateral
     pre/poststate, and `dclutch portfolio <cohort-owner>` output.

The terminal payout wrapper begins only after the Market has an actual terminal
state and the portfolio reports a redeemable position. It accepts explicit
wallet payout inputs or completed campaign plan/evidence; it does not derive a
terminal result from this staging checklist.
