#!/usr/bin/env bash
# tools/ci/run.sh -- the tiers, and which gate is in which one.
#
# This script does not implement a single gate. Every gate it runs was written
# by the lane that owns it and lives in that lane's directory; this file is
# only the answer to a question that previously had no written answer at all:
# WHICH OF THEM RUNS AUTOMATICALLY, AND WHEN.
#
# Before this, that answer lived in a YAML file in a DIFFERENT REPOSITORY (the
# public wrapper's .github/workflows/), which is a bad place for it twice over:
# a lane working in this tree cannot see it, and the wrapper observes a
# vendored snapshot rather than this tree. So the tiering is stated here, in
# the tree the gates live in, and the wrapper's workflows CALL THIS rather than
# restating it. One definition, two callers.
#
#   tools/ci/run.sh <tier> [<tier> ...]   run those tiers
#   tools/ci/run.sh --list                the table, with costs and prerequisites
#
# THE TIER TABLE IS NOT REPEATED HERE, and that is a repair rather than an
# omission. It used to be, and it had drifted: this header listed six tiers
# when the dispatch ran nine, and described `cheap` as `census + seam` when it
# had become `census seam release`. `--list` was correct the whole time, which
# is precisely why nobody noticed the other copy going stale -- and `--help`
# PRINTS this header, so the stale copy was a runbook teaching commands the
# tree does not have. That is this project's signature defect in its own CI
# runner: a value duplicated instead of read agrees right up until it does not.
# `--help` now prints this prose and then calls `list_tiers`, so there is one
# table with two callers, exactly as this file's opening paragraph argues the
# tiering itself should work.
#
# ---------------------------------------------------------------------------
# EXIT CODES, and this is the load-bearing part.
#
#   0  every requested tier RAN and PASSED
#   1  a gate FAILED -- this tree has the defect that gate detects
#   2  a PREREQUISITE IS MISSING -- nothing was proven, either way
#
# THESE ARE NOT INVENTED HERE. They are `tools/seam-audit/seam_audit.py`'s
# codes, adopted deliberately so this tree has ONE convention rather than two:
# 0 green, 1 this tree has a disagreement, 2 the checker could not run.
#
# 1 and 2 are different facts and conflating them is a real, already-paid-for
# defect: the seam audit used to exit 1 on a host with no `ast-grep` -- the
# SAME CODE it uses for "this tree has a seam defect" -- because the "install
# ast-grep" message sat behind a returncode check that an absent binary never
# reaches, since subprocess.run RAISES instead. It was fixed in c3de7b46 by
# making it a 2. A CI job reading only the status would have called a clean
# tree broken; worse, a lane that learns "that code is fine" from a missing
# tool learns nothing about whether the gate would have passed.
#
# So a missing prerequisite is loud, distinct, and NEVER silent -- and where a
# gate already reports its own 2, THIS SCRIPT READS THAT ANSWER RATHER THAN
# RE-DERIVING IT. Checking for ast-grep here as well would be a second author
# for a question the gate already answers, which is this project's signature
# defect: a value duplicated instead of read agrees right up until it does not.
#
# `--require` turns 2 into 1. Use it wherever an unrun gate is not an
# acceptable answer -- a release candidate, the cut. The wording is the web
# suite's own, about a Lean ABI check it refuses to skip: "an unverifiable ABI
# is not a verified one".
#
# A usage error exits 64 (sysexits EX_USAGE), so that "you typed a tier name
# wrong" can never be mistaken for either of the two answers above.
# ---------------------------------------------------------------------------

set -uo pipefail

readonly EXIT_PASS=0
readonly EXIT_GATE_FAILED=1
readonly EXIT_PREREQ_MISSING=2
readonly EXIT_USAGE=64

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

require_mode=""
commit_rev=""
verdicts=()
worst=$EXIT_PASS

say() { printf '\n=== %s ===\n' "$*"; }
note() { printf '    %s\n' "$*"; }

# Record one tier's outcome. Missing-prerequisite is promoted to failure under
# --require; otherwise it is kept distinct all the way to the summary, so the
# last line a reader sees still tells them which gates did not actually run.
record() {
  local tier="$1" code="$2" detail="${3:-}"
  if [ "$code" = "$EXIT_PREREQ_MISSING" ] && [ -n "$require_mode" ]; then
    code=$EXIT_GATE_FAILED
    detail="${detail} (--require: an unrun gate is not a passing gate)"
  fi
  case "$code" in
  "$EXIT_PASS") verdicts+=("PASS      $tier") ;;
  "$EXIT_PREREQ_MISSING")
    verdicts+=("NOT RUN   $tier -- $detail")
    [ "$worst" = "$EXIT_PASS" ] && worst=$EXIT_PREREQ_MISSING
    ;;
  *)
    verdicts+=("FAILED    $tier${detail:+ -- $detail}")
    worst=$EXIT_GATE_FAILED
    ;;
  esac
}

# A prerequisite check that reports WHAT is missing and HOW to get it, because
# "command not found" in a CI log is a dead end for whoever reads it next.
have() {
  command -v "$1" >/dev/null 2>&1
}

# ---------------------------------------------------------------------------
# census -- milliseconds, no toolchain.
#
# `emission_guard.py --verify` byte-gates COVERAGE.md, the committed census of
# which generated files a re-emit check actually guards. It never runs Lean, so
# it costs nothing and belongs in the cheapest tier that exists.
#
# What it catches: a new generated file arriving with NO guard, or an existing
# guard disappearing. That is the ratchet -- the unguarded count can only move
# when somebody looks at it and decides.
#
# What it does NOT catch, stated because it would otherwise be assumed: whether
# the guarded bytes still match. That needs `lake` and it is the `emission`
# tier below. A green census means we know which bytes nobody is checking.
# ---------------------------------------------------------------------------
tier_census() {
  say "census -- generated-file coverage ratchet"
  local tool="$repo_root/tools/emission-guard/emission_guard.py"
  if [ ! -f "$tool" ]; then
    note "tools/emission-guard/emission_guard.py is not in this tree"
    record census $EXIT_PREREQ_MISSING "emission-guard absent from this tree"
    return
  fi
  if ! have python3; then
    record census $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi
  local code=0
  (cd "$repo_root" && python3 "$tool" --verify) || code=$?
  case "$code" in
  0) record census $EXIT_PASS ;;
  2)
    note "The census could not be taken -- it reads \`git ls-files\`, so it"
    note "needs a git checkout. An exported or vendored copy is not one."
    note "This says NOTHING about whether the census would pass."
    record census $EXIT_PREREQ_MISSING "the census could not run (its exit 2)"
    ;;
  *)
    note "COVERAGE.md no longer describes this tree. Re-run with --write and"
    note "read the diff: a generated file gained or lost a guard."
    record census $EXIT_GATE_FAILED
    ;;
  esac
}

# ---------------------------------------------------------------------------
# fmt -- seconds, and the gate that could not exist until 6e50a185.
#
# Not "is the tree formatted" but "does rustfmt still disagree with exactly the
# files we already know it disagrees with", read from tools/ci/fmt-baseline.txt.
# Both directions fail: an unformatted file NOT in the baseline, and a baseline
# line that is no longer true. That is COVERAGE.md's ratchet applied to
# formatting -- the list moves when a person looks at it and decides.
#
# WHY A BASELINE AT ALL, since "run rustfmt" is a complete fix and a strange
# thing to defer. Until rustfmt.toml existed this tree had TWO formatters that
# disagreed -- `cargo fmt` passes each crate's declared edition, `lane.sh fmt`
# hardcodes --edition 2024, and dclutch-dealer-codec declares 2021 -- so the
# same file was reformatted back and forth by lanes who were each running the
# tool correctly. 50 files had drifted. 874e6c34 canonicalised the 31 nobody
# was holding; the other 19 are live-lane files, and reformatting a file
# somebody is mid-edit in destroys their work, which is worse than the drift.
# So the baseline is a HANDOFF LIST with an owner per block, not an exemption,
# and it only shrinks.
#
# WHAT THIS DOES NOT COVER, stated because the tier name promises more than it
# delivers: `cargo fmt --all` reads the ROOT workspace, and this tree has 57
# tracked Cargo workspaces. The nested program-test and tool workspaces are
# formatted by nothing here. Running rustfmt over all 1,147 tracked .rs files
# instead was measured and abandoned -- rustfmt follows `mod` from every file
# it is handed, so passing all of them re-formats the same trees repeatedly and
# did not finish in two minutes, which is not a cheap-tier cost. Closing it
# wants one rustfmt invocation per workspace root, and it is owed.
# ---------------------------------------------------------------------------
tier_fmt() {
  say "fmt -- rustfmt against the committed baseline"
  local baseline="$repo_root/tools/ci/fmt-baseline.txt"
  if [ ! -f "$baseline" ]; then
    note "tools/ci/fmt-baseline.txt is not in this tree, and without it this"
    note "gate has nothing to compare against -- it would report every drifted"
    note "file as new. That is a missing prerequisite, not a clean tree."
    record fmt $EXIT_PREREQ_MISSING "the fmt baseline is absent"
    return
  fi
  if [ ! -f "$repo_root/rustfmt.toml" ]; then
    note "rustfmt.toml is not in this tree. It is what makes \`cargo fmt\`,"
    note "\`tools/lane.sh fmt\` and a bare \`rustfmt\` agree; without it"
    note "\"formatted\" has no single answer and this gate would enforce"
    note "whichever one cargo happened to pick. Nothing is claimed."
    record fmt $EXIT_PREREQ_MISSING "rustfmt.toml absent -- 'formatted' is undefined"
    return
  fi
  if ! have cargo; then
    record fmt $EXIT_PREREQ_MISSING "cargo not on PATH"
    return
  fi
  if ! (cd "$repo_root" && cargo fmt --version) >/dev/null 2>&1; then
    note "cargo is present but the rustfmt component is not. Install it with:"
    note "    rustup component add rustfmt"
    record fmt $EXIT_PREREQ_MISSING "the rustfmt component is not installed"
    return
  fi

  # This reads the WORKING TREE, like every other cheap tier -- `--commit` does
  # not reach here. On a shared checkout that matters more for this gate than
  # most: an unformatted file a neighbouring lane is mid-edit in is
  # indistinguishable from drift somebody landed, and reformatting theirs to
  # clear it is the one repair that is never yours to make.
  local dirty
  dirty="$(cd "$repo_root" && git status --porcelain -- '*.rs' 2>/dev/null |
    wc -l | tr -d ' ')"
  if [ "${dirty:-0}" != 0 ]; then
    note "the working tree has $dirty uncommitted .rs files; a finding below"
    note "may be a neighbouring lane's, and is theirs to format, not yours"
  fi

  local found expected new gone
  found="$(cd "$repo_root" && cargo fmt --all --check 2>/dev/null |
    sed -n 's|^Diff in ||p' | sed 's|:[0-9]*:$||' |
    sed "s|^$repo_root/||" | sort -u | grep -v '^$')"
  expected="$(grep -v '^[[:space:]]*#' "$baseline" | grep -v '^[[:space:]]*$' |
    sort -u)"

  new="$(comm -23 <(printf '%s\n' "$found" | grep -v '^$') \
    <(printf '%s\n' "$expected" | grep -v '^$'))"
  gone="$(comm -13 <(printf '%s\n' "$found" | grep -v '^$') \
    <(printf '%s\n' "$expected" | grep -v '^$'))"

  if [ -n "$new" ]; then
    note "rustfmt disagrees with files that are NOT in the baseline:"
    printf '      %s\n' $new
    note "The fix is the FILE, never this list: \`tools/lane.sh fmt <file>\`"
    note "(--allow-root for a lib.rs/main.rs/mod.rs, and then look at what"
    note "else it reflowed -- rustfmt follows \`mod\` out of the file you"
    note "named). A line may be added to the baseline only for the reason"
    note "every line there already carries: another lane owns that file now."
    record fmt $EXIT_GATE_FAILED "$(printf '%s\n' $new | wc -l | tr -d ' ') unformatted file(s) outside the baseline"
    return
  fi
  if [ -n "$gone" ]; then
    note "These baseline lines are no longer true -- rustfmt is happy with"
    note "them now, so the list is claiming work that is already done:"
    printf '      %s\n' $gone
    note "Delete those lines from tools/ci/fmt-baseline.txt and commit it with"
    note "whatever formatted them. The gate fails in this direction on purpose:"
    note "a ratchet nobody has to unwind is a ratchet that never reaches zero."
    record fmt $EXIT_GATE_FAILED "$(printf '%s\n' $gone | wc -l | tr -d ' ') stale baseline line(s)"
    return
  fi
  note "$(printf '%s\n' "$expected" | grep -c '^..*$') file(s) still owed, each named in the baseline with its lane"
  record fmt $EXIT_PASS
}

