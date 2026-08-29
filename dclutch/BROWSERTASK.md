# Public devnet browser task

Use this task after the final devnet market has opened, the activity campaign
has produced public transactions, and the latest web build has been deployed to
GitHub Pages. The public entry point is:

<https://clutch.dregg.pro>

The purpose is to prove that a new visitor can understand and inspect the live
devnet system without relying on cached state or private operator artifacts.

## Before you start

1. Confirm that browser control is attached to this session. If it is not,
   report that plainly and stop this task; do not substitute unrelated browser
   automation.
2. Confirm that the final Pages workflow succeeded after the market and
   activity manifest was committed.
3. Obtain the final public manifest path or URL from the release handoff. It
   must name the permanent seven program IDs, the open Market address, its
   public lifecycle records, and the activity transaction signatures.
4. Start with a fresh browser tab that has not visited the site in this
   session. Do not inspect cookies, local storage, browser profiles, passwords,
   or wallet secrets.
5. Treat the site and its indexes as untrusted projections of onchain state.
   Cross-check important addresses and lifecycle claims against finalized
   devnet RPC reads or Solana Explorer.

Browser verification is read-only by default. Do not connect a wallet, sign a
message, submit a transaction, or spend devnet SOL unless the user gives a
current authorization naming that action. If an interactive wallet check is
authorized, use a disposable devnet wallet, never the deployer or upgrade
authority.

## Cold-visitor route

1. Open <https://clutch.dregg.pro> and confirm HTTPS succeeds with no warning,
   redirect loop, blank shell, or visible loading failure.
2. Read the landing page as a first-time visitor. It should explain in plain
   language what Dragon's Clutch does, say that this is devnet, and provide one
   obvious path into the live market.
3. Open the live market. Confirm its Realm, collateral, outcomes, fee display
   (0.50% per side), lifecycle phase, oracle source, and market address are
   visible and internally consistent.
4. Follow every program, account, market, and transaction link shown in the
   interface. Explorer links must select `cluster=devnet`, resolve to the
   displayed address or signature, and show real public activity rather than a
   fixture or replay transcript.
5. Open the activity view. Confirm it shows the campaign wallets' real devnet
   transactions in lifecycle order and does not silently fall back to operator
   or simulated data.
6. Inspect the available participant flows: admission/funding, Direct trade,
   resolution, redemption, and retirement. Disabled actions must explain the
   actual lifecycle reason in reader language. Enabled actions must expose the
   required inputs before any signing request.
7. Reload the market route directly, then navigate away and back. Public state
   must be rediscovered rather than depending on navigation history.
8. Check the page at a desktop width and a narrow mobile width. Important
   addresses, amounts, action buttons, tables, and error messages must remain
   readable without horizontal page overflow.
9. Inspect the browser console and failed network requests after the landing,
   market, activity, and wallet-flow pages have loaded. Record every error or
   unexplained request failure; do not dismiss errors because the page looks
   usable.

## Onchain cross-check

For each displayed permanent program, confirm the public devnet account is
executable and that its ProgramData address and deployment slot match the final
release manifest. Preserve the seven permanent Program IDs; a different ID is
not an acceptable substitute.

For the featured Market, cross-check at least:

- Market address, Realm, collateral mint, outcome count, and lifecycle phase.
- Resolution source and the terminal outcome or payout vector.
- The displayed fee rate and integer units.
- One successful participant/admission transaction.
- One successful Direct trade transaction.
- Resolution/admission-to-terminal transactions.
- One replay-safe redemption and one payout transaction.
- The final public-ledger reconciliation totals.

Do not infer success from the site's own copy. A claim passes only when the
linked finalized devnet account or transaction supports it.

## Capture and report

Capture screenshots of:

1. The landing page with its live-devnet call to action.
2. The featured Market header and outcome/fee state.
3. The activity timeline with at least one Explorer-visible transaction.
4. The Direct trade flow before signing.
5. The resolution and redemption state.
6. The narrow/mobile layout.

Report the deployed Pages workflow run, tested URL, final manifest revision,
Market address, checked transaction signatures, browser/viewport, console and
network findings, and a pass/fail result for every route above. File concrete
bugs with the URL, visible symptom, expected behavior, and screenshot.

## Completion condition

This task is complete only when the public site is reachable cold, shows the
same real devnet market and activity that finalized RPC reads show, links to
working devnet Explorer records, keeps unsupported actions honest, has no
unexplained console or network failures, and remains usable at desktop and
mobile widths.

As of 2026-08-29, the CLI session that created this file had no attached
browser, and the final market/activity Pages cut was still in progress. Do not
mistake the existing pre-launch microsite for completion of this task.
