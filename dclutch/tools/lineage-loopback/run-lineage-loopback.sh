#!/usr/bin/env bash
# Drive the successor-declaration caller against a real loopback validator.
#
# What this proves that the program test cannot: the CALLER's plumbing. The
# program test drives the instruction through the real ELF but builds its
# accounts in-process; everything between "an account exists on a cluster" and
# "the builder is handed an ObservedAccount" -- the finalized multi-account
# read, the vacant-address mapping, the keyless simulation's wire format, the
# signer set, and the read-back that compares the landed bytes to the projected
# ones -- only exists on this path.
#
# What it does NOT prove: that a real activation ladder produced these caches.
# They are composed offline and injected at genesis, which is legal because the
# declaration route authenticates a cache's owner, width, decodability and
# derived address and asks nothing about provenance. No cluster has a genesis
# anyone can write into. Cut-day evidence comes from devnet's real caches.
set -euo pipefail

# Resolved from this script's own location, not from `git rev-parse`: this tree
# is vendored as a `dclutch/` subtree inside another repository, where the
# toplevel is one directory up and every path would lose its `dclutch/` segment.
here="$(cd "$(dirname "$0")" && pwd)"
workspace="$(cd "$here/../.." && pwd)"
port="${LINEAGE_LOOPBACK_PORT:-21990}"
work="$(mktemp -d /tmp/dclutch-lineage-loopback.XXXXXX)"
validator=""

# Where the three evidence documents are kept after the ledger is swept.
#
# This run is the only thing in the repository that drives `DCLRLND1` on a real
# Agave runtime, and until this flag existed it threw its own evidence away with
# the ledger -- so a green run and a run that never happened left the same trace
# and `registry/lineage_v1::process` read as NEVER-EXECUTED while this script
# passed. `tools/gauntlet/lineage/run-lineage.sh` passes a directory and folds
# what lands here into the census. Nothing else about the run changes.
evidence_dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --evidence-dir) evidence_dir="${2:?--evidence-dir needs a directory}"; shift 2 ;;
        *) echo "run-lineage-loopback.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
