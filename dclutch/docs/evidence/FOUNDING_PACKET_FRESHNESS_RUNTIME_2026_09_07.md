# Founding-packet freshness runtime record — 2026-09-07

This dated record preserves one failed packet and one fresh, terminal local
campaign. It does not identify a deterministic source-code latency defect:
the preserved failure has only its final `Dispatching` row and no phase timing
markers. The new markers bound the successful path. Consequently the measured
conclusion is *transient host/runtime contention or scheduling in the failed
run*, not a proven attribution to fee RPC, account rereads, hash generation,
or filesystem sync.

No journal transition, validity rule, or recovery permission was weakened.
In particular, no expired packet was re-signed. The fresh campaign created a
new signed packet against a new genesis and finalized it before its recorded
last-valid block height.

## Exact sources, runtime, and command

| Fact | Value |
| --- | --- |
| Driver source | clean hbox clone `/tank/dregg-build/dclutch-founding-packet-3f51d00f0`, revision `3f51d00f0c79ac9644f3dec254549d01ae5417ac` |
| Checked program gate | `/tank/dregg-build/dclutch-df6-candidate-20260907/CHECKED_UPGRADE_GATE.json` |
| Gate SHA-256 | `d88a1b858fafd3fff79cec2f3f70e39610c1bf3c4033afe3539761a3506172d8` |
| Checked program source | `df6fddba9fd48d827f92104d65b77f32e9c5c002`, source-tree SHA-256 `9acecb6c093240a7f34622b253e6a6d0ee677bae23a1fd7363c6c6fe5b0beb79` |
| CLI/runtime | `solana-cli 4.0.2 (src:1845f426; feat:6ff76655, client:Agave)`; driver lock resolves `solana-runtime 4.3.0-beta.2` |
| Native build profile | hbox `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=6` through the harness's `swarm-build` path |
| Retained run | `/tank/dregg-build/dclutch-founding-packet-3f51d00f0-df6-run16/runs/20260908T013050Z-capture` |
| Harness command | `tools/gauntlet/ladder/run-ladder.sh --walk capture --worktree --repo /tank/dregg-build/dclutch-founding-packet-3f51d00f0 --work /tank/dregg-build/dclutch-founding-packet-3f51d00f0-df6-run16 --checked-release-gate /tank/dregg-build/dclutch-df6-candidate-20260907/CHECKED_UPGRADE_GATE.json --rpc-port auto` |
| New genesis | `3r7CGRpRw1NCmAMC1dkt97HsCmtXSmFA7MSpG9CVNoJa` |
| Validator profile | default chain-derived `--ticks-per-slot 16`, RPC `127.0.0.1:40968` |

The checked ELF SHA-256 values used by the chain were Core
`a08769a886cf03a061376dc5741c4141c545554830a3c9b79423477b94115c97`,
Resolution `87661a9764839945cce082feccb5095c8b7c6412fc7f65cd0114be222d84698a`,
and Claims `7656cade51da56014f0be94988330a50ba9ec75cdf8b1fb176c5d5881c48afe3`.

This is a live checked-program local-validator result driven by the named
working-tree clone. It is not a release cut from either source revision.

## Failure boundary and fresh-packet measurement

The original preserved runtime-567 evidence is
`/tank/dregg-build/dclutch-capture-transition-40d880df4-run/runs/20260908T002200Z-capture/capture/campaign/founding-evidence.json`,
SHA-256 `08b50da0a4c7edfff7bd5ea962eb69d0fffaed7654b9c7402255822d8c20c1b0`.
Its `core-funding-accept-v1` row remained `Dispatching`, carried last-valid
block height 8049, and had no signature. The contemporaneous height was
8112: the unsigned-to-send interval crossed 63 heights after expiry (and 213
heights after the prior relevant snapshot). The older driver had no named
phase observations, so this record deliberately does not assign that interval
to any individual RPC or fsync.

The driver now emits named observations before each freshness gate, including
the failed boundary, and preserves the same exact message fee quote. In the
fresh run, the accepted `core-funding-accept-v1` packet measured:

| Boundary | Elapsed | Finalized block height where read |
| --- | ---: | ---: |
| blockhash acquired | 0 ms | — |
| fee quoted | 0 ms | — |
| planned journal fsynced | 50 ms | — |
| before sign / freshness gate | 51 ms | 7811 |
| prepared journal fsynced | 84 ms | — |
| before dispatch / freshness gate | 85 ms | 7811 |
| dispatching journal fsynced | 127 ms | — |
| before send / freshness gate | 128 ms | 7812 |

