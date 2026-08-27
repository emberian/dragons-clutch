# The Solana programs

(Rewritten 2026-08-22; the original 08-18 text predated the program and
claimed none existed. `CURRENT_TRUTH.md` supersedes any status claim here.)

- `clutch-sbf/` — **the deployable SBF program**: the only `entrypoint!`,
  the only workspace permitted Anza SDK dependencies. Its own tree holds
  the real-bank test suites (`svm-tests/`, separate workspace and
  toolchain pin), the fixture/transaction `harness/` (a library + thin
  CLI; signs nothing), the loopback `operatord/` daemon behind
  `apps/operator`, the permissionless `keeper/` cranker, the
  committed-walk runner (`committed-harness/`), and the artifact audit
  under `audit/`. Sealed identities live in
  `research/liveness-policy-profile/artifacts/`.
- `solana-layout/` — the dependency-free `no_std` byte codec the program
  decodes through: every account format, the `Intent` wire (tags 1–73),
  canonical padding, domain-separated identities. Live production
  surface, not a prototype.
- `solana-reference/` — the host-side differential oracle: a pure
  adapter over the same codec used by host tests to predict exact
  program behavior byte-for-byte. Never deployed; deliberately refuses
  intents whose semantics only the SBF plane carries.

Nothing here is a deployment, release, or audit claim; see the claim
vocabulary in `CURRENT_TRUTH.md` §1.
