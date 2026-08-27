#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: $0 DCLUTCH_DIR QEDSVM_DIR LLVM_OBJCOPY" >&2
  exit 64
fi

dclutch_dir=$1
qedsvm_dir=$2
objcopy=$3
evidence_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
controller="$dclutch_dir/target/deploy/dclutch_controller_proof_sbf.so"
claims="$dclutch_dir/target/deploy/dclutch_claims_proof_sbf.so"
qedlift="$qedsvm_dir/qedsvm-rs/target/debug/qedlift"
temporary_dir=$(mktemp -d /tmp/dclutch-qedsvm-direct-verify.XXXXXX)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

check_sha256() {
  expected=$1
  file=$2
  actual=$(shasum -a 256 "$file" | awk '{print $1}')
  if [ "$actual" != "$expected" ]; then
    echo "sha256 mismatch: $file: expected $expected, got $actual" >&2
    exit 1
  fi
}

check_size() {
  expected=$1
  file=$2
  actual=$(wc -c < "$file" | tr -d ' ')
  if [ "$actual" != "$expected" ]; then
    echo "byte-size mismatch: $file: expected $expected, got $actual" >&2
    exit 1
  fi
}

check_lines() {
  expected=$1
  file=$2
  actual=$(wc -l < "$file" | tr -d ' ')
  if [ "$actual" != "$expected" ]; then
    echo "line-count mismatch: $file: expected $expected, got $actual" >&2
    exit 1
  fi
}

check_sha256 e0371f3595232e8a430574fc784cd90685139265105ccadf23da5828475b4515 "$controller"
check_size 172984 "$controller"
check_sha256 75f2ef597f8e6e5466a5c9537fe30612cc496629082211558369aa86634423bb "$claims"
check_size 22584 "$claims"
check_sha256 3e4d9c9b5a43c09c5f1104bb6d9c7fbd5ff63f40c6b4406bb0b8eaa55fa7e028 "$evidence_dir/direct-success.pcs"
check_size 10180 "$evidence_dir/direct-success.pcs"
check_lines 1871 "$evidence_dir/direct-success.pcs"
check_sha256 5edcc2d149982588a93592f695714efc014998acfd2afd714458423f91c14599 "$evidence_dir/direct-stale-sequence.pcs"
check_size 6164 "$evidence_dir/direct-stale-sequence.pcs"
check_lines 1108 "$evidence_dir/direct-stale-sequence.pcs"
check_sha256 457120d154099b214d06188542fbccd46c0fb9ff01297b626114d466d871e170 "$evidence_dir/dclutch_direct_mollusk_trace.rs"
check_sha256 5b87a5e228eeb7d4dbb0d8caaf6e81cbe1efdc845933948d9381e1c2f1b5e137 "$evidence_dir/dclutch_trace_to_pcs.rs"

"$objcopy" --dump-section .text="$temporary_dir/controller.text" "$controller"
"$objcopy" --dump-section .text="$temporary_dir/claims.text" "$claims"
check_sha256 20158370b6f4660f8064568c999a1811cb6a531dee2906fc7dacfc4c7c6335bc "$temporary_dir/controller.text"
check_size 158288 "$temporary_dir/controller.text"
check_sha256 668cabd7d656719c332d61c98cd08111a7a704b214fb46b02018d5f14460a4a0 "$temporary_dir/claims.text"
check_size 21192 "$temporary_dir/claims.text"
check_sha256 1d8b8f2e312e12a06a8f508f705dad795b2752d554978755f6508885c40309f2 "$objcopy"

if [ "$(git -C "$qedsvm_dir" rev-parse HEAD)" != "99bd5ede85374adc7fc5c835c2432ecf4e123fd1" ]; then
  echo "qedsvm checkout is not the pinned v0.12.0 commit" >&2
  exit 1
fi
if [ "$(git -C "$qedsvm_dir" rev-parse 'HEAD^{tree}')" != "6cb10570a567dfe64fbc68ecc3fdd46f97de9500" ]; then
  echo "qedsvm committed tree mismatch" >&2
  exit 1
fi
check_sha256 406f753b5a68e7e67b72a488727ef4c98ca44fbcde0e863065bb4c4d10a6b113 "$qedlift"
check_sha256 c81938feba65ad0c5172f7039cf7f2485d23204c69998b65c174502902c2b0ac "$qedsvm_dir/SVM/SBPF/Decode.lean"

expected_error='create_pda at pc 2806: only the single-seed (n_seeds = 1) shape is modelled so far, got 2'
for trace in direct-success.pcs direct-stale-sequence.pcs; do
  log="$temporary_dir/$trace.log"
  output="$temporary_dir/$trace.lean"
  if "$qedlift" --so "$controller" --trace "$evidence_dir/$trace" \
      --module DClutchDirectCapture --output "$output" >"$log" 2>&1; then
    echo "unexpected successful lift: $trace" >&2
    exit 1
  fi
  if ! grep -F "$expected_error" "$log" >/dev/null; then
    echo "qedlift failed at an unrecorded boundary: $trace" >&2
    tail -n 20 "$log" >&2
    exit 1
  fi
  if [ -e "$output" ]; then
    echo "qedlift wrote unexpected Lean output after refusal: $output" >&2
    exit 1
  fi
done

echo "exact Direct traces and fail-closed qedsvm v0.12 boundary verified"