Thus the measured driver path crossed one height and sent within 128 ms. It
refutes a persistent 16-tick driver-side 200-height path; it does not prove
which external activity caused the old run's stall. The retained instrumentation
is the diagnostic/fix for a recurrence: it records the named blocking boundary
before refusing, while the immutable journal remains the only authority for a
future exact-signature recovery.

## Accepted funding and poststates

`core-funding-accept-v1` finalized at slot 7891 with last-valid block height
7961, fee 75,000 lamports, and 213,429 compute units. Its exact signature is
`85u1kAuSc2Y75ZrSLrXuHviP171VoE8pboqq6vXXdTVuPUaRkxdtqDMYJ4gGzzRaBJr3Lj37iD3db8RViG9RP1G`.
The journal's finalized-poststates digest is
`f997a19e52e588e9a35bebf4a906e384abe9abd523bc328bf90a3dd1ce728e34`.

The journal's accepted Core, Resolution, and Claims poststates are the
following exact account evidence entries:

| Owner domain | Account | Address | Data SHA-256 |
| --- | --- | --- | --- |
| Core | founding market | `ESrsAB1NiH2B7G9QFW3VvFtfzuzWh9xFsoq8M3GtnT76` | `cb312ea1ad1b60ead57343dc41dc96e65cd239a628938b5f69523bbe52e70c2f` |
| Resolution | source state | `3tYLSNg4F7JbKKqHN9coNaCrDs5FZrWkMBhf5EkHjznF` | `1d4f5439a90b039b63e3bcc80a41faa40546f66ee5adabb3b7e10e820b638737` |
| Resolution | funding ledger | `HcMzDF5Ev29DjMZGTD5SthoCZ5omyEXqn2DMovuWTQtx` | `7dc84dba8201bf00e812fd88cd44db1cced067cb30049a4662d6295f72d19637` |
| Resolution | activation receipt | `6ZGXNYB4CohzmVZjkVDvgyp69wN8WaFP7RFEuS9msgBc` | `cba95cce8d6c96c2144e07968d68f833e40f637c23982f97825b552590af7640` |
| Claims | admission | `6gEsnMedJPNyeSEqGWaVddHcF3ff3NpmTbAmwcTpfhEt` | `8f23c688e894a743c0f401213345cb0a84090b13305666e76a6b805ab2c69914` |
| Claims | aggregate | `EiSr2sK12fHxD1ZNXMou1thKLGHURtgF2djJYPMDGbH9` | `bee8dd5382ebf8d05ed1503a2850da071c65a925bdbc67a65ce5d72421f3d8b3` |

The complete founding evidence has SHA-256
`038cee2cb156e9ebe0c9ab8d980df8f2cd6a1f5501a726ef0d4da86989d5aaf1`.

## Pyth capture and terminal poststate

The transcript SHA-256 is
`aa903f2db9b14614df1961b8806b9aee013037cf87d48f7bb4d6784daeb6d808`.
It records a Pyth publication at cluster unix time 1788831234, then a real
Wormhole verification and Pyth receiver update. Resolution submitted the
funded rung and produced certificate
`D69ojguLFwYcPfxKa3zfuzWSojDvxYk1sFNzqKtmzm9T`; the capture readback is
`Resolved`, route `Recovery`, attempt index 1, selector 0, terminal sequence
1. Core then admitted that certificate: the transcript's final stage records
Market phase `Terminal`, terminal sequence 1, and winner 0. The transcript
finished at cluster unix time 1788832538.

The frame captures retained with this result are
`campaign/captures/provider-submit.capture.json` SHA-256
`0faad3ff91f9485501cf0e60478a3b0e53f5aa620af6a1a7c7d834090786f5cb` and
`campaign/captures/provider-execute.capture.json` SHA-256
`4d9a68f78547b39cd7d3361b24a02adc659e647da4e9d9b394d291124d8733ee`.

## Separated 64-tick control

Commit `3f51d00f0c79ac9644f3dec254549d01ae5417ac` also makes the explicit
`DCLUTCH_TICKS_PER_SLOT` profile reach both validator spawn sites. A bounded
64-tick control verified the argv and was stopped before any founding packet
was planned. Its retained scheduling-wall record is
`/tank/dregg-build/dclutch-founding-packet-3f51d00f0-run64/64tick-scheduling-wall.json`,
SHA-256 `20913112d52992be6cd4cc7b21c0060e39d6d9b2f2a03e0da59cfef39750eddb`.

That control published its immutable source window at 2026-09-08T01:23:45Z
with a 1200-second primary deadline, while 5900 slots remained before Found.
At 64 ticks per slot the conservative completion time was about 2360 seconds.
It could not accept Capture under that authored window, so it is evidence of
a campaign-scheduling wall only; it makes no accepted-capture claim and does
not justify changing the source-window policy.

