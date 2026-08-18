# Competitive position and product test

## 1. What already exists

Dragon's Clutch does not claim invention of fully collateralized outcome tokens.
Gnosis Conditional Tokens already define collateral-backed positions with valid
partitions, split, merge, nested conditions, and redemption. Nor does it claim
invention of tokenized prediction distribution: DFlow/Kalshi and Jupiter expose
binary prediction products on Solana. Manifest already offers a low-rent,
Token-2022-compatible, permissionless spot orderbook.

Primary references:

- [Gnosis Conditional Tokens split/merge guide](https://ct-docs.gnosis.io/conditionaltokens/docs/devguide05)
- [Jupiter Prediction Market documentation](https://developers.jup.ag/docs/prediction/index)
- [DFlow tokenized Kalshi markets](https://dflow.net/blog/prediction-markets-api)
- [Kalshi's Solana tokenization announcement](https://news.kalshi.com/p/kalshi-solana-tokenized-predictions)
- [Manifest source and design](https://github.com/Bonasa-Tech/manifest)

Continuous/range prediction designs add a useful aspirational benchmark:
discretized range claims, graded payouts, and passive single-sided liquidity.
Clutch already contains that finite payoff family—and a strictly larger bounded
coefficient language—at an equal admitted grid. The open differentiation is
[proof-constrained passive range liquidity](design/continuous-claims/README.md).
The design refuses variable-depth quote fields without one convex potential,
stacked independent pricing curves, uncapitalized insurance, and
VaR/liquidation as a solvency substitute.

The project should rerun this primary-source comparison before any public novelty
claim. A protocol can be valuable without pretending every primitive is new.

## 2. Actual alternatives for a user

| Need | Existing alternative | Clutch must be better or different by |
|---|---|---|
| Bet on a popular yes/no event | Regulated prediction venue or tokenized Kalshi/Jupiter market | Clutch probably should not compete here |
| Issue collateralized conditional tokens | Gnosis Conditional Tokens | Objective compiler, Solana cost/composability, verification, shared observations |
| Trade one outcome token | Manifest or another spot/RFQ/AMM venue | Use the existing venue; do not rebuild it |
| Directional token exposure | Spot/perpetual/futures | Bounded loss, no liquidation, exact non-linear/path payoff |
| Standard option payoff | Options venue or OTC/RFQ | Permissionless token-specific partitions where standard markets do not exist |
| Hedge LP/treasury path risk | Dynamic spot/perp hedging or bespoke OTC | Finite prepaid state payoff, objective settlement, composable claim |
| Estimate an entire future distribution | Many binaries or an options surface | One exhaustive basis and one coherent simplex |
| Run an agent against market beliefs | Hosted trading APIs | Self-verifying onchain artifacts and static-client transaction construction |

## 3. Distinctive conjunction

The product thesis depends on a conjunction rather than one buzzword:

1. **State-space compiler.** A closed, audited program compiles onchain source and
   path statistics into canonical exhaustive partitions.
2. **Basis rather than question.** One Clutch supports many exact payoff vectors
   and exposes a full distribution.
3. **Coupled native clearing.** The simplex auction prices all outcomes together,
   uses complete-set conversion, and accepts bounded atomic payoff intents.
4. **Cheap composability frontier.** Internal balances make native use cheap;
   selected Eggs materialize as standard Token-2022 assets for Manifest/Jupiter-
   eligible venue adapters.
5. **Prepaid truth work.** Shared accumulator obligations are capitalized even if
   trading volume disappears.
6. **Named verification boundary.** Rocq proves the abstract algebra, Verus checks
   the executable kernel, and the unverified SBF adapter is kept small and tested.
7. **No required operator backend.** Source, work, auction verification, and static
   Glass are permissionless and reproducible.

If we remove the compiler and coupled auction, the result is mainly a careful
Solana reimplementation of established conditional-token ideas. That could still
be useful, but it is not the ambitious project described here.

## 4. Algorithmic research claims

Potentially distinctive work includes:

- exact simplex candidate verification with virtual complete-set conversion;
- a tractable proportional payoff-intent language;
- best-submitted-candidate competition with deterministic public score;
- the state-contingent Gini fee that generalizes `q*p*(1-p)`, ignores risk-free
  complete sets, and remains invariant to identical-payoff partition refinement;
- shared monoidal path accumulators tied directly to partition compiler proofs;
- formal correspondence among partition, issuance, auction, and settlement.

These are research hypotheses, not established novelty. Before publication as
novel algorithms, conduct a dedicated literature, protocol, and patent search
covering combinatorial prediction markets, pari-mutuel mechanisms, Arrow-Debreu
securities, call auctions, automated market makers, securities lending/netting,
proper scoring rules, and Gini/energy risk measures.

## 5. The product-quality bar

The best possible Clutch should:

- make one useful token-risk hedge possible in fewer concepts and transactions
  than constructing it manually;
- show exact worst-state payout before signing;
- keep ordinary split/merge/redeem free;
- quote coherent state prices and label external disagreement honestly;
- remain usable from a static accessible client;
- settle without Dragon intervention even if the team and venue volume vanish;
- expose proofs, costs, failure incentives, and deployed byte identity;
- interoperate rather than compete gratuitously with excellent Solana primitives;
- give JOSHI and other agents strict artifact contracts, not a privileged API;
- refuse unsupported sources, collateral profiles, basket languages, or legal
  deployment claims instead of broadening by aspiration.

## 6. Bootstrap hypothesis

The narrowest credible wedge is repeated crypto-native distribution surfaces, not
one-off headlines:

- a small family of terminal-price and drawdown Templates;
- 4–8 states, several fixed horizons, collateral chosen per Realm;
- one shared source/window substrate;
- exact crash/range/tail portfolios;
- native simplex auction plus optional Manifest materialization;
- JOSHI-style distributional analysis and read-only agent interface.

Recurring standardized Instances permit calibration, habitual liquidity, solver
reuse, and coherent surfaces. If this wedge does not produce counterparties, the
project should remain a high-quality protocol public good rather than manufacture
wash activity or token emissions.
