# Generation 1

Moved here from the repository root on 2026-08-27; nothing was rewritten in the
move. This directory **is** the old repository root, so a generation-1 citation
written as a root-relative path resolves by prefixing `archive/gen1/`: for
example `docs/decisions/REPORT_fee-base-selection` is
`archive/gen1/docs/decisions/REPORT_fee-base-selection`, and
`crates/clutch-price-measure/tests/adversarial.rs` is
`archive/gen1/crates/clutch-price-measure/tests/adversarial.rs`.

The two exceptions are the root documents that did not move here: the
session-era handoffs (`CURRENT_TRUTH.md`, `GOAL.md`, `PROJECT.md`,
`CODEX_HANDOFF.md`, `CLAUDE_HANDOFF.md`, `MACRO_AND_MICRO_OPTIMIZATION.md`) are
under [`../handoffs/`](../handoffs/), and `AGENTS.md`, `LICENSE`,
`SECURITY.md`, and `README.md` remain at the repository root.

The living protocol is [`../../dclutch`](../../dclutch).
