# What happened?

Nothing bad happened. I have not received a letter, deployed a venue, accepted
funds, or discovered an existing compliance problem. This is a pre-deployment
design question, not self-reporting.

I became interested in building an open-source Solana protocol called **Dragon's
Clutch**. A user would deposit collateral into an onchain program vault and
receive a complete set of claims over an exhaustive, disjoint partition of an
objective future state—for example, several token-price bands at a fixed time.
Every allowed payout would be fully collateralized in advance. There would be no
debt, margin, liquidation, discretionary resolver, or human custody. The initial
subject matter would be deterministic crypto-native facts such as token prices,
ranges, crossings, or path statistics, not politics or subjective events.

The design also contemplates an onchain batch venue for trading those claims, a
permissionless observation and settlement mechanism, and a static GitHub
Pages/IPFS client. That makes it technically elegant and potentially independent
of a conventional backend operator, but it does not answer the legal
classification. Current CFTC materials say price- and event-contingent claims may
be swaps, futures, or commodity options, and that a public multilateral trading
facility may require DCM or SEF registration even when blockchain-based and fully
collateralized.

So, before I spend serious development resources or deploy anything, I want to
ask the CFTC Innovation Task Force how it would classify the exact architecture,
who it would regard as operating it, and whether there is a viable no-action,
exemptive, limited-pilot, registered-partnership, or registration path that would
let a small open-source developer deploy a bounded version to Solana mainnet.
`$DREGG` is one possible collateral profile; the protocol is collateral-generic,
and the inquiry is not about the token itself.

The “bad news” is only that I am a baby protocol engineer approaching a very
large regulatory vocabulary for the first time. :joy:

