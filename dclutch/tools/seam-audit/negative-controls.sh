#!/usr/bin/env bash
# Prove each seam-audit reader catches a defect this repository actually had.
#
# A checker that has never caught a known defect is decoration.  Every reader
# below is held to one of three bars, and which bar a class gets is decided by
# what the tree can actually supply rather than by what would look tidiest:
#
#   historical  a real commit fixed this defect.  Check out that commit's
#               PARENT into a throwaway worktree -- the tree as it stood with
#               the defect in it -- and require the reader to find it there AND
#               to be silent at HEAD.  Silence after is half the bar: a reader
#               that also fires on the fix is reading the code around the
#               defect, not the defect.
#
#   live        the defect is documented, unfixed, and in the tree right now.
#               Require the reader to name it at HEAD, at the right function.
#               This is a stronger bar than historical, not a weaker one -- the
#               checker is finding a real always-refuses route unaided.
#
#   synthetic   the class has no defect behind it, so there is nothing to
#               check out.  Mutate a worktree to introduce the defect and
#               require the reader to notice.  Used for exactly one class, and
#               that class says so in its own docstring.
#
# Worktrees only.  Never `git stash`: this is a shared tree with other lanes
# working in it, and stashing is not a safe operation in one.
#
#   tools/seam-audit/negative-controls.sh          # every control
#   tools/seam-audit/negative-controls.sh SEED_LEN # one class

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AUDIT="${ROOT}/tools/seam-audit/seam_audit.py"
SCRATCH="${TMPDIR:-/tmp}/seam-audit-controls.$$"
ONLY="${1:-}"

# kind|commit|class|code|needle (a grep -E pattern)|what the defect was
CONTROLS=(
  "historical|fb076ec6|SEED_LEN|SEED_LEN_OVER_MAX|STRUCTURED_RECEIPT_MINT_PDA_SEED_V2|two Structured V2 seed domains at 34 and 35 bytes: no derivable address for any bump (SEAM_AUDIT #8)"
  "historical|fee868c5|SEED_LEN|SEED_LEN_OVER_MAX|DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1|four dealer-codec PDA domains over the 32-byte seed maximum"
  "historical|9a9f1b5c|DERIVATION|DERIVATION_DOMAIN_ERASED|admitted_composition_v3|Trading derived ten Registry records under an opaque domain parameter, at two seeds where the Registry uses three (SEAM_AUDIT #3)"
  "historical|eae9a0c9|DERIVATION|DOMAIN_ARITY_SPLIT|DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1|Trading authenticated a three-seed batch address for an account Custody signs into existence with two"
  "historical|3b98ea3a|PIN_CENSUS|CENSUS_ARMS_DISAGREE|capability|ActivateCapability ran a blanket no-duplicate census over a frame structurally requiring seven repeats (SEAM_AUDIT #12)"
  "historical|16351a13|PRIVILEGE|PRIVILEGE_PIN_UNEXEMPTED|dealer_reservation_v1.rs.require_frame$|Custody pinned the checkpoint readonly while its atomic partner instruction must take it writable"
  "live|HEAD|PRIVILEGE|TRANSACTION_LEVEL_SIGNER_CENSUS|projected_custody_bootstrap_v1.rs.authenticate_expired_checkpoint_v1|SEAM_AUDIT #13b, unfixed: a blanket signer refusal over a frame the builder puts the fee payer in, so an expired founding can never be unwound"
  "historical|b209be565|DOMAIN_DUP|DOMAIN_BYTES_COLLIDE|CLAIMS_FOUNDING_AGGREGATE_SEED_V4|the V4 and V5 founding aggregate domains were byte-identical, so the version bump lived in the name and not in the address; b209be565 made both ALIASES of the owner constant, which is one author for the bytes -- was a live row until 2026-09-03 and is a stronger bar as a historical one, because silence on the fix is half of it"
  "synthetic|HEAD|UNSET_PIN|UNSET_GUARD_PRESENT|series/kernel_adapter.rs|no 2026-08-29 defect exists for this class; the bar is that deleting a live unset-pubkey guard is noticed"
  "synthetic|HEAD|AUTHORITY|AUTHORITY_CACHE_UNDERIVED|lib.rs.authenticate_reservation_frame_v1|no historical defect exists for this class either; the bar is that deleting Custody delegation to the blessed activation authenticator turns a derived cache read into an asserted one, and is noticed"
  "silent|HEAD|PRIVILEGE|TRANSACTION_LEVEL_SIGNER_CENSUS|terminal_sequence.rs.authenticate_lookup_infrastructure_planned_journal_v1|not a refusal at all: both is_signer reads classify the coordinate into TerminalAddressClassV1::InlineSigner and no Err is reachable, so reporting it as an always-refuses frame was the reader mistaking a read for a rejection"
  "silent|HEAD|PRIVILEGE|TRANSACTION_LEVEL_SIGNER_CENSUS|direct_hot_route_manifest.rs.project_manifest_document_v3|refuses the fee payer being named in the frame at all, one line below the census, so the harm this class states -- dead for any builder that pays with an account it also names -- is refused in place by its own error code"
)

