# dClutch controller authority-membrane experiment

This program is a narrow integration experiment, not a deployable trading
controller. It proves one physical architecture claim: a controller program can
authenticate its own PDA, mutate caller-owned state, and lend that PDA signer to
the separately owned exact-account Effect executor through CPI. A child refusal
must roll back both the child projection and the caller mutation.

It does **not** validate Direct intents, prove semantic admission, authenticate a
release artifact, transfer collateral, or establish that Solana's loader/runtime
implements the abstract atomic-commit model. Those are separate successor gates.

Accounts are exactly:

1. read-only controller-authority PDA;
2. writable controller-owned journal;
3. writable claim projection;
4. read-only executable exact-account claim program.

Instruction data is one bump byte followed by one 72-byte Effect V1 claim plan. The
controller derives `PDA("dclutch-controller-v1", bump)` under its runtime program
ID and forwards only the claim plan.
