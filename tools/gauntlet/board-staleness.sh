#!/usr/bin/env bash
# tools/gauntlet/board-staleness.sh -- the abandonment tripwire (ledger M-46).
#
# Two lanes were silently abandoned on the cross-lane board and nobody
# noticed for hours: `DA` (devnet adaptation) posted a START and never posted
# again; `FD` (frontend demo cut)'s abandonment was found three hours later by
# a different lane reading commit history, not by anyone watching the board.
# Verified: each left exactly one `##` heading in 262 posts. The board's own
# protocol already asks a lane to post a START and, later, a FINISH; nothing
# ever checked that the second post actually showed up. This script is that
# check.
#
# WHAT THIS IS: a heuristic read of a free-text, append-only markdown log
# (`tools/lane.sh board`'s own doc calls it "not authority"). It cannot know
# a lane's true status -- only whether the board looks like it stopped
# hearing from one. Treat every line below as "go look", not as a verdict.
#
# Detection, per heading (`## ...` line), oldest to newest:
#   1. Pull a date/time if the heading carries one (the canonical
#      `tools/lane.sh board` shape is `## YYYY-MM-DD HH:MM TZ -- LANE`; older
#      free-text headings are matched best-effort -- see PARSING NOTES below).
#      A heading with a time but no date inherits the most recent date seen
#      above it, since the board is append-only and therefore chronological.
#   2. Pull a lane token: the first word right after a " -- " separator if
#      the heading has one (both the canonical `## DATE TIME TZ -- LANE`
#      shape and every older heading observed to use a literal double hyphen
#      put the lane name right there, status words and all -- "DATE START --
#      GITSCAN-2" and "DATE FINISH -- GITSCAN-2" both yield "GITSCAN-2");
#      otherwise the first word whose first character is uppercase, once any
#      date/time has been stripped. This is why "orchestrator update (...)"
#      is never mistaken for a lane (lowercase first word) and why
#      "RELAY-REHOME", "TA-CL", "W2b", "DP2" all are.
#   3. A heading (or, for the canonical shape, its opening body line -- the
#      board helper never puts a status word in the heading itself) counts
#      as a START if "START" or "STARTING" appears as a whole word anywhere
#      in it, as a FINISH if "FINISH", "FINISHED", "DONE", "COMPLETE", or
#      "CLOSED" does. Most historical headings are neither -- an ordinary
#      progress post -- and only matter here as proof the lane is still
#      posting.
#
# A lane is flagged ABANDONED if its oldest START is more than --hours old
# (default 3) and NO LATER heading anywhere in the board names the same lane
# token again, by any status. This is deliberately the plain "posted a START
# and never posted again" test DA and FD actually failed, not a strict
# "must contain the literal word FINISH" test: this board's prose is not
# structured enough to promise a finish is always spelled that way (compare
# "TASK B complete, lane done" or "the port lane is LANDED" -- both real
# finishes with no FINISH-shaped heading), and a false "still alive because
# it posted again" is the safe direction to be wrong in for a tripwire.
#
# A lane whose START is old AND which HAS posted again, but with no heading
# anywhere reading as a FINISH, is reported separately and with lower
# severity: probably fine (mid-flight progress notes), worth a glance.
#
# PARSING NOTES (read before trusting a clean report):
#   - A lane that renames itself between its START and its FINISH heading
#     will not be matched across the rename. `OPS-FINISH`'s own board entry
#     is exactly this shape (a finish announcement whose lane token embeds
#     "FINISH"); if its start ever used a different token, that pairing is
#     invisible here.
#   - Fuzzy minutes ("22:5x", "19:1x", this project's own notation for "some
#     time in this ten-minute window") are rounded down to :X0, i.e. the
#     EARLIEST time in the window, which makes an entry look OLDER rather
#     than newer -- ages skew stale, not fresh, when the exact minute is
#     unknown, since the whole point of a tripwire is to prefer a false
#     alarm over a missed one.
#   - A date with no time is treated as that date's 00:00, for the same
#     reason.
#   - All timestamps are read in the local system timezone regardless of any
#     zone abbreviation in the text (EDT/EST/UTC/...); the board is a single
#     project's informal log, not a multi-timezone record, and this script is
#     advisory, not a clock.
#   - A heading this script cannot extract a date *or* time for at all is
#     listed under UNDATED rather than silently dropped or silently assumed
#     fresh.
#
# usage: board-staleness.sh [--hours N] [--board FILE] [--now WHEN]
#
#   --hours N     age threshold in hours (default 3)
#   --board FILE  board file to read (default $DCLUTCH_BOARD_FILE or
#                 /private/tmp/dclutch-wave-board.md, tools/lane.sh's own
#                 default)
#   --now WHEN    anything `date -d`/`date -j -f "%Y-%m-%d %H:%M"` accepts
#                 (default: the current time) -- for testing, or for asking
#                 "was this stale as of an earlier moment"
set -euo pipefail

