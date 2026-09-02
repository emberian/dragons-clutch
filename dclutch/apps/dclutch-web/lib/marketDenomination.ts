import { type MarketCollateralV1, type MarketHoardV1 } from './marketDiscovery';
import { type DenominationV1 } from './quantity';

/**
 * The display denomination for one Market's collateral.
 *
 * Two surfaces need the same answer -- the market page and the market list --
 * and until this module existed only the page had it. So the detail page
 * printed "500 collateral" out of the mint's own decimals byte while the card
 * for the same market printed `500000000`, on the same site, three clicks
 * apart. A quantity has one scale; the two readings were the same scale twice,
 * written once.
 *
 * The mint address falls through to the Realm binding when the Hoard was not
 * derived, and to the empty string when neither read, which is the honest
 * reading of "not read" rather than a placeholder address. The unit stays null
 * until something names one; absent that, nothing is invented and the word is
 * `collateral`.
 */
export function collateralDenominationV1(hoard: MarketHoardV1, collateral: MarketCollateralV1): DenominationV1 {
  return Object.freeze({
    decimals: hoard.status === 'derived' ? hoard.mintDisplayDecimals : null,
    unit: null,
    mint: hoard.status === 'derived'
      ? hoard.collateralMint
      : collateral.status === 'bound' ? collateral.collateralMint : '',
  });
}
