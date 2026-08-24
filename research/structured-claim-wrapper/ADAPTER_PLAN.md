# Withdrawn structured-claim adapter plan

Status: **WITHDRAWN DESIGN HISTORY; NOT CURRENT IMPLEMENTATION AUTHORITY**
(2026-08-24).

This file formerly proposed a Structured adapter built around a public General
V2 action-35 Position transfer, Position V2, a parallel modeled Market ledger,
canonical actions 2 and 4, an eight-handler claim, and obsolete descriptor and
account-frame coordinates. Those execution and authority surfaces have been
physically removed. Retaining the old plan in prose made deleted code appear
current, so the plan is intentionally reduced to this tombstone.

The enduring product algebra and comparative research model remain in
[`README.md`](README.md). Current implementation boundaries live in:

- [`crates/clutch-structured-claim/README.md`](../../crates/clutch-structured-claim/README.md)
- [`crates/clutch-structured-claim-runtime-contract/README.md`](../../crates/clutch-structured-claim-runtime-contract/README.md)
- [`programs/structured-claim-adapter/README.md`](../../programs/structured-claim-adapter/README.md)
- [`programs/structured-claim-sbf/README.md`](../../programs/structured-claim-sbf/README.md)
- [`docs/implementation/CENTRAL_INTENT_REGISTRY.md`](../../docs/implementation/CENTRAL_INTENT_REGISTRY.md)

The current family has exact account contracts only for actions 1, 3, 5, 6,
7, and 8. The former action-2/4 current action and payload variants are
deleted; their registry coordinates do not decode through the current wire or
Replay V3. The unified successor development profile admits only those six
actions through exact wrapper/base/Token-2022 release manifests.

This tombstone is not evidence that the successor has been built, measured,
deployed, or validated.
