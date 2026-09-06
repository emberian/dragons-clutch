/**
 * The canonical accelerator transcript digests, mirrored from
 * `crates/dclutch-market/src/execution_strategy/shadow_digest_v3.rs`.
 *
 * # There are TWO request digests on the Hot path and they are not the same number
 *
 * A reader that knows only "the request digest" will pick whichever one is
 * nearest and be wrong half the time, so both are named here:
 *
 * - The FAMILY request digest, {@link familyRequestDigestV3} — domain
 *   separated, and what an operator plan publishes as `familyRequestDigest`.
 *   Its Rust author is `shadow_digest_v3.rs::family_request_digest_v3`, which
 *   every operator site has reached since COHORT-16F.
 * - The BARE `sha256(request_bytes)` the Hot prelude computes at
 *   `programs/dclutch-trading-sbf/src/hot_v3.rs:1117`
 *   (`hash(family_request)`) and carries as `parent_request_digest`. It is the
 *   chain's own author for the Hot acknowledgement's `request_digest` field,
 *   filled from `prepared.request_digest` at
 *   `programs/dclutch-trading-sbf/src/hot_v3/execute.rs:1771`, and for the
 *   Trading- and Claims-role child caller authorities. There is nothing to
 *   sweep here: it is a different fact with a different author.
 *
 * Comparing a plan's digest to the bare hash, or a receipt's digest to the
 * domain-separated one, is always false. The browser did both until this
 * module existed, and its own fixtures agreed with it because they recomputed
 * the same wrong number.
 */
import { requireNonzero, sha256 } from './bytes';
import { FAMILY_REQUEST_DIGEST_DOMAIN_V3 } from './generated/protocolConstantsV1';

const MAX_U32 = 4_294_967_295;

/**
 * Digest the exact complete family request, the domain-separated V3 way.
 *
 * The preimage is `domain ‖ 0x00 ‖ len_le32 ‖ bytes`. The zero byte is the
 * domain terminator every digest in this family carries (`oracle_begin` in the
 * Rust), and the explicit length is what keeps a domain-plus-body preimage
 * unambiguous; neither is optional and neither may be inferred from the shape
 * of the input.
 */
export async function familyRequestDigestV3(bytes: Uint8Array): Promise<Uint8Array> {
  if (bytes.length > MAX_U32) throw new Error('family request exceeds its exact encoded u32 width');
  const preimage = new Uint8Array(FAMILY_REQUEST_DIGEST_DOMAIN_V3.length + 1 + 4 + bytes.length);
  preimage.set(FAMILY_REQUEST_DIGEST_DOMAIN_V3, 0);
  const lengthOffset = FAMILY_REQUEST_DIGEST_DOMAIN_V3.length + 1;
  new DataView(preimage.buffer).setUint32(lengthOffset, bytes.length, true);
  preimage.set(bytes, lengthOffset + 4);
  const digest = await sha256(preimage);
  // `ContentId::new` refuses the reserved all-zero identity, so a caller that
  // compares against one never gets there by accident.
  requireNonzero(digest, 'family request digest');
  return digest;
}