# ---------------------------------------------------------------------------
# runbooks -- seconds. Every command a runbook publishes, replayed as `--help`.
#
# The complement of the release tier's `usage_parity.py`, one layer out. That
# gate holds a TOOL's usage text to its own parser; this one holds a RUNBOOK to
# the program it instructs. Both close the same shape -- a sentence outliving
# the interface it describes -- and neither can see the other's half.
#
# It runs `--help` and nothing else, and only against a program whose own
# source shows it handles a help flag. A program it cannot probe is reported
# unprobed WITH THE REASON and exits 2, never 0: the gate keeps "could not be
# checked" apart from "checked and fine", which is this file's whole exit-code
# argument applied to itself.
# ---------------------------------------------------------------------------
tier_runbooks() {
  say "runbooks -- every published command, replayed as --help"
  local tool="$repo_root/tools/doc-commands/doc_commands.py"
  if [ ! -f "$tool" ]; then
    note "tools/doc-commands/doc_commands.py is not in this tree"
    record runbooks $EXIT_PREREQ_MISSING "doc-commands absent from this tree"
    return
  fi
  if ! have python3; then
    record runbooks $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi
  local code=0
  (cd "$repo_root" && python3 "$tool" --root . \
     --baseline tools/doc-commands/baseline.json --check) || code=$?
  case "$code" in
  0) record runbooks $EXIT_PASS ;;
  2)
    note "Some published command could not be probed -- its program is not"
    note "built, or handles no help flag. Nothing is claimed about those."
    record runbooks $EXIT_PREREQ_MISSING "a published command was not probed (its exit 2)"
    ;;
  *)
    note "A runbook publishes a command a reader cannot run as written. The"
    note "fix is the DOC or the PROGRAM, never the baseline: an accepted entry"
    note "there has to carry a written reason a reader would agree with."
    record runbooks $EXIT_GATE_FAILED
    ;;
  esac
}

# ---------------------------------------------------------------------------
# seam -- ~20 seconds over ~960 Rust files, no cargo build.
#
# Six defect classes, each with a real pre-fix commit from this repository as
# its negative control. Exits nonzero on NEW findings only; the triaged
# baseline is committed beside it with a written reason per accepted entry.
#
# THE PREREQUISITE IS THE POINT, and note how it is handled. This gate needs
# `ast-grep`, and it ALREADY reports a missing one as its own exit 2. So this
# tier runs it and reads that code straight through. It deliberately does NOT
# check for ast-grep itself: a second detector for a condition the gate already
# detects is a second author who can disagree with the first, and the whole
# reason the codes line up is so that no translation is needed here.
# ---------------------------------------------------------------------------
tier_seam() {
  say "seam -- structural seam register"
  local tool="$repo_root/tools/seam-audit/seam_audit.py"
  if [ ! -f "$tool" ]; then
    note "tools/seam-audit/seam_audit.py is not in this tree"
    record seam $EXIT_PREREQ_MISSING "seam-audit absent from this tree"
    return
  fi
  if ! have python3; then
    record seam $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi
  local code=0
  (cd "$repo_root" && python3 "$tool") || code=$?
  case "$code" in
  0) record seam $EXIT_PASS ;;
  2)
    note "The seam checker could not run -- most often ast-grep is not on"
    note "PATH. This says NOTHING about this tree. Install it with one of:"
    note "    npm install -g @ast-grep/cli      (or --no-save, locally)"
    note "    brew install ast-grep             (macOS)"
    note "    cargo install ast-grep --locked"
    record seam $EXIT_PREREQ_MISSING "the seam checker could not run (its exit 2)"
    ;;
  *)
    note "NEW seam findings against the committed baseline. Triage them --"
    note "confirmed defect, written exception, or checker false positive with"
    note "a negative control. Do not widen the baseline without a reason."
    record seam $EXIT_GATE_FAILED
    ;;
  esac
}

# ---------------------------------------------------------------------------
# web -- the vitest suites, ~1 minute.
#
# Two files are excluded and each exclusion has a reason that is about cost,
# never about the assertion being unwelcome:
#
#   lib/abiVerification.test.ts  enumerates every `abi:*:verify` script and
#   runs it; four shell out to `lake build`. By deliberate design they FAIL
#   rather than skip when lake is absent. They belong to the `emission` tier,
#   which has the toolchain.
#
#   lib/sbomVerify.test.ts  runs the full SBOM closure and needs a populated
#   cargo registry. It is gated in the wrapper's hygiene job on terms that
#   make sense for a vendored snapshot.
#
# Everything else in both suites runs unfiltered. The live-devnet tests are
# env-gated off by default and reach no network.
# ---------------------------------------------------------------------------
tier_web() {
  say "web -- web + SDK vitest suites"
  if ! have npx; then
    record web $EXIT_PREREQ_MISSING "node/npx not on PATH"
    return
  fi
  local failed=0 ran=0
  local dir
  for dir in apps/dclutch-web packages/dclutch-sdk; do
    local full="$repo_root/$dir"
    [ -d "$full/node_modules" ] || {
      note "$dir: node_modules absent -- run npm ci there first"
      continue
    }
    ran=$((ran + 1))
    note "$dir"
    (cd "$full" && npx vitest run --config vitest.config.ts \
      --exclude 'lib/abiVerification.test.ts' \
      --exclude 'lib/sbomVerify.test.ts') || failed=1
  done
  if [ "$ran" = 0 ]; then
    record web $EXIT_PREREQ_MISSING "no suite had its dependencies installed"
  elif [ "$failed" = 0 ]; then
    record web $EXIT_PASS
  else
    record web $EXIT_GATE_FAILED
  fi
}

# ---------------------------------------------------------------------------
# emission -- the byte-identity guards for real. Needs `lake`, minutes.
#
# 52-odd guards across two kinds: hand-written `check-generated.sh` scripts and
# byte-identity gates written as Rust integration tests. The census above knows
# they exist; this tier RUNS them, which is the only thing that proves a
# generated file still matches the Lean that claims to have printed it.
#
# Scoped by a git range when one is given, because a push that cannot have
# moved an emission should not pay a Lean build. `--all` is the release answer.
# ---------------------------------------------------------------------------
tier_emission() {
  say "emission -- Lean byte-identity guards"
  local tool="$repo_root/tools/emission-guard/emission_guard.py"
  if [ ! -f "$tool" ]; then
    record emission $EXIT_PREREQ_MISSING "emission-guard absent from this tree"
    return
  fi
  if ! have lake; then
    note "lake is not installed, so no byte-identity guard ran. The census"
    note "tier still tells you WHICH files are guarded; it cannot tell you"
    note "that their bytes still match. Those are different claims."
    record emission $EXIT_PREREQ_MISSING "lake (Lean) not on PATH"
    return
  fi
  local scope=("--run" "--all")
  [ -n "${DCLUTCH_CI_RANGE:-}" ] && scope=("--run" "$DCLUTCH_CI_RANGE")
  if (cd "$repo_root" && python3 "$tool" "${scope[@]}"); then
    record emission $EXIT_PASS
  else
    record emission $EXIT_GATE_FAILED "a generated file no longer matches its emitter"
  fi
}

# ---------------------------------------------------------------------------
# programs -- the SBF tier. Minutes, and the only tier that builds a program.
#
# This is where the Direct Hot compute margin gate lives, and that gate is the
# reason this tier exists at all. The public Direct route runs about 18,000 CU
# under a hard protocol ceiling, and this repository's own history contains a
# commit that ate 7,520 of that margin while its message said it had changed no
# program -- true, and still costly, because it changed two SHARED contract
# crates. A margin that thin is only honestly accepted if something notices it
# eroding, at the author, rather than on devnet a month later.
#
# THE NUMBER IS NOT IN THIS FILE AND MUST NEVER BE. The gate owns its own
# constant; this tier only runs it. That is deliberate and it is this project's
# signature defect being avoided on purpose: a value duplicated instead of read
# agrees right up until the day it does not, and this one is a RATCHET -- when
# a lane makes the route cheaper it LOWERS the constant, and a second copy here
# would make that ordinary act of progress into a two-file chore that someone
# eventually gets half-right.
#
# The eight manifests are the five protocol programs the fixture installs plus
# the three test-only callers `waist::elves` loads beside them. All eight, or
# the harness panics looking for one and every seed fails for a reason that is
# not compute.
# ---------------------------------------------------------------------------
readonly PROGRAM_MANIFESTS="\
programs/dclutch-trading-sbf/Cargo.toml
programs/dclutch-registry-sbf/Cargo.toml
programs/dclutch-core-sbf/Cargo.toml
programs/dclutch-claims-sbf/Cargo.toml
programs/dclutch-custody-sbf/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml"

