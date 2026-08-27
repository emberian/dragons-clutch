# tools/lane.sh

WAVE.md's "closing pattern language" (2026-08-27), pattern 7:

> LANE WRAPPER: tools/lane.sh — enforced --only, pinned rustfmt, board
> helper; retires four recurring accident classes.

The raw git/rustfmt/board commands documented in `WAVE.md` and
`tools/gauntlet/{TIERS,README}.md` remain valid on their own — this wrapper
does not invent new policy, it just refuses to run those commands the ways
that have already cost real lane-time. Run `tools/lane.sh <subcommand>
--help` for the full incident text; the summary below is enough to pick the
right subcommand.

## Subcommands

### `lane.sh commit <message> -- <path> [<path> ...]`

Runs exactly `git add -- <path> ...` (named paths only, so a brand-new file
is visible to `--only` at all) then `git commit --only --no-gpg-sign
-m <message> -- <path> ...`, then reads back the commit's actual changed
paths (`git show --name-only`) and fails loudly (without rewriting
history — the commit stays) if anything outside the given list was touched.

Refuses: an empty or wildcard/whole-tree path list (`.`, `..`, `/`, `*`,
`-A`, `--all`) and being run from anywhere but the repository root.

Incident: the old protocol — inspect `git status`/`git diff --cached`, then
`git commit` — is a race against every other lane's concurrent `git add` on
the same shared index. WAVE.md records "two collisions on 2026-08-26" before
the protocol changed to `--only` exclusively. `--only` is race-proof *only*
when given a real, non-empty path list; an empty one degrades it back to
"commit whatever the index/working tree holds."

### `lane.sh fmt [--allow-root] <file.rs> [<file.rs> ...]`

Runs exactly `rustup run 1.97.1 rustfmt --edition 2024 -- <file.rs> ...`.
Never `cargo fmt -p <crate>` (reformats the whole crate) and never a bare
`rustfmt` (whatever toolchain/edition happens to be ambient).

Refuses a bare `lib.rs` / `main.rs` / `mod.rs` unless `--allow-root` is
given.

Incident: WAVE.md's cook summary — "bare rustfmt is unpinned and reflows
~178 lines of hot_v3." Commits `3b0c588`, `d394cd9`, `d7bfb7d` each had to
hand-untangle formatter drift from real statement changes in a file several
lanes share. The `--allow-root` guard is separate: rustfmt run on a crate or
module root follows every `mod` declaration the file contains and reformats
each of those files too — the "mod-following hazard," which silently
reformats far more than the one file you named.

### `lane.sh board <text...>`

Appends a timestamped entry to the cross-lane board (default
`/private/tmp/dclutch-wave-board.md`; override with `$DCLUTCH_BOARD_FILE`),
attributed to `$DCLUTCH_LANE`. Refuses if that variable is unset.

Incident: the board's own protocol already asks lanes to sign their entries
with a lane name. The cost of not doing so is on the board itself: the
TA-SER entry ("TIER NUMBER COLLISION, my fault") where two lanes clobbered
each other's `tools/gauntlet/tier2/` files for about fifteen minutes, and
DA2's leaked-validator note that containment on a shared resource "was a
lane remembering to be polite rather than something structural."

### `lane.sh guard-script <script> -- <cmd...>`

Snapshots `<script>`'s inode + sha256, runs `<cmd...>` to completion
(its exit status becomes this wrapper's exit status), then warns loudly on
stderr if `<script>` changed mid-run. It cannot make a mid-run edit safe —
nothing can — it only guarantees you find out.

Incident: `tools/gauntlet/TIERS.md` / `README.md` — "never edit run.sh
while a run is in flight. Bash reads a script incrementally by byte offset,
so an edit mid-run shifts what it reads next and it will re-execute or skip
a block" (the README calls this "a corollary that cost this lane an hour").
`tools/gauntlet/direct/` lives in its own directory rather than as a
`run.sh` stage for the same reason, compounded by a second, independent
incident the same day: "three lanes claimed the same two [tier] numbers
inside twenty minutes" while a `--mode full` run was already in flight and
editing `run.sh` mid-run was unsafe.

## Self-test

`tools/lane/test.sh` exercises every refusal path and the commit/board/fmt
happy paths against a scratch git repository under `/tmp` (never the real
repo, board, or a real crate). Run it with:

```
bash tools/lane/test.sh
```

It cleans up its scratch directory on exit and skips the two live-rustfmt
checks if the pinned `1.97.1` toolchain isn't installed.
