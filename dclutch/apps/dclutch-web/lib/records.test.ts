import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/record-pda.json';
import { deriveRecordAddresses } from './records';

describe('finalized record address derivation', () => {
  it('matches the canonical Rust PDA fixture', () => {
    expect(deriveRecordAddresses(
      fixture.programId,
      fixture.record.schemaReleaseIdHex,
      fixture.record.contentDigestHex,
    )).toEqual({ raw: fixture.record.rawAddress, staging: fixture.record.stagingAddress });
  });

  it('refuses malformed identity bytes', () => {
    expect(() => deriveRecordAddresses(fixture.programId, '00', fixture.record.contentDigestHex)).toThrow();
    expect(() => deriveRecordAddresses(fixture.programId, fixture.record.schemaReleaseIdHex, 'ff')).toThrow();
  });
});