# WHICH TREE THIS BUILDS, and it is the difference between a measurement and a
# rumour. `cargo build-sbf` compiles WHAT IS ON DISK. On this repository's
# shared working tree that is routinely a dozen half-written files belonging to
# three or four other lanes -- so a default build measures a franken-tree that
# nobody has committed and nobody ever will, and reports a CU number that looks
# exactly as authoritative as a real one.
#
# This was not hypothetical. Writing this file, the gate went red at `seed 20:
# the public Direct route must EXECUTE, not refuse`, and it was one tool call
# from being posted as a regression at HEAD. `git status -- programs crates`
# then showed NINETEEN uncommitted files, nine of them under trading-sbf's own
# `series/`. Under Ledger M-61 a ONE BYTE ELF difference redraws every fixture
# seed by up to +/-46,000 CU, which is more than twice this route's entire
# margin -- so on a compute gate specifically, "whatever was on disk" is not a
# slightly noisy answer, it is a different question.
#
# `--commit REV` archives that revision to a scratch root and builds there.
# `git archive` and not `git worktree add`, following tools/seam-audit's fix
# for the same class: it touches no repository state, so it cannot contend on
# .git locks with the other lanes, and cleaning up is an `rm`.
#
# With no --commit it builds the working tree, which is the RIGHT default for
# the author of an edit -- you want to be told about the defect you just wrote.
# It says loudly whose tree it measured either way.
archive_revision() {
  local rev="$1" root="$2"
  rm -rf -- "$root"
  mkdir -p -- "$root"
  (cd "$repo_root" && git archive "$rev") | tar -x -C "$root"
}

# ---------------------------------------------------------------------------
# frameguard -- all twelve SBF links, complete per-function frame map.
#
# The diagnostic grep in `programs` catches a frame only once it reaches the
# 4,096-byte SBPF wall. It is silent at 4,095 and was silent when one ordinary
# dispatch arm grew its shared function by 640 bytes (3,072 -> 3,712). This
# tier gives that below-wall class an owner: fresh `-Zemit-stack-sizes` objects,
# zero overwrite diagnostics preserved as a separate admission, then an exact
# comparison against the committed canonical function multiset.
#
# The gate itself owns the baseline, normalization and 0/1/2 exit distinction.
# This runner only selects which source tree is measured and reads that answer.
# A committed revision is archived because a shared-tree measurement is useful
# to an author but not quoteable evidence.
#
# RED HERE NAMES ITS DEBTORS. An exact ratchet cannot be recaptured after the
# fact by a bystander in a busy tree -- the double build is longer than the
# interval between program commits, so three correct recaptures were each
# invalidated before they could be reviewed (2026-09-02). The rule is therefore
# that a commit touching `programs/*/src/**` carries its own baseline rows or
# says it leaves the gate red; `--repo` lets `run.sh` read the range back from
# the baseline's own recorded commit and print the commits that did neither. So
# a reader of a red frameguard tier learns WHO owes rows, not only that the
# frames disagree.
# ---------------------------------------------------------------------------
tier_frameguard() {
  say "frameguard -- exact per-function SBF frame ratchet"
  local build_root="$repo_root" archive_root=""
  if [ -n "$commit_rev" ]; then
    local resolved
    resolved="$(cd "$repo_root" && git rev-parse --verify "$commit_rev^{commit}" 2>/dev/null)" || {
      record frameguard $EXIT_PREREQ_MISSING "--commit $commit_rev does not name a commit"
      return
    }
    archive_root="${DCLUTCH_CI_BUILD_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-frame-src.XXXXXX")}"
    note "measuring COMMIT $resolved (clean git archive)"
    archive_revision "$resolved" "$archive_root"
    build_root="$archive_root"
  else
    note "measuring the working tree; use --commit HEAD for a quoteable ratchet run"
  fi

  local dir="$build_root/tools/frameguard"
  if [ ! -f "$dir/run.sh" ] || [ ! -f "$dir/frameguard.py" ] \
      || [ ! -f "$dir/test_frameguard.py" ] || [ ! -f "$dir/test-runner.sh" ]; then
    record frameguard $EXIT_PREREQ_MISSING "frameguard or its refusal tests are absent from the measured tree"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi
  if ! have python3; then
    record frameguard $EXIT_PREREQ_MISSING "python3 not on PATH"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi

  if ! (cd "$build_root" && python3 "$dir/test_frameguard.py" && bash "$dir/test-runner.sh"); then
    record frameguard $EXIT_GATE_FAILED "the hermetic delta/diagnostic controls failed"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi

  # `--repo` is the LIVE repository even when the measured source is an
  # unpacked archive: the archive has no `.git`, and the attribution question
  # ("which commits since the baseline moved a frame") is about history, not
  # about the bytes being compiled.
  local code=0
  (cd "$build_root" && bash "$dir/run.sh" --source "$build_root" --repo "$repo_root") || code=$?
  case "$code" in
  0) record frameguard $EXIT_PASS ;;
  2) record frameguard $EXIT_PREREQ_MISSING "the exact frame measurement could not run (its exit 2)" ;;
  *) record frameguard $EXIT_GATE_FAILED "the per-function frame ratchet disagrees" ;;
  esac
  [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
}

tier_programs() {
  say "programs -- SBF build and the trading program-test suite"
  if ! have cargo-build-sbf; then
    note "cargo-build-sbf is not installed, so NO program was built and no"
    note "compute measurement was taken. Install the Solana/Agave toolchain:"
    note "    sh -c \"\$(curl -sSfL https://release.anza.xyz/stable/install)\""
    record programs $EXIT_PREREQ_MISSING "cargo-build-sbf not on PATH"
    return
  fi
  if [ ! -f "$repo_root/programs/dclutch-trading-sbf/program-test/Cargo.toml" ]; then
    record programs $EXIT_PREREQ_MISSING "the trading program-test is not in this tree"
    return
  fi

  # An SBF_OUT_DIR the caller can supply, so a CI job or a bisect can reuse a
  # completed build. Default is a fresh temporary directory, removed on exit --
  # never the checkout's target/deploy, which parallel lanes share and which
  # therefore holds ELFs of MIXED VINTAGE. Measuring compute across a mixed set
  # is measuring nothing: a one-byte ELF difference redraws every fixture seed.
  local elf_dir="${DCLUTCH_CI_SBF_OUT_DIR:-}" owned=""
  if [ -z "$elf_dir" ]; then
    elf_dir="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-elf.XXXXXX")"
    owned=1
  fi
  mkdir -p "$elf_dir"

  # Whose tree is being measured, said out loud before a single number is
  # produced, so no reader has to reconstruct it from context afterwards.
  local build_root="$repo_root" archive_root=""
  if [ -n "$commit_rev" ]; then
    local resolved
    resolved="$(cd "$repo_root" && git rev-parse --verify "$commit_rev^{commit}" 2>/dev/null)" || {
      record programs $EXIT_PREREQ_MISSING "--commit $commit_rev does not name a commit"
      [ -n "$owned" ] && rm -rf -- "$elf_dir"
      return
    }
    archive_root="${DCLUTCH_CI_BUILD_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-src.XXXXXX")}"
    note "measuring COMMIT $resolved (clean git archive)"
    archive_revision "$resolved" "$archive_root"
    build_root="$archive_root"
  else
    local dirty
    dirty="$(cd "$repo_root" && git status --porcelain -- programs crates 2>/dev/null | wc -l | tr -d ' ')"
    if [ "${dirty:-0}" != 0 ]; then
      note "measuring the WORKING TREE, and it has $dirty uncommitted files under"
      note "programs/ and crates/. On a shared tree those may belong to other"
      note "lanes, and the ELFs below are then of a revision nobody committed."
      note "For a number you intend to quote, re-run with --commit HEAD."
    else
      note "measuring the working tree (clean under programs/ and crates/)"
    fi
  fi

  # WHY THE BUILD OUTPUT IS READ AND NOT DISCARDED.
  #
  # `cargo build-sbf` exits ZERO when the SBF backend reports that a call
  # overwrites its own stack frame and "may cause undefined behavior during
  # execution". The ELF that comes out is well formed and every downstream
  # check passes on it, so the diagnostic is the only signal there is -- and
  # this loop used to send stdout to /dev/null, which meant the one gate that
  # builds the DEPLOYED role links could not see it even in principle.
  #
  # It went unseen on 2026-08-30: seven diagnostics on the shipped Trading
  # link, in `direct_replay_setup_v1::invoke_replay_child_v1`, introduced by an
  # eight-byte account widening that pushed a 4,088-byte frame to 4,096. The
  # accelerator links have had a hard gate since 2026-08-27
  # (programs/dclutch-trading-sbf/program-test/run-program-test.sh); the role
  # links had one nowhere, and a lane found these by reading build output on
  # the way past.
  #
  # The count is a detector, not a measurement -- it counts call sites inside
  # an already-over-bound function, so it says nothing until the wall is hit.
  # `tools/sbf-frame-sizes.py` measures the frames themselves and is what to
  # reach for after this refuses.
  local diagnostic_pattern='overwrites values in the frame'
  local manifest built_all=1 frame_diagnostics=0 link count
  for manifest in $PROGRAM_MANIFESTS; do
    link="$(basename "$(dirname "$manifest")")"
    note "build $link"
    if ! (cd "$build_root" && cargo build-sbf --manifest-path "$manifest" \
      --sbf-out-dir "$elf_dir" >"$elf_dir/build-$link.log" 2>&1); then
      note "BUILD FAILED: $manifest"
      tail -n 40 "$elf_dir/build-$link.log" >&2
      built_all=0
      break
    fi
    count="$(grep -c "$diagnostic_pattern" "$elf_dir/build-$link.log" || true)"
    if [ "${count:-0}" != 0 ]; then
      note "$link: $count SBF stack-frame-overwrite diagnostics"
      grep "$diagnostic_pattern" "$elf_dir/build-$link.log" | sort -u >&2
      frame_diagnostics=$((frame_diagnostics + count))
    fi
  done

  if [ "$built_all" = 0 ]; then
    # A build failure IS a gate failure, not a missing prerequisite: the
    # toolchain was present and this tree did not compile.
    record programs $EXIT_GATE_FAILED "an SBF program did not build"
    [ -n "$owned" ] && rm -rf -- "$elf_dir"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi

  if [ "$frame_diagnostics" != 0 ]; then
    note "REFUSING: $frame_diagnostics SBF stack-frame-overwrite diagnostics on"
    note "a link this gate builds. The toolchain says these calls may cause"
    note "undefined behavior during execution, so no measurement is taken on"
    note "top of them. Measure the frames with tools/sbf-frame-sizes.py."
    record programs $EXIT_GATE_FAILED \
      "$frame_diagnostics SBF stack-frame-overwrite diagnostics"
    [ -n "$owned" ] && rm -rf -- "$elf_dir"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi

  # The TEST runs from the same tree the ELFs came from, and that is not
  # pedantry. The gate's constant and the fixture harness that draws its keys
  # both live in this tree, so running a working-tree harness against archived
  # ELFs would compare one revision's threshold to another revision's route --
  # and produce a number belonging to neither.
  # THE THREE SKIPPED CASES, and why skipping them is the correct answer rather
  # than the convenient one.
  #
  # They are `registry_hot_continuation` rows that each stage an isolated child
  # ADVERSARY -- a corrupted Claims, Custody or Token program -- and prove
  # Trading refuses the exact post-child mismatch and rolls the whole
  # transaction back. They read `POSTJOIN_SBF_OUT_DIR` for those hostile ELFs.
  # This tier builds the real release set and has no hostile directory to give
  # them, and `POSTJOIN_SBF_OUT_DIR` appeared NOWHERE in this file, so all three
  # failed here on an unset variable while proving nothing -- the same shape as
  # the fee-leg probe above.
  #
  # Setting the variable is not the fix either. They exercise the Hot
  # CONTINUATION, which decision 0030 demoted to harness-only after HEAPRED
  # measured it +35,127 CU above the top-level route the public actually uses.
  # A demoted route must not gate the production tier: this tier's red means the
  # PUBLIC Direct route lost margin, and that sentence has to stay true.
  #
  # Their real home is `run-postjoin-hostiles.sh`, which builds both the real
  # set and the three adversaries and sets all three variables itself. It is now
  # a row of the `suites` tier, so these cases run -- they just run where their
  # prerequisites exist. If a fourth hostile case is added and not listed here,
  # it fails loudly in this tier rather than passing silently, which is the
  # right way for this list to go stale.
  local result=0
  (cd "$build_root" && SBF_OUT_DIR="$elf_dir" cargo test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    ${DCLUTCH_CI_PROGRAM_TESTS:+$DCLUTCH_CI_PROGRAM_TESTS} \
    -- --nocapture \
    --skip nonselected_claims_supply_corruption_after_real_child_commit_rolls_back \
    --skip omitted_token_close_authority_corruption_after_real_custody_commit_rolls_back \
    --skip omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back) || result=1

  [ -n "$owned" ] && rm -rf -- "$elf_dir"
  [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"

  if [ "$result" = 0 ]; then
    record programs $EXIT_PASS
  else
    note "If the failure names the compute margin gate: something in this"
    note "change, or in a shared contract crate it pulled in, made the public"
    note "Direct route more expensive. Find it before raising the gate's"
    note "constant -- raising it IS the act of spending margin, and there is"
    note "very little of it. Check the shared contract crates first."
    record programs $EXIT_GATE_FAILED
  fi
}

