#!/bin/sh
# Both directions, because a checker that cannot fail is indistinguishable from
# a clean tree, and a tripwire that cannot fire is worse than none.
#
#   1. On this tree it still resolves and probes what it says it does.
#   2. On a synthetic tree it accepts a runbook whose commands work, and fires
#      on each of the three defects it exists to find: a program that is not
#      there, a flag the program's own --help does not name, and a required
#      argument the runbook omits.
#   3. It keeps "could not be checked" apart from "checked and fine": an
#      unprobed command exits 2, never 0 and never 1.
#   4. It descends into a subcommand's own help page -- a CLI that documents
#      its subcommands there is healthy -- without that descent swallowing a
#      flag no page on the path names.
#
# The synthetic half runs in a temp directory on purpose: a shared checkout must
# not have a control planting broken commands in another lane's runbook.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
tool="$root/tools/doc-commands/doc_commands.py"

out="$(python3 "$tool" --root "$root")"
printf '%s' "$out" | grep -q 'against their own --help' || {
    echo "control FAILED: the survey no longer reports how many commands it probed." >&2
    exit 1
}
printf '%s' "$out" | grep -q 'not probed' || {
    echo "control FAILED: the survey no longer states what it did not probe." >&2
    exit 1
}
echo "control 1/5: still reports what it probed and what it did not"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
(cd "$work" && git init --quiet .)
mkdir -p "$work/docs/guides" "$work/tools"

cat > "$work/tools/thing.sh" <<'SH'
#!/bin/sh
# A program with a real help page.
case "${1:-}" in
    --help) printf 'usage: thing.sh [--loud]\n\n  --loud   say it twice\n' ; exit 0 ;;
esac
SH
chmod +x "$work/tools/thing.sh"

cat > "$work/tools/needy.py" <<'PY'
import argparse
parser = argparse.ArgumentParser()
parser.add_argument("--must", required=True)
parser.add_argument("--maybe")
parser.parse_args()
PY

cat > "$work/docs/guides/good.md" <<'MD'
# A runbook whose commands work

```sh
tools/thing.sh --loud
python3 tools/needy.py --must yes --maybe no
```
MD

(cd "$work" && git add -A && git -c user.email=c@x -c user.name=c commit --quiet -m seed)

report="$(python3 "$tool" --root "$work")"
printf '%s' "$report" | grep -q 'unresolved program\|rejected by its own program\|incomplete as published' && {
    echo "control FAILED: a runbook whose commands work was reported as broken." >&2
    printf '%s\n' "$report" >&2
    exit 1
}
echo "control 2/5: a runbook whose commands work is reported clean"

cat > "$work/docs/guides/bad.md" <<'MD'
# A runbook that has rotted

```sh
tools/gone.sh --loud
tools/thing.sh --quiet
python3 tools/needy.py --maybe no
```
MD
(cd "$work" && git add -A && git -c user.email=c@x -c user.name=c commit --quiet -m rot)

report="$(python3 "$tool" --root "$work")"
for expected in 'unresolved program' 'rejected by its own program' 'incomplete as published'; do
    printf '%s' "$report" | grep -q "$expected" || {
        echo "control FAILED: '$expected' was not reported." >&2
        printf '%s\n' "$report" >&2
        exit 1
    }
done
echo "$report" | grep -q -- '--quiet' || {
    echo "control FAILED: the rejected flag was not named." >&2
    exit 1
}
if python3 "$tool" --root "$work" --check >/dev/null 2>&1; then
    echo "control FAILED: --check did not fire on a rotted runbook." >&2
    exit 1
fi
echo "control 3/5: names each of the three defects, and --check exits nonzero"

rm "$work/docs/guides/bad.md"
cat > "$work/tools/opaque.sh" <<'SH'
#!/bin/sh
echo "this program handles no help flag"
SH
chmod +x "$work/tools/opaque.sh"
cat > "$work/docs/guides/unprobed.md" <<'MD'
# A runbook naming a program that cannot be probed

```sh
tools/opaque.sh --whatever
```
MD
(cd "$work" && git add -A && git -c user.email=c@x -c user.name=c commit --quiet -m opaque)

set +e
python3 "$tool" --root "$work" --check >/dev/null 2>&1
code=$?
set -e
[ "$code" -eq 2 ] || {
    echo "control FAILED: an unprobed command exited $code; it must be 2, which is 'nothing was proven'." >&2
    exit 1
}
echo "control 4/5: an unprobed command exits 2, not 0 and not 1"

rm "$work/docs/guides/unprobed.md" "$work/tools/opaque.sh"
cat > "$work/tools/nested.sh" <<'SH'
#!/bin/sh
# A program whose subcommand documents itself, which is healthy, not broken.
case "${1:-}" in
    inner)
        case "${2:-}" in
            --help) printf 'usage: nested.sh inner [--deep VALUE]\n\n  --deep   only ever named here\n' ; exit 0 ;;
        esac ;;
    --help) printf 'usage: nested.sh <command>\n\ncommands:\n  inner   do the inner thing\n' ; exit 0 ;;
esac
SH
chmod +x "$work/tools/nested.sh"
cat > "$work/docs/guides/nested.md" <<'MD'
# A runbook using a flag only the subcommand page names

```sh
tools/nested.sh inner --deep 3
```
MD
(cd "$work" && git add -A && git -c user.email=c@x -c user.name=c commit --quiet -m nested)

report="$(python3 "$tool" --root "$work")"
printf '%s' "$report" | grep -q 'rejected by its own program' && {
    echo "control FAILED: a flag documented on the subcommand's own page was reported missing." >&2
    printf '%s\n' "$report" >&2
    exit 1
}
cat > "$work/docs/guides/nested.md" <<'MD'
# A runbook using a flag no page names

```sh
tools/nested.sh inner --shallow 3
```
MD
(cd "$work" && git add -A && git -c user.email=c@x -c user.name=c commit --quiet -m nested-bad)
python3 "$tool" --root "$work" | grep -q -- '--shallow' || {
    echo "control FAILED: the descent swallowed a flag no page names." >&2
    exit 1
}
echo "control 5/5: descends into a subcommand page, and still refuses a flag no page names"
