# Counted retirement seam

`clutch-retirement` is the production-bound, allocation-free `no_std` owner of
ADR-0007's retirement-specific bytes and pure state transitions. It does not
yet allocate a live instruction, dispatch a request, perform a CPI, resize an
account, or re-enable either legacy close.

The existing `clutch-sbf` program remains fail-closed. A future adapter composes
these exact tails with the base account bodies still owned by
`clutch-solana-layout`:

- Position V2: existing 220 bytes + exact 60-byte retirement tail = 280;
- Epoch V3: existing 329 bytes + exact 100-byte retirement tail = 429;
- Market V2: existing 726 bytes + exact 8-byte cursor = 734;
- Reservation V5: existing 618 bytes + exact 9-byte count tail = 627;
- Position tombstone: complete exact 76-byte codec; and
- general Epoch tombstone: complete exact 84-byte codec.

The pure transitions calculate complete post-state values before returning.
An error returns no post-state, so a caller can encode only after every checked
transition succeeds. That is the host half of rollback safety; Solana account
locking, resize/CPI rollback, lamport movement, PDA authentication, and ELF
correspondence still require local-bank tests.

The appended byte order is frozen:

| Tail | Offsets | Bytes |
| --- | --- | ---: |
| Position retirement | `0..4 outstanding`, `4..60 rent split` | 60 |
| Epoch retirement | `0..8 generation`, `8..44 nine counts`, `44..100 rent split` | 100 |
| Market cursor | `0..8 next_general_epoch_index` | 8 |
| Reservation count | `0..8 epoch_generation`, `8 counted boolean` | 9 |
| Other child generation | `0..8 epoch_generation` | 8 |

The nine count words are candidate bundle, CandidateIndex page, candidate
verdict, candidate escrow, ClearWork bundle, order page, reservation archive,
settlement receipt, and final pot, in that order. Every integer is little
endian. Tombstone vectors use codec-local provisional `0x75/0x76` tag bytes.
They are not live wire allocations: integration must first reconcile and
reserve them in the authoritative account-tag registry and live router.

Candidate lifecycle state is not a second wire truth here. The transition seam
accepts an opaque `(candidate tag, candidate version, status)` witness only
after the owning candidate decoder and state machine validate it. Retirement
never interprets that status: every admitted status keeps exactly one candidate
bundle counted, and a status update cannot silently switch account schemas.

Run:

```sh
cargo test --manifest-path crates/clutch-retirement/Cargo.toml
cargo test --release --manifest-path crates/clutch-retirement/Cargo.toml
cargo clippy --manifest-path crates/clutch-retirement/Cargo.toml \
  --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-retirement/Cargo.toml --no-deps
```
