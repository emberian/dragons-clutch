#!/usr/bin/env bash
set -euo pipefail

# Real-ELF ProgramTest evidence. One build/test owner for both the suite gate
# and the census runner; target selection never substitutes for a validator life.
repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
package=dclutch-fractional-atomic-program-test
all_targets=(claims_founding claims_conservation fractional_atomic permissioned_burn_wall fractional_compaction escrow_pda_handover)
targets=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --test)
      [ "$#" -ge 2 ] || { echo "--test needs a target" >&2; exit 2; }
      valid=0
      for target in "${all_targets[@]}"; do
        if [ "$2" = "$target" ]; then valid=1; fi
      done
      [ "$valid" -eq 1 ] || { echo "unknown test target: $2" >&2; exit 2; }
      for target in "${targets[@]}"; do
        [ "$2" != "$target" ] || { echo "duplicate test target: $2" >&2; exit 2; }
      done
      targets+=("$2"); shift 2 ;;
    --list) printf '%s\n' "${all_targets[@]}"; exit 0 ;;
    --help|-h)
      echo "usage: $0 [--test TARGET ...] [--list]"
      echo "Runs selected targets serially, reports every result, refuses an empty execution."
      exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
if [ "${#targets[@]}" -eq 0 ]; then targets=("${all_targets[@]}"); fi
cd "$repo_root"
git rev-parse --show-toplevel
git rev-parse HEAD

# Preserve each attempt, including a failed build. Never fold stale evidence.
work="${DCLUTCH_FRACTIONAL_ATOMIC_WORK:-${TMPDIR:-/tmp}/dclutch-fractional-atomic}"
mkdir -p "$work"
run="$(mktemp -d "$work/run.XXXXXX")"
sbf_out="${SBF_OUT_DIR:-$run/sbf-out}"
mkdir -p "$sbf_out"
verdicts=(); codes=()
for target in "${targets[@]}"; do verdicts+=(never-ran); codes+=(-); done
report() {
  printf 'target\tverdict\texit\tlog\n' > "$run/results.tsv.tmp"
  for ((i=0; i<${#targets[@]}; i++)); do
    printf '%s\t%s\t%s\t%s\n' "${targets[$i]}" "${verdicts[$i]}" "${codes[$i]}" "$run/test-${targets[$i]}.log" >> "$run/results.tsv.tmp"
  done
  mv "$run/results.tsv.tmp" "$run/results.tsv"
  cat "$run/results.tsv"
}
trap report EXIT

# Token-2022 is the audited v11 fixture. The campaign's Token behaviour is only
# evidence if this is that exact artifact, so the digest is checked against the
# provenance rather than trusting whatever the caller points at.
provenance="programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance"
token_2022_so="${TOKEN_2022_SO:-}"
# EXIT 2 AND NOT 1 WHEN THE ARTIFACT IS SIMPLY NOT HERE. `tools/ci/run.sh`
# reads 1 as "this tree has the defect the gate detects" and 2 as "the gate
# could not run", and its `suites` tier honours that PER ROW. An absent fixture
# is the second thing: nothing was proven, either way. A WRONG digest below
# stays 1, because that is a real finding about what the caller pointed at.
if [[ -z "$token_2022_so" ]]; then
  echo "TOKEN_2022_SO is unset, so this suite DID NOT RUN. Build the audited" >&2
  echo "fixture once:" >&2
  echo "  programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh \\" >&2
  echo "    <spl-token-2022-11.0.0.crate> <output dir>" >&2
  echo "then point TOKEN_2022_SO at <output dir>/spl_token_2022.so" >&2
  exit 2
fi
if [[ ! -f "$token_2022_so" ]]; then
  echo "TOKEN_2022_SO does not exist, so this suite DID NOT RUN: $token_2022_so" >&2
  exit 2
fi
actual_token_sha="$(shasum -a 256 "$token_2022_so" | awk '{print $1}')"
canonical_token_sha="$(awk -F= '/^canonical_elf_sha256=/{print $2}' "$provenance")"
audit_token_sha="$(awk -F= '/^macos_arm64_audit_elf_sha256=/{print $2}' "$provenance")"
if [[ "$actual_token_sha" != "$canonical_token_sha" \
   && "$actual_token_sha" != "$audit_token_sha" ]]; then
  echo "TOKEN_2022_SO is not the audited v11 fixture." >&2
  echo "  saw       $actual_token_sha" >&2
  echo "  canonical $canonical_token_sha" >&2
  echo "  macos     $audit_token_sha" >&2
  exit 1
fi

run_build() {
  if command -v swarm-build >/dev/null 2>&1; then swarm-build "$@"; else "$@"; fi
}
# Check the touched workspace before the first SBF link.
run_build cargo check --locked -p "$package" --tests > "$run/check.log" 2>&1 \
  || { tail -40 "$run/check.log" >&2; exit 1; }
if grep -q 'Unit .* was already loaded' "$run/check.log"; then
  echo "scheduler ran no command; retry this campaign" >&2; exit 2
fi

manifests=(
  programs/dclutch-claims-sbf/Cargo.toml
  programs/dclutch-registry-sbf/Cargo.toml
  programs/dclutch-core-sbf/Cargo.toml
  programs/dclutch-custody-sbf/Cargo.toml
  programs/dclutch-rent-sbf/Cargo.toml
  programs/dclutch-claims-sbf/test-programs/fractional-atomic-caller/Cargo.toml
  programs/dclutch-claims-sbf/test-programs/claim-check-escrow-signer/Cargo.toml
  programs/dclutch-claims-sbf/test-programs/fractional-compaction-caller/Cargo.toml
  programs/dclutch-claims-sbf/test-programs/founding-caller/Cargo.toml
)
for manifest in "${manifests[@]}"; do
  log="$run/build-$(basename "$(dirname "$manifest")").log"
  run_build cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out" -- --locked > "$log" 2>&1 \
    || { tail -40 "$log" >&2; exit 1; }
  if grep -q 'Unit .* was already loaded' "$log"; then
    echo "scheduler ran no build: $manifest; retry this campaign" >&2; exit 2
  fi
  if grep -Eq 'overwrites values in the frame|overflows the maximum allowed frame space' "$log"; then
    echo "SBF frame diagnostics: $log; no campaign ran" >&2; exit 1
  fi
done
cp "$token_2022_so" "$sbf_out/spl_token_2022.so"
shasum -a 256 "$sbf_out"/*.so > "$run/elf-sha256.txt"
failed=0
for ((i=0; i<${#targets[@]}; i++)); do
  target="${targets[$i]}"
  log="$run/test-$target.log"
  status=0
  SBF_OUT_DIR="$sbf_out" run_build cargo test --locked -p "$package" --test "$target" \
    -- --nocapture --test-threads=1 > "$log" 2>&1 || status=$?
  codes[$i]="$status"
  verdicts[$i]=failed
  if grep -q 'Unit .* was already loaded' "$log"; then
    verdicts[$i]=never-ran; failed=1
  elif [ "$status" -eq 0 ] && grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed;' "$log"; then
    verdicts[$i]=passed
  else
    failed=1
    tail -40 "$log" >&2
  fi
  printf '%s: %s (exit %s)\n' "$target" "${verdicts[$i]}" "$status"
done
exit "$failed"
