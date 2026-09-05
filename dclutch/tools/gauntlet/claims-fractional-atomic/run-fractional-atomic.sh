#!/usr/bin/env bash
# Run the fractional-atomic ProgramTest campaign and fold it into the census.
#
# WHY THIS EXISTS. The campaign has passed against real ELFs since `8fdcdc56`
# and was invisible to the census the whole time: it took no
# `dclutch-program-test-evidence` dependency, so it called `record()` for
# nothing, emitted no evidence document, and no `bindings.json` could be
# corroborated against it. Its routes therefore read NEVER-EXECUTED in
# `docs/reference/routes.md` while its own suite was green -- which is the
# register reporting an absence that was really an instrument gap.
#
# The order here is the point and it is not negotiable: EMIT, then RUN, then
# FOLD, and only then author bindings against what the ledger OBSERVED. A
# binding is what flips a row to `witnessed` in a register that never consults
# the ledger (`tools/genref/generate.mjs` reads bindings alone), so a binding
# written from what a campaign OUGHT to touch manufactures exactly the false
# green this tier exists to remove.
#
#   tools/gauntlet/claims-fractional-atomic/run-fractional-atomic.sh
#
# Builds nothing on a chain, signs nothing, contacts no cluster.

set -eu

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/claims-fractional-atomic"
work="${DCLUTCH_FRACTIONAL_ATOMIC_WORK:-/private/tmp/dclutch-fractional-atomic-campaign}"
sbf_out="${SBF_OUT_DIR:-$work/sbf-out}"
out="${DCLUTCH_GAUNTLET_OUT:-/private/tmp/dclutch-gauntlet/out}"
inventory="$out/inventory.json"
ledger="$out/ledger.json"

mkdir -p "$work" "$sbf_out" "$out"
cd "$repo_root"

build() {
    if command -v swarm-build >/dev/null 2>&1; then
        swarm-build cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    else
        cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    fi
}

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame. An artifact the toolchain calls
# potentially-undefined has no business entering a campaign unnoticed.
diagnostics=0
for manifest in \
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-rent-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/fractional-compaction-caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-72s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "fractional-atomic: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to" >&2
    echo "  run a campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

# Token-2022 is a third-party artifact this tier does not build. `claims-extended`
# owns its provenance requirement; point TOKEN_2022_ELF at the same file.
if [ ! -f "$sbf_out/spl_token_2022.so" ]; then
    if [ -n "${TOKEN_2022_ELF:-}" ] && [ -f "${TOKEN_2022_ELF}" ]; then
        cp "${TOKEN_2022_ELF}" "$sbf_out/spl_token_2022.so"
    else
        echo "fractional-atomic: spl_token_2022.so is absent from $sbf_out." >&2
        echo "  Set TOKEN_2022_ELF to a matching artifact; see claims-extended's own" >&2
        echo "  host-provenance requirement for where it must come from." >&2
        exit 1
    fi
fi

evidence_dir="$work/evidence"
# A re-run must not accumulate records from a previous shape.
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: fractional-atomic (fractional_compaction)"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --manifest-path programs/dclutch-claims-sbf/program-test/fractional-atomic/Cargo.toml \
    --test fractional_compaction -- --test-threads=1

evidence="$work/fractional-atomic.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

if [ ! -f "$inventory" ]; then
    echo "fractional-atomic: no inventory at $inventory; produce one first with" >&2
    echo "  cargo run --release -p dclutch-route-census -- inventory --root . --out $inventory" >&2
    exit 1
fi

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "fractional-atomic: folded into $ledger"
echo "fractional-atomic: render the report with 'tools/gauntlet/run.sh --mode census'"
