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
#   tools/ci/run.sh census            the generated-file ratchet (milliseconds)
#   tools/ci/run.sh seam              the seam register (~20s, needs ast-grep)
#   tools/ci/run.sh web               web + SDK vitest suites (needs node)
#   tools/ci/run.sh emission          Lean byte-identity guards (needs lake)
#   tools/ci/run.sh frameguard        exact per-function SBF frame ratchet
#   tools/ci/run.sh programs          SBF build + trading program-test (minutes)
#   tools/ci/run.sh cheap             census + seam
#   tools/ci/run.sh --list            the table, with costs and prerequisites
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
# frameguard -- all thirteen SBF links, complete per-function frame map.
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

  local code=0
  (cd "$build_root" && bash "$dir/run.sh" --source "$build_root") || code=$?
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
# journey -- does the journey campaign still COMPILE. Minutes, cargo only.
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
# WHY `cargo check` AND NOT A CAMPAIGN RUN, said plainly because the cheaper
# thing is the one that gets to run often. A full journey campaign needs a real
# `solana-test-validator`, stages a whole founding through open, and is tens of
# minutes -- that belongs to the cut, not to a push. `cargo check` catches the
# entire two-day class (a moved or reshaped upstream module) for the price of a
# type-check, and it is the only part of the journey that a push can afford. It
# does NOT tell you the campaign still passes; those are different claims and
# this tier only makes the first one.
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
  say "journey -- the journey campaign still compiles"
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
    cargo check --manifest-path "$manifest") || code=$?
  [ -n "$archive_root" ] && rm -rf -- "$archive_root"
  if [ "$code" = 0 ]; then
    record journey $EXIT_PASS
  else
    note "The journey campaign does not compile. Most often this is the"
    note "\`#[path]\` tripwire doing its job: a module under"
    note "tools/local-validator/bootstrap/successor/src/ moved or changed"
    note "shape, and the journey links those files verbatim rather than"
    note "copying them. Fix the journey to match its upstream -- do NOT fork"
    note "the module, which is the exact mirror this arrangement prevents."
    record journey $EXIT_GATE_FAILED
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
#     hole ee3dbe8f closed in two other places. (`tools/gauntlet/dealer` and
#     `tools/gauntlet/claims-extended` DO refuse on it; the four rows below are
#     the cheaper per-suite runners, which do not.)
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
#            and tools/gauntlet/dealer's family test was red from 2026-08-27
#            until 33a61576, both times because a release-path change touched
#            seven programs and zero campaigns.
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
  local failed=() code=0
  for name in "${present[@]}"; do
    code=0
    (cd "$repo_root" && bash "$dir/$name") || code=$?
    [ "$code" = 0 ] || failed+=("$name")
  done
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
    record release $EXIT_PREREQ_MISSING "${#missing[@]} of ${#scripts[@]} release suites absent from this tree"
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
seam      ~20s         ast-grep           six structural seam defect classes,
                                          new findings against a triaged baseline
release   ~5s          python3            the four release-tooling REFUSAL
                                          suites: build-freshness admission,
                                          the devnet activity and demo-pulse
                                          wrappers, the sponsored-market-open
                                          stager. All hermetic -- stub binaries
                                          and an invalid RPC, never a chain
web       ~1 min       node               the web + SDK vitest suites
emission  minutes      lake (Lean)        every generated file still byte-
                                          matches the emitter that printed it
frameguard minutes     cargo-build-sbf    every function in the exact thirteen
                                          SBF links retains its admitted frame;
                                          catches growth below the 4,096 wall
journey   ~2 min       cargo              the journey campaign still COMPILES.
                                          Not that it passes -- a real campaign
                                          needs a validator and is tens of
                                          minutes, so it belongs to the cut.
                                          This catches the class that hid a
                                          two-day breakage: a `#[path]` module
                                          upstream moving out from under it
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

aliases:  cheap = census seam release
          all   = census seam release web emission frameguard journey programs suites
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
      sed -n '2,45p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    cheap) tiers+=(census seam release) ;;
    all) tiers+=(census seam release web emission frameguard journey programs suites) ;;
    census | seam | release | web | emission | frameguard | journey | programs | suites | workspaces)
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
