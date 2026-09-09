# Series durable operation entrance — 2026-09-09

The production bootstrap now exposes paired
`local-private-validator-series-act-v1` and `devnet-series-act-v1` entrances.
Both consume a digest-bound source emitted from the native constructor,
reacquire finalized accounts through the current Series semantic owners,
bind the user's root/action/Template/occurrence/Ticket intent, and use the
same fsynced signed-packet journal. Public execution requires the existing
explicit devnet acknowledgment and actual genesis; local execution binds an
existing canonical ledger. Dry-run acquisition reads no signing key.

The Found/Prepare entrance emits its canonical source and executes Prepare
through this durable engine. It records exact signed-packet simulation,
resolved account-key count, packet bytes and reported compute units before
sending that same packet. Dispatching recovery polls landed history before
reacquiring mutable prestate and otherwise resends the identical bytes.
Submitted recovery polls only. No recovery path signs a replacement packet.
The native Hot heap declaration is included and authenticated at dispatch,
polling and already-landed recovery.

Prepare's canonical funding wallet may also pay the outer fee. The observed
account set is the union of native writable roles, Market evidence and payer;
it cannot invent a second physical account for that wallet. A writable
protocol role cannot masquerade as the payer, and root/Ticket/Market aliases
retain their exact refusals. The funding role comes from the native selected
report, not a caller-authored signer exception.

Validation ran through swarm-build in the borrowed, paired Claims V2 checkout
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909/source` and sibling
`target`, base host revision `da35e8e70e7fb662a73efe954342709a9c7e6eba`, with
only coordinated host patches and the owner's preserved test changes. No
binary from that older checkout was used against the fresh C native bank.

| Filter/control | Result |
| --- | --- |
| bootstrap and operator cargo check, locked | passed |
| portable intent rejects retargeting and changed consequence | passed |
| generated source round-trips native five-action child bank; malformed width refuses exactly | passed |
| paired-cluster guard refuses loopback in devnet entrance before file reads | passed |
| exact persisted v0 packet binds native heap; missing/changed heap refuses | passed |
| native Prepare wallet/payer account union and hostile PDA alias control | passed |
| old distinct-payer cardinality restored temporarily | red at exact prestate cardinality refusal |
| existing Retire durable phases and full Ticket rent-credit conservation | passed after restoring production source |

Logs include `series-production-controls.log`, the three named filter logs,
`series-prepare-funding-control.log`, `series-prepare-funding-red.log`,
`series-final-production-check.log`, and `series-final-retire-control.log` in
the borrowed checkout's parent. `series-payer-red-control.log` is under
`/tank/dregg-build` and records RED_EXIT=101, CHECK_EXIT=0, RETIRE_EXIT=0.

This commit provides the durable generic executor and typed Prepare producer.
The live selected-runtime Prepare transaction, subsequent-act source producers,
web controls, and complete two-occurrence execution remain outstanding.
The portable `series_intent_v1::SeriesOperationIntentV1` is available without
native Series feature dependencies for the existing Workbench integration;
it is an intent binding, never a substitute for native acquisition.
