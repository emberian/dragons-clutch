#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT; mkdir -p "$WORK/bin" "$WORK/state"
printf '%s\n' '#!/usr/bin/env bash' 'echo EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG' > "$WORK/bin/solana"
printf '%s\n' '#!/usr/bin/env bash' 'if [ "$1" = devnet-user-position-admission-v1 ]; then echo admission; else echo direct; fi' > "$WORK/bin/boot"
printf '%s\n' '#!/usr/bin/env bash' 'echo "{\"status\":\"finalized\",\"signature\":\"MockPayout111111111111111111111111111111111111111111111111111111111111111111\"}"' > "$WORK/bin/dclutch"
chmod +x "$WORK/bin"/*; for f in owner fee plan evidence direct payout session; do printf x > "$WORK/$f"; done
printf 'one\tOwner111\t%s\tFee111\t%s\t%s\t%s\t1\t%s\t%s\tRecipient111\t%s\n' "$WORK/owner" "$WORK/fee" "$WORK/plan" "$WORK/evidence" "$WORK/admission" "$WORK/direct" "$WORK/payout" > "$WORK/cycles.tsv"
PATH="$WORK/bin:$PATH" "$ROOT/tools/release/devnet-demo-pulse.sh" --rpc-url https://example.invalid --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG --state-dir "$WORK/state" --cycles "$WORK/cycles.tsv" --bootstrap-bin "$WORK/bin/boot" --session "$WORK/session" --market Market111 --public-manifest "$WORK/public.json" --execute >/dev/null
grep -q 'dclutch-devnet-demo-pulse-public-v1' "$WORK/public.json"; grep -q MockPayout "$WORK/public.json"; test -s "$WORK/state/cycles/one/payout.signature"; echo 'devnet demo pulse mock test: PASS'