hours=3
board_file="${DCLUTCH_BOARD_FILE:-/private/tmp/dclutch-wave-board.md}"
now_arg=""

usage() {
    sed -n '/^# usage:/,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'
}

while [ "$#" -gt 0 ]; do
    case "$1" in
    --hours)
        hours="${2:?--hours needs a number}"
        shift 2
        ;;
    --board)
        board_file="${2:?--board needs a file}"
        shift 2
        ;;
    --now)
        now_arg="${2:?--now needs a date}"
        shift 2
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "board-staleness.sh: unknown argument $1" >&2
        usage >&2
        exit 2
        ;;
    esac
done

if [ ! -f "$board_file" ]; then
    echo "board-staleness.sh: no board at $board_file -- nothing to check" >&2
    exit 0
fi

# Portable epoch-seconds parse of "YYYY-MM-DD HH:MM": GNU date (hbox, most
# Linux) understands the string directly; BSD date (macOS) needs -j -f with
# the exact input format spelled out.
to_epoch() {
    local text="$1"
    if date --version >/dev/null 2>&1; then
        date -d "$text" +%s
    else
        date -j -f "%Y-%m-%d %H:%M" "$text" +%s
    fi
}

if [ -n "$now_arg" ]; then
    now="$(to_epoch "$now_arg")"
else
    now="$(date +%s)"
fi
threshold_seconds=$((hours * 3600))

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Pass 1 (awk, pure text): turn every `## ...` heading into one 0x1f-delimited
# row: date, time, lane, status, heading. `date`/`time` are empty when this
# heading carries none; `time` alone carries forward the last explicit `date`
# seen above it. `status` is `start`, `finish`, or `-`. A heading with no lane
# token is dropped here -- it is a section header or an orchestrator note,
# never a lane's own post.
awk '
BEGIN { FS = "\n"; last_date = ""; pending = 0; heading = ""; body = "" }

# The lane token: prefer the text right after the FIRST " -- " (the
# lane.sh board separator -- both its canonical
# "DATE TIME TZ -- LANE" shape and every historical heading observed to use
# a literal double hyphen put the lane name right after it, even when a
# status word sits on either side, e.g. "2026-08-27 START -- GITSCAN-2" and
# "2026-08-27 FINISH -- GITSCAN-2" both yield "GITSCAN-2"). Falling back to
# the first capitalized word only for headings with no " -- " at all (the
# older free-text style, e.g. "RELAY-REHOME -- START 2026-08-27" -- no,
# those use an em dash, see below; this fallback is for headings with
# neither).
function lane_token(text,    sepidx, remainder, words, n, stripped, i) {
    sepidx = index(text, " -- ")
    if (sepidx > 0) {
        remainder = substr(text, sepidx + 4)
        n = split(remainder, words, /[ \t]+/)
        if (n >= 1 && words[1] != "") {
            return words[1]
        }
    }
    stripped = text
    gsub(/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/, " ", stripped)
    gsub(/[0-9][0-9]:[0-9][0-9a-zA-Z]?/, " ", stripped)
    gsub(/--/, " ", stripped)
    gsub(/\xe2\x80\x94/, " ", stripped) # em dash, UTF-8
    n = split(stripped, words, /[^A-Za-z0-9-]+/)
    for (i = 1; i <= n; i++) {
        if (words[i] == "") { continue }
        # A plain string/ASCII range compare here is locale-sensitive in awk
        # (case-insensitive collation under some UTF-8 locales quietly let
        # "orchestrator" match as if it were uppercase); a regex match
        # against the POSIX/ASCII class is not.
        if (words[i] ~ /^[A-Z]/) {
            return words[i]
        }
    }
    return ""
}

function status_of(text,    up, uwords, nwords, i, w, found) {
    up = toupper(text)
    nwords = split(up, uwords, /[^A-Z]+/)
    found = "-"
    for (i = 1; i <= nwords; i++) {
        w = uwords[i]
        if (w == "START" || w == "STARTING") { found = "start" }
        if (w == "FINISH" || w == "FINISHED" || w == "DONE" || w == "COMPLETE" || w == "CLOSED") {
            return "finish"
        }
    }
    return found
}

