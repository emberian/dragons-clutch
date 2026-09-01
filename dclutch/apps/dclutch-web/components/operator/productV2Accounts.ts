import {
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from '@/lib/generated/productRuntimeV2Admission';
import { deriveFinalizedRecordAddressesV1 } from '@/lib/releaseRegistry';

import { readHex64V1, readPubkeyV1 } from './fieldReadings';

/**
 * The six Registry record accounts `/product-v2` step 03 asks an operator to
 * type, computed instead.
 *
 * Each one is `PublicKey.findProgramAddressSync([seed, schema, digest],
 * registry)` -- a pure function of the Registry program, a schema id pinned in
 * `lib/generated/productRuntimeV2Admission.ts`, and a digest already on the
 * form. There is no chain read here and no new RPC dependency: the same
 * arithmetic the adapter runs on chain to authenticate the PDA, run in the
 * browser before the request is composed.
 *
 * This is OPERATOR_FORMS_V1 §3.2's compute-instead-of-ask case, and the audit
 * found it to be the sharpest one on the site: six `required` inputs, eight
 * unlabeled `Invalid public key input` paths, and a console that refuses any
 * mismatch -- which is to say, a console checking an answer it could have
 * computed.
 *
 * Returns null rather than throwing when the inputs are not all readable yet,
 * because a half-typed form is not an error state; the fields themselves say
 * what is still missing.
 */
export type DerivedRecordAccountsV1 = Readonly<{
  productRaw: string;
  productStaging: string;
  domainRaw: string;
  domainStaging: string;
  portfolioRaw: string;
  portfolioStaging: string;
}>;

/** The digests step 03 reads, as typed. */
export type RecordDigestsV1 = Readonly<{ product: string; domain: string; portfolio: string }>;

function bytesFromHex64(text: string): Uint8Array | null {
  if (readHex64V1(text).state !== 'resolved') return null;
  return Uint8Array.from(text.trim().match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

export function deriveProductV2AccountsV1(
  registry: string,
  digests: RecordDigestsV1,
): DerivedRecordAccountsV1 | null {
  if (readPubkeyV1(registry).state !== 'resolved') return null;
  const product = bytesFromHex64(digests.product);
  const domain = bytesFromHex64(digests.domain);
  const portfolio = bytesFromHex64(digests.portfolio);
  if (product === null || domain === null || portfolio === null) return null;

  try {
    const productPdas = deriveFinalizedRecordAddressesV1(registry, PRODUCT_RECORD_SCHEMA_ID_V2, product);
    const domainPdas = deriveFinalizedRecordAddressesV1(registry, RESULT_DOMAIN_SCHEMA_ID_V2, domain);
    const portfolioPdas = deriveFinalizedRecordAddressesV1(registry, PORTFOLIO_SCHEMA_ID_V2, portfolio);
    return Object.freeze({
      productRaw: productPdas.record,
      productStaging: productPdas.staging,
      domainRaw: domainPdas.record,
      domainStaging: domainPdas.staging,
      portfolioRaw: portfolioPdas.record,
      portfolioStaging: portfolioPdas.staging,
    });
  } catch {
    // The one reachable throw is the all-zero content identity, which
    // `ContentId::new` refuses on chain as well. The digest field owns that
    // refusal; this function simply has nothing to offer until it is fixed.
    return null;
  }
}

/**
 * What the form will actually send: the derived address, unless the operator
 * deliberately overrode it.
 *
 * An override is legitimate -- an operator is frequently the person who knows
 * a record moved -- so it stays possible. What the DERIVE rule requires is
 * that it be a visible act rather than an indistinguishable one, which is why
 * the override lives behind its own disclosure and this function reports which
 * of the two it used.
 */
export function effectiveAccountV1(derived: string | null, override: string): string {
  return override.trim() === '' ? derived ?? '' : override.trim();
}
