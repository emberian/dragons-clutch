#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$here/common.sh"

usage() {
  echo "usage: $0 PID RPC_PORT FAUCET_PORT [EXPECTED_BINARY]" >&2
  exit 2
}

[ "$#" -ge 3 ] && [ "$#" -le 4 ] || usage
pid="$1"
rpc_port="$2"
faucet_port="$3"
expected_binary="${4:-$binary_path}"

case "$pid" in *[!0-9]*|"") usage ;; esac
case "$rpc_port" in *[!0-9]*|"") usage ;; esac
case "$faucet_port" in *[!0-9]*|"") usage ;; esac
[ "$rpc_port" -gt 0 ] && [ "$rpc_port" -lt 65535 ] ||
  die "RPC port must leave room for its WebSocket successor"
[ "$faucet_port" -gt 0 ] && [ "$faucet_port" -le 65535 ] ||
  die "faucet port is out of range"
ws_port="$((rpc_port + 1))"
[ "$faucet_port" -ne "$rpc_port" ] && [ "$faucet_port" -ne "$ws_port" ] ||
  die "faucet port must be disjoint from RPC $rpc_port and WebSocket $ws_port"

for command in lsof curl rg awk sed ps ifconfig nc mktemp env; do
  require_command "$command"
done
kill -0 "$pid" 2>/dev/null || die "PID $pid is not running"
[ -x "$expected_binary" ] || die "expected binary is not executable: $expected_binary"

canonical_path() {
  local path="$1"
  local directory basename
  directory="$(cd "$(dirname "$path")" && pwd -P)"
  basename="$(basename "$path")"
  printf '%s/%s\n' "$directory" "$basename"
}

actual_binary="$(
  lsof -a -p "$pid" -d txt -Fn 2>/dev/null |
    sed -n 's/^n//p' |
    sed -n '1p'
)"
[ -n "$actual_binary" ] || die "could not resolve executable for PID $pid"
actual_binary="$(canonical_path "$actual_binary")"
expected_binary="$(canonical_path "$expected_binary")"
[ "$actual_binary" = "$expected_binary" ] ||
  die "PID $pid runs $actual_binary, expected $expected_binary"

probe_dir="$(mktemp -d "${TMPDIR:-/tmp}/agave-loopback-probe.XXXXXX")"
cleanup() {
  rm -f "$probe_dir/tcp" "$probe_dir/udp" "$probe_dir/rpc-health"
  rmdir "$probe_dir" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

ready=0
for _attempt in $(seq 1 60); do
  kill -0 "$pid" 2>/dev/null || die "PID $pid exited during readiness wait"
  if env \
      -u http_proxy -u https_proxy -u all_proxy \
      -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
      -u no_proxy -u NO_PROXY \
      curl -q --noproxy '*' --proxy '' -fsS --max-time 1 \
      -H 'content-type: application/json' \
      --data '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
      "http://127.0.0.1:$rpc_port" >"$probe_dir/rpc-health" 2>/dev/null &&
     rg -q '"result"[[:space:]]*:[[:space:]]*"ok"' "$probe_dir/rpc-health" &&
     nc -z -G 1 -w 1 127.0.0.1 "$ws_port" >/dev/null 2>&1 &&
     nc -z -G 1 -w 1 127.0.0.1 "$faucet_port" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
[ "$ready" -eq 1 ] ||
  die "required loopback RPC/WS/faucet endpoints did not become ready"

lsof -nP -a -p "$pid" -iTCP -sTCP:LISTEN >"$probe_dir/tcp" ||
  die "could not enumerate TCP listeners for PID $pid"
lsof -nP -a -p "$pid" -iUDP >"$probe_dir/udp" ||
  die "could not enumerate UDP sockets for PID $pid"

require_tcp_listener() {
  local port="$1"
  local label="$2"
  awk -v endpoint="127.0.0.1:$port" \
    'NR > 1 && $8 == "TCP" && $9 == endpoint && $10 == "(LISTEN)" { found = 1 }
     END { exit(found ? 0 : 1) }' "$probe_dir/tcp" ||
    die "$label is not listening exactly on 127.0.0.1:$port"
}

require_tcp_listener "$rpc_port" RPC
require_tcp_listener "$ws_port" WebSocket
require_tcp_listener "$faucet_port" faucet

bad_tcp="$(
  awk 'NR > 1 && $8 == "TCP" && $10 == "(LISTEN)" && $9 !~ /^127[.]0[.]0[.]1:/ { print }' \
    "$probe_dir/tcp"
)"
[ -z "$bad_tcp" ] || die "non-loopback TCP listener detected: $bad_tcp"
bad_udp="$(
  awk 'NR > 1 && $8 == "UDP" && $9 !~ /^127[.]0[.]0[.]1:/ { print }' \
    "$probe_dir/udp"
)"
[ -z "$bad_udp" ] || die "non-loopback UDP bind detected: $bad_udp"

lan_addresses="$(
  ifconfig |
    awk '$1 == "inet" && $2 != "127.0.0.1" && $2 !~ /^169[.]254[.]/ { print $2 }' |
    LC_ALL=C sort -u
)"
if [ -n "$lan_addresses" ]; then
  while IFS= read -r address; do
    [ -n "$address" ] || continue
    for port in "$rpc_port" "$ws_port" "$faucet_port"; do
      if nc -z -G 1 -w 1 "$address" "$port" >/dev/null 2>&1; then
        die "listener is reachable through non-loopback address $address:$port"
      fi
    done
  done <<<"$lan_addresses"
fi

echo "loopback listener probe: PASS"
echo "pid: $pid"
echo "binary: $actual_binary"
echo "rpc: 127.0.0.1:$rpc_port"
echo "websocket: 127.0.0.1:$ws_port"
echo "faucet: 127.0.0.1:$faucet_port"
echo "non_loopback_addresses_tested: ${lan_addresses:-none}"
echo "tcp listeners:"
sed -n '2,$p' "$probe_dir/tcp"
echo "udp sockets:"
sed -n '2,$p' "$probe_dir/udp"