# ---------------------------------------------------------------------------
# journey -- does the journey campaign compile AND do its host tests pass.
# Minutes, cargo only, no validator.
#
# THE CLASS THIS EXISTS FOR, and it is not hypothetical: on 2026-08-30 this
# binary had not built on main for about two days, and nobody knew.
#
# It is built to break that way ON PURPOSE. The tier-1 producer's modules are
# compiled into it VERBATIM by `#[path]` out of
# tools/local-validator/bootstrap/successor/src/, so the journey does not fork
# the founding and cannot drift into a mirror of it. Its own Cargo.toml says
# the resulting fragility "is the intended tripwire". That is a good design and
# it has exactly one requirement: SOMETHING HAS TO PULL THE TRIPWIRE. Nothing
# did, so a deliberate alarm rang into an empty room for two days.
#
# WHY NOT A CAMPAIGN RUN, said plainly because the cheaper thing is the one
# that gets to run often. A full journey campaign needs a real
# `solana-test-validator`, stages a whole founding through open, and is tens of
# minutes -- that belongs to the cut, not to a push.
#
# WHY `cargo test` AND NOT `cargo check`, which is what this was until
# 2026-09-01 and why it changed. The reasoning above is right about the
# CAMPAIGN and was wrong about the crate's own TESTS, which need no validator
# and cost seconds on a build this tier already pays for. Two defects hid
# behind the difference, both of them host tests:
#
#   - `run-journey.sh` called `demo-market`, a subcommand the binary now
#     refuses unconditionally, so the tier could not run at all. It still
#     COMPILED, because the subcommand is still dispatched.
#   - `general_market::tests::the_neutral_seam_derives_the_entry_the_general_
#     publication_authors` asserted seven General actions while General had
#     grown to fifteen: 281 passed, 1 FAILED, behind a green check.
#
# Neither is reachable by a type-check, and a job named "the journey campaign
# compiles" was true and useless throughout. Compiling was never the property
# anyone wanted from this tier.
#
# `--bins` keeps it to the campaign binary's own tests, so this stays a push-
# affordable gate. It still does NOT tell you the campaign PASSES against a
# chain; that claim needs a validator and belongs to the cut. What it now tells
# you is that the binary compiles and everything it asserts about itself holds.
# `--commit` REACHES THIS TIER, and the author needed teaching twice.
#
# I built the archive machinery below for the `programs` tier and framed it as
# a COMPUTE concern -- a CU number off a shared tree is worthless. Then I ran
# this tier, watched it go red on a real-looking compile error, and was about
# to report a live breakage. It was another lane's uncommitted mid-edit state:
# nineteen files dirty under crates/ and programs/ at the time, zero an hour
# later, and the commit I suspected of fixing it was already an ancestor of the
# revision I had "measured".
#
# So the lesson generalises past compute, and this is the corrected form of it:
# ON A SHARED WORKING TREE, ANY TIER THAT COMPILES IS A TIER THAT NEEDS A
# REVISION. A red that belongs to a colleague's half-written file is worse than
# no gate, because it is a gate that cries wolf at whoever runs it next.
tier_journey() {
  say "journey -- the journey campaign, and every other tool workspace, compiles"
  if ! have cargo; then
    record journey $EXIT_PREREQ_MISSING "cargo not on PATH"
    return
  fi

  local root="$repo_root" archive_root=""
  if [ -n "$commit_rev" ]; then
    local resolved
    resolved="$(cd "$repo_root" && git rev-parse --verify "$commit_rev^{commit}" 2>/dev/null)" || {
      record journey $EXIT_PREREQ_MISSING "--commit $commit_rev does not name a commit"
      return
    }
    archive_root="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-journey.XXXXXX")"
    note "checking COMMIT $resolved (clean git archive)"
    archive_revision "$resolved" "$archive_root"
    root="$archive_root"
  else
    local dirty
    dirty="$(cd "$repo_root" && git status --porcelain -- programs crates tools 2>/dev/null | wc -l | tr -d ' ')"
    if [ "${dirty:-0}" != 0 ]; then
      note "checking the WORKING TREE, and it has $dirty uncommitted files."
      note "A compile error here may belong to a neighbouring lane rather than"
      note "to this revision. For a red you intend to REPORT, use --commit."
    fi
  fi

  local manifest="$root/tools/gauntlet/journey/Cargo.toml"
  if [ ! -f "$manifest" ]; then
    record journey $EXIT_PREREQ_MISSING "the journey tier is not in this tree"
    [ -n "$archive_root" ] && rm -rf -- "$archive_root"
    return
  fi
  # Its own `[workspace]` table, so it resolves independently of the protocol
  # workspace and gets its own target directory whether we ask or not.
  local code=0
  (cd "$root" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
    cargo test --manifest-path "$manifest" --bins) || code=$?
  if [ "$code" != 0 ]; then
    note "The journey campaign does not compile, or one of its host tests"
    note "failed. If it is a compile error, most often this is the"
    note "\`#[path]\` tripwire doing its job: a module under"
    note "tools/local-validator/bootstrap/successor/src/ moved or changed"
    note "shape, and the journey links those files verbatim rather than"
    note "copying them. Fix the journey to match its upstream -- do NOT fork"
    note "the module, which is the exact mirror this arrangement prevents."
  fi

  # THE OTHER TOOL WORKSPACES, for the reason the journey itself is here.
  #
  # The journey is not the only campaign that links successor modules by
  # `#[path]`, but it was the only one any tier compiled -- so the tripwire
  # protected exactly one of its dependants. tools/gauntlet/relayed-vertical
  # links the same bootstrap modules and rotted unnoticed: its Cargo.toml never
  # caught up with the dependencies 8a64178a added, and by 2026-09-02 it did not
  # build at all. Nothing was red, because nothing built it.
  #
  # DISCOVERED, NEVER LISTED. `find` locates every Cargo.toml under
  # tools/gauntlet and tools/local-validator that carries its own `[workspace]`
  # table, minus the journey checked above. A workspace added tomorrow is
  # checked tomorrow; a hardcoded row here would be this file's signature
  # defect, a value duplicated instead of read.
  #
  # `cargo check`, not `cargo test`: this is the compiles-at-all gate the
  # journey row already is, extended to its siblings. What each campaign
  # ASSERTS is its own runner's business and mostly needs a validator.
  local -a rotted=() declined=()
  local tool_manifest tool_code tool_log
  tool_log="$(mktemp "${TMPDIR:-/tmp}/dclutch-ci-toolws.XXXXXX")"
  while IFS= read -r tool_manifest; do
    [ -n "$tool_manifest" ] || continue
    [ "$tool_manifest" = "tools/gauntlet/journey/Cargo.toml" ] && continue
    tool_code=0
    (cd "$root" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
      cargo check --manifest-path "$tool_manifest" 2>&1 | tee "$tool_log") ||
      tool_code=${PIPESTATUS[0]}
    if [ "$tool_code" != 0 ]; then
      # A crate that fails through its OWN `compile_error!` is not rotted; it
      # is refusing a question this gate asked badly. tools/gauntlet/aot-cu
      # carries three mutually exclusive evaluator features and says "select
      # exactly one", so a featureless check cannot succeed and never could.
      # Guessing a feature here would make this file the author of a choice the
      # crate owns. So it is reported as NOT CHECKED, which is a 2 and becomes
      # a failure under --require -- the distinction this whole script exists
      # to keep: "could not be measured" is not "measured and fine".
      if grep -q 'compile_error!' "$tool_log"; then
        declined+=("$tool_manifest")
      else
        rotted+=("$tool_manifest")
        code=1
      fi
    fi
  done < <(cd "$root" && find tools/gauntlet tools/local-validator \
    -name Cargo.toml -not -path '*/target/*' 2>/dev/null | sort |
    while IFS= read -r candidate; do
      grep -q '^\[workspace\]' "$candidate" && printf '%s\n' "$candidate"
    done)

  rm -f -- "$tool_log"
  [ -n "$archive_root" ] && rm -rf -- "$archive_root"

  if [ "${#rotted[@]}" -gt 0 ]; then
    note "These tool workspaces do not compile:"
    printf '      %s\n' "${rotted[@]}"
    note "A campaign nothing builds is a campaign that has already stopped"
    note "being true; that is why they are checked here rather than trusted."
  fi
  if [ "${#declined[@]}" -gt 0 ]; then
    note "NOT CHECKED -- these decline a featureless \`cargo check\` through"
    note "their own \`compile_error!\`, which is a crate demanding a choice,"
    note "not a broken one. Nothing is claimed about them either way:"
    printf '      %s\n' "${declined[@]}"
  fi

  if [ "$code" != 0 ]; then
    record journey $EXIT_GATE_FAILED
  elif [ "${#declined[@]}" -gt 0 ]; then
    record journey $EXIT_PREREQ_MISSING \
      "${#declined[@]} tool workspace(s) declined a featureless check"
  else
    record journey $EXIT_PASS
  fi
}

