# Runtime-567 preserved-ledger terminal retirement — 2026-09-07

This is an immutable, dated diagnostic record for a preserved owned-loopback
ledger. It is **not** evidence for the final selected runtime or a fresh
campaign. The resumed validator used the checked runtime below without reset,
warp, account injection, or fixture reload.

## Runtime and source provenance

| Fact | Exact value |
| --- | --- |
| Original run | `/tank/dregg-build/dclutch-codex-journey-tail-run-da8dbc88c7e8-20260907/runs/20260907T213520Z-56767e555d05-h4` |
| Additive resume directory | `…/resume-20260907T220000Z` |
| Checked gate | `/tank/dregg-build/dclutch-codex-release-b-20260907/CHECKED_UPGRADE_GATE.json` |
| Gate SHA-256 | `6031115cd75da111c10d248a8489d7179d8edd5d320bc59e024a5f76e7107909` |
| Runtime source revision | `56767e555d05ecd005edec4fc939d269e8757417` |
| Runtime source-tree SHA-256 | `35013864908446676eb583d7a27eaf97755c2f7639f4ec7f110829a6c0710d94` |
| Resume mode | `no-reset-no-warp`; recorded in `restart.json` SHA-256 `be3e488df67d5f84e71c7cf2f861ad3304565694a8787ea8e6b7c472a4522f76` |
| Diagnostic source host | `/tank/dregg-build/dclutch-codex-journey-tail-runtime567-profilefix-96761292-v2` |
| Host revision | `fa513df7f4aa9fd9a66e4852ba53a967139f7516` (source tree was clean before generated build logs) |
| Native bootstrap SHA-256 | `2a5eb0577f71b82d9bd622b1ea9a3672f52d76058478f1142b8aca1b44c19f4f` |
| Selected-profile snapshot-owner integration commit | `96761292acf8d6493600fc9363d9934889a48c7c` |
| Owned retirement-table producer integration commit | `ca0f111382033d564a3bda11f4c745b9f36d96bf` |
| Compatibility proxy evidence SHA-256 | `7f11d62c7d2cb87d9a065208d6b43f8cc7495a6bb50a829d5a2538a09efab277` |

The profile-owner repair derives Registry and Rent `ArtifactRelease` raw and
staging PDAs from the decoded selected V2 Core infrastructure profile and
checks the fetched raw bytes hash to the selected digest. It therefore does
not select those accounts from stale plan records.

## Terminal predecessor stages

All six prerequisite journals are finalized. The journal SHA-256 covers every
recorded exact poststate; signatures, slots, fees, compute, and packet hashes
are reproduced here for audit.

| Stage | Signature | Finalized slot | Fee | CU | Packet SHA-256 | Journal SHA-256 |
| --- | --- | ---: | ---: | ---: | --- | --- |
| Core begin retiring | `3C3vwLRz1mgU71UbZf6jsPziHDtGcj9HTajJj1rZ3gvJCYcJj2XaggsutcUwDMocWKGihjeY2QFgZesxmW4CDvT1` | 10755 | 5000 | 22764 | `7bd2007b799ff628565672d0c84f42f6bd08336fe891f06ff2442c915ac390bf` | `bcbc78550bf4ca55112fb18411754df789f7d4d08a1840135f97f4a7589b5166` |
| Direct begin retiring | `5kqoBJP1vZRU8yDwPvr5GVY2MDWRv7feaa6P6qZHo4wcLgY74o41Adtj6pz7xJmLSqErDyKxHJTvKJJDpdz3Ccd2` | 10796 | 5000 | 88272 | `d8e82c0fb729d734d1a41f571e90d77211b32de1f8aaa094cda04385f770fa28` | `7093eede2e8be9960af5b2716aff3776b011954d9e4f30efc56145b3f1eac69d` |
| Resolution receipt prepay | `5G6PNE51FxKd6QwqUuS6AmiXr2rAnsBwSPh9FUa5aty4EJvAbkq31Gcgo48YzCrMA3ArdfdukpbKxHnVpixtLT5j` | 17055 | 5000 | 150 | `0ab4bc8c8f6dec97e599df0a60d8f52ae5d21d192c6767fec1365119bdb14122` | `e4ed191d2d7f0a8f37e2f553d2ed7b22acc3b6db7360083a608978d181b2bc16` |
| Resolution close fund | `GfqKde8eP3fYcpmuf1gJwT6sf4P9XLPT1kyq2jqePK6wcEdBbHVDn2CAj8grbfeRNEhFPtFi6KR4E6ibSS6rtcS` | 27653 | 5000 | 273739 | `3a1c71d8c6ba5229254388ebdae4630ca1c085f98378dad406d14b2c26968dbe` | `c6f9854ac387743a1023de924ab20d434f5f49247824761e6518c176956a35d0` |
| Direct close capability | `2nXCqZTDxXxNmUWrTNpNd47ku8HJJgL1CayzZsgqkhTvNZH3nVakwQ9Djm5VFVGfcFqQyedByop9Exp7MBHfAzb7` | 16810 | 5000 | 508733 | `7c27c8f7a2d4497769aec5a2bb1d382dfd13c99bb13428e552afbcd2c486d286` | `831ff84828c6b862cb95b46dc556738f9e058c37c3e3930b93b0163b30d3a463` |
| Retirement replay handoff | `4ovNzemPAWf7yeLzQNbUVGqY4ZeLyxWBkbpWScTk7JRxwRVCk1ExjbhYLLtWLahMoQs2pnYULhVNPQzqK68CnCAT` | 27850 | 5000 | 174547 | `ae168b1cdb8b773c47880821e8f81936ce49e7d3101c4b98f5b91a08b05e640a` | `0dad55b000328871142dbd2570349136c163cd37e3e6e059445c7be9fe4924c8` |

