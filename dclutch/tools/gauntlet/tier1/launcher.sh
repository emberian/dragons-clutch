#!/usr/bin/env bash
# The gauntlet's launcher shim for tier 1.
#
# It is now a pass-through with ONE job: pin the campaign's tick rate at the
# gauntlet's boundary before handing off to `dclutch-successor-validator`.
#
# ## What used to be here, and why it is gone (JRNY-2, 2026-08-27)
#
# This file used to carry a recorded fixture-pin OVERRIDE path, written when
# `30bfc71` rewrote the Pyth fixture `PROVENANCE.md` and added
# `guardian-set-0.account.hex` without regenerating
# `tools/local-validator/fixture-sha256.txt`, and when the verifier additionally
# hardcoded `listed -eq 10`. With stale pins and an explicit opt-in the shim
# copied BOTH launcher scripts into the run directory, regenerated the pin list,
# patched the hardcoded count, and ran the copy.
#
# Its own header said: "When W1d regenerates the pin file this shim becomes a
# pass-through and the override path below goes dead." Both halves happened.
# `8e97b58` ("Derive the fixture pin count instead of asserting ten") replaced
# the literal with a two-way cover — every pin resolves to a file whose digest
# matches, and the directory holds no file that is not pinned — and the pin file
# verifies at HEAD: eleven artifacts, no drift, nothing unpinned.
#
# It did not just go dead. It went RED, and it took every campaign with it. The
# override path copied `tools/local-validator/dclutch-local-validator`, which
# `6a477bf` banished with the DCLTCAT1 stratum after porting its `verify_fixtures`
# into `dclutch-successor-validator` — and this shim checked for that file's
# existence UNCONDITIONALLY, at the top, before it ever looked at whether the
# pins verified. So at HEAD every run of this launcher died with
# `missing .../dclutch-local-validator` before the validator was reached:
# `run.sh --mode full`, this tier, and the journey alike. That is what a
# deletion sweep looks like from one file downstream of it.
#
# The lesson worth keeping: an override path guarded by a condition that is
# false is not inert. It is unexecuted code holding a hard dependency on
# something else, and the dependency is checked whether or not the path runs.
#
# If the pins ever drift again, `dclutch-successor-validator start` refuses with
# the exact mismatch, at its owner, which is where that refusal belongs.
set -euo pipefail

SOURCE_ROOT="${GAUNTLET_SOURCE_ROOT:?GAUNTLET_SOURCE_ROOT must name the archived source tree}"
REAL_LAUNCHER="$SOURCE_ROOT/tools/local-validator/dclutch-successor-validator"

# --------------------------------------------------------- ticks per slot
# The gauntlet pins the campaign's tick rate at ITS boundary, through the
# launcher's own documented knob, rather than inheriting whatever default that
# file happens to carry. `12347de` set the launcher default to 16 after the
# measurement below; this line means a later change to that default cannot
# silently cost the gauntlet twenty minutes a run without anyone noticing.
#
# What 16 changes is how long a slot takes and NOTHING ELSE. The campaign is
# ~100 transactions submitted strictly in sequence, each waited to FINALIZED
# before the next is derived from it, and that discipline is the campaign's
# whole epistemic claim. At the stock 64 ticks a slot is 400 ms and the campaign
# spends about twenty-five minutes waiting for a clock; at 16 it is 100 ms, the
# same transactions in the same order under the same finality rule, in about a
# quarter of the time. 16 and not 8: a validator that cannot keep up with its
# own tick rate skips slots, and a skipped slot IS a semantic difference
# underneath a campaign that reads finalized state.
export DCLUTCH_TICKS_PER_SLOT="${DCLUTCH_TICKS_PER_SLOT:-16}"

die() { printf 'gauntlet-launcher: %s\n' "$*" >&2; exit 1; }

[ -f "$REAL_LAUNCHER" ] || die "missing $REAL_LAUNCHER"

# A knob only pins something if the thing it points at still reads it. If the
# launcher stops honouring DCLUTCH_TICKS_PER_SLOT the export above becomes a
# comment, and the failure mode is a campaign that quietly takes four times as
# long -- exactly the kind of regression nobody reports because it looks like a
# slow machine. Warn rather than refuse: this is W1d's file, the campaign is
# still CORRECT at any tick rate, and a gauntlet that refuses to run over
# somebody else's wall-clock choice would be worse than a slow one.
grep -q 'DCLUTCH_TICKS_PER_SLOT' "$REAL_LAUNCHER" || {
    echo "gauntlet-launcher: WARNING - $REAL_LAUNCHER no longer reads DCLUTCH_TICKS_PER_SLOT." >&2
    echo "gauntlet-launcher: WARNING - the campaign is still correct, but expect it to take roughly four times as long." >&2
}

# Accepted and inert, so a runner that still passes it gets a diagnosis rather
# than an unknown-flag death. Nothing here can override a fixture pin any more:
# the launcher verifies its own, and that is the only copy.
if [ "${GAUNTLET_ALLOW_STALE_FIXTURE_PINS:-false}" = "true" ]; then
    echo "gauntlet-launcher: GAUNTLET_ALLOW_STALE_FIXTURE_PINS is set and has no effect." >&2
    echo "gauntlet-launcher: the fixture-pin override path was removed once the pins verified and the second launcher it copied was banished; dclutch-successor-validator now verifies its own pins and refuses on drift." >&2
fi

chmod +x "$REAL_LAUNCHER"
exec "$REAL_LAUNCHER" "$@"