# ---------------------------------------------------------------------------
# suites -- the SBF program-test suites that are not the trading one.
#
# EACH ROW NAMES A RUNNER, NEVER AN ELF LIST. That is the whole design of this
# tier. Every one of these suites needs a DIFFERENT set of built programs --
# core wants registry, rent, custody and a series-consume caller; custody wants
# its own test caller; claims wants a resolution-proof link and a token fixture
# -- and each of those sets is already written down, correctly and by the lane
# that owns it, inside the runner script beside the suite.
#
# Restating those lists here would be this project's signature defect with a
# fresh coat of paint: a value duplicated instead of read, agreeing right up
# until the day somebody adds a program to their runner and not to my table.
# So this tier discovers nothing and asserts nothing about ELFs. It runs the
# script the suite's owner maintains, and reports what it said.
#
# WHAT THESE RUNNERS DO NOT DO, named as inherited debt rather than papered
# over, because a reader will otherwise assume this tier is as strict as
# `programs`:
#
#   - They build the WORKING TREE. None of them takes a revision, so `--commit`
#     cannot reach them and this tier says so instead of pretending. For a
#     pass/fail compile-and-run answer that is tolerable in a way it is NOT for
#     a compute number; it is still worth fixing at the runners.
#   - They do not carry the SBF stack-frame-overwrite refusal that `programs`
#     and the accelerator links have. A frame diagnostic on one of these links
#     would exit zero here. That is owed by the runners, and it is the same
#     hole ee3dbe8f closed in two other places.
#     (`tools/gauntlet/dealer-checkpoint` and `tools/gauntlet/claims-extended`
#     DO refuse on it; the four rows below are the cheaper per-suite runners,
#     which do not.)
#
# WHAT EACH ROW COSTS AND NEEDS, from the runners themselves:
#
#   custody  three SBF links, its own test caller. The cheapest row.
#   core     six links, and now 5 of 5 targets. The gap this comment used to
#            NAME (five targets in programs/dclutch-core-sbf/tests/, three
#            driven; capability_close_alias and retirement_replay_handoff run
#            by nothing at all) was closed on 2026-08-30 by the runner rather
#            than by a list here: it globs its own tests/ directory, so a
#            sixth target is run the day it lands and cannot rot unrun. Both
#            orphans were GREEN at the first real run -- but between them they
#            carried fifteen hostile assertions that named no refusal code,
#            which is the shape 67e96e5b caught passing for the wrong reason;
#            all fifteen now assert an exact code. The sixth link is Trading:
#            capability_close_alias closes through the real Core-to-Trading
#            native-close route.
#   claims   needs an AUDITED Token-2022 v11 fixture, digest-checked against
#            programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance.
#            The runner builds it from the cargo registry's
#            spl-token-2022-11.0.0.crate, so a host with no populated registry
#            cannot run this row -- and that is a MISSING PREREQUISITE, not a
#            claims defect. It is why this tier reports absence per row.
#   dealer   six links, about three minutes cold. This row has the strongest
#            history for a gate: its campaign was uncompilable for days once,
#            and the retired tools/gauntlet/dealer family test was red from
#            2026-08-27 until 33a61576, both times because a release-path change
#            touched seven programs and zero campaigns. This row has always
#            driven the SHIPPED link (the accelerator's program-test); the
#            gauntlet evidence tier that drove the unshipped dclutch-dealer-sbf
#            was retired 2026-09-02 in favour of tools/gauntlet/dealer-checkpoint.
#   registry the release-set successor declaration on a real Registry ELF.
#            Added 2026-08-31, having run in NO tier since it was written: the
#            campaign that red-proofed d6e43b11's consent geometry was reachable
#            only by someone typing its runner path by hand, which is the
#            "unrun gate is not a passing gate" shape this tier exists to close.
#            It is also the gate for the 7-to-8 and 8-to-9 declarations the
#            cohort-9 cut must land.
#
# NOT A ROW, and the reason is a cost rather than a judgement: the SUCCESSOR
# BOOTSTRAP has no runner script, needs a real solana-test-validator, and its
# founding is about thirteen minutes with NO resume. That is a cut-tier
# campaign, not a push-tier suite. Its HOST tests
# (`cargo test --manifest-path tools/local-validator/bootstrap/successor/`)
# are ordinary and would fit here; they are simply not wired yet.
#
# SCRATCH DISCIPLINE, because this tier can start several of these at once:
# every runner builds into its own `mktemp -d` and traps EXIT/HUP/INT/TERM, so
# a clean exit cleans up. A killed run does NOT -- each leaks 3-7 GB, and
# /tmp/dclutch-* reached 373 GB and filled the volume once. If you interrupt
# this tier, check /tmp yourself.
readonly SUITE_RUNNERS="\
custody|programs/dclutch-custody-sbf/run-program-test.sh|Custody vault routes against a real caller link
core|programs/dclutch-core-sbf/run-open-market-program-test.sh|every core program-test target, discovered from tests/
claims|programs/dclutch-claims-sbf/run-rational-representation-v2-program-test.sh|the rational representation V2 lowering
dealer|programs/dclutch-dealer-accelerator-sbf/program-test/run-program-test.sh|the dealer accelerator link and its family tests
registry|programs/dclutch-registry-sbf/run-lineage-program-test.sh|the release-set successor declaration and the walk that follows the hop
fee2tx|programs/dclutch-trading-sbf/program-test/run-fee-second-transaction.sh|the Direct fee leg in a transaction of its own, against real Custody
postjoin|programs/dclutch-trading-sbf/program-test/run-postjoin-hostiles.sh|Trading refuses three isolated child adversaries and rolls the whole transaction back"

tier_suites() {
  say "suites -- the other SBF program-test suites"
  if ! have cargo-build-sbf; then
    note "cargo-build-sbf is not installed, so NO suite ran and no program was"
    note "built. Install the Solana/Agave toolchain:"
    note "    sh -c \"\$(curl -sSfL https://release.anza.xyz/stable/install)\""
    record suites $EXIT_PREREQ_MISSING "cargo-build-sbf not on PATH"
    return
  fi
  if [ -n "$commit_rev" ]; then
    note "NOTE: --commit does not reach this tier. These runners build the"
    note "working tree and take no revision; see the comment above. The"
    note "suites below measured whatever is on disk."
  fi

  local wanted="${DCLUTCH_CI_SUITES:-}"
  local row name script what present=0 failed=0 absent="" unrun="" row_code=0
  local IFS_SAVE="$IFS"
  while IFS='|' read -r name script what; do
    [ -n "$name" ] || continue
    if [ -n "$wanted" ]; then
      case " $wanted " in *" $name "*) ;; *) continue ;; esac
    fi
    if [ ! -x "$repo_root/$script" ]; then
      note "$name: runner absent ($script)"
      absent="$absent $name"
      continue
    fi
    present=$((present + 1))
    note "$name -- $what"
    # A row exiting 2 means IT could not run -- the same convention this whole
    # script uses, now honoured per row instead of only for the whole tier.
    # Before this, every nonzero row exit became a gate failure, so a host
    # without the claims fixture's builder archive produced "an SBF
    # program-test suite failed ... treat this as a real finding about the
    # protocol". The comment above SUITE_RUNNERS already claimed this tier
    # "reports absence per row"; it did not, because the rows had no way to say
    # it. The wrapper's own missing-prerequisite branch was unreachable for the
    # exact case its author wrote it for.
    row_code=0
    (cd "$repo_root" && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
      "$repo_root/$script") || row_code=$?
    case "$row_code" in
    0) ;;
    "$EXIT_PREREQ_MISSING")
      note "$name: DID NOT RUN -- the row reports a missing prerequisite"
      unrun="$unrun $name"
      ;;
    *)
      note "$name: FAILED"
      failed=$((failed + 1))
      ;;
    esac
  done <<EOF
$SUITE_RUNNERS
EOF
  IFS="$IFS_SAVE"

  [ -n "$absent" ] && note "runners not in this tree:$absent"
  [ -n "$unrun" ] && note "rows that did not run:$unrun"
  # A real failure outranks an absence: if one row genuinely failed, the tier
  # failed, and the rows that never ran are reported beside it rather than
  # softening it.
  if [ "$present" = 0 ]; then
    record suites $EXIT_PREREQ_MISSING "no suite runner is present in this tree"
  elif [ "$failed" -gt 0 ]; then
    record suites $EXIT_GATE_FAILED "$failed of $present suites failed"
  elif [ -n "$unrun" ]; then
    record suites $EXIT_PREREQ_MISSING "rows did not run:$unrun"
  else
    record suites $EXIT_PASS
  fi
}