The one-shot aggregate stage made no submission. Its profile-authenticated
preflight found a seated failure escrow and refused by name, directing the
market to its four-packet checkpointed route. That route is the required
retirement shape because its prepare packet burns the failure column.

## Separated non-successes

The historical `ResolutionCloseFund` packet
`4GqfLnXZCR6GCtRhMLjwjYhgzah1UKfmB4YUcgWBEWuaeXv89aNWL4zB6yJ4dUVZ2n3kCQrJiKeDZsurNuLGa36d`
was superseded only after the recorded block height 18716 exceeded its last
valid block height 17223. The supersession report has SHA-256
`91bc9796e0197be270064530e707e9f3891c32b72a982f9e7b5faf14c8dc136d`;
the immutable superseded journal has SHA-256
`dfe1a35bf07ea136088fd822ea8062b75b1ade92f1a4d2763cb0118384f15878`.
It has no finalized poststate, so the later fresh `GfqK…rtcS` packet above is
the sole accepted close-fund mutation.

`resumed-tail-profilefix-evidence.json` is a preserved **empty** failed first
attempt to render the new evidence: SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
Its renderer rejected an undefined jq variable before producing evidence; it
was not replaced. This document cites the valid v2 output instead.

The submitted close-vault and close-replay packets were polled before their
next finalized observation. They were not re-signed: each journal retained
one frozen packet and later recorded its finalized status. The successful
signatures below distinguish this observation delay from an expiry or timeout.

## Accepted checkpointed retirement and conservation

The producer froze routing table
`AYUpgvHgiACNSMNkHsJ671PppsF77h8AchTminQ1Mx3D` over 37 derived addresses.
Its evidence is `retirement-lookup-table.json`, SHA-256
`f10877fedd2681d9f9462c31f7d166522045702c658949a7a2b3ddac86aa542b`.
The four accepted checkpoint journals are:

| Operation | Signature | Finalized slot | Fee | CU | Packet SHA-256 | Poststate SHA-256 | Journal SHA-256 |
| --- | --- | ---: | ---: | ---: | --- | --- | --- |
| Prepare | `3fgr72Jeq8jADDGFmSp6TM1mmEdPnEeauU6GucC1G27pEL4Bs3H6phmsWToJ7K4Ja1qxCqn6FHN4rZfMnnQPUWhh` | 43624 | 75000 | 158920 | `23d3e093ce678d03622f47c9268890d4e066316eb147500877ef4ebe5e2c45c7` | `2887f184b44ec323ca14446ba9f9f5be79f4380eec8477b3e0ef6f30aac34fac` | `6a1fa87ab724075a8bf5a34b8c829c2c0d43640ae89bfcd230a627a355752056` |
| Close vault | `3QJH7X3Gxx6w126xBvzueWHFjjQoMDLUW32jfd7wanRYZ2MTVJWuT6NvTePFZSR3eZSQQ3YPsxZdQvqjnhAbiuv9` | 43773 | 75000 | 142139 | `bb87974278ed1500e65231b3819704f8848872255b0eb544bdc0fef108eff06c` | `64176392664046dff71455c542bac1c8264503dabe34869fbffe0d983bed9631` | `70a36cf1fb9a2bd1ea9f0ced9950329e0bf286295c755f05a9cc64584a0fe240` |
| Close replay | `4Ms8rFaTkxyzYfCGwx1FFjvaGi9MDy6jcZJQw2n8n4wYqt2fDshsdPREAYFFGUxZnKDpbpozRtR1pdedFtJCvon6` | 44176 | 75000 | 125755 | `a6b6f601f8d80b634d4e6015ad402ad165a00a65b128175ba67129c26eb4d077` | `d6f6aa5f2a05486d915ed347c29c08121c8a75ab448a6e69868c144577ba5c97` | `11dec4eb5792f3d9fa34d5c841c0fa4412e21566eca3bd8f064af80c9d8e0b95` |
| Finish | `4A9LdKKXjitKd58v3snNT8PjHYvhJj2ka59BC36J3W91ApxjT8xk5NaYzj7qQvwzs7yJgAmBBQzLWHLT25PYnZdf` | 44492 | 75000 | 133075 | `9caf06dfda565c12b9a0b73a23337ace333090450c675c709ddcffc18b1463a5` | `58b3cd8f21f02a55d3c1d61f9e102f12346f3a7cda33804bd90f88ff1637fdcd` | `4ed580efb97dabc4037551f46bff6762fb009880f42ccfe67bae79e287b534dd` |

`retirement-completion.json` SHA-256 is
`d6cbf8ef6b5a538f54577ea41d73edaa06749ffd3e1e498afa5dee6414115c97`.
It classifies Market 3,452,160; RentCredit 17,873,283; claims refund
13,765,041; custody replay 2,895,360; and hoard vault 2,039,280 lamports,
with expected refund delta 40,025,124, terminal refund-wallet balance
60,327,450, and exact transaction fees 300,000 lamports.

The final finalized reread at slot 45466 is
`retirement-poststate-finalized.json` SHA-256
`c6754087d6fd9397ffbdf9299d04d49ac81191741f0a851ce845a839f81b7e55`.
It found Market, hoard vault, custody replay, RentCredit, and checkpoint all
absent. Thus the market has completed the terminal checkpointed route and the
hoard has no remaining account or balance.

The additive summary `resumed-tail-profilefix-evidence-v2.json` has SHA-256
`a46f2702c33b3d1ac5a554f067d5261a8ef9819e4c52438a98ca28187edc8137`.
