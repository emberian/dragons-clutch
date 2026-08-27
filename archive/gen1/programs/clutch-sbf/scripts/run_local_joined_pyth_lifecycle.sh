#!/usr/bin/env bash
# NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE.
#
# Explicit second-cycle mode for the signed local-real Pyth campaign. The base
# runner retains the same loopback listener plan, source/build gates, ephemeral
# signer handling, and transcript contract; this wrapper changes only the
# requested campaign mode.

set -euo pipefail

repo="$(cd "$(dirname "$0")/../../.." && pwd)"
export CLUTCH_LOCAL_REAL_PYTH_CAMPAIGN_MODE=joined-user-lifecycle-v1
exec "$repo/programs/clutch-sbf/scripts/run_local_real_pyth.sh" "$@"
