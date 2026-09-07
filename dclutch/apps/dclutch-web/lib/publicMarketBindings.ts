import { PublicKey } from '@solana/web3.js';
import source from '@/fixtures/public-market-bindings-v1.json';

export const PUBLIC_MARKET_BINDINGS_SCHEMA_V1 = 'dclutch-public-market-bindings-v1';

type Binding = Readonly<{ linkedBasisRecordDigest: string; foundingReportSha256: string }>;

function parse(value: unknown): ReadonlyMap<string, Binding> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('public market bindings must be one object');
  const root = value as Record<string, unknown>;
  if (root.schema !== PUBLIC_MARKET_BINDINGS_SCHEMA_V1 || root.markets === null || typeof root.markets !== 'object' || Array.isArray(root.markets)) throw new Error('public market bindings have another schema');
  const out = new Map<string, Binding>();
  for (const [market, raw] of Object.entries(root.markets as Record<string, unknown>)) {
    if (new PublicKey(market).toBase58() !== market || raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('public market binding has a noncanonical market');
    const entry = raw as Record<string, unknown>;
    const digest = entry.linked_basis_record_digest;
    const report = entry.founding_report_sha256;
    if (typeof digest !== 'string' || typeof report !== 'string' || !/^[0-9a-f]{64}$/.test(digest) || !/^[0-9a-f]{64}$/.test(report)) throw new Error(`public market binding ${market} has invalid digest`);
    out.set(market, Object.freeze({ linkedBasisRecordDigest: digest, foundingReportSha256: report }));
  }
  return out;
}
const BINDINGS = parse(source);
export function publicFirstAdmissionBindingV1(market: string): Binding | undefined { return BINDINGS.get(market); }
