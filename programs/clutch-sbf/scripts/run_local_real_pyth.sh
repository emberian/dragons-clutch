#!/usr/bin/env bash
# NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE.
#
# Builds a test-only Clutch ELF, injects exact captured loader-owned Pyth
# Program/ProgramData accounts into a fresh loopback validator, and drives the
# signed-RPC real-router -> real-receiver -> Clutch lifecycle. It never reads
# Solana CLI config or a user wallet.

set -euo pipefail
umask 077
unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY no_proxy
unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS CARGO_ENCODED_RUSTFLAGS

repo="$(cd "$(dirname "$0")/../../.." && pwd)"
crate="$repo/programs/clutch-sbf/local-real-pyth"
program_manifest="$repo/programs/clutch-sbf/program/Cargo.toml"

validator="${CLUTCH_LOOPBACK_TEST_VALIDATOR:-${SOLANA_TEST_VALIDATOR:-}}"
if [ -z "$validator" ]; then
  echo "FAIL: set CLUTCH_LOOPBACK_TEST_VALIDATOR (or SOLANA_TEST_VALIDATOR) to an explicitly selected loopback-patched solana-test-validator" >&2
  exit 1
fi
if [ ! -x "$validator" ]; then
  echo "FAIL: validator is not executable: $validator" >&2
  exit 1
fi
for command in awk cargo cargo-build-sbf cp curl git grep ln lsof mktemp rustc sed seq shasum tee tr; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "FAIL: required command is absent: $command" >&2
    exit 1
  }
done

source_paths=(
  programs/clutch-sbf/local-real-pyth
  programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh
  programs/clutch-sbf/scripts/run_local_real_pyth.sh
  programs/clutch-sbf/program
  programs/clutch-sbf/svm-tests
  programs/clutch-sbf/source-profiles/devnet-real-source-snapshot-2026-08-22.json
  programs/clutch-sbf/Cargo.lock
  programs/clutch-sbf/vendor
  programs/solana-layout
  programs/solana-reference
  crates
  research/batch-policy-identity
  tools/agave-loopback-validator
)
for source_path in "${source_paths[@]}"; do
  git -C "$repo" ls-files -- "$source_path" | grep -q . || {
    echo "FAIL: campaign source path is not tracked at HEAD: $source_path" >&2
    exit 1
  }
done
git -C "$repo" diff --quiet --no-ext-diff -- "${source_paths[@]}" || {
  echo "FAIL: campaign source paths have unstaged changes" >&2
  exit 1
}
git -C "$repo" diff --cached --quiet --no-ext-diff -- "${source_paths[@]}" || {
  echo "FAIL: campaign source paths have staged changes" >&2
  exit 1
}
untracked_source_paths="$(
  git -C "$repo" ls-files --others --exclude-standard -- "${source_paths[@]}"
)"
if [ -n "$untracked_source_paths" ]; then
  echo "FAIL: campaign source paths contain untracked build inputs:" >&2
  echo "$untracked_source_paths" >&2
  exit 1
fi
repository_head="$(git -C "$repo" rev-parse --verify 'HEAD^{commit}')"
case "$repository_head" in
  [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]) ;;
  *) echo "FAIL: git did not return a full lowercase 40-hex HEAD" >&2; exit 1 ;;
esac

host_cargo=(cargo +1.93.1)
host_rustc=(rustc +1.93.1)
expected_cargo='cargo 1.93.1 (083ac5135 2025-12-15)'
expected_rustc='rustc 1.93.1 (01f6ddf75 2026-02-11)'
expected_sbf=$'cargo-build-sbf 4.0.0\nplatform-tools v1.53\nrustc 1.89.0'
[ "$("${host_cargo[@]}" --version)" = "$expected_cargo" ] || {
  echo "FAIL: cargo version differs from $expected_cargo" >&2
  exit 1
}
[ "$("${host_rustc[@]}" --version)" = "$expected_rustc" ] || {
  echo "FAIL: rustc version differs from $expected_rustc" >&2
  exit 1
}
[ "$(cargo-build-sbf --version)" = "$expected_sbf" ] || {
  echo "FAIL: cargo-build-sbf/platform-tools/SBF Rust version differs" >&2
  exit 1
}
sbf_builder="$(command -v cargo-build-sbf)"
[ "$(shasum -a 256 "$sbf_builder" | awk '{print $1}')" = \
  '37c37d1a2ef0aa44065cde8c6ad07f0685bcef24699b4a9dd101372d7d4ef6e7' ] || {
  echo "FAIL: cargo-build-sbf binary hash differs" >&2
  exit 1
}
host_lock="$crate/Cargo.lock"
sbf_lock="$repo/programs/clutch-sbf/Cargo.lock"
host_lock_sha='1734d5b8834363f4dc9f72ce88a8ff86614a1b67df092b76ad5cae5ede4029ad'
sbf_lock_sha='ebe9451a7f6e72bd1e5747c21d44df152742e15f69cfdbda30e7415ed9dda0f0'
[ "$(shasum -a 256 "$host_lock" | awk '{print $1}')" = "$host_lock_sha" ] || {
  echo "FAIL: local-real Cargo.lock hash differs" >&2
  exit 1
}
[ "$(shasum -a 256 "$sbf_lock" | awk '{print $1}')" = "$sbf_lock_sha" ] || {
  echo "FAIL: Clutch SBF Cargo.lock hash differs" >&2
  exit 1
}

