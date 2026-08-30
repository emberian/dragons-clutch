# Geist subsets for share-card generation

`geist-regular.ttf`, `geist-bold.ttf`, and `geist-mono-regular.ttf` are
static latin-subset instances of the Geist and Geist Mono variable fonts
(Copyright 2024 The Geist Project Authors,
https://github.com/vercel/geist-font), instantiated with fonttools
(`wght` pinned at 400/700) from the same woff2 subsets the app itself
serves via `next/font`. Licensed under the SIL Open Font License 1.1 —
full text in `OFL.txt` beside this file.

They exist only for `scripts/og-cards.sh` (authoring-time share-card
composition) so the cards render in the same face as the site. They are
not part of the app bundle and not an npm dependency.
