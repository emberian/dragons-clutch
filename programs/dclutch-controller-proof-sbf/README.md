# dClutch multiprogram controller experiment

This program is a narrow integration experiment, not a complete trading
controller. It authenticates a global controller PDA and the existing Direct V2
maker replay-root PDA coordinate, increments a 16-byte journal, then calls the
claim executor followed by the real custody adapter. Child failure must roll
back the journal and every earlier child mutation.

It does **not** validate Direct intents, derive the plans itself, authenticate a
release artifact, or load a Realm. Those are explicit successor gates.

The instruction is two PDA bumps, Market key, generation, maker key, the
72-byte claim plan, and the 40-byte custody plan. Accounts are controller PDA,
replay PDA, journal, claim projection, claim program, custody program, Mint,
buyer source, seller destination, venue destination, and token program.
