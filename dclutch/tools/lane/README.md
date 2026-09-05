# tools/lane.sh

WAVE.md's "closing pattern language" (2026-08-27), pattern 7:

> LANE WRAPPER: tools/lane.sh — enforced --only, pinned rustfmt, board
> helper; retires three recurring accident classes.

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

Also writes a `Lane: $DCLUTCH_LANE` **trailer** on the commit, as does
`commit-patch`. See "the `Lane:` trailer" below.

Incident: the old protocol — inspect `git status`/`git diff --cached`, then
`git commit` — is a race against every other lane's concurrent `git add` on
the same shared index. WAVE.md records "two collisions on 2026-08-26" before
the protocol changed to `--only` exclusively. `--only` is race-proof *only*
when given a real, non-empty path list; an empty one degrades it back to
"commit whatever the index/working tree holds."

### `lane.sh commit-patch <message> <patch-file>`

Commits **HEAD's blob plus your hunk** for a path another lane is also editing,
then brings that path's working tree forward to match.

`commit --only` protects other PATHS and structurally cannot protect other
HUNKS inside a path you name, because it takes that path's whole current
working-tree content. `commit-patch` applies your patch to the index instead
(`git apply --cached`, on top of the blob the index already holds from HEAD),
so another lane's uncommitted line in the same file is neither committed nor
disturbed.

Refuses a non-empty index, a staged path set that differs from the patch's, a
missing patch file, and being run outside the repository root. Reads the commit
back the way `lane.sh commit` does.

**After the commit it reconciles the working tree**, per path, three ways: the
patch applies, so it is applied; it does not, but its reverse does, so the hunk
is already there and nothing happens; or neither, so a foreign hunk is in the
way and the path is left alone and named. `git apply` writes nothing unless
every context line matches, so the first branch can add your hunk and can never
overwrite another lane's.

That step is not tidiness. Until 2026-09-02 the index was written and the
working tree was not, so every path in a patch built in a detached worktree —
the house pattern for a shared file — read afterwards as a REVERSAL of the
commit just made, and the next `--only` on one of them would have silently
reverted it. The mirror incident the same day: `9efc24cf` committed a shared
file with `--only` while another lane's call sites sat in its working-tree
copy, carrying them to HEAD without the function they call, and main stopped
compiling. Both tools leave a footgun on the side you are not looking at, so
read `git diff` on your own paths right after either.

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

### The `Lane:` trailer

`commit` and `commit-patch` both add a `Lane: <id>` trailer, read from
`$DCLUTCH_LANE` — the same variable `board` already requires. Unset, it falls
back to the session id and then to `unknown`; unlike `board` it never refuses,
because an unset variable must not be able to block a commit in a tree a dozen
lanes are pushing through, and a session id still discriminates two lanes that
both forgot.

Read it back with `git log --format='%(trailers:key=Lane,valueonly)'`.
`tools/gate frames owed` prints it beside every debtor it names,
and a commit made without this wrapper is printed as *unattributed* rather than
guessed at.

Incident: every lane in this tree commits as the same git author, so `git log`
could name a commit and no instrument could name its lane. On 2026-09-02 three
lanes mis-attributed each other's commits in one afternoon, and `owed` — whose
entire output is a ledger of *who owes frame rows* — printed one identical
author beside every row it accused.

It is a trailer and not message prose for two reasons: a reader gets it from
git's own parser instead of a second regex over subject lines, and a trailer
written by the wrapper cannot be eaten by the backtick command-substitution
that takes code spans out of shell-quoted `-m` messages (the hazard `AGENTS.md`
records for both commit messages and board posts).

## Self-test

`tools/lane/test.sh` exercises every refusal path and the commit/board/fmt
happy paths against a scratch git repository under `/tmp` (never the real
repo, board, or a real crate). Run it with:

```
bash tools/lane/test.sh
```

It cleans up its scratch directory on exit and skips the two live-rustfmt
checks if the pinned `1.97.1` toolchain isn't installed.
