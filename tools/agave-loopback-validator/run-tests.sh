#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
python3 -m unittest -v \
  "$here/test_verify_runtime.py" \
  "$here/test-launcher-contract.py"