pass=0
fail=0
skipped=0

report() {
  python3 "${AUDIT}" --root "$1" --class "$2" --report 2>/dev/null
}

echo "seam-audit negative controls -- each reader against a defect this tree had"
echo

for control in "${CONTROLS[@]}"; do
  IFS='|' read -r kind commit class code needle story <<<"${control}"
  if [ -n "${ONLY}" ] && [ "${ONLY}" != "${class}" ]; then
    continue
  fi

  case "${kind}" in
    live)
      here="$(report "${ROOT}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
      if [ "${here}" -gt 0 ]; then
        echo "PASS ${code} (live)"
        echo "     names the defect at HEAD, unaided"
        pass=$((pass + 1))
      else
        echo "FAIL ${code} (live)"
        echo "     0 findings at HEAD, want >0 for a documented unfixed defect"
        fail=$((fail + 1))
      fi
      echo "     ${story}"
      echo
      continue
      ;;
    silent)
      # The mirror of `live`, and it needs its own kind for the same reason
      # `live` does: a reader that fires on code which has closed the hole in
      # place is not stricter, it is wrong, and a class nobody can trust gets
      # switched off. Both of these sites were reported by the reader this
      # replaced, so the control fails against it.
      here="$(report "${ROOT}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
      if [ "${here}" -eq 0 ]; then
        echo "PASS ${code} (silent)"
        echo "     stays silent at HEAD on a site that closed the hole in place"
        pass=$((pass + 1))
      else
        echo "FAIL ${code} (silent)"
        echo "     ${here} findings at HEAD, want 0 -- the reader is crying wolf"
        fail=$((fail + 1))
      fi
      echo "     ${story}"
      echo
      continue
      ;;
  esac

  if ! git -C "${ROOT}" rev-parse --verify --quiet "${commit}^{commit}" >/dev/null; then
    echo "SKIP ${class}/${code} -- ${commit} is not in this checkout"
    skipped=$((skipped + 1))
    continue
  fi

  tree="${SCRATCH}/${commit}-${code}"
  mkdir -p "${SCRATCH}"
  target="${commit}^"
  [ "${kind}" = "synthetic" ] && target="${commit}"
  if ! git -C "${ROOT}" worktree add --detach --quiet "${tree}" "${target}" 2>/dev/null; then
    echo "SKIP ${class}/${code} -- cannot create a worktree at ${target}"
    skipped=$((skipped + 1))
    continue
  fi

  if [ "${kind}" = "synthetic" ] && [ "${class}" = "AUTHORITY" ]; then
    # The mirror of the ratchet below: this class counts UP.  Custody's
    # `authenticate_market` reads the activation cache and is silent today
    # because it delegates to the blessed authenticator, which performs owner,
    # width and derived address before a role byte is trusted.  Remove the
    # delegation and the same function is reading a cached role out of an
    # account whose provenance it never established -- an authority asserted
    # rather than derived, which is the whole subject of the class.
    victim="${tree}/programs/dclutch-custody-sbf/src/lib.rs"
    before="$(report "${tree}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
    python3 - "${victim}" <<'AUTHPY'