rpc_port="${CLUTCH_LOCAL_REAL_PYTH_RPC_PORT:-18537}"
faucet_port="${CLUTCH_LOCAL_REAL_PYTH_FAUCET_PORT:-18539}"
gossip_port="${CLUTCH_LOCAL_REAL_PYTH_GOSSIP_PORT:-18540}"
dynamic_port_range="${CLUTCH_LOCAL_REAL_PYTH_DYNAMIC_PORT_RANGE:-18541-18640}"
campaign_mode="${CLUTCH_LOCAL_REAL_PYTH_CAMPAIGN_MODE:-source-only-v1}"
case "$campaign_mode" in
  source-only-v1|joined-user-lifecycle-v1) ;;
  *) echo "FAIL: unknown CLUTCH_LOCAL_REAL_PYTH_CAMPAIGN_MODE=$campaign_mode" >&2; exit 1 ;;
esac
for named in "$rpc_port" "$faucet_port" "$gossip_port"; do
  case "$named" in
    ''|*[!0-9]*) echo "FAIL: every configured port must be decimal" >&2; exit 1 ;;
  esac
  if [ "$named" -lt 1024 ] || [ "$named" -gt 65535 ]; then
    echo "FAIL: configured port $named is outside 1024..65535" >&2
    exit 1
  fi
done
if [ "$rpc_port" -eq 65535 ]; then
  echo "FAIL: RPC port 65535 has no representable RPC WebSocket port" >&2
  exit 1
fi
ws_port=$((rpc_port + 1))
if [ "$rpc_port" -eq "$faucet_port" ] || [ "$ws_port" -eq "$faucet_port" ] || \
   [ "$rpc_port" -eq "$gossip_port" ] || [ "$ws_port" -eq "$gossip_port" ] || \
   [ "$faucet_port" -eq "$gossip_port" ]; then
  echo "FAIL: RPC, WebSocket, faucet, and gossip ports must be distinct" >&2
  exit 1
fi
dynamic_start="${dynamic_port_range%%-*}"
dynamic_end="${dynamic_port_range#*-}"
case "$dynamic_start" in
  ''|*[!0-9]*) echo "FAIL: dynamic range start must be decimal" >&2; exit 1 ;;
esac
case "$dynamic_end" in
  ''|*[!0-9]*) echo "FAIL: dynamic range end must be decimal" >&2; exit 1 ;;
esac
if [ "$dynamic_start" -lt 1024 ] || [ "$dynamic_end" -gt 65535 ] || \
   [ "$dynamic_start" -gt "$dynamic_end" ]; then
  echo "FAIL: dynamic port range is outside 1024..65535 or reversed" >&2
  exit 1
fi
for named in "$rpc_port" "$ws_port" "$faucet_port" "$gossip_port"; do
  if [ "$named" -ge "$dynamic_start" ] && [ "$named" -le "$dynamic_end" ]; then
    echo "FAIL: explicit port $named collides with dynamic range $dynamic_port_range" >&2
    exit 1
  fi
done

url="http://127.0.0.1:$rpc_port"
work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-local-real-pyth.XXXXXX")"
validator_pid=""
keep="${CLUTCH_LOCAL_REAL_PYTH_KEEP_WORK:-0}"

stop_validator() {
  if [ -n "$validator_pid" ] && kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    for _ in $(seq 1 50); do
      kill -0 "$validator_pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$validator_pid" 2>/dev/null; then
      kill -KILL "$validator_pid" 2>/dev/null || true
    fi
    wait "$validator_pid" 2>/dev/null || true
  fi
  validator_pid=""
}

wait_ready() {
  local log="$1"
  local ready=0
  for _ in $(seq 1 90); do
    if ! kill -0 "$validator_pid" 2>/dev/null; then
      break
    fi
    if curl -q -fsS --noproxy '*' --proxy '' --max-time 2 \
      -H 'Content-Type: application/json' -X POST \
      --data-binary '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$url" \
      2>/dev/null | grep -q '"result":"ok"'; then
      ready=1
      break
    fi
    sleep 1
  done
  if [ "$ready" != 1 ]; then
    echo "FAIL: validator never became healthy; log follows" >&2
    tail -120 "$log" >&2 || true
    return 1
  fi
}

