# Series pre-Market expiry — real-ELF ProgramTest, 2026-09-07

Evidence level: real SBF execution in an in-process ProgramTest bank. This is
not local-validator founding, packet-size/finalization evidence, devnet execution,
or evidence for the complete two-occurrence Series lifecycle.

## Source and result

The test host and Trading source were the clean committed revision
`548a2f1f624f900fd99eb9aeff362af2972a1dbd`, checked at
`hbox:/tank/dregg-build/dclutch-integrated-fa0198512` immediately before each run.
Companion programs were the controlled release-B artifacts built at
`56767e555d05ecd005edec4fc939d269e8757417`. The mixed artifact set is deliberate
component evidence; it is not a checked release of the newer source.

Both shipped and profiled Trading runs passed all seven named-target tests:

- `current_source_series_expire_lands_before_the_future_market_exists`: accepted
  expiry, all selected child calls and exact replay/resource poststates.
- `a_permit_prepaid_below_todays_minimum_still_expires_on_the_deployed_elf`:
  accepted under the permit's recorded funded-rent basis.
- `a_restated_config_identity_refuses_before_the_composition`: its accepted
  control and the hostile root-config substitution remain distinct.
- `precommit_caller_substitutions_refuse_with_exact_state_reversion`: all four
  hostile rows run and assert their exact Trading refusal and full rollback.
- Three native fixture controls: controller-manifest substitution, exact/disjoint
  root-independent substrate, and the single config-identity author.

The four caller refusals are `TradingSbfError::SeriesPrecommitCallerKey`,
`SeriesPrecommitCallerPrivileges`, `SeriesPrecommitCallerOwner`, and
`SeriesPrecommitCallerData`. Their discriminants are owned by the enum and
namespaced census; no earlier generic refusal or prior-CPI log substitutes for
those assertions. The earlier assertion that Custody must already have executed
before Core preflight was incorrect: preflight precedes every CPI.

For each accepted ordinary expiry control, Trading consumed 1,216,428 CU in the
shipped build and 1,251,928 CU in the profiled build. The enclosing Registry
continuation consumed 1,304,551 and 1,338,676 CU respectively, under the test's
1,399,700 available units after compute-budget instructions. These are measured
costs of these artifacts, not new protocol limits.

## Poststates and repairs

The future Market stays vacant. The transient projected Custody state and vault
close, their exact recorded rent returns to RentCredit, the permit closes, and
Trading commits the expected root and Ticket replay bytes. Ordinary collateral
refund ownership remains the Ticket's wallet owner; rent credit is a different
physical recipient. The comparison checks data, owner, lamports, executable bit
and rent epoch. Expected surviving modified accounts use Agave 4.3's bank-owned
`u64::MAX` rent-epoch normalization; no economic or byte comparison is omitted.

The integrated repair in `fa0198512` authenticates the semantic Template id from
the finalized config bytes, derives the Custody transfer bump for the future
Market, carries the physical rent-credit address, specializes projected abort to
its exact 768-byte wire and 11-account frame, verifies its typed receipt and rent
poststate, and downgrades Core's two replay observations without granting it
Trading's commit authority.

That combined shipped build initially emitted 58 frame-overwrite diagnostics in
`authenticate_and_execute_hot_v3`. It was rejected before execution. Extracting
selected-entry decoding into a non-inlined helper in `548a2f1f6` produced shipped
and profiled Trading ELFs with zero occurrences of either compiler frame-error
pattern. This does not discharge the later all-program frame baseline debt.

## Reproduction and artifacts

Run from the exact clean host revision, on hbox through `swarm-build`, with
`SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6`. Set `SBF_OUT_DIR` to the controlled
`release-b-test-elf` directory below and `DCLUTCH_TRADING_ELF_PATH` to one of the
explicit new Trading artifacts. The named target is intentionally serial:

```sh
cargo test --locked -p dclutch-trading-program-test \
  --test series_pre_market_expiry_program_test -- --nocapture --test-threads=1
```

All relative paths in this table are under
`hbox:/tank/dregg-build/dclutch-codex-evidence-20260907/`.

| Artifact | SHA-256 |
| --- | --- |
| `integrated-548a2f1f6/shipped/dclutch_trading_sbf.so` | `d334cc52c57d796f8d1beccff1d8a273688e626246008ede64b2e500064a1e38` |
| `integrated-548a2f1f6/profiled/dclutch_trading_sbf.so` | `60cce050098221081f6562c6ae5a3755b2db1be76268cd21ac16cd9ebec18021` |
| `integrated-548a2f1f6/series-target-shipped.log` | `30d929d975490b49a4003890ee4d27643fe514245a91e7929ec831aae8b50966` |
| `integrated-548a2f1f6/series-target-profile.log` | `15024ee651af88d1e34d4955e37c0fcada0e58b34ce6c873e2cf64fb09a52baf` |
| `integrated-548a2f1f6/trading-shipped-build.log` | `b2f9c87ccdbd0617deed8487b74d85b3b25500e3c9ad6893ceda22c021483412` |
| `integrated-548a2f1f6/trading-profiled-build.log` | `d6c31a0f24242a666d1f213c86c3e01f2a8fe24005bbdc4fd592093ee676ac11` |
| `release-b-test-elf/dclutch_core_sbf.so` | `7013924151b5fbd1f3d5abf0c88fb6199adaf2a6ba7f2a91da89014c4cf086a9` |
| `release-b-test-elf/dclutch_claims_sbf.so` | `19a3be2fcb625bf56166f8b638adba2e7d970421d1a08e9fdd1bc63d7ddaef5d` |
| `release-b-test-elf/dclutch_custody_sbf.so` | `178bb636165036ef061dc2268868c845fed47e3f9bdd1632d2c06161b242a5b7` |
| `release-b-test-elf/dclutch_registry_sbf.so` | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| `release-b-test-elf/dclutch_rent_sbf.so` | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| `release-b-test-elf/spl_token_2022.so` | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |

The native seam controls also passed: five Core precommit tests, two projected
cleanup tests and the three family's mined-bump controls. The Series bump
control was first observed red before the future-Market derivation repair.
Complete Found→Prepare→Consume/Expire→retirement execution on a local validator
remains assigned to the Series producer lane.
