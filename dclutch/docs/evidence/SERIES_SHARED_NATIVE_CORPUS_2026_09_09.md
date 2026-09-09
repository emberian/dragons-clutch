# Shared native Series corpus acquisition — 2026-09-09

The operator now owns `series_operation_corpus_v1`, extracted from the production
Series CLI. Its single aggregate accepts a typed Market/root/payer intent,
cluster genesis, one finalized account corpus and optional opaque native source
bytes. It returns `NeedSource`, the complete native-derived `NeedAccounts`
vector, native lifecycle acquisition requirements, a schedule wait, or the
native unsigned report bound to the accepted intent. The source schema and
native request/role decoding have one owner; the CLI imports that owner.

`NeedSource` is an explicit outstanding acquisition requirement. This change
does not reconstruct a missing authored source bank from root alone and does
not turn absent source/proof material into a ready operation. The browser may
transport or persist the opaque producer artifact; it does not author its
semantic request bodies or role frame. Subsequent-occurrence authoring and
selected-runtime two-occurrence execution remain outstanding.

An RPC result of `None` is retained as observed native vacancy only for a key
in the requested corpus. A key omitted from the acquisition map is refused.
The zero System key requires native-loader ownership and executable state;
it is never substituted for an absent System observation. Prepare/Consume/
Expire geometry share the same source-address predicate. The native role
validator continues to decide whether a particular role admits vacancy.

Wallet transport receives the authenticated frozen lookup key, ordered
addresses and exact account-data digest plus the native hot heap request.
The CLI's former lookup authentication body delegates to the same operator
function; mutable, deactivated, unactivated or mismatched routing is refused.

The CLI pages its inherited bounded acquisition vector using the canonical
100-address RPC page size and shared parallel read-only transport. Pages are
accepted only at one exact finalized slot, retaining order and explicit
absence. Eight attempts are a provisional transport retry budget, not a
protocol or schedule bound. Exhaustion is a named refusal before journaling;
a caller can repeat acquisition. No page context is relabeled to another slot.

Validation used the paired hbox checkout and target beneath
`/tank/dregg-build/dclutch-claims-sparse-a8e3b4e8c-v2-20260909`, whose checkout
HEAD was `da35e8e70e7fb662a73efe954342709a9c7e6eba`, with the exact scoped host
and operator extraction copied into that checkout. Its original Claims test
patches were preserved. These are host checks and filtered fixture controls,
not execution against the fresh V2 diagnostic bank.

- Locked operator and bootstrap checks passed (`series-final-corpus-check.log`).
- All three final native corpus controls passed, including authenticated lookup
  address order and exact mutable-table refusal (`series-final-corpus-native-control.log`).
  Final logs show the operator compiled and both operator/bootstrap were checked;
  changed files were copied without preserving old source mtimes.
- Native corpus controls passed. Restoring the old present-only converter made
  the vacancy control fail at `future native PDA was absent`; restoring the
  fix passed (`series-corpus-vacancy-red.log`, `series-corpus-vacancy-green.log`).
- Existing native child-bank source round trip, Prepare funding-wallet payer,
  complete Retire durable phases, and shared Prepare System controls passed
  (`series-corpus-<filter>.log`; driver `/tank/dregg-build/series-corpus-controls.log`).
- The exact finalized-page ordering/absence and unequal-slot refusal control
  passed (`series-final-corpus-page-control.log`). Its first attempt did not
  run: the test fixture lacked `RpcAccount::rent_epoch`; the corrected fixture
  compiled and executed successfully.

No native wire, economics, authority, schedule or SBF selection changed. The
fresh C V2 geometry remains diagnostic-only at the paths recorded in
`SERIES_CURRENT_V2_GEOMETRY_CAPTURE_2026_09_09.md`; no V1 geometry was reused and
no retained validator was restarted.