cleanup() {
  stop_validator
  case "$work" in
    "${TMPDIR:-/tmp}"/clutch-local-real-pyth.*)
      if [ "$keep" = 1 ]; then
        echo "WARNING: retained NON-PRODUCTION lab directory and ephemeral secrets at $work" >&2
      else
        rm -rf -- "$work"
      fi
      ;;
    *) echo "WARNING: refusing to remove unexpected work path $work" >&2 ;;
  esac
}
trap cleanup EXIT INT TERM

if curl -q -fsS --noproxy '*' --proxy '' --max-time 1 \
  -H 'Content-Type: application/json' -X POST \
  --data-binary '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$url" \
  2>/dev/null | grep -q '"result":"ok"'; then
  echo "FAIL: $url was already serving; refusing a possibly unrelated ledger" >&2
  exit 1
fi
for port in "$rpc_port" "$ws_port" "$faucet_port" "$gossip_port"; do
  if lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | grep -q . || \
     lsof -nP -iUDP:"$port" 2>/dev/null | grep -q .; then
    echo "FAIL: configured port $port is already in use" >&2
    exit 1
  fi
done

echo "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE"
echo "campaign_mode=$campaign_mode"
vendor="$repo/.cache/clutch-local-real-pyth/vendor"
if [ ! -d "$vendor" ]; then
  echo "== seed exact locked dependency source offline =="
  mkdir -p "$(dirname "$vendor")"
  CARGO_NET_OFFLINE=true "${host_cargo[@]}" vendor --locked --offline \
    --manifest-path "$crate/Cargo.toml" "$vendor" >"$work/cargo-vendor.log"
fi
test -f "$vendor/borsh/.cargo-checksum.json"
mkdir -p "$work/cargo-home" "$work/host-target" "$work/sbf-target"
ln -s "$vendor" "$work/vendor"
cp "$crate/cargo-home-config.toml" "$work/cargo-home/config.toml"
echo "== build standalone signed-RPC driver =="
CARGO_HOME="$work/cargo-home" CARGO_TARGET_DIR="$work/host-target" \
  CARGO_NET_OFFLINE=true "${host_cargo[@]}" build --locked --offline --release \
  --manifest-path "$crate/Cargo.toml"
driver="$work/host-target/release/clutch-local-real-pyth"

echo "== build unmistakably test-only Clutch ELF =="
mkdir -p "$work/elf"
CARGO_HOME="$work/cargo-home" CARGO_TARGET_DIR="$work/sbf-target" \
  CARGO_NET_OFFLINE=true cargo-build-sbf \
  --manifest-path "$program_manifest" \
  --sbf-out-dir "$work/elf" \
  --features non-production-real-pyth-lab \
  --offline --tools-version v1.53 \
  -- --locked \
  >"$work/build-sbf.log" 2>&1
elf="$work/elf/clutch_sbf.so"
test -s "$elf"
[ "$(shasum -a 256 "$host_lock" | awk '{print $1}')" = "$host_lock_sha" ] || {
  echo "FAIL: host Cargo.lock drifted during build" >&2
  exit 1
}
[ "$(shasum -a 256 "$sbf_lock" | awk '{print $1}')" = "$sbf_lock_sha" ] || {
  echo "FAIL: SBF Cargo.lock drifted during build" >&2
  exit 1
}

echo "== probe the same warped validator Clock before proof generation =="
"$validator" \
  --ledger "$work/clock-probe-ledger" --reset --quiet \
  --bind-address 127.0.0.1 \
  --rpc-port "$rpc_port" --faucet-port "$faucet_port" \
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range" \
  --warp-slot 460336312 \
  >"$work/clock-probe-validator.log" 2>&1 &
validator_pid=$!
wait_ready "$work/clock-probe-validator.log"
"$repo/tools/agave-loopback-validator/probe-listeners.sh" \
  "$validator_pid" "$rpc_port" "$faucet_port" "$validator" \
  >"$work/clock-probe-listeners.txt"
clock_probe_time="$("$driver" clock --work "$work" --url "$url")"
case "$clock_probe_time" in
  ''|*[!0-9]*) echo "FAIL: Clock probe did not return a positive decimal timestamp" >&2; exit 1 ;;
esac
if [ "$clock_probe_time" -le 300 ]; then
  echo "FAIL: Clock probe timestamp is too small" >&2
  exit 1
fi
publish_time=$(( ((clock_probe_time - 180) / 60) * 60 ))
stop_validator

"$driver" prepare --work "$work" --repository-head "$repository_head" \
  --clock-probe-time "$clock_probe_time" \
  --publish-time "$publish_time" --clutch-elf "$elf" --validator "$validator" \
  --campaign-mode "$campaign_mode"
