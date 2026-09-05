import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DealerLiquidityWorkspace, { dealerRouteManifestScaffoldV3 } from './DealerLiquidityWorkspace';
import { DIRECT_HOT_FIXED_ROLE_LABELS_V3 } from '@dclutch/sdk/directHotRouteManifest';
import { HOT_FIXED_ACCOUNT_COUNT_V3, HOT_ROOT_ACCOUNT_V3 } from '@dclutch/sdk/generated/directInlineV3';

describe('Dealer V3 liquidity workbench', () => {
  it('exposes only executable chain-derived equity routes and the explicit wallet boundary', () => {
    const html = renderToStaticMarkup(<DealerLiquidityWorkspace />);
    expect(html).toContain('Check an equity request against the chain');
    expect(html).toContain('Selectors 1–6 only.');
    expect(html).toContain('Dealer equity request');
    expect(html).toContain('produced by the operator program');
    expect(html).toContain(`${HOT_FIXED_ACCOUNT_COUNT_V3} fixed roles + admitted AOT + runtime`);
    expect(html).toContain('Build exact unsigned v0 transaction');
    expect(html).toContain('Sign as transaction payer');
    expect(html).toContain('Download exact packet');
    expect(html).toContain('LP open/close and scenario trading remain hidden');
    expect(html).not.toContain('Submit signed transaction');
    expect(html).not.toContain('mock balance');
    expect(html).not.toContain('sample liquidity');
    expect(html).not.toContain('Hot38');
  });

  it('scaffolds the canonical fixed frame from its one SDK label owner', () => {
    const scaffold = JSON.parse(dealerRouteManifestScaffoldV3()) as {
      fixedAccounts: Array<{ role: string; isSigner: boolean; isWritable: boolean }>;
    };
    expect(scaffold.fixedAccounts).toHaveLength(HOT_FIXED_ACCOUNT_COUNT_V3);
    expect(scaffold.fixedAccounts.map(({ role }) => role)).toEqual(DIRECT_HOT_FIXED_ROLE_LABELS_V3);
    expect(scaffold.fixedAccounts[HOT_FIXED_ACCOUNT_COUNT_V3 - 1]?.role).toBe('Capability seal');
    expect(scaffold.fixedAccounts.filter(({ isWritable }) => isWritable)).toEqual([
      scaffold.fixedAccounts[HOT_ROOT_ACCOUNT_V3],
    ]);
    expect(scaffold.fixedAccounts.some(({ isSigner }) => isSigner)).toBe(false);
  });
});