function emit(    date, time, lane, status, out_date) {
    date = ""
    if (match(heading, /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/)) {
        date = substr(heading, RSTART, RLENGTH)
        last_date = date
    }
    time = ""
    if (match(heading, /[0-9][0-9]:[0-9][0-9a-zA-Z]?/)) {
        time = substr(heading, RSTART, RLENGTH)
        # Fuzzy minute ("22:5x") rounds its second digit down to 0, the
        # earliest instant the window could mean -- see PARSING NOTES.
        if (time !~ /^[0-9][0-9]:[0-9][0-9]$/) {
            time = substr(time, 1, 4) "0"
        }
    }
    lane = lane_token(heading)
    if (lane == "") {
        pending = 0
        return
    }
    # A canonical `tools/lane.sh board` heading never carries a status word
    # itself -- the lane wrote its own opening line as the body, right after
    # the blank line the helper inserts -- so the status word has to be
    # looked for there too, not only in the heading.
    status = status_of(heading)
    if (status == "-") {
        status = status_of(body)
    }
    out_date = (date != "") ? date : last_date
    # Field separator is 0x1f (ASCII unit separator), not a tab: tab is one of
    # bash reads three IFS whitespace characters, which collapses consecutive
    # delimiters and silently eats empty fields (a date-only heading with no
    # time has exactly that shape) -- 0x1f is not, so an empty field between
    # two 0x1f bytes reads back as empty, not absent. NOTE: no apostrophes in
    # this awk block, anywhere, ever -- this whole script is one bash single
    # quoted string and an apostrophe here silently closes it early.
    printf "%s\037%s\037%s\037%s\037%s\n", out_date, time, lane, status, heading
    pending = 0
}

/^## / {
    if (pending) { emit() }
    heading = substr($0, 4)
    body = ""
    pending = 1
    next
}
pending && body == "" && $0 !~ /^[ \t]*$/ {
    body = $0
    emit()
    next
}
END {
    if (pending) { emit() }
}
' "$board_file" >"$work/headings.tsv"

# Pass 2 (bash): group by lane token, find each lane's oldest START, check
# whether any later heading (any status) names the same lane again.
declare -A first_start_epoch
declare -A first_start_heading
declare -A last_epoch
declare -A last_status
declare -A post_count
declare -a undated=()

while IFS=$'\037' read -r fdate ftime flane fstatus fheading; do
    [ -z "$flane" ] && continue
    # Only a start/finish-shaped heading could ever matter to the check
    # below; an ordinary undated progress note is not worth reporting.
    if [ -z "$fdate" ]; then
        if [ "$fstatus" != "-" ]; then
            undated+=("$flane"$'\037'"$fheading")
        fi
        continue
    fi
    ts="$fdate ${ftime:-00:00}"
    epoch="$(to_epoch "$ts" 2>/dev/null || true)"
    if [ -z "$epoch" ]; then
        if [ "$fstatus" != "-" ]; then
            undated+=("$flane"$'\037'"$fheading")
        fi
        continue
    fi

    post_count[$flane]=$(( ${post_count[$flane]:-0} + 1 ))
    if [ -z "${last_epoch[$flane]:-}" ] || [ "$epoch" -ge "${last_epoch[$flane]}" ]; then
        last_epoch[$flane]="$epoch"
        last_status[$flane]="$fstatus"
    fi
    if [ "$fstatus" = "start" ]; then
        if [ -z "${first_start_epoch[$flane]:-}" ] || [ "$epoch" -lt "${first_start_epoch[$flane]}" ]; then
            first_start_epoch[$flane]="$epoch"
            first_start_heading[$flane]="$fheading"
        fi
    fi
done <"$work/headings.tsv"

abandoned=0
no_finish=0

echo "board-staleness: $board_file, threshold ${hours}h, as of $(date -r "$now" '+%Y-%m-%d %H:%M' 2>/dev/null || date -d "@$now" '+%Y-%m-%d %H:%M' 2>/dev/null || echo "$now")"
echo

: >"$work/report.txt"
for lane in "${!first_start_epoch[@]}"; do
    start_epoch="${first_start_epoch[$lane]}"
    age=$((now - start_epoch))
    [ "$age" -lt "$threshold_seconds" ] && continue

    age_hours=$((age / 3600))
    posts="${post_count[$lane]:-1}"

    if [ "$posts" -le 1 ]; then
        abandoned=$((abandoned + 1))
        printf 'ABANDONED  %-16s START %dh ago, no post since: %s\n' \
            "$lane" "$age_hours" "${first_start_heading[$lane]}" >>"$work/report.txt"
    elif [ "${last_status[$lane]}" != "finish" ]; then
        no_finish=$((no_finish + 1))
        printf 'NO FINISH  %-16s START %dh ago, %d posts since, none read as a finish: %s\n' \
            "$lane" "$age_hours" "$((posts - 1))" "${first_start_heading[$lane]}" >>"$work/report.txt"
    fi
done
sort "$work/report.txt"

if [ "${#undated[@]}" -gt 0 ]; then
    echo
    echo "UNDATED (could not compute an age -- not counted above):"
    printf '%s\n' "${undated[@]}" | sort -u | while IFS=$'\037' read -r lane heading; do
        printf '  %-16s %s\n' "$lane" "$heading"
    done
fi

echo
echo "$abandoned abandoned, $no_finish started-but-unconfirmed, threshold ${hours}h."

[ "$abandoned" -eq 0 ]