# ---------------------------------------------------------------------------
# locks -- does every tracked Cargo workspace's lockfile still RESOLVE.
#
# The cheap half of `workspaces`, split out because the expensive half is not on
# `all` and this class of defect is therefore caught only at a cut. It is caught
# late and it is caught expensively: on 2026-09-02 THREE separate checked release
# candidates refused on a stale lock, each after eight minutes of building
# everything -- `b2ac8a79` for cohort-11, the successor workspace in `9c5e039a`,
# and the root workspace in the commit that added this tier. Each was ONE LINE:
# a lane added a path dependency to a Cargo.toml and did not record it.
#
# `cargo metadata --locked --offline` compiles NOTHING, so this is 70 workspaces
# in under thirty seconds -- measured 28.7s on the author's laptop, against the
# many minutes `workspaces` needs. That is cheap enough to run on every push,
# which is the whole argument for splitting it out.
#
# One root failure CASCADES: a member crate resolves through its workspace root,
# so a stale root lock reports every member stale too. Fixing the root's single
# line cleared all fourteen rows in the run that motivated this. The report says
# so, because fourteen rows reads as fourteen problems.
#
# WHAT IT DOES NOT CATCH, stated so the green is not read as more than it is.
# `--manifest-path` on a crate that is a MEMBER of an outer workspace resolves
# that outer workspace, so a member carrying its own `Cargo.lock` has that lock
# checked only insofar as the root's is: breaking one directly leaves this tier
# green, which I measured before shipping it. Discovering true workspace roots
# from their `[workspace]` tables is what `check-all-workspaces.py` does, and it
# stays the cut tier for that reason. This one is the cheap net under the root
# and the genuine standalone roots, not a replacement for it.
tier_locks() {
  say "locks -- every tracked Cargo workspace lock resolves"
  if ! have cargo; then
    record locks $EXIT_PREREQ_MISSING "cargo not on PATH"
    return
  fi
  local stale=() manifest dir
  # EVERY row runs and every row is reported; no early stop.
  while IFS= read -r manifest; do
    dir="$(dirname "$manifest")"
    [ -f "$dir/Cargo.toml" ] || continue
    if ! (cd "$repo_root" && env -u CARGO_TARGET_DIR cargo metadata \
        --locked --offline --format-version 1 \
        --manifest-path "$dir/Cargo.toml" >/dev/null 2>&1); then
      stale+=("${dir#"$repo_root"/}")
    fi
  done < <(find "$repo_root" -name Cargo.lock -not -path '*/target/*' | sort)
  if [ "${#stale[@]}" != 0 ]; then
    note "These workspaces have a lockfile that no longer resolves:"
    local row
    for row in "${stale[@]}"; do note "    $row"; done
    note "A member resolves through its workspace ROOT, so if '.' is listed the"
    note "others may be its cascade: fix the root lock and re-run before reading"
    note "this as ${#stale[@]} separate problems."
    note "Each is one \`cargo metadata --offline\` in a clean worktree, committed"
    note "with the manifest change that needed it."
    record locks $EXIT_GATE_FAILED "${#stale[@]} workspace lockfile(s) do not resolve under --locked"
    return
  fi
  record locks $EXIT_PASS
}

# workspaces -- does EVERY tracked Cargo workspace still check. The cut tier.
#
# The root workspace is not an inventory of this repository: program-test,
# fixture, generator and operator trees carry their own `[workspace]` tables on
# purpose, so a `cargo check` at the root is silent about most of them. That is
# the general shape of the journey break above, and this repository already had
# the right tool for it -- `tools/release/check-all-workspaces.py`, which
# discovers every workspace from an ARCHIVED revision, gives each a fresh
# target directory, and proves no Cargo invocation moved a lockfile.
#
# It had NO CALLERS ANYWHERE. A fifth gate that existed and never ran.
#
# It is genuinely expensive -- a cold locked/offline check of every workspace
# with no shared target directory -- so it is not in `all` and does not belong
# on a push. It belongs to the cut and to the nightly schedule, which is where
# the wrapper puts it.
tier_workspaces() {
  say "workspaces -- every tracked Cargo workspace checks"
  local tool="$repo_root/tools/release/check-all-workspaces.py"
  if [ ! -f "$tool" ]; then
    record workspaces $EXIT_PREREQ_MISSING "check-all-workspaces.py is not in this tree"
    return
  fi
  if ! have cargo; then
    record workspaces $EXIT_PREREQ_MISSING "cargo not on PATH"
    return
  fi
  # --work must not already exist, which is the tool's own guard against
  # reporting a previous run's artifacts as this run's evidence.
  local work
  work="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-ws.XXXXXX")/run"
  local code=0
  (cd "$repo_root" && python3 "$tool" --work "$work" \
    --commit "${commit_rev:-HEAD}") || code=$?
  rm -rf -- "$(dirname "$work")"
  if [ "$code" = 0 ]; then
    record workspaces $EXIT_PASS
  else
    note "A tracked Cargo workspace does not check at this revision, or a"
    note "Cargo invocation moved a lockfile inside the archive. The root"
    note "workspace passing says nothing about the others -- that is why"
    note "this tier exists."
    record workspaces $EXIT_GATE_FAILED
  fi
}

# ---------------------------------------------------------------------------
# release -- the release-tooling refusal suites, which ran nowhere.
#
# tools/release/ holds the machinery that decides whether a build may be
# released at all: SBF build-freshness admission, the devnet activity and
# demo-pulse wrappers, and the sponsored-market-open stager. Each of those
# carries a test script sitting directly beside it. NOTHING RAN ANY OF THEM.
# That is the same defect as `check-all-workspaces.py` above -- a gate that
# exists and never runs -- except four times over.
#
# They are the cheapest thing in this file: about five seconds for all four,
# needing bash, python3 and git and nothing else. Despite three of them having
# "devnet" in the name NONE of them reaches a chain. Each builds a scratch
# sandbox, writes stub `solana`, `solana-keygen`, `spl-token` and `dclutch`
# executables onto PATH, and points the tool under test at
# `https://example.invalid` so that a real fetch would fail loudly rather than
# quietly succeed. That is why this is a push tier and not a cut tier.
#
# WHAT THEY ACTUALLY GATE IS REFUSALS -- the cases where the release tooling
# has to say no. Stale or forged build evidence must not be admitted. A market
# must not be founded at a nonzero Direct fee rate, which founds a market that
# can never trade, nor against a founder key nobody holds, which strands
# collateral forever. Both are irreversible. A refusal test that never runs is
# indistinguishable from a tool that has quietly stopped refusing.
#
# ONE HONEST LIMIT, because it changes what a green here means: the stager
# suite reaches for real git history, to re-run its red controls against the
# last revision BEFORE its guards existed. That is the right design -- a
# control pinned to HEAD stops discriminating the moment the fix lands -- but
# on a shallow clone, or in a vendored subtree whose history does not carry
# that path, it prints its own note and two of its thirteen cases do not run.
# It reports that itself. This tier deliberately does not restate the count,
# because a second copy of it here would be one more number to get wrong.
# ---------------------------------------------------------------------------
# sbfcontracts -- every non-program first-party crate, compiled for the target
# it actually ships to. Needs cargo-build-sbf's toolchain.
#
# THE HOLE THIS FILLS, and it is a hole with a measured casualty.
#
# The contract and codec crates are libraries, so nothing in this runner built
# them for `target_os = "solana"`. `programs` and `frameguard` build the
# thirteen SBF LINKS -- which reach these crates only through whatever surface
# the links happen to use -- and every other cargo tier checks the HOST target.
# A host `cargo check` on a crate destined for SBF is a check that cannot fail
# in the way that matters, because the whole point of the defect class is that
# the two targets compile different code.
#
# Measured 2026-09-01: `cargo check -p dclutch-direct-aot-v3-contract` was
# green on the host while `cargo build-sbf` on the same crate produced 176
# errors, every one of them in that crate's own `src/registered.rs`. The cause
# was one `#[cfg(not(target_os = "solana"))]` in
# `crates/dclutch-direct-codec/src/registered_fill_artifacts_v4.rs` hiding the
# generated register schema on the only target that ships. Host-green,
# SBF-impossible, and no gate anywhere disagreed.
#
# THE SET IS EVERY NON-PROGRAM FIRST-PARTY CRATE REACHABLE FROM A PROGRAM, and
# it is derived here rather than listed. Measured from `cargo metadata` over
# all thirteen program manifests, that set is currently all 88 of them -- so a
# hand-written list would be a second author for a question the dependency
# graph already answers, and would drift the moment a crate joined. Deriving it
# also means the gate can never be quietly narrowed to the crates that already
# pass, which is the failure mode this tier exists to prevent.
# ---------------------------------------------------------------------------
tier_sbfcontracts() {
  say "sbfcontracts -- non-program crates built for target_os=solana"
  if ! have cargo-build-sbf; then
    note "cargo-build-sbf is not installed, so NO crate was compiled for the"
    note "target it ships to. Install the Solana/Agave toolchain:"
    note "    sh -c \"\$(curl -sSfL https://release.anza.xyz/stable/install)\""
    record sbfcontracts $EXIT_PREREQ_MISSING "cargo-build-sbf not on PATH"
    return
  fi
  if ! have python3; then
    record sbfcontracts $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi

  local build_root="$repo_root" archive_root=""
  if [ -n "$commit_rev" ]; then
    local resolved
    resolved="$(cd "$repo_root" && git rev-parse --verify "$commit_rev^{commit}" 2>/dev/null)" || {
      record sbfcontracts $EXIT_PREREQ_MISSING "--commit $commit_rev does not name a commit"
      return
    }
    archive_root="${DCLUTCH_CI_BUILD_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-sbfc-src.XXXXXX")}"
    note "measuring COMMIT $resolved (clean git archive)"
    archive_revision "$resolved" "$archive_root"
    build_root="$archive_root"
  else
    note "measuring the working tree; use --commit HEAD for a quoteable run"
  fi

  # Derived, never listed. An empty set is a BROKEN DERIVATION, not a clean
  # tree, so it refuses rather than reporting a vacuous pass.
  local packages
  packages="$(cd "$build_root" && python3 - <<'PY'
import json, pathlib, subprocess, sys
root = pathlib.Path.cwd()
reach = set()
manifests = sorted(root.glob("programs/*/Cargo.toml"))
if not manifests:
    sys.exit(3)
for m in manifests:
    r = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(m)],
        capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(4)
    md = json.loads(r.stdout)
    pkgs = {p["id"]: p for p in md["packages"]}
    nodes = {n["id"]: n for n in md["resolve"]["nodes"]}
    rootid = md["resolve"].get("root")
    stack = [rootid] if rootid else [
        p["id"] for p in md["packages"] if p["manifest_path"] == str(m)]
    seen = set()
    while stack:
        cur = stack.pop()
        if cur in seen or cur not in nodes:
            continue
        seen.add(cur)
        for dep in nodes[cur]["deps"]:
            # NORMAL edges only. A dev- or build-dependency never reaches an ELF,
            # and following those is what made an earlier version of this gate
            # demand `getrandom` compile for SBF -- a red that said nothing.
            if any(k.get("kind") is None for k in dep.get("dep_kinds", [])):
                stack.append(dep["pkg"])
    for pid in seen:
        p = pkgs.get(pid)
        if not p:
            continue
        path = p.get("manifest_path", "")
        if path.startswith(str(root)) and "/programs/" not in path \
                and p["name"].startswith("dclutch-"):
            reach.add(p["name"])
