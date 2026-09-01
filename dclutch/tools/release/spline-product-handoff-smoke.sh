#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --node ABS_NODE22 --cli ABS_DCLUTCH_MJS --successor ABS_SUCCESSOR --work ABS_NEW_DIR" >&2
  exit 2
}

node_bin=
cli_bin=
successor_bin=
work=
while [[ $# -gt 0 ]]; do
  flag=$1
  shift
  [[ $# -gt 0 ]] || usage
  value=$1
  shift
  case "$flag" in
    --node) node_bin=$value ;;
    --cli) cli_bin=$value ;;
    --successor) successor_bin=$value ;;
    --work) work=$value ;;
    *) usage ;;
  esac
done

for value in "$node_bin" "$cli_bin" "$successor_bin" "$work"; do
  [[ "$value" = /* ]] || usage
done
[[ -x "$node_bin" ]] || { echo "--node is not executable: $node_bin" >&2; exit 2; }
[[ -f "$cli_bin" ]] || { echo "--cli is not a file: $cli_bin" >&2; exit 2; }
[[ -x "$successor_bin" ]] || { echo "--successor is not executable: $successor_bin" >&2; exit 2; }
[[ ! -e "$work" ]] || { echo "--work already exists: $work" >&2; exit 2; }

"$node_bin" -e 'const [major, minor] = process.versions.node.split(".").map(Number); if (major < 22 || (major === 22 && minor < 13)) { console.error(`Node >=22.13.0 required; found ${process.versions.node}`); process.exit(2); }'

script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)
fixture="$repo_root/docs/operator/examples/spline-product-degree2.json"
verifier="$script_dir/verify-spline-product-handoff.mjs"
[[ -f "$fixture" ]] || { echo "missing canonical fixture: $fixture" >&2; exit 2; }
[[ -f "$verifier" ]] || { echo "missing smoke verifier: $verifier" >&2; exit 2; }

work_parent=$(dirname "$work")
canonical_parent=$(cd "$work_parent" && pwd)
[[ "$work_parent" = "$canonical_parent" ]] || { echo "--work parent is not canonical; expected $canonical_parent" >&2; exit 2; }

mkdir "$work"
"$node_bin" "$cli_bin" \
  --bootstrap-bin "$successor_bin" \
  --input "$fixture" \
  --output-dir "$work/product" \
  --json product spline > "$work/completion.json"
"$node_bin" "$cli_bin" \
  --report "$work/product/report.json" \
  --json product inspect > "$work/inspection.json"
"$node_bin" "$verifier" \
  "$fixture" "$work/product" "$work/completion.json" "$work/inspection.json" \
  > "$work/smoke-report.json"

sed -n '1p' "$work/smoke-report.json"
