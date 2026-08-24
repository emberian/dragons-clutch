# Operator Attach

This directory is the small read-only operator surface for the current
chain-attached Dragon's Clutch client. Open it through the loopback operatord
static server and enter that server's explicit URL. The page does not infer a
cluster, release, account, cursor, wallet, or transaction from fixtures or
browser persistence.

Attachment brackets `/v1/actions` with two `/v1/session` reads. A session is
accepted only when its RPC bindings, genesis hash, Program/ProgramData,
deployment slot, ELF digest, release-manifest digest, capability-profile ID,
enabled-intent set, canonical decoder set, finalized account identities, and
onchain-derived restart cursors agree.

An enabled release coordinate is not automatically callable. Its control stays
disabled until operatord supplies exactly one opaque semantic-owner construction
which also agrees with the current finalized cursor. A callable projection
contains:

- the semantic owner's exact ordered account roles and privileges;
- explicit public signer addresses and semantic roles, with no key access;
- one deterministic serialized transaction with a zero/absent recent blockhash;
- an explicit fee payer selected by the semantic role contract;
- exact-integer equations as decimal strings; and
- observed/valid-before slot bounds plus mandatory pre-sign and post-submit
  reacquisition rules.

The page can only inspect that unsigned draft. It has no validator RPC client,
wallet adapter, key reader, blockhash acquisition, signing, submission, or
optimistic poststate path. After any future external submission, the draft must
be discarded and both session and actions reacquired from canonical onchain
state.

The relevant files are `index.html`, `styles.css`, and `app.js`. The remaining
legacy modules are not loaded by this attach document and are not a fallback
data source.
