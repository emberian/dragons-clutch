import { describe, expect, it } from 'vitest';

import checkpointJson from '../fixtures/successor-checkpoint.json';
import {
  LOCAL_SUCCESSOR_CHECKPOINT,
  decodeCheckpointFixtureAccount,
  decodeLocalSuccessorCheckpoint,
  parseSuccessorAccount,
} from './localSuccessor';
import type { RpcAccount } from './rpc';

function fixture(name: keyof typeof checkpointJson.parser_fixtures.accounts): RpcAccount {
  return decodeCheckpointFixtureAccount(checkpointJson.parser_fixtures.accounts[name].account);
}

function mutate(account: RpcAccount, offset: number, value: number): RpcAccount {
  const data = account.data.slice();
  data[offset] = value;
  return Object.freeze({ ...account, data });
}

describe('immutable localhost successor checkpoint', () => {
  it('binds one loopback genesis, immutable Loader programs, and explicit evidence limits', () => {
    expect(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url).toBe('http://127.0.0.1:20890/');
    expect(LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash).toBe('F6x7Lf6PBNu5e8cecisH2o25y9cj4xW9xd4iZzWKZeqn');
    expect(Object.keys(LOCAL_SUCCESSOR_CHECKPOINT.expected_accounts)).toHaveLength(33);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.expected_transactions).toHaveLength(9);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.evidence.checked_production_release_claimed).toBe(false);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.evidence.captured_release_identity_claimed).toBe(false);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.programs.registry.upgrade_authority).toBeNull();
    expect(LOCAL_SUCCESSOR_CHECKPOINT.programs.resolution.upgrade_authority).toBeNull();
  });

  it('decodes representative live RPC bodies through exact account layouts', () => {
    const activation = parseSuccessorAccount('registry.activation', fixture('registry.activation'));
    const certificate = parseSuccessorAccount('primary.certificate.success', fixture('primary.certificate.success'));
    const state = parseSuccessorAccount('lifecycle.state', fixture('lifecycle.state'));
    const funding = parseSuccessorAccount('lifecycle.funding.failure', fixture('lifecycle.funding.failure'));
    const hostile = parseSuccessorAccount('rollback.certificate.failure.occupied', fixture('rollback.certificate.failure.occupied'));

    expect([activation.kind, activation.headline]).toEqual(['Registry activation cache', 'five checked roles']);
    expect([certificate.kind, certificate.headline]).toEqual(['signed Resolution certificate', 'primary success']);
    expect([state.kind, state.headline]).toEqual(['Source resolution state', 'failure committed']);
    expect([funding.kind, funding.headline]).toEqual(['typed capability funding', 'active']);
    expect([hostile.kind, hostile.headline]).toEqual(['hostile preoccupied certificate', 'deliberately malformed']);
  });

  it('refuses reserved bytes, invalid certificate kinds, and substituted hostile output', () => {
    expect(() => parseSuccessorAccount('registry.activation', mutate(fixture('registry.activation'), 12, 1))).toThrow('reserved');
    expect(() => parseSuccessorAccount('primary.certificate.success', mutate(fixture('primary.certificate.success'), 10, 0))).toThrow('certificate');
    expect(() => parseSuccessorAccount('rollback.certificate.failure.occupied', mutate(fixture('rollback.certificate.failure.occupied'), 311, 0))).toThrow('occupied pattern');
  });

  it('refuses a checkpoint that promotes localhost evidence into a release claim', () => {
    const promoted = structuredClone(checkpointJson) as unknown as Record<string, unknown>;
    (promoted.evidence as Record<string, unknown>).checked_production_release_claimed = true;
    expect(() => decodeLocalSuccessorCheckpoint(promoted)).toThrow('must not claim production');
  });
});
