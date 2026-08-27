#!/usr/bin/env bash
# NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE.
#
# Explicit two-boundary Source V2 campaign. The base runner retains the
# loopback/provenance/build gates and ephemeral signer handling; this wrapper
# selects the schema that proves ordered multi-record admission, exact
# resolution, redemption, and withdrawal.

set -euo pipefail

repo="$(cd "$(dirname "$0")/../../.." && pwd)"
export CLUTCH_LOCAL_REAL_PYTH_CAMPAIGN_MODE=joined-multiboundary-v1
exec "$repo/programs/clutch-sbf/scripts/run_local_real_pyth.sh" "$@"