import pathlib, sys

# The delegation moved on 2026-09-03. Custody used to authenticate the cache
# inside `authenticate_market` through `authenticate_activation_cache_bump_v1`;
# `5709672aa` and the SEAM consolidation after it made every route decode the
# cache ONCE and hand the view down, so the delegation now lives in
# `process_instruction` and in `authenticate_reservation_frame_v1`, and the
# functions that read a role take an already-authenticated view as a parameter.
# The mutation follows it: delete the identity call and the same wrapper is
# reading a cached role out of an account whose provenance it never
# established. If this string ever stops matching the control FAILS LOUDLY --
# 0 before and 0 after -- rather than passing on a mutation it did not make,
# which is how the previous spelling was caught.
path = pathlib.Path(sys.argv[1])
text = path.read_text()
delegation = """    authenticate_activation_cache_identity_v1(
        registry,
        cache_account,
        &request.release_set,
        activated,
    )
    .map_err(CustodySbfError::from)?;
"""
if delegation not in text:
    raise SystemExit(
        "negative control: the activation-cache delegation is not spelled the "
        "way this mutation expects; retarget it rather than skipping it"
    )
path.write_text(text.replace(delegation, ""))
AUTHPY
    after="$(report "${tree}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
    git -C "${ROOT}" worktree remove --force "${tree}" 2>/dev/null
    if [ "${before}" -eq 0 ] && [ "${after}" -gt 0 ]; then
      echo "PASS ${code} (synthetic)"
      echo "     silent at HEAD, ${after} finding after deleting the delegation --"
      echo "     the reader names the derivation it can no longer see"
      pass=$((pass + 1))
    else
      echo "FAIL ${code} (synthetic)"
      echo "     ${before} before the deletion, ${after} after (want 0 then >0)"
      fail=$((fail + 1))
    fi
    echo "     ${story}"
    echo
    continue
  fi

  if [ "${kind}" = "synthetic" ]; then
    # Delete the guard rather than weaken it: the ratchet's whole claim is that
    # a guard cannot leave quietly, so removing one is the honest mutation.
    victim="${tree}/programs/dclutch-trading-sbf/src/series/kernel_adapter.rs"
    before="$(report "${tree}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
    python3 - "${victim}" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
text = text.replace("*core_program == Pubkey::default() || *registry_program == Pubkey::default()", "false")
text = text.replace("ticket_account == Pubkey::default()", "false")
path.write_text(text)
PY
    after="$(report "${tree}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
    git -C "${ROOT}" worktree remove --force "${tree}" 2>/dev/null
    if [ "${before}" -gt 0 ] && [ "${after}" -lt "${before}" ]; then
      echo "PASS ${code} (synthetic)"
      echo "     ${before} guards inventoried, ${after} after deleting one -- the gate's"
      echo "     two-way ratchet reports the shortfall as a GONE failure"
      pass=$((pass + 1))
    else
      echo "FAIL ${code} (synthetic)"
      echo "     ${before} before the deletion, ${after} after (want fewer)"
      fail=$((fail + 1))
    fi
    echo "     ${story}"
    echo
    continue
  fi

  before="$(report "${tree}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
  after="$(report "${ROOT}" "${class}" | grep -cE "^${code}.*${needle}" || true)"
  git -C "${ROOT}" worktree remove --force "${tree}" 2>/dev/null

  if [ "${before}" -gt 0 ] && [ "${after}" -eq 0 ]; then
    echo "PASS ${code}"
    echo "     caught ${before} at ${commit}^, silent at HEAD"
    pass=$((pass + 1))
  else
    echo "FAIL ${code}"
    echo "     ${before} findings at ${commit}^ (want >0), ${after} at HEAD (want 0)"
    fail=$((fail + 1))
  fi
  echo "     ${story}"
  echo
done

rmdir "${SCRATCH}" 2>/dev/null
echo "${pass} passed, ${fail} failed, ${skipped} skipped"
[ "${fail}" -eq 0 ]