# Plus every ROOT-WORKSPACE crate that declares it knows about this target.
# A crate carrying `check-cfg = ['cfg(target_os, values("solana"))']` is stating
# that it compiles differently on SBF, which is precisely the claim this gate
# exists to test -- and the crate whose 176 errors motivated the gate is
# reachable only as a dev-dependency, so the shipped closure alone would miss it.
r = subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"],
                   capture_output=True, text=True)
if r.returncode == 0:
    for p in json.loads(r.stdout)["packages"]:
        mp = pathlib.Path(p["manifest_path"])
        if "/programs/" in str(mp) or not p["name"].startswith("dclutch-"):
            continue
        try:
            if 'cfg(target_os, values("solana"))' in mp.read_text():
                reach.add(p["name"])
        except OSError:
            pass
if not reach:
    sys.exit(5)
print("\n".join(sorted(reach)))
PY
  )" || {
    note "the crate set could not be derived from cargo metadata. That is a"
    note "prerequisite failure, never an empty pass -- a gate over zero crates"
    note "is not a green gate."
    record sbfcontracts $EXIT_PREREQ_MISSING "could not derive the SBF-reachable crate set"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  }

  local count
  count="$(printf '%s\n' "$packages" | grep -c .)"
  note "$count crates: the programs' normal-dependency closure, plus every
         root-workspace crate declaring solana-target awareness"

  local args=() name
  while IFS= read -r name; do
    [ -n "$name" ] && args+=(-p "$name")
  done <<< "$packages"

  # `cargo build-sbf` is the wrong instrument for LIBRARY crates: it compiles
  # correctly and then its post-processing looks for a `.so` a lib never emits
  # and exits 1 anyway. Measured: 52 crates, zero compile errors, exit 1. A gate
  # that reports failure on a clean tree is worse than no gate.
  #
  # So this checks the same target without the link step. The toolchain name is
  # DISCOVERED rather than pinned -- `1.89.0-sbpf-solana-v1.53` today -- because
  # a hardcoded one silently stops matching on the next platform-tools bump and
  # the gate would quietly become a prerequisite-missing that nobody reads.
  local sbf_toolchain
  sbf_toolchain="$(rustup toolchain list 2>/dev/null | awk '/sbpf-solana/{print $1; exit}')"
  if [ -z "$sbf_toolchain" ]; then
    note "cargo-build-sbf is present but its platform-tools rustup toolchain is"
    note "not installed, so nothing was compiled for the SBF target. Run any"
    note "\`cargo build-sbf\` once to provision it."
    record sbfcontracts $EXIT_PREREQ_MISSING "no sbpf-solana rustup toolchain"
    [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
    return
  fi
  note "toolchain $sbf_toolchain, target sbpf-solana-solana"

  local code=0
  (cd "$build_root" && cargo "+$sbf_toolchain" check --locked --offline \
      --target sbpf-solana-solana "${args[@]}") || code=$?
  if [ "$code" != 0 ]; then
    note "a crate that ships to SBF does not COMPILE for target_os=solana."
    note "A host-green \`cargo check\` proves nothing about this: the targets"
    note "compile different code, which is the whole defect class. Look for a"
    note "\`#[cfg(not(target_os = \"solana\"))]\` hiding a surface the crate"
    note "still uses. Do NOT narrow this gate to the crates that already pass."
    record sbfcontracts $EXIT_GATE_FAILED "$count crates requested; the SBF build failed"
  else
    record sbfcontracts $EXIT_PASS
  fi
  [ -n "$archive_root" ] && [ -z "${DCLUTCH_CI_BUILD_ROOT:-}" ] && rm -rf -- "$archive_root"
}

# ---------------------------------------------------------------------------
# sbom -- the dependency/licence closure, ~3 minutes, needs a populated cargo
# registry.
#
# WHY THIS TIER HAD TO BE WRITTEN, and it is the same defect twice.
#
# C-14 wants SBOM/licences to reproduce on supported builders. The instrument
# exists and passes: 59 manifests, 2,151 unique rows, zero failures, zero
# unresolvable. It regenerates byte-identically. It was also, until this tier,
# gated NOWHERE in this tree -- and two separate places said otherwise.
#
# `tools/sbom/SBOM.md` said the drift check was "also wired into
# tools/gauntlet". `grep -rn sbom tools/gauntlet/` returns nothing at all, and
# the README that line points the reader to never mentions CI either. Second,
# `apps/dclutch-web/lib/sbomVerify.test.ts` IS a real automatic gate on
# `npm test` -- and the `web` tier below excludes it by name, on the stated
# grounds that "it is gated in the wrapper's hygiene job". That wrapper is a
# different repository observing a VENDORED SNAPSHOT, which this file's opening
# paragraph already names as the wrong place for the answer. So the one gate
# that ran was switched off in the one runner that defines what runs.
#
# The exclusion in `web` is kept -- it is a real cost and prerequisite
# difference, not an unwelcome assertion -- and this tier is where that
# assertion goes instead, with the registry prerequisite it actually needs and
# a 2 rather than a fake green when the registry is absent.
#
# Two things run: the classification-logic unit tests (hermetic, stdlib-only,
# no registry) and then `--verify`, which writes nothing and exits 1 on drift
# or on any classification failure. The unit tests run FIRST and separately so
# that "the checker is broken" and "this tree has a licence defect" cannot
# arrive as the same number.
# ---------------------------------------------------------------------------
tier_sbom() {
  say "sbom -- dependency and licence closure"
  local tool="$repo_root/tools/sbom/sbom_check.py"
  local unit="$repo_root/tools/sbom/test_sbom_check.py"
  if [ ! -f "$tool" ]; then
    note "tools/sbom/sbom_check.py is not in this tree"
    record sbom $EXIT_PREREQ_MISSING "sbom_check absent from this tree"
    return
  fi
  if ! have python3; then
    record sbom $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi
  if ! have cargo; then
    note "the closure resolves every tracked Cargo workspace with"
    note "\`cargo metadata --locked --offline\`, so with no cargo NOTHING was"
    note "checked. This says nothing about whether the closure would pass."
    record sbom $EXIT_PREREQ_MISSING "cargo not on PATH"
    return
  fi
  if [ -f "$unit" ]; then
    if ! (cd "$repo_root/tools/sbom" && python3 "$unit" >/dev/null 2>&1); then
      note "the classification-logic tests failed, so the CHECKER is suspect."
      note "Fix those before reading anything into the closure below."
      record sbom $EXIT_GATE_FAILED "sbom_check's own classification tests failed"
      return
    fi
  else
    note "tools/sbom/test_sbom_check.py is absent, so the checker ran with no"
    note "control of its own."
  fi
  # WHICH TREE THE CLOSURE MEASURED, and on this gate it is not a detail.
  #
  # `sbom_check.py` reads each workspace's Cargo.lock OFF DISK. On this shared
  # checkout that is a dozen lanes' uncommitted files, so the local answer and
  # the CI answer are answers to different questions -- and they have already
  # disagreed. Measured 2026-09-01: the working tree said `manifests=59
  # unresolvable=0 drift=False PASS`, while the SAME checker in a clean
  # worktree at the SAME commit said `manifests=58 unresolvable=1 drift=True
  # STOP`. A lockfile that only exists uncommitted made the gate green for
  # whoever held it and red for everybody else. That is not a flaky gate, it is
  # a gate measuring the wrong object.
  #
  # So `--commit` is honoured here exactly as it is by frameguard, and for the
  # same stated reason: any number you intend to QUOTE comes from a clean tree.
  # `git archive` is not usable -- the closure discovers manifests with `git
  # ls-files`, which needs a real checkout -- so this uses a detached worktree
  # and removes it afterwards.
  local measure_root="$repo_root" worktree_root=""
  if [ -n "$commit_rev" ]; then
    local resolved
    resolved="$(cd "$repo_root" && git rev-parse --verify "$commit_rev^{commit}" 2>/dev/null)" || {
      record sbom $EXIT_PREREQ_MISSING "--commit $commit_rev does not name a commit"
      return
    }
    worktree_root="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-ci-sbom-src.XXXXXX")"
    rm -rf -- "$worktree_root"
    if ! (cd "$repo_root" && git worktree add --detach "$worktree_root" "$resolved" >/dev/null 2>&1); then
      record sbom $EXIT_PREREQ_MISSING "could not create a clean worktree at $resolved"
      return
    fi
    note "measuring COMMIT $resolved (clean detached worktree)"
    measure_root="$worktree_root"
  else
    note "measuring the WORKING TREE, which on this checkout is several lanes'"
    note "uncommitted files. Use --commit HEAD for a quoteable answer -- an"
    note "uncommitted lockfile makes this gate green only for whoever holds it."
  fi

  local code=0
  (cd "$measure_root" && python3 "$measure_root/tools/sbom/sbom_check.py" --verify >/dev/null) || code=$?
  if [ -n "$worktree_root" ]; then
    (cd "$repo_root" && git worktree remove --force "$worktree_root" >/dev/null 2>&1) \
      || rm -rf -- "$worktree_root"
  fi
  case "$code" in
  0) record sbom $EXIT_PASS ;;
  1)
    note "SBOM drift, or a dependency this repository cannot license-classify."
    note "Rerun it directly to see the offending rows -- and against the same"
    note "tree this tier just measured, or you will chase a disagreement that"
    note "is only your working copy:"
    note "    python3 tools/sbom/sbom_check.py --verify"
    note "A git-sourced or checksum-less dependency is a FAILURE by design."
    note "Do not weaken a version constraint to make the closure proceed."
    record sbom $EXIT_GATE_FAILED
    ;;
  *)
    note "the closure could not run -- most often a Cargo workspace whose lock"
    note "cannot resolve offline, which is a stale-lockfile defect wearing an"
    note "SBOM costume. Check \`cargo metadata --locked --offline\` first."
    record sbom $EXIT_PREREQ_MISSING "sbom_check exited $code"
    ;;
  esac
}

