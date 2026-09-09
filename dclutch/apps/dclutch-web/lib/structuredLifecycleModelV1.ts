import { PublicKey } from '@solana/web3.js';

/** UI intent only. Rust owns selection, account discovery, rent and instructions. */
export const STRUCTURED_LIFECYCLE_ACTIONS_V1 = [
  'activate-receipt', 'activate-coordinate', 'retire-coordinate', 'retire-receipt',
] as const;
export type StructuredLifecycleActionV1 = (typeof STRUCTURED_LIFECYCLE_ACTIONS_V1)[number];

export type StructuredLifecycleIntentV1 = Readonly<{
  market: string;
  payer: string;
  action: StructuredLifecycleActionV1;
  coordinate: number | null;
  selectedCapability: string | null;
  representationDescriptor: string | null;
  expectedPosition: string | null;
}>;

export const STRUCTURED_LIFECYCLE_LABELS_V1: Readonly<Record<StructuredLifecycleActionV1, string>> = Object.freeze({
  'activate-receipt': 'Prepare receipt mint',
  'activate-coordinate': 'Prepare coordinate',
  'retire-coordinate': 'Retire coordinate',
  'retire-receipt': 'Retire receipt mint',
});

export function isStructuredCoordinateActionV1(action: StructuredLifecycleActionV1): boolean {
  return action === 'activate-coordinate' || action === 'retire-coordinate';
}

function address(value: unknown, name: string): string {
  if (typeof value !== 'string') throw new Error(`${name} must be a Solana address`);
  let decoded: string;
  try { decoded = new PublicKey(value).toBase58(); } catch { throw new Error(`${name} must be a Solana address`); }
  if (decoded !== value) throw new Error(`${name} must use canonical base58`);
  return decoded;
}

/** Changing receipt/coordinate action never silently retains another action's coordinate. */
export function structuredLifecycleIntentV1(input: Readonly<{
  market: string; payer: string; action: string; coordinate: string;
  selectedCapability?: string; representationDescriptor?: string; expectedPosition?: string;
}>): StructuredLifecycleIntentV1 {
  if (!STRUCTURED_LIFECYCLE_ACTIONS_V1.includes(input.action as StructuredLifecycleActionV1)) throw new Error('Choose a supported Structured lifecycle action');
  const action = input.action as StructuredLifecycleActionV1;
  let coordinate: number | null = null;
  if (isStructuredCoordinateActionV1(action)) {
    if (!/^(0|[1-9][0-9]*)$/.test(input.coordinate)) throw new Error('Choose one whole representation coordinate');
    const value = BigInt(input.coordinate);
    if (value > 0xffff_ffffn) throw new Error('Representation coordinate exceeds u32');
    coordinate = Number(value);
  } else if (input.coordinate !== '') {
    throw new Error('Receipt actions do not select a coordinate');
  }
  const capability = input.selectedCapability || null;
  if (capability !== null && !/^[0-9a-f]{64}$/.test(capability)) throw new Error('Selected capability must be an authenticated lowercase digest');
  const representation = input.representationDescriptor || null;
  if (representation !== null && !/^[0-9a-f]{64}$/.test(representation)) throw new Error('Choose an authenticated receipt descriptor');
  const position = input.expectedPosition ? address(input.expectedPosition, 'Expected Position') : null;
  if (position !== null && !isStructuredCoordinateActionV1(action)) throw new Error('A Position pin applies only to coordinate actions');
  return Object.freeze({ market: address(input.market, 'Market'), payer: address(input.payer, 'Payer'),
    action, coordinate, selectedCapability: capability, representationDescriptor: representation, expectedPosition: position });
}

/** Exact transport keys keep a saved operation from changing its human intent. */
export function parseStructuredLifecycleIntentV1(value: unknown): StructuredLifecycleIntentV1 {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('Structured intent is not an object');
  const raw = value as Record<string, unknown>;
  if (Object.keys(raw).sort().join(',') !== 'action,coordinate,expectedPosition,market,payer,representationDescriptor,selectedCapability'
    || typeof raw.market !== 'string' || typeof raw.payer !== 'string' || typeof raw.action !== 'string'
    || (raw.coordinate !== null && (!Number.isSafeInteger(raw.coordinate) || (raw.coordinate as number) < 0))
    || (raw.expectedPosition !== null && typeof raw.expectedPosition !== 'string')
    || (raw.selectedCapability !== null && typeof raw.selectedCapability !== 'string')
    || (raw.representationDescriptor !== null && typeof raw.representationDescriptor !== 'string')) throw new Error('Structured intent has missing, unknown or invalid fields');
  return structuredLifecycleIntentV1({ market: raw.market, payer: raw.payer, action: raw.action,
    coordinate: raw.coordinate === null ? '' : String(raw.coordinate),
    selectedCapability: raw.selectedCapability === null ? undefined : raw.selectedCapability,
    representationDescriptor: raw.representationDescriptor === null ? undefined : raw.representationDescriptor,
    expectedPosition: raw.expectedPosition === null ? undefined : raw.expectedPosition });
}

/** Captured async results cannot replace a newer Market/action/wallet selection. */
export class StructuredLifecycleSelectionV1 {
  private revision = 0;
  invalidate(): number { this.revision += 1; return this.revision; }
  current(): number { return this.revision; }
  require(revision: number): void {
    if (this.revision !== revision) throw new Error('Structured selection changed; review the current Market and action');
  }
}
