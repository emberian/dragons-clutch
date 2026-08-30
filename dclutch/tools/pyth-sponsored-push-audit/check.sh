#!/bin/sh
set -eu

tool_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s "$tool_dir" -p 'test_*.py'
PYTHONDONTWRITEBYTECODE=1 python3 "$tool_dir/audit.py"
