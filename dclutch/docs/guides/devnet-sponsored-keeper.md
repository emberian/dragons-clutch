# Sponsored Pyth devnet keeper

`tools/release/devnet-sponsored-keeper.py` is a one-step coordinator over the
existing `devnet-sponsored-push-v1` and `devnet-terminal-sequence-v1` callers.
It never constructs protocol instructions, never contacts Hermes or Pyth Price
Service, and has no bearer/API credential input. The sponsored exterior input
must name the fixed finalized SOL/USD account
`7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE`.

The strict keeper spec supplies the existing sponsored input, public signer,
named signer path, and terminal caller inputs. Each invocation selects exactly
one durable action: capture, settle, or the terminal sequence. Without
`--execute` it creates/rechecks only a key-free planned receipt. With
`--execute` the selected existing caller alone reads its named key, records the
signed packet before send, and resumes its own receipt on rerun.

```sh
tools/release/devnet-sponsored-keeper.py \
  --spec /absolute/keeper.json --work /absolute/keeper-receipts

# Only under separate authorization:
tools/release/devnet-sponsored-keeper.py \
  --spec /absolute/keeper.json --work /absolute/keeper-receipts --execute
```

Use `tools/release/stage-story-market-exchange.py --work ABSOLUTE_FRESH_DIR` to
emit the finite flagship, graduation, and abandoned story plan. It uses the
canonical scenario generator, marks adapter-required stages honestly, and
states that any graduation mainnet observation is read-only: it has no mainnet
signer, transaction, or spend path.
