/**
 * Base58 decoding, for instruction data the RPC returns encoded.
 *
 * `getTransaction` under a `base64` request still returns `meta.innerInstructions[].data`
 * in base58, and instruction data is arbitrary-length, so `PublicKey` — which
 * decodes base58 only at exactly 32 bytes — cannot be used for it.
 *
 * Base58 is a transport encoding, not a protocol fact, so writing it here
 * restates nothing the schemas own. It is held to the Bitcoin alphabet Solana
 * uses, and `base58.test.ts` checks it against `PublicKey`'s own encoder on
 * every-byte-value inputs and on the leading-zero case that naive
 * implementations get wrong.
 */
const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

const VALUES: ReadonlyMap<string, number> = new Map(
  [...ALPHABET].map((character, index) => [character, index]),
);

/**
 * Decode base58 text to bytes.
 *
 * Throws on any character outside the alphabet rather than skipping it: a
 * silently-dropped character produces plausible bytes for data that was never
 * sent, which is exactly the class of wrong this explorer must not produce.
 */
export function decodeBase58(text: string): Uint8Array {
  if (text.length === 0) return new Uint8Array(0);

  // Leading '1's are leading zero bytes and carry no positional value.
  let zeros = 0;
  while (zeros < text.length && text[zeros] === ALPHABET[0]) zeros += 1;

  const digits: number[] = [];
  for (let index = zeros; index < text.length; index += 1) {
    const value = VALUES.get(text[index]);
    if (value === undefined) throw new Error(`base58 text carries ${JSON.stringify(text[index])}, which is outside the alphabet`);
    let carry = value;
    for (let position = 0; position < digits.length; position += 1) {
      carry += digits[position] * 58;
      digits[position] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      digits.push(carry & 0xff);
      carry >>= 8;
    }
  }

  const out = new Uint8Array(zeros + digits.length);
  for (let index = 0; index < digits.length; index += 1) {
    out[zeros + index] = digits[digits.length - 1 - index];
  }
  return out;
}
