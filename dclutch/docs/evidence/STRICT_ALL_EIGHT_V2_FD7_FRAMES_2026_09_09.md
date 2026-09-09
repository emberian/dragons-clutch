# Strict all-eight ArtifactRelease V2 candidate and frame baseline — 2026-09-09

## Verdict

The exact committed source `fd7e5fec68cb948a326b41dc69be76f27156da10`
completed the checked genesis-candidate runner on hbox with exit 0. All eight
program links were freshly compiled under `swarm-build`, their shipped build
diagnostic count was zero, and the freshness gate admitted all eight. The
source-pinned Product handoff, eight role manifests, five-role execution set,
genesis infrastructure profile pair, checked Upgrade gate, reproducible gate,
run record and successor campaign pack all sealed.

The reproducible gate and campaign pack were then independently reopened with
their source-pinned verifiers. A separate canonical `tools/gate frames`
capture and an independent capture assembled from fresh exact-source objects
agreed byte-for-byte. The accepted baseline in
`tools/gates/frames-baseline.json` therefore covers this commit and these eight
links. It does not cover later General, Structured, or Claims native changes;
those changes remain frame debt until the next exact-source completion build
and two-capture admission.

This is local reproducible-build evidence. It is not validator execution, a
deployment, or chain evidence.

## Source and builder

| fact | value |
| --- | --- |
| source revision | `fd7e5fec68cb948a326b41dc69be76f27156da10` |
| source Git tree | `5f0a4d457f42b5093a948b182be7acb48ae48131` |
| complete source inventory SHA-256 | `05fb7efad13d45e91e65c462f3de3ed0b77dc0f03fc3f6773ddfca98ecc75b07` |
| clean hbox source | `/tank/dregg-build/dclutch-v2-source-fd7e5fec6` |
| checked candidate root | `/tank/dregg-build/dclutch-strict-fd7e5fec6-v2-no-general-owner-20260909-run2` |
| builder | hbox, Linux/x86_64, through `swarm-build`, `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=4` |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Solana | `solana-cli 4.0.2 (src:1845f426; feat:6ff76655, client:Agave)` |
| SBF builder | `cargo-build-sbf 4.0.0`, target `sbpf-solana-solana` |
| root Cargo.lock SHA-256 | `4a9fe5c9ac40966146b07d1f16903a8bfe03275c1879c0b0721980b301c53ebf` |
| Cargo.lock set SHA-256 | `5e32474700f4497c350ef051740f4e45b4400243bd0bc37c15e8c30dbeb23c70` (one lock; immutability passed) |
| Node input | v26.4.0; archive SHA-256 `5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431` |

The run began at 2026-09-09 00:37:49 EDT and wrote exit status 0 at
00:53:45 EDT. Its persistent driver log contains no `Unit run-u` scheduler
no-op marker and no runner refusal.

## Shipped links

`sbf_shipped_feature_suffix` is unconditional and empty at the source
revision: every ELF below is the ordinary production build with no Cargo
feature. The role provenance descriptors record the exact locked invocation.

| role | package | bytes | ELF SHA-256 | deepest production frame |
| --- | --- | ---: | --- | ---: |
| accelerator | `dclutch-accelerator-sbf` | 833,024 | `21af0964b31e842d6ee55c22bab03466727dbe4c8d5f9eea5a1502bc8240d5e9` | 3,136 |
| claims | `dclutch-claims-sbf` | 1,460,240 | `0a4b3527076bd0f32963dcbb54d06ad2361f1831bcb7d576532975dca454afb6` | 3,904 |
| core | `dclutch-core-sbf` | 1,174,368 | `d242da365eac4e7f6cea239b2649d2dd9ab6d358bf38be55974ccb09f30a492d` | 3,968 |
| custody | `dclutch-custody-sbf` | 470,400 | `c8072930ad813a19ba8355d38fd41857d97a78990812a0e52757ccc9ba124d6c` | 3,968 |
| registry | `dclutch-registry-sbf` | 251,024 | `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f` | 2,048 |
| rent | `dclutch-rent-sbf` | 143,496 | `6d00dfde5c764c7039e58df784ec91a88aa216e513a9f5fb79815a10cc358ac7` | 1,344 |
| resolution | `dclutch-resolution-proof-sbf` | 998,664 | `cf976a6008d2adaa722acf4b04e03179e35df90d792391dfc19dc4810fc29247` | 3,968 |
| trading | `dclutch-trading-sbf` | 2,737,576 | `8afe5107d2e87d4cf2d2f4b4504af9261eba1ff9493d074048bb21602cdf8160` | 4,032 |