tier_release() {
  say "release -- the release-tooling refusal suites"
  local dir="$repo_root/tools/release"
  # Named one by one and never globbed. A glob turns a script that was DELETED
  # into a tier that silently got smaller, which is a quieter version of the
  # exact defect this tier exists to end.
  local scripts=(
    test-checked-release-freshness.sh
    test-devnet-activity.sh
    test-devnet-demo-pulse.sh
    test-stage-devnet-sponsored-market-open.sh
  )
  local present=() missing=() name
  for name in "${scripts[@]}"; do
    if [ -f "$dir/$name" ]; then
      present+=("$name")
    else
      missing+=("$name")
    fi
  done
  if [ "${#present[@]}" = 0 ]; then
    record release $EXIT_PREREQ_MISSING "tools/release test scripts are not in this tree"
    return
  fi
  if ! have python3; then
    record release $EXIT_PREREQ_MISSING "python3 not on PATH"
    return
  fi
  # The PYTHON refusal suites, which until now ran NOWHERE. Measured
  # 2026-09-01: `test_preflight.py` was 12 failures + 3 errors at HEAD and had
  # been for some time, because this tier ran only the four shell scripts above
  # -- red where nothing looks, the same shape as a tier that does not exist.
  #
  # Each is invoked with cwd = its OWN directory and the repo root on
  # PYTHONPATH, because these suites do not share one import convention:
  # `test_dryplan.py` does a sibling `import dryplan` and needs its directory,
  # while `test_rehearsal.py` does `from tools.release...` and needs the repo
  # root. Running them all one way reports a false red on the other half, which
  # is a defect this runner has already paid for once today.
  local py_suites=(
    private-validator-lifecycle/test_preflight.py
    private-validator-lifecycle/test_chaos.py
    private_validator_upgrade/test_rehearsal.py
    devnet-flight/test_devnet_flight.py
    devnet_upgrade_dryplan/test_dryplan.py
    lifecycle-chaos/test_lifecycle_chaos.py
    test_usage_parity.py
  )
  local py_present=() py_missing=() suite
  for suite in "${py_suites[@]}"; do
    if [ -f "$dir/$suite" ]; then py_present+=("$suite"); else py_missing+=("$suite"); fi
  done

  local failed=() code=0
  for name in "${present[@]}"; do
    code=0
    (cd "$repo_root" && bash "$dir/$name") || code=$?
    [ "$code" = 0 ] || failed+=("$name")
  done
  # EVERY row runs and EVERY row is reported. A `set -e`-style early stop here
  # would report one failure where the true figure is several -- measured in
  # this tree on 2026-09-01, one reported against ten real.
  for suite in "${py_present[@]}"; do
    code=0
    note "python: $suite"
    (cd "$dir/$(dirname "$suite")" && PYTHONPATH="$repo_root" python3 "$(basename "$suite")") \
      || code=$?
    [ "$code" = 0 ] || failed+=("$suite")
  done
  if [ "${#py_missing[@]}" -gt 0 ]; then
    for suite in "${py_missing[@]}"; do missing+=("$suite"); done
  fi

  # The usage/parser parity GATE itself, distinct from its tests above. A tool
  # that teaches a flag its parser rejects is a runbook for a command that
  # cannot run, which is C-13's "runbooks contain only commands actually
  # replayed" one layer down -- and nothing goes red when it drifts.
  local parity="$dir/usage_parity.py"
  local successor="$repo_root/tools/local-validator/bootstrap/successor/src"
  if [ -f "$parity" ] && [ -d "$successor" ]; then
    note "usage/parser parity"
    code=0
    (cd "$repo_root" && python3 "$parity" --crate-src "$successor") || code=$?
    case "$code" in
    0) ;;
    2)
      note "the parity checker could not run; nothing was proven either way"
      missing+=("usage_parity.py could not run")
      ;;
    *) failed+=("usage_parity.py") ;;
    esac
  else
    missing+=("usage_parity.py or the successor crate")
  fi
  if [ "${#failed[@]}" -gt 0 ]; then
    note "a release-tooling refusal suite FAILED:"
    for name in "${failed[@]}"; do note "  $name"; done
    note "Each of these proves the release machinery still says no to something"
    note "irreversible -- forged build evidence, a market founded at a fee rate"
    note "that can never trade, a founder key nobody holds. Read the failing"
    note "case before you change either side of it."
    record release $EXIT_GATE_FAILED
    return
  fi
  if [ "${#missing[@]}" -gt 0 ]; then
    # Everything present passed, and that is still not a pass for this tier:
    # some of it was not here to run. Under --require this becomes a failure,
    # which is the correct behaviour for a release candidate.
    note "these release suites are not in this tree and did NOT run:"
    for name in "${missing[@]}"; do note "  $name"; done
    record release $EXIT_PREREQ_MISSING "${#missing[@]} of $(( ${#scripts[@]} + ${#py_suites[@]} )) release suites absent from this tree"
    return
  fi
  record release $EXIT_PASS
}

# ---------------------------------------------------------------------------

list_tiers() {
  cat <<'EOF'
tier      cost         prerequisite       what it gates

census    milliseconds python3            a generated file arriving with no
                                          re-emit guard, or losing one
fmt       ~10s         cargo, rustfmt     rustfmt disagreeing with a file that
                                          is not in tools/ci/fmt-baseline.txt,
                                          or a baseline line that is no longer
                                          true. Root workspace only; the 56
                                          nested ones are owed
seam      ~20s         ast-grep           six structural seam defect classes,
                                          new findings against a triaged baseline
runbooks  seconds      python3            every command README.md, docs/guides
                                          and docs/operators publish, replayed
                                          as `--help`: the program exists, and
                                          it names the subcommand and flags the
                                          runbook passes it. An unprobed
                                          command is a 2, never a pass
release   ~5s          python3            the four release-tooling REFUSAL
                                          suites: build-freshness admission,
                                          the devnet activity and demo-pulse
                                          wrappers, the sponsored-market-open
                                          stager. All hermetic -- stub binaries
                                          and an invalid RPC, never a chain
sbfcontracts
          minutes      cargo-build-sbf    every non-program first-party crate
                                          compiled for target_os=solana, the
                                          target it actually ships to. A host
                                          `cargo check` cannot fail the way
                                          this does: measured 176 SBF errors on
                                          a crate that was host-green
sbom      ~3 min       cargo, python3     the dependency/licence closure over
                                          every tracked Cargo workspace and npm
                                          package tree: 59 manifests, 2,151
                                          rows. A git-sourced or checksum-less
                                          dependency, or drift in the committed
                                          SBOM/NOTICES, is a failure
web       ~1 min       node               the web + SDK vitest suites
emission  minutes      lake (Lean)        every generated file still byte-
                                          matches the emitter that printed it
frameguard minutes     cargo-build-sbf    every function in the exact twelve
                                          SBF links retains its admitted frame;
                                          catches growth below the 4,096 wall,
                                          and names the commits since the
                                          baseline's own recorded commit that
                                          moved program sources without
                                          carrying frame rows
journey   minutes      cargo              the journey campaign still COMPILES,
                                          and so does every other workspace
                                          under tools/gauntlet and
                                          tools/local-validator. Not that they
                                          pass -- a real campaign needs a
                                          validator and is tens of minutes, so
                                          that belongs to the cut. This catches
                                          the class that hid a two-day
                                          breakage: a `#[path]` module upstream
                                          moving out from under it -- exactly
                                          how relayed-vertical rotted, unbuilt
                                          by any tier and so never red. It was
                                          ~2 min when it built one workspace;
                                          each of the others has its own target
                                          directory and one exceeded 15 minutes
                                          cold on a loaded machine
programs  minutes      cargo-build-sbf    the programs build with no SBF stack-
                                          frame diagnostic, and the public
                                          Direct route holds its compute margin
                                          across 32 pinned seeds
suites    ~15 min      cargo-build-sbf    the other SBF program-test suites:
                                          custody, core, claims, dealer, plus the
                                          fee2tx and postjoin probes, which own
                                          the cases the programs tier cannot
                                          stage. Each row runs the runner its
                                          owning lane maintains, never a copy of
                                          its ELF list
workspaces  slow       cargo              EVERY tracked Cargo workspace checks
                                          from an archived revision. The general
                                          form of the journey break. Cut tier --
                                          fresh target dir per workspace, so it
                                          is not in `all`

aliases:  cheap = census fmt seam runbooks release
          all   = census fmt seam runbooks release sbom sbfcontracts web emission frameguard
                  journey programs suites
          (`workspaces` is deliberately outside `all` -- it is the cut tier)

environment:
  DCLUTCH_CI_SUITES="core custody"   run only those rows of the suites tier
  CARGO_BUILD_JOBS=4                 honoured by the cargo tiers (default 4)

options:
  --commit REV   build and test a clean `git archive` of REV instead of the
                 working tree. Use it for any number you intend to QUOTE: on
                 this shared tree the default measures whatever a dozen lanes
                 have half-written, and a compute figure from that looks
                 exactly as authoritative as a real one.
  --require      a missing prerequisite becomes a failure. For a release
                 candidate, where an unrun gate is not a passing gate.

Exit 0 all ran and passed, 1 a gate failed, 2 a prerequisite was missing and
nothing was proven (seam_audit.py's convention, adopted so there is one),
64 you typed something wrong. --require makes 2 into 1.
EOF
}

main() {
  local tiers=()
  while [ $# -gt 0 ]; do
    case "$1" in
    --require) require_mode=1 ;;
    --commit)
      shift
      [ $# -gt 0 ] || {
        printf 'tools/ci/run.sh: --commit needs a revision\n' >&2
        exit $EXIT_USAGE
      }
      commit_rev="$1"
      ;;
    --commit=*) commit_rev="${1#--commit=}" ;;
    --list | -l)
      list_tiers
      exit 0
      ;;
    -h | --help)
      # The prose, then the ONE tier table -- never a second copy of it.
      sed -n '2,62p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      list_tiers
      exit 0
      ;;
    cheap) tiers+=(census fmt locks seam runbooks release) ;;
    all) tiers+=(census fmt locks seam runbooks release sbom sbfcontracts web emission frameguard journey programs suites) ;;
    census | fmt | locks | seam | runbooks | release | sbom | sbfcontracts | web | emission | frameguard | journey | programs | suites | workspaces)
      tiers+=("$1")
      ;;
    *)
      printf 'tools/ci/run.sh: unknown tier %s\n\n' "$1" >&2
      list_tiers >&2
      exit $EXIT_USAGE
      ;;
    esac
    shift
  done

  if [ "${#tiers[@]}" = 0 ]; then
    list_tiers
    exit $EXIT_USAGE
  fi

  local tier
  for tier in "${tiers[@]}"; do
    "tier_$tier"
  done

  say "verdict"
  printf '%s\n' "${verdicts[@]}"
  case "$worst" in
  "$EXIT_PASS") printf '\nall requested tiers ran and passed\n' ;;
  "$EXIT_PREREQ_MISSING")
    printf '\nno gate failed, but a tier above DID NOT RUN. That is not a\n'
    printf 'passing tree, it is an unmeasured one. Exit %s.\n' "$EXIT_PREREQ_MISSING"
    ;;
  *) printf '\na gate failed. Exit %s.\n' "$EXIT_GATE_FAILED" ;;
  esac
  exit "$worst"
}

main "$@"