if [ -n "$evidence_dir" ]; then
    case "$evidence_dir" in
        /*) ;;
        *) echo "run-lineage-loopback.sh: --evidence-dir must be absolute" >&2; exit 1 ;;
    esac
    mkdir -p "$evidence_dir"
fi

cleanup() {
  if [ -n "$validator" ] && kill -0 "$validator" 2>/dev/null; then
    kill "$validator" 2>/dev/null || true
    wait "$validator" 2>/dev/null || true
  fi
  # Not EXIT alone: a killed run leaks a ledger per invocation.
  rm -rf "$work"
}
trap cleanup EXIT HUP INT TERM

command -v solana-test-validator >/dev/null || {
  echo "lineage-loopback: solana-test-validator is not on PATH" >&2; exit 1; }

echo "== building the Registry link, the stager and the caller"
(cd "$workspace" && cargo build-sbf \
  --manifest-path programs/dclutch-registry-sbf/Cargo.toml \
  --sbf-out-dir "$work/elf") >"$work/build-sbf.log" 2>&1 \
  || { tail -n 60 "$work/build-sbf.log" >&2; exit 1; }
# One workspace, one target directory: both binaries land under the repository
# root, so they are built and named by package rather than by directory.
target="${CARGO_TARGET_DIR:-$workspace/target}"
(cd "$workspace" && cargo build --release -p dclutch-lineage-loopback) >"$work/build-stager.log" 2>&1 \
  || { tail -n 40 "$work/build-stager.log" >&2; exit 1; }
(cd "$workspace" && cargo build --release -p dclutch-local-successor-bootstrap) \
  >"$work/build-caller.log" 2>&1 || { tail -n 40 "$work/build-caller.log" >&2; exit 1; }

caller="$target/release/dclutch-local-successor-bootstrap"

echo "== staging two activation caches and the Registry into a genesis"
"$target/release/dclutch-lineage-loopback" \
  --work "$work" --registry-elf "$work/elf/dclutch_registry_sbf.so" >"$work/genesis.env"
cat "$work/genesis.env"
# shellcheck disable=SC1090
set -a; . "$work/genesis.env"; set +a

echo "== starting the loopback validator on :$port"
solana-test-validator \
  --config /dev/null \
  --ledger "$work/ledger" \
  --account-dir "$ACCOUNTS" \
  --mint "$PAYER" \
  --ticks-per-slot 16 \
  --bind-address 127.0.0.1 \
  --rpc-port "$port" \
  --faucet-port $((port + 2)) \
  --gossip-port $((port + 3)) \
  --dynamic-port-range $((port + 10))-$((port + 41)) \
  --reset --quiet >"$work/validator.log" 2>&1 &
validator=$!

for _ in $(seq 1 60); do
  if curl -s -X POST -H 'content-type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
      "http://127.0.0.1:$port" 2>/dev/null | grep -q '"result":"ok"'; then
    break
  fi
  sleep 1
done
curl -s -X POST -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "http://127.0.0.1:$port" \
  | grep -q '"result":"ok"' || { tail -n 40 "$work/validator.log" >&2; exit 1; }

echo
echo "== PREFLIGHT: builds, simulates, opens no key, sends nothing"
"$caller" local-private-validator-declare-successor-v1 \
  --rpc-url "http://127.0.0.1:$port" \
  --registry "$REGISTRY" \
  --predecessor "$PREDECESSOR" \
  --successor "$SUCCESSOR" \
  --evidence "$work/preflight.json" \
  --fee-payer-keypair "$PAYER_KEYPAIR"

grep -q '"landed": null' "$work/preflight.json" \
  || { echo "lineage-loopback: a preflight recorded a landing" >&2; exit 1; }
grep -q '"simulation_refusal": null' "$work/preflight.json" \
  || { echo "lineage-loopback: the cluster refused the frame in simulation" >&2;
       cat "$work/preflight.json" >&2; exit 1; }

echo
echo "== EXECUTE: one transaction, then read the record back off the chain"
export DCLUTCH_LINEAGE_AUTHORITY="$AUTHORITY_KEYPAIR"
"$caller" local-private-validator-declare-successor-v1 \
  --rpc-url "http://127.0.0.1:$port" \
  --registry "$REGISTRY" \
  --predecessor "$PREDECESSOR" \
  --successor "$SUCCESSOR" \
  --evidence "$work/execute.json" \
  --execute \
  --fee-payer-keypair "$PAYER_KEYPAIR" \
  --authority-keypair-env DCLUTCH_LINEAGE_AUTHORITY

grep -q '"signature"' "$work/execute.json" \
  || { echo "lineage-loopback: execute recorded no landing" >&2; exit 1; }

# The two runs projected the same record, because the record carries no clock.
preflight_record="$(grep '"projected_record"' "$work/preflight.json")"
execute_record="$(grep '"projected_record"' "$work/execute.json")"
[ "$preflight_record" = "$execute_record" ] \
  || { echo "lineage-loopback: the projected record changed between runs" >&2; exit 1; }

echo
echo "== REPLAY: lineage never forks, and the second attempt must find it taken"
if "$caller" local-private-validator-declare-successor-v1 \
    --rpc-url "http://127.0.0.1:$port" \
    --registry "$REGISTRY" \
    --predecessor "$PREDECESSOR" \
    --successor "$SUCCESSOR" \
    --evidence "$work/replay.json" \
    --fee-payer-keypair "$PAYER_KEYPAIR" >"$work/replay.log" 2>&1; then
  echo "lineage-loopback: a replay was admitted; the no-fork guarantee is gone" >&2
  exit 1
fi
grep -q 'LineageAlreadyDeclared' "$work/replay.log" \
  || { echo "lineage-loopback: the replay refused for the wrong reason" >&2;
       cat "$work/replay.log" >&2; exit 1; }
echo "replay refused: LineageAlreadyDeclared"

if [ -n "$evidence_dir" ]; then
    # Copied, not moved: the ledger sweep below must still find a complete work
    # directory, and a partial copy that a retry then skips is the failure mode
    # AGENTS.md names under "Distrust silent success". Each file is required to
    # arrive, so a missing one is a refusal here rather than an empty campaign
    # three steps later.
    for document in preflight execute replay; do
        if [ -f "$work/$document.json" ]; then
            cp "$work/$document.json" "$evidence_dir/$document.json"
        fi
    done
    [ -f "$evidence_dir/execute.json" ] \
      || { echo "lineage-loopback: no execute.json to keep" >&2; exit 1; }
    echo
    echo "== evidence kept in $evidence_dir"
fi

echo
echo "== lineage loopback: preflight, execute and replay all behaved"
