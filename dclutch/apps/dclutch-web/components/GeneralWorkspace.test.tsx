import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import GeneralWorkspace from './GeneralWorkspace';
import { GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3 } from '@/lib/generated/generalSuccessorV5';

describe('General successor workspace presentation', () => {
  it('renders the full General action lifecycle, status, and receipt workflow without implying execution', () => {
    const html = renderToStaticMarkup(<GeneralWorkspace />);
    expect(html).toContain('General market operator.');
    expect(html).toContain('order collection, candidate verification, settlement, and cleanup');
    expect(html).toContain('not a form for inventing chain facts');
    expect(html).toContain('No RPC request has been made.');
    expect(html).toContain('Inspect one chain-derived operator plan');
    expect(html).toContain('Reacquire exact chain status');
    expect(html).toContain('Verify the commit-last execution receipt');
    expect(html).toContain('Nothing is signed or submitted.');
    expect(html).toContain('candidate verification');
    expect(html).toContain('canonical lifecycle bumps');
    expect(html).toContain('dclutch general plan --route /absolute/route.json --output /absolute/plan.json');
    expect(html).toContain(`${GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3}-coordinate Hot frame`);
    expect(html).not.toContain('Hot38');
    expect(html).not.toContain('settlement transaction remains unavailable');
    expect(html).not.toContain('pending Claims/Custody child wires');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
  });
});