The separate Trading CU diagnostic is
`profiled/trading.so`, SHA-256
`6397b6fc1a7ed30d3c79a8241b3d068607bdcb33235019a2a65d21a51a3ca6dd`.
It alone carries `--features hot-cu-profile`; it is measurement-only and is
absent from every checked role manifest and release set. The candidate's own
Trading frame report uses that diagnostic feature and was consequently not
used to admit the production frame baseline.

## Sealed candidate

| artifact | SHA-256 |
| --- | --- |
| checked Upgrade gate | `3e9f1b4c9a269a257e88cf2a6b8da565a0868a2b267bd05ed991078e1b8b76ac` |
| reproducible release gate | `a2423b95a11f0917a16b0d5199c761eb81c7db20c8e6a57a92de07d890a32e69` |
| release-gate run record | `ac6444b58ec4262ef6d53802d7395c14e81bba0a4a4527ba83855ac3eb4b53ed` |
| successor campaign pack | `c8f14757c784744be98ed4726f608ef5838e4c480882c18193db13b32e55140e` |
| five-role multiprogram manifest | `bbeaeef6ed64862a79b02193d7f33df2d48ff3d8fcd56cd99bff8fdcc5f347c1` |
| genesis infrastructure manifest | `db9be736aeef99f189889930fe0b5fcea40af74b48150adc93e956a21b62e657` |

The candidate records `infrastructure_lineage=genesis` and profile version
`1+2`. These independent reopen checks passed after the runner exited:

```text
artifact_provenance.py verify-reproducible-gate
  gate_sha256=a2423b95a11f0917a16b0d5199c761eb81c7db20c8e6a57a92de07d890a32e69
  link_count=8
  source_revision=fd7e5fec68cb948a326b41dc69be76f27156da10

successor_campaign_pack.py verify
  successor campaign release pack verified
  sha256=c8f14757c784744be98ed4726f608ef5838e4c480882c18193db13b32e55140e
```

## Independent frame admission

Capture B was a full fresh `tools/gate frames --at fd7e5fec... --capture`
run in a detached checkout, with all source, targets, logs and output under
`/tank/dregg-build/dclutch-frames-fd7e5fec6-v2-20260909`.

Capture A reused the checked candidate's seven fresh, exact-semantics
production frame objects. Trading was rebuilt alone, without
`hot-cu-profile`, in a new target under `swarm-build`; its log contains the
fresh top-package compile marker and zero over-bound diagnostics. The
source-pinned frame parser produced all eight complete per-function reports,
and the source-pinned `frames assemble` command constructed Capture A.

The two captures are byte-identical:

```text
capture A SHA-256 = f661439753dc4d10ea9fecff9385da3b59d42cb15ca7ab5601a45f9e3f6f4a3d
capture B SHA-256 = f661439753dc4d10ea9fecff9385da3b59d42cb15ca7ab5601a45f9e3f6f4a3d
```

`tools/gate frames accept` emitted the baseline now installed in the tree:

```text
schema=dclutch-sbf-frame-baseline-v1
commit=fd7e5fec68cb948a326b41dc69be76f27156da10
link_count=8
bound_bytes=4096
sha256=c6bf2d9704362434a9230b3519ebd0d57ea9ca7510de97c3ecfd0beaa7ba6088
```

`frames check` passed against the accepted baseline. `frames owed` from that
baseline through the captured commit reported zero intervening commits and
all eight dependency closures resolved over twenty first-party crates. The
capture-composition record SHA-256 is
`f61f3e16a816319b119f4b704932fb6076f3a69abbd31c030a6695e0470b8b5a`;
its eight object/report/build-log rows hash to
`e17b360c5b1c37bd1178d31e97c10fef3c71a97044f090b6f78f8a6ce3146a24`.

The 4,032-byte Trading maximum is 64 bytes below the 4,096-byte SBPF v0
wall. It is admitted evidence, not spare budget: later changes must be
remeasured rather than inferred safe from this margin.

At integration HEAD `c76ab9c64fa1119bbe441c7d27de409e964b7a25`, the
closure-aware `frames owed` census correctly refused on seven post-fd7
commits: four General changes, two Structured changes, and one Claims change.
That red is the intended handoff to the next all-eight build, not evidence
against the accepted fd7 capture.
