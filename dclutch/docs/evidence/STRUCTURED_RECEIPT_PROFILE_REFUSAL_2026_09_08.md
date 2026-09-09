# Structured receipt profile refusal — 2026-09-08

The checked `bf06c8d752eafa572a433b49cb0db38fe413fbbd` runtime accepted
Structured founding, capability-root activation, and the receipt capability
seal. The subsequent receipt Hot transaction refused with
`TradingSbfError::Content` before invoking Claims. This is not an accepted
receipt or a completed Structured lifecycle.

The retained run is available through
`hbox:/home/hbox/dclutch-structured-completion-bf06c8d75-20260908`, now a
compatibility symlink into `/tank/dregg-build/archive-home-20260909/`. Its
validator was stopped gracefully before archival. Future runs allocate under
`/tank`.

## Located cause

The actual transaction consumed 208,789 CU in total; Trading consumed 208,181.
The captured finalized account frame is from slot 27,503. Its Profile13
declared logical coordinate 14, the representation descriptor, as
`AdapterAuthenticatedVariableData`. The native Hot adapter authenticates
variable bodies only at shared-prefix coordinates 1 and 4. Claims owns the
descriptor body's Registry finality, digest, schema, and physical-address
join. The coordinate-activation profile made the same erroneous declaration
for its child-owned Domain at coordinate 35.

An instrumented real-ELF replay stopped at `account-projection` with the exact
VM error `InvalidVariableDataPrestate`. Its width diagnostic reported no
fixed-rule disagreement. The producer correction uses
`AuthenticatedOpaqueReadonlyData` for these child-owned bodies, preserving
Claims' native authentication.

The initial offline projector had itself trusted every variable-data marker
declared by the profile. It consequently reported a false success. The
corrected projector mirrors the adapter's independently authenticated
shared-prefix boundary. Against the unchanged capture it reports:

| Profile | Exact projection result |
| --- | --- |
| Captured `df1b87e41b1177fce99a03b19ace68eb67ca5ce8070944ed4db815ae3b940426` | `Err(InvalidVariableDataPrestate)` |
| Opaque-record control `be55f1e1f39ea4153037876289c8cec49f7e88498ebe9fa29b2327900f2d71c9` | `Ok(())` |

Both rows have zero fixed-width mismatches. This control changes only the
profile's child-record prestate declarations; it does not establish later
Hot execution or use the subsequent V2 root geometry.

Source inspection also located a later identity-domain defect: Profile13
projected coordinate 14's physical Registry PDA and compared it with the
request's descriptor content digest. The effect then used that projected
identity in the Claims child. The existing Claims adapter already joins the
request digest to the finalized descriptor and its Registry address. The
artifact convergence at `8e1e2392e43230714b438c22b9222ede36f7f4c3`
removes the redundant physical-address comparison and
preserves that native semantic owner. Fresh immutable publication and actual
execution remain necessary after this repair.

## Replay boundary and artifacts

The ProgramTest replay required explicit harness normalization: Core and
Claims ProgramData deployment slots and their activation-cache counterparts
were aligned to Trading's slot 6; the captured frozen ALT's last-extension
slot was changed to zero so the replay bank could use its existing addresses.
The original capture was unchanged. With those harness adjustments, the
uninstrumented checked ELF reproduced the actual 208,789 CU total and the
same refusal. The instrumented Trading ELF measured 222,510 CU in Trading;
that instrumented number is diagnostic overhead, not the accepted runtime's
cost.

Artifacts under
`hbox:/tank/dregg-build/structured-host-c7e74e543-20260908/`:

| Artifact | SHA-256 |
| --- | --- |
| `receipt-frame-bf06.json` | `bb87416864619a694be46e1fb3ec78f20c9f2615c763dda08eae71feaf057378` |
| `receipt-opaque-profile-redgreen.json` | `06503086779e2bede6685b670224498023de26bc9161443150d0a6fa33a36506` |
| `build-opaque-projection.log` | `3142699c90a0915dbb049c09cff7181157314e9a176d00aa366864241d2b2a07` |

Artifacts under
`hbox:/tank/dregg-build/structured-native-bf06c8d75-20260908/`:

| Artifact | SHA-256 |
| --- | --- |
| `receipt-bf06-replay-normalized2.log` | `85e66366ca38fbc1e386d93cb6bb445522f91cb40f879abb79b2148c8d022100` |
| `receipt-bf06-replay-profile.log` | `3c90d672f6afc02b50a7bd3aa793c45709b22d2533b6891e79a67cbe9a689e83` |
| `receipt-replay-normalization.json` | `45cc6e7a5dce35422dece2924046f5ce934f625d952d04b8734bcdfb1da7c28e` |
| `receipt-replay-arguments.json` | `98ea58b1bb27ba2cb3cec79d1957aac5429c416c020ef284efb4039f38a18b48` |
| `trading-profile-elf/dclutch_trading_sbf.so` | `429d925816107d7177ad8cdb233a85f9f6e4e853b136a2236dbc784715ff0a19` |

The projection-control host checkout is an explicitly isolated diagnostic
overlay over the prior `c7e74e543` host archive, with its recorded committed
host fixes. The control's `structured_campaign.rs` hashes to
`8b8e92a15e99cd66034ec946d8da64fbc0ea62fd94627276d300e88c396c4ff9`;
its `selected_profile_v5.rs` hashes to
`154548c3379be2c52ad28d8fa6a3a3fec1326428fa2242f904b025db11738c43`.
Its own-target release build completed successfully through `swarm-build`.
The complete run's host/runtime source boundary remains in the retained
`HOST_SOURCE_BOUNDARY.json`; these overlays are not described as the checked
runtime source.
