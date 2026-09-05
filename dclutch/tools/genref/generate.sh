#!/usr/bin/env bash
# `tools/gate reference` is the generator's driver. This name survives because
# every generated page's banner and tools/release/final-generated-convergence.py
# spell it; the arguments are the gate's own (--check, --converge, --allow-dirty).
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/tools/gate" reference "$@"
