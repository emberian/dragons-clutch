#!/bin/sh
# Both directions, because a checker that cannot fail is indistinguishable from
# a clean tree, and a tripwire that cannot fire is worse than none.
#
#   1. On this tree it still reports the citation it was built from.
#   2. On a synthetic tree it resolves what exists and refuses what does not,
#      and --check exits 1 on a citation absent from the baseline.
#
# The synthetic half runs in a temp directory on purpose: a shared checkout must
# not have a control injecting doc comments into another lane's file.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
tool="$root/tools/doc-citations/doc_citations.py"

out="$(python3 "$tool" --root "$root")"
printf '%s' "$out" | grep -q 'process_funded_transition' || {
    echo "control FAILED: the known dangling citation was not reported." >&2
    echo "Either it was fixed (update this control) or the tool stopped measuring." >&2
    exit 1
}
printf '%s' "$out" | grep -q 'declined is not passed' || {
    echo "control FAILED: the report no longer states what it declined." >&2
    exit 1
}
echo "control 1/3: still reports the citation it was built from"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/src"
cat > "$work/Cargo.toml" <<'TOML'
[package]
name = "synthetic-control"
TOML
cat > "$work/src/lib.rs" <<'RS'
/// Resolves: `holder::live_item`, `Shape::Variant`, `Shape::field`.
pub mod holder {
    /// A real item.
    pub fn live_item() {}
}
/// A shape with a variant and a field.
pub enum Shape { Variant }
/// A struct whose field is cited as `Carrier::field`.
pub struct Carrier { pub field: u8 }
/// Dangles: `holder::vanished_item`.
pub fn cite() {}
RS

report="$(python3 "$tool" --root "$work")"
printf '%s' "$report" | grep -q 'vanished_item' || {
    echo "control FAILED: a citation of a missing symbol was not reported." >&2
    exit 1
}
for resolved in live_item Variant field; do
    printf '%s' "$report" | grep -q "\`holder::$resolved\`\|\`Shape::$resolved\`\|\`Carrier::$resolved\`" && {
        echo "control FAILED: $resolved is declared and must not be reported." >&2
        exit 1
    }
done
echo "control 2/3: resolves items, enum variants and struct fields; refuses the absent one"

python3 "$tool" --root "$work" --baseline "$work/base.json" --write --quiet >/dev/null
cat >> "$work/src/lib.rs" <<'RS'
/// A new dangle: `holder::second_vanished_item`.
pub fn cite_again() {}
RS
if python3 "$tool" --root "$work" --baseline "$work/base.json" --check --quiet >/dev/null 2>&1; then
    echo "control FAILED: --check did not fire on a citation absent from the baseline." >&2
    exit 1
fi
echo "control 3/3: --check exits nonzero on a new dangling citation"
