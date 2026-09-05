import Anchor from '@/components/Anchor';
import ExposureBand, { type ExposureBandRowV1 } from '@/components/charts/ExposureBand';
import NumberStrip, { type NumberStripStatV1 } from '@/components/charts/NumberStrip';
import { type BundleV1, type BundleExposureV1 } from '@dclutch/sdk/bundleExposure';
import { shortAddressV1 } from '@dclutch/sdk/marketDiscovery';
import { marketDetailHrefV1 } from '@dclutch/sdk/marketHref';

/**
 * What the positions above can pay, taken together.
 *
 * The panel exists because the true answer across markets is usually the sum,
 * and saying so out loud is worth a section. Every figure on it is an exact
 * integer of collateral atoms derived from balances this browser read; nothing
 * is estimated, and where an exact answer needs a record this surface has not
 * read, the panel says which record rather than filling the gap.
 */

function statsV1(bundle: BundleV1): ReadonlyArray<NumberStripStatV1> {
  const base: NumberStripStatV1[] = [
    Object.freeze({
      label: 'Arrives whatever happens',
      value: bundle.floorAtoms,
      detail: 'The smallest claim balance in each Position, added up. Every outcome pays at least this, and so does a Market that never resolves at all.',
    }),
    Object.freeze({
      label: 'The most it can pay',
      value: bundle.ceilingAtoms,
      detail: bundle.sharedTerms
        ? 'The largest claim balance in each Position, added up. Some of these Markets are locked to each other, so a narrower ceiling holds while they all resolve — it is beside this one, not folded into it.'
        : 'The largest claim balance in each Position, added up — and with these Markets settling against different things, that sum is exactly attainable rather than a safe overstatement.',
    }),
    Object.freeze({
      label: 'Decided by the outcomes',
      value: bundle.swingAtoms,
      detail: 'The gap between the two figures beside this one: the part of the bundle that the results actually move. The rest is already yours.',
    }),
  ];
  if (!bundle.sharedTerms) return Object.freeze(base);
  return Object.freeze([...base, Object.freeze({
    label: 'Cannot both be paid',
    value: bundle.releaseAtoms,
    detail: `Markets here that settle against the same thing cannot both land on their best case unless it is the same case. While every one of them resolves, the ceiling is ${bundle.coResolvedCeilingAtoms} and the floor is ${bundle.coResolvedFloorAtoms}.`,
  })]);
}

function rowsV1(bundle: BundleV1): ReadonlyArray<ExposureBandRowV1> {
  const legs = bundle.legs.map((leg) => Object.freeze({
    label: shortAddressV1(leg.marketAddress, 6),
    floorAtoms: leg.floorAtoms,
    ceilingAtoms: leg.ceilingAtoms,
    emphasis: false,
    note: leg.settled ? 'this Market has settled' : 'this Market has not settled',
  }));
  return Object.freeze([
    Object.freeze({
      label: `All ${bundle.legs.length} together`,
      floorAtoms: bundle.floorAtoms,
      ceilingAtoms: bundle.ceilingAtoms,
      emphasis: true,
      note: bundle.sharedTerms ? 'the sum of the legs; a narrower conditional ceiling is marked' : 'the exact sum of the legs',
    }),
    ...legs,
  ]);
}

function Bundle({ bundle }: Readonly<{ bundle: BundleV1 }>) {
  return <article className="bundle-exposure">
    <div className="market-card-top">
      <strong>{bundle.legs.length} Position{bundle.legs.length === 1 ? '' : 's'}</strong>
      <span className="provenance-chip" title={bundle.collateralMint}>{bundle.collateralMintShort} · one collateral mint</span>
    </div>
    <NumberStrip
      stats={statsV1(bundle)}
      provenance="Raw u64 collateral atoms, added and compared in exact integers. There is no division anywhere in this arithmetic, so there is nothing here that could have been rounded in either direction."
    />
    <p className="bundle-exposure-line">{bundle.headline}</p>
    <ExposureBand
      rows={rowsV1(bundle)}
      scaleAtoms={bundle.ceilingAtoms}
      conditionalCeilingAtoms={bundle.sharedTerms ? bundle.coResolvedCeilingAtoms : null}
      conditionalLabel={bundle.sharedTerms ? 'ceiling while every locked Market resolves' : null}
      caption="One row per Position and one for all of them. Each band runs from what that Position pays under its worst outcome to what it pays under its best; the whole scale is what the bundle can pay together."
      emptyReason="Every Position in this bundle holds nothing, so there is no band to draw."
    />
    <p className="bundle-exposure-line">{bundle.netting}</p>
    {bundle.clusters.map((cluster) => <div key={cluster.termsKey} className={cluster.status === 'locked' ? 'portfolio-claim' : 'market-capability-refusal'}>
      {cluster.status === 'locked'
        ? <>
          <span>Locked to each other · {cluster.marketAddresses.length} Markets</span>
          <strong>{cluster.jointCeilingAtoms}</strong>
          <p>{cluster.note}</p>
          <ul className="bundle-exposure-members">
            {cluster.marketAddresses.map((address) => <li key={address}>
              <Anchor href={marketDetailHrefV1(address)} title={address}>{shortAddressV1(address, 8)}</Anchor>
            </li>)}
          </ul>
        </>
        : <><span>no netting claimed</span>{cluster.reason}</>}
    </div>)}
    <p className="bundle-exposure-line">{bundle.settlement}</p>
  </article>;
}

export default function BundleExposurePanel({ exposure }: Readonly<{ exposure: BundleExposureV1 }>) {
  return <>
    <p className="direct-status">{exposure.reason}</p>
    {exposure.bundles.map((bundle) => <Bundle key={bundle.collateralMint} bundle={bundle} />)}
    {exposure.excluded.length > 0 && <div className="market-capability-refusal">
      <span>left out of every bundle</span>
      <ul className="bundle-exposure-members">
        {exposure.excluded.map((item) => <li key={item.marketAddress}>
          <Anchor href={marketDetailHrefV1(item.marketAddress)} title={item.marketAddress}>{shortAddressV1(item.marketAddress, 8)}</Anchor>
          <small>{item.reason}</small>
        </li>)}
      </ul>
    </div>}
    <p className="market-empty">{exposure.boundary}</p>
  </>;
}