payer="$(tr -d '\n' < "$work/payer.pubkey")"

validator_args=(
  --ledger "$work/ledger" --reset --quiet
  --bind-address 127.0.0.1
  --rpc-port "$rpc_port" --faucet-port "$faucet_port"
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range"
  --mint "$payer"
  --warp-slot 460336312
)
while IFS=$'\t' read -r role address file; do
  [ -n "$role" ] || continue
  validator_args+=(--account "$address" "$work/$file")
done < "$work/genesis.tsv"

echo "== start explicitly selected validator =="
echo "validator=$validator"
"$validator" "${validator_args[@]}" >"$work/validator.log" 2>&1 &
validator_pid=$!

wait_ready "$work/validator.log"

# Health can turn green at the warp target before the validator has produced
# enough descendant banks for the warped Clock timestamp to settle.  The
# driver's bounded clock command waits through CLOCK_SETTLED_SLOT on this exact
# campaign validator; without it, a correct fresh observation can be compared
# against the pre-warp timestamp from the first healthy bank.
campaign_clock_time="$("$driver" clock --work "$work" --url "$url")"
case "$campaign_clock_time" in
  ''|*[!0-9]*) echo "FAIL: campaign Clock did not settle to a positive decimal timestamp" >&2; exit 1 ;;
esac
echo "campaign_clock_settled=$campaign_clock_time"

# Stock Agave 4.0.2 hard-codes wildcard listeners for these services. Merely
# passing --bind-address is insufficient. The pinned validator lane owns this
# stronger audit: exact child executable, exact RPC/WS/faucet listeners, every
# TCP/UDP socket loopback-bound, and failed reachability through each LAN IP.
"$repo/tools/agave-loopback-validator/probe-listeners.sh" \
  "$validator_pid" "$rpc_port" "$faucet_port" "$validator" \
  | tee "$work/probe-before.txt"

echo "== real provider / joined Clutch campaign =="
"$driver" run --work "$work" --repository-head "$repository_head" \
  --url "$url" --validator "$validator" --campaign-mode "$campaign_mode"
# Some validator client pools create UDP/QUIC sockets lazily only after the
# bank processes traffic. Re-run the exact strong isolation proof before a
# successful transcript can leave the temporary directory.
"$repo/tools/agave-loopback-validator/probe-listeners.sh" \
  "$validator_pid" "$rpc_port" "$faucet_port" "$validator" \
  | tee "$work/probe-after.txt"
probe_before_sha="$(shasum -a 256 "$work/probe-before.txt" | awk '{print $1}')"
probe_after_sha="$(shasum -a 256 "$work/probe-after.txt" | awk '{print $1}')"
validator_log_sha="$(shasum -a 256 "$work/validator.log" | awk '{print $1}')"
validator_sha="$(shasum -a 256 "$validator" | awk '{print $1}')"
cat >"$work/probe-evidence.json" <<EOF
{
  "claim": "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE",
  "selected_validator_sha256": "$validator_sha",
  "rpc": "127.0.0.1:$rpc_port",
  "websocket": "127.0.0.1:$ws_port",
  "faucet": "127.0.0.1:$faucet_port",
  "gossip": "127.0.0.1:$gossip_port",
  "configured_dynamic_port_range": "$dynamic_port_range",
  "probe_before_sha256": "$probe_before_sha",
  "probe_after_sha256": "$probe_after_sha",
  "validator_log_sha256": "$validator_log_sha",
  "scope": "proves all child TCP listeners and UDP sockets observed by lsof were loopback-bound; does not claim every loopback socket fell inside the configurable service ranges"
}
EOF
transcript_dir="${CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR:-}"
if [ -n "$transcript_dir" ]; then
  mkdir -p "$transcript_dir"
  if [ -e "$transcript_dir/campaign.json" ] || [ -e "$transcript_dir/result.json" ] || \
     [ -e "$transcript_dir/probe-evidence.json" ] || \
     [ -e "$transcript_dir/probe-before.txt" ] || [ -e "$transcript_dir/probe-after.txt" ]; then
    echo "FAIL: refusing to overwrite an existing retained transcript" >&2
    exit 1
  fi
  cp "$work/campaign.json" "$transcript_dir/campaign.json"
  cp "$work/result.json" "$transcript_dir/result.json"
  cp "$work/probe-evidence.json" "$transcript_dir/probe-evidence.json"
  cp "$work/probe-before.txt" "$transcript_dir/probe-before.txt"
  cp "$work/probe-after.txt" "$transcript_dir/probe-after.txt"
  echo "PASS; public truth-labeled transcripts copied to $transcript_dir"
else
  echo "PASS; result transcript (set CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR to retain):"
  sed -n '1,240p' "$work/result.json"
fi
