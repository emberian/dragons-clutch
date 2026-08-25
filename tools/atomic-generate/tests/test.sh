#!/usr/bin/env bash
set -euo pipefail

tool=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)/dclutch-atomic-generate
scratch=$(mktemp -d "${TMPDIR:-/tmp}/dclutch-atomic-generate-test.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT HUP INT TERM

target="$scratch/canonical.rs"
baseline="$scratch/baseline.rs"
expected="$scratch/expected.rs"
stderr_file="$scratch/stderr"

printf '%s\n' '// accepted-v1' 'const OLD: u8 = 1;' > "$target"
cp "$target" "$baseline"

assert_unchanged() {
  cmp "$baseline" "$target" \
    || { printf 'canonical target changed after a refused generation\n' >&2; exit 1; }
}

if "$tool" \
    --output "$target" \
    --expect-first-line '// generated-v1' \
    -- sh -c 'printf "%s\n" "// generated-v1"; printf "%s\n" "producer diagnostic" >&2; exit 7' \
    2> "$stderr_file"; then
  printf 'failing producer unexpectedly succeeded\n' >&2
  exit 1
fi
grep -q -e 'producer diagnostic' "$stderr_file"
assert_unchanged

if "$tool" \
    --output "$target" \
    --expect-first-line '// generated-v1' \
    -- sh -c ':'; then
  printf 'empty producer unexpectedly succeeded\n' >&2
  exit 1
fi
assert_unchanged

if "$tool" \
    --output "$target" \
    --min-lines 2 \
    --expect-first-line '// generated-v1' \
    -- sh -c 'printf "%s\n" "// wrong-header" "payload=ready"'; then
  printf 'wrong-header producer unexpectedly succeeded\n' >&2
  exit 1
fi
assert_unchanged

if "$tool" \
    --output "$target" \
    --min-lines 3 \
    --expect-pattern '^required=true$' \
    -- sh -c 'printf "%s\n" "// generated-v1" "payload=ready"'; then
  printf 'short or missing-pattern producer unexpectedly succeeded\n' >&2
  exit 1
fi
assert_unchanged

if "$tool" \
    --output "$target" \
    --expect-first-line '// generated-v1' \
    --validator grep \
    --validator-arg -q \
    --validator-arg -e \
    --validator-arg '^never-present$' \
    -- sh -c 'printf "%s\n" "// generated-v1" "payload=ready"'; then
  printf 'failing validator unexpectedly succeeded\n' >&2
  exit 1
fi
assert_unchanged

printf '%s\n' '// generated-v1' 'payload=ready' 'required=true' > "$expected"
"$tool" \
  --output "$target" \
  --min-lines 3 \
  --expect-first-line '// generated-v1' \
  --expect-pattern '^required=true$' \
  --validator sh \
  --validator-arg -c \
  --validator-arg "[ \"\$(dirname -- \"\$2\")\" = \"\$1\" ] && grep -E -q -e \"^payload=ready\$\" \"\$2\"" \
  --validator-arg validator \
  --validator-arg "$scratch" \
  -- sh -c 'printf "%s\n" "// generated-v1" "payload=ready" "required=true"'

cmp "$expected" "$target"
if compgen -G "$scratch/.canonical.rs.atomic.*" > /dev/null; then
  printf 'same-directory temporary file survived completion\n' >&2
  exit 1
fi

printf 'atomic generator shell tests passed\n'
