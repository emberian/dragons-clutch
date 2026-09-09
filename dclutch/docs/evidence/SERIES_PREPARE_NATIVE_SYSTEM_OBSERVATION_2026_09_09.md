# Series Prepare native System observation — 2026-09-09

The fresh local-validator V2 canary used native commit
`a3dd6962f6fd366e90e512ffbf15c21d89cd6541`, checked gate SHA-256
`051aad59506c18b7140e417cea3818a0ff835ec47aae0fcec69a68b4761bc638`,
and retained bank `/tank/dregg-build/series-a3dd-v2-runtime-20260909`
(RPC 31100). It authored its Template at finalized slot 5678, published its
schedule by 6158, and prepaid Claims by 6190. No parent Found or Prepare
transaction was submitted: the host refused the canonical System program's
zero address as `Series Prepare role named the default address`.

Prepare now delegates source-address validation to the same canonical
System/NativeLoader predicate used by Consume and Expire. Only finalized
System with its expected NativeLoader owner receives the zero-address
exception; an incorrect owner or a default future PDA still refuses.

An owned exact-C checkout at
`/tank/dregg-build/series-a3dd-v2-host-20260909/source` carries this host-only
change as `bc2691253db08563d2f343fd9a12c189dc331a32`, paired solely with its
sibling `target`. Cargo check, the filtered
`prepare_system_source_uses_shared_native_owner_predicate` control (one
passed), and the release bootstrap build passed. Logs are `check.log`,
`system-test.log`, `build.log`, and `driver.log` in that owned directory.
The sealed all-eight candidate was not modified.

The retained bank is continuing through the canonical diagnostic publisher.
Original `geometry.stdout`, `geometry.stderr`, and `geometry.status` remain
intact. This evidence establishes the host predicate correction, not native
Prepare execution, selected Series geometry, or a completed recurrence.
