#!/usr/bin/env sh
# Share-card generator: composes public/og/*.jpg from the committed key art,
# the committed Geist subsets, and the market registry's own titles.
#
# AUTHORING-TIME, not build-time: the outputs are committed, the Pages build
# only serves them, and this script exists so the derivation is reproducible
# and re-runnable when the registry gains a market. It needs ImageMagick 7
# (`magick`) and node on PATH; neither joins the app's dependency closure.
# Small byte drift across ImageMagick versions is expected and fine — the
# cards are presentation, not fixtures; nothing verifies them byte-exactly.
#
# Run from apps/dclutch-web:  sh scripts/og-cards.sh
set -eu

ART=public/art/dragons-clutch-key-art-v1.png
FONTS=scripts/og/fonts
GB="$FONTS/geist-bold.ttf"
GR="$FONTS/geist-regular.ttf"
GM="$FONTS/geist-mono-regular.ttf"
OUT=public/og
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$OUT"

# The shared 1200x630 backdrop: a center crop of the key art with a scrim
# rising toward the right edge so the text zone stays legible.
magick "$ART" -gravity center -crop 1672x878+0+20 +repage -resize 1200x630 "$WORK/base.png"
magick "$WORK/base.png" \
  \( -size 630x1200 gradient:'rgba(4,10,7,0)-rgba(4,10,7,0.92)' -rotate 90 \) \
  -compose over -composite "$WORK/scrimmed.png"

# The site-wide card.
magick -background none -font "$GB" -pointsize 96 -fill '#b9ff64' label:'dC' "$WORK/wm1.png"
magick -background none -font "$GB" -pointsize 96 -fill '#e9eee9' label:'lutch' "$WORK/wm2.png"
magick "$WORK/wm1.png" "$WORK/wm2.png" +append -background none "$WORK/wordmark.png"
magick -background none -font "$GR" -pointsize 34 -fill '#c7d3c9' label:'Fully collateralized markets' "$WORK/tag1.png"
magick -background none -font "$GR" -pointsize 34 -fill '#c7d3c9' label:'on real-world numbers' "$WORK/tag2.png"
magick -background none -font "$GM" -pointsize 21 -fill '#9ba79d' label:'Solana devnet · nothing for sale' "$WORK/foot1.png"
magick -background none -font "$GM" -pointsize 23 -fill '#b9ff64' label:'clutch.dregg.pro' "$WORK/foot2.png"
magick -background none -size 320x24 xc:none "$WORK/sp.png"
magick -background none -size 320x14 xc:none "$WORK/sps.png"
magick "$WORK/wordmark.png" "$WORK/sps.png" "$WORK/tag1.png" "$WORK/tag2.png" \
  "$WORK/sp.png" "$WORK/foot1.png" "$WORK/sps.png" "$WORK/foot2.png" \
  -background none -gravity East -append "$WORK/block.png"
magick "$WORK/scrimmed.png" "$WORK/block.png" -gravity East -geometry +56+0 \
  -compose over -composite -quality 88 "$OUT/site-card-v1.jpg"

# One card per registered market, titled from the registry itself so the
# cards can never drift from the words the pages render.
#
# THE ROWS ARE DERIVED IN A MODULE `npm test` RUNS: scripts/og-cards.mjs. They
# used to be derived inside a `node -e` string, and when the registry stopped
# writing titles for live markets that string threw on the first titleless row
# and this loop produced NO cards at all -- not a bad card, none -- because it
# is a shell script nothing in the suite runs. `lib/ogCards.test.ts` now runs
# the derivation against the shipped registry, and asserts that this file still
# calls it.
node scripts/og-cards.mjs --rows | while IFS="$(printf '\t')" read -r ADDRESS LEAD REST; do
  magick -background none -font "$GB" -pointsize 40 -fill '#b9ff64' label:'dC' "$WORK/mwm1.png"
  magick -background none -font "$GB" -pointsize 40 -fill '#e9eee9' label:'lutch' "$WORK/mwm2.png"
  magick "$WORK/mwm1.png" "$WORK/mwm2.png" +append -background none "$WORK/mwordmark.png"
  magick -background none -font "$GB" -pointsize 62 -fill '#e9eee9' label:"$LEAD" "$WORK/mt1.png"
  if [ -n "$REST" ]; then
    magick -background none -font "$GR" -pointsize 38 -fill '#c7d3c9' label:"$REST" "$WORK/mt2.png"
  else
    magick -background none -size 1x1 xc:none "$WORK/mt2.png"
  fi
  magick -background none -font "$GM" -pointsize 20 -fill '#9ba79d' label:'a market on Solana devnet · nothing for sale' "$WORK/mf1.png"
  magick -background none -font "$GM" -pointsize 23 -fill '#b9ff64' label:'clutch.dregg.pro' "$WORK/mf2.png"
  magick -background none -size 320x30 xc:none "$WORK/msp.png"
  magick -background none -size 320x10 xc:none "$WORK/msps.png"
  magick "$WORK/mwordmark.png" "$WORK/msp.png" "$WORK/mt1.png" "$WORK/msps.png" "$WORK/mt2.png" \
    "$WORK/msp.png" "$WORK/mf1.png" "$WORK/msps.png" "$WORK/mf2.png" \
    -background none -gravity East -append "$WORK/mblock.png"
  magick "$WORK/scrimmed.png" "$WORK/mblock.png" -gravity East -geometry +56+0 \
    -compose over -composite -quality 88 "$OUT/market-$ADDRESS.jpg"
  echo "wrote $OUT/market-$ADDRESS.jpg"
done

# The landing display cut: same art, one tenth the bytes.
magick "$ART" -quality 82 -define webp:method=6 "$OUT/../art/dragons-clutch-key-art-v1-1672w.webp"
echo "wrote public/art/dragons-clutch-key-art-v1-1672w.webp"
echo "done"
