#!/usr/bin/env bash
# Sourced by the runner and its launch controls. A successful containment
# wrapper must have launched the command; an already-loaded scope did not.
run_build() {
    local capture status=0
    capture="$(mktemp "${TMPDIR:-/tmp}/dclutch-build-command.XXXXXX")" || return 1
    if [ -n "${WRAP:-}" ]; then
        "$WRAP" "$@" > "$capture" 2>&1 || status=$?
    else
        "$@" > "$capture" 2>&1 || status=$?
    fi
    cat "$capture"
    if grep -Eq 'Unit run-u[0-9]+[.]scope was already loaded' "$capture"; then
        printf 'gauntlet: build command never ran (scope already loaded); retry this stage\n' >&2
        status=1
    fi
    rm -f "$capture"
    return "$status"
}
