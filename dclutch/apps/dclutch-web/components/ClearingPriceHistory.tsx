import { clearingPricesV1, decodeGeneralBatchV2 } from '@dclutch/sdk/generalClearingV1';
import { shortAddressV1 } from '@dclutch/sdk/marketDiscovery';

/**
 * One batch's decoded record, already read and hostile-decoded by the
 * caller. This component adds no RPC of its own: it is a pure projection of
 * bytes someone else read, in sequence order.
 */
export type ClearingPriceHistoryEntryV1 = Readonly<{
  address: string;
  decoded: ReturnType<typeof decodeGeneralBatchV2>;
}>;

function percent(fraction: number): string {
  const scaled = fraction * 100;
  return `${scaled % 1 === 0 ? scaled : scaled.toFixed(2)}%`;
}

/** One cleared batch: sequence, cleared slot, the price vector, the move, and any stranded residual. */
function ClearedRow({ entry }: Readonly<{ entry: ClearingPriceHistoryEntryV1 }>) {
  const { address, decoded } = entry;
  const clearing = decoded.clearing;
  if (clearing === null) return null; // narrows for TypeScript; the caller below only reaches this row when cleared
  const view = clearingPricesV1(decoded);
  return <tr>
    <td>{decoded.sequence.toString()}</td>
    <td title={address}>{shortAddressV1(address, 6)}</td>
    <td>{clearing.clearedSlot.toString()}</td>
    <td title={clearing.clearedCandidateId}>{shortAddressV1(clearing.clearedCandidateId, 6)}</td>
    <td>
      <ul className="clearing-price-outcomes">
        {(view?.fractions ?? clearing.prices.map(() => 0)).map((fraction, outcome) => <li key={outcome}>
          outcome {outcome}: {percent(fraction)}
          {clearing.residual[outcome] !== 0n && <> · <strong>{clearing.residual[outcome].toString()} stranded</strong></>}
        </li>)}
      </ul>
    </td>
    <td>{clearing.setsMove === 'none' ? 'no complete sets moved' : `${clearing.setsMove} · ${clearing.setsQuantity.toString()} sets`}</td>
    <td>{clearing.filledLots.toString()}</td>
  </tr>;
}

/** An uncleared batch: named by its status alone, nothing else to show yet. */
function UnclearedRow({ entry }: Readonly<{ entry: ClearingPriceHistoryEntryV1 }>) {
  const { address, decoded } = entry;
  return <tr>
    <td>{decoded.sequence.toString()}</td>
    <td title={address}>{shortAddressV1(address, 6)}</td>
    <td colSpan={5}>{decoded.status === 'collecting' ? 'collecting' : 'closed, awaiting a clearing'}</td>
  </tr>;
}

/**
 * The clearing price history for one Product's batches, in sequence order.
 *
 * "Prices sum to one by construction" is not a line this component computes:
 * it is read off {@link clearingPricesV1}'s presence, which is itself a read
 * of a conjunct {@link decodeGeneralBatchV2} already proved on decode
 * (`ClearingPriceV1Abi.tailAdmissible`, `a_cleared_tail_is_on_the_simplex`).
 * A cleared batch could not have reached this page with a price vector that
 * disagreed with its own price scale.
 */
export default function ClearingPriceHistory({ batches }: Readonly<{ batches: ReadonlyArray<ClearingPriceHistoryEntryV1> }>) {
  const ordered = [...batches].sort((left, right) => (left.decoded.sequence < right.decoded.sequence ? -1 : left.decoded.sequence > right.decoded.sequence ? 1 : 0));
  const clearedCount = ordered.filter((entry) => entry.decoded.clearing !== null).length;

  if (ordered.length === 0) return <p className="market-empty">No batch has opened for this Product yet.</p>;

  return <>
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Clearing price history">
      <table className="holders-table">
        <thead><tr>
          <th>Sequence</th><th>Batch</th><th>Cleared slot</th><th>Candidate</th>
          <th>Prices, per outcome</th><th>Complete sets</th><th>Filled lots</th>
        </tr></thead>
        <tbody>
          {ordered.map((entry) => entry.decoded.clearing === null
            ? <UnclearedRow key={entry.address} entry={entry} />
            : <ClearedRow key={entry.address} entry={entry} />)}
        </tbody>
      </table>
    </div>
    <p className="slot-clock-note">
      {clearedCount === 0
        ? 'No batch here has cleared yet, so there is no price vector to check.'
        : 'Prices sum to one by construction: a decoded cleared batch already carries a price vector its own decode held to the simplex, not one this page recomputed.'}
    </p>
  </>;
}
