# Dragon's Clutch market-quality kernel

This pure kernel turns an already-admitted exact simplex price vector and a
nonnegative native Egg coefficient vector into a small, checked economic
certificate. It reports:

- the exact integer numerator and denominator of the simplex mark;
- the unique collateral-atom floor/ceiling envelope around that rational;
- the complete-set (guaranteed) payoff floor;
- the full-simplex payoff cap; and
- the contingent range between the floor and cap.

The distinction matters for Dragon's Clutch. A native coefficient portfolio
is not a bag of unrelated categorical tokens: at resolution its payout is a
convex combination of its Egg coefficients. The minimum coefficient is
therefore a fully backed complete-set floor, and the coefficient range is the
remaining state-contingent exposure. These bounds are exact over the full
simplex. A smooth basis can have a smaller reachable payout set, so this crate
does not claim tighter basis-specific bounds.

## Trust boundary

This crate is `no_std`, allocation-free, safe Rust, and independent of Solana.
It authenticates no account, oracle, candidate, order, or basis certificate.
In particular, an exact simplex is not necessarily a fair price. An adapter
must first obtain and authenticate the price and payoff vectors from their
semantic owners.

There is exactly one rounding boundary: the final conversion from exact
`price-unit * collateral-atom / price-scale` units to whole collateral atoms.
The certificate exposes both floor and ceiling plus the retained exact
remainder. It does not silently choose which party receives a remainder.
