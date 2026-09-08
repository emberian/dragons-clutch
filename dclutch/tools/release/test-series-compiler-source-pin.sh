#!/usr/bin/env bash
# Execute the candidate's actual staging block. The generator is a sentinel:
# this tests source admission before that boundary, not generator acceptance.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
RUNNER="${1:-$HERE/checked-release-candidate.sh}"
REPO="$(git -C "$HERE" rev-parse --show-toplevel)"
git -C "$REPO" rev-parse --show-toplevel
git -C "$REPO" rev-parse HEAD
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-series-source-pin.XXXXXX")"
SCRATCH="$(cd "$SCRATCH" && pwd -P)"
trap 'rm -rf "$SCRATCH"' EXIT

# Read the production code, including its complete staging order. Do not mirror
# the comparison predicate in a fixture verifier or run the candidate's builds.
python3 - "$RUNNER" "$SCRATCH/staging.sh" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text()
blocks = []
for name in ("require_regular_canonical_series_shadow_input", "step_refused"):
    start = source.index(name + "() {\n")
    end = source.index("\n}\n", start) + 3
    blocks.append(source[start:end])
blocks.extend(line for line in source.splitlines() if line.startswith("sha256() {"))
start = source.index('if [ -n "$SERIES_SHADOW_GENERATED_INCLUDE" ]; then',
                     source.index('# A selected Series Shadow ELF'))
end = source.index("# Cargo's `--locked` refusal", start)
blocks.append(source[start:end])
Path(sys.argv[2]).write_text("\n".join(blocks))
PY

SOURCE_TREE="$SCRATCH/source-tree.txt"
git -C "$REPO" ls-tree -r --full-tree HEAD > "$SOURCE_TREE"

write_case() {
    local directory=$1 inventory=$2
    mkdir -p "$directory"
    cp "$inventory" "$directory/compiler.bin"
    printf 'source-pin boundary toolchain fixture\n' > "$directory/toolchain.bin"
    python3 - "$directory" <<'PY'
from pathlib import Path
import hashlib, json, sys
root = Path(sys.argv[1])
digest = hashlib.sha256((root / "compiler.bin").read_bytes()).hexdigest()
include = ("// staging fixture compiler digest: " + digest + "\n").encode()
(root / "include.rs").write_bytes(include)
# Both supplied witnesses are rehashed consistently after substitution. These
# are explicit staging fixtures; the sentinel does not accept their wire codec.
(root / "manifest.bin").write_text(json.dumps({
    "compiler_source_sha256": digest,
    "generated_include_sha256": hashlib.sha256(include).hexdigest(),
}, sort_keys=True, separators=(",", ":")) + "\n")
PY
}

run_case() (
    local directory=$1
    SOURCE="$REPO"
    HOST_TARGET="$directory/unused-target"
    SERIES_SHADOW_DIR="$directory/staged"
    SERIES_SHADOW_GENERATED_INCLUDE="$directory/include.rs"
    SERIES_SHADOW_SOURCE_MANIFEST="$directory/manifest.bin"
    SERIES_SHADOW_COMPILER_SOURCE="$directory/compiler.bin"
    SERIES_SHADOW_TOOLCHAIN_MANIFEST="$directory/toolchain.bin"
    SERIES_SHADOW_STAGED_INCLUDE="$SERIES_SHADOW_DIR/include.rs"
    SERIES_SHADOW_STAGED_SOURCE_MANIFEST="$SERIES_SHADOW_DIR/manifest.bin"
    SERIES_SHADOW_STAGED_COMPILER_SOURCE="$SERIES_SHADOW_DIR/compiler.bin"
    SERIES_SHADOW_STAGED_TOOLCHAIN_MANIFEST="$SERIES_SHADOW_DIR/toolchain.bin"
    SERIES_SHADOW_VERIFY_LOG="$directory/generator.log"
    cargo() {
        printf 'generator boundary reached\n' > "$directory/generator-reached"
        return 77
    }
    source "$SCRATCH/staging.sh"
)

write_case "$SCRATCH/canonical" "$SOURCE_TREE"
status=0
run_case "$SCRATCH/canonical" > "$SCRATCH/canonical.log" 2>&1 || status=$?
if [ "$status" -ne 77 ] || [ ! -f "$SCRATCH/canonical/generator-reached" ]; then
    cat "$SCRATCH/canonical.log" >&2
    echo 'canonical compiler inventory did not reach the generator boundary' >&2
    exit 1
fi
printf 'ok - canonical complete source inventory reaches the generator\n'

printf 'unrelated nonempty compiler inventory\n' > "$SCRATCH/unrelated.txt"
write_case "$SCRATCH/substitution" "$SCRATCH/unrelated.txt"
status=0
run_case "$SCRATCH/substitution" > "$SCRATCH/substitution.log" 2>&1 || status=$?
if [ "$status" -ne 1 ] \
    || ! grep -Fxq 'Series Shadow compiler source is not the pinned complete source inventory' \
        "$SCRATCH/substitution.log" \
    || [ -e "$SCRATCH/substitution/generator-reached" ]; then
    cat "$SCRATCH/substitution.log" >&2
    echo 'self-consistent unrelated inventory did not refuse before the generator' >&2
    exit 1
fi
printf 'ok - rehashed unrelated compiler inventory refuses before the generator\n'
