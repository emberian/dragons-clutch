import { sha256 } from './bytes';
import { CODE_COMMITMENT_CHUNK_BYTES_V2 } from './generated/coreFound';
import {
  CODE_COMMITMENT_CHUNK_DOMAIN_V2,
  CODE_COMMITMENT_INITIAL_DOMAIN_V2,
} from './generated/protocolConstantsV1';

function littleEndian(value: number): Uint8Array {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, BigInt(value), true);
  return bytes;
}

async function digestParts(parts: readonly Uint8Array[]): Promise<Uint8Array> {
  const bytes = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) { bytes.set(part, offset); offset += part.length; }
  return sha256(bytes);
}

/** Browser projection of Registry's ordered code commitment, checked against native vectors. */
export async function codeCommitmentV2(elf: Uint8Array): Promise<Uint8Array> {
  if (elf.length === 0) throw new Error('code commitment requires a nonempty Loader code region');
  let rolling = await digestParts([
    CODE_COMMITMENT_INITIAL_DOMAIN_V2,
    littleEndian(elf.length),
    littleEndian(CODE_COMMITMENT_CHUNK_BYTES_V2),
  ]);
  for (let offset = 0; offset < elf.length; offset += CODE_COMMITMENT_CHUNK_BYTES_V2) {
    const chunk = elf.subarray(offset, Math.min(offset + CODE_COMMITMENT_CHUNK_BYTES_V2, elf.length));
    rolling = await digestParts([
      CODE_COMMITMENT_CHUNK_DOMAIN_V2, rolling, littleEndian(offset),
      littleEndian(chunk.length), await sha256(chunk),
    ]);
  }
  return rolling;
}
