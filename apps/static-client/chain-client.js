/*
 * Bounded GET-only client for the operatord fork-aware index projection.
 *
 * It never calls a validator RPC directly. URLs, release coordinates, and all
 * returned state are untrusted inputs checked against explicit user-selected
 * browser bounds. Cluster, endpoint, and release truth come only from the
 * daemon's release-bound transport projection.
 */
(function (root) {
  "use strict";

  const UINT = /^(0|[1-9][0-9]*)$/;
  const HASH32 = /^[0-9a-f]{64}$/;
  const HEX_BYTES = /^(?:[0-9a-f]{2})+$/;
  const COMMIT = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;
  const U64_MAX = (1n << 64n) - 1n;
  const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  const BASE58_INDEX = Object.freeze(Object.fromEntries(Array.from(BASE58_ALPHABET, (character, index) => [character, index])));
  const DECODER_SET = "dragons-clutch/canonical-account-decoders/v7-product-v5-authority";
  const SOURCE_COORDINATE = Object.freeze({ familyTag: "77", familyVersion: "2", family: "source", flow: "source-plane-v3", messageVersion: "legacy", lookupTables: 0 });
  const STRUCTURED_COORDINATE = Object.freeze({ familyTag: "75", familyVersion: "1", family: "structured-claim", flow: "structured-claim", messageVersion: "v0", lookupTables: 1 });
  const DIRECT_COORDINATE = Object.freeze({ familyTag: "80", familyVersion: "1", family: "direct", flow: "direct-market-v1", messageVersion: "legacy", lookupTables: 0 });
  const FRACTIONAL_COORDINATE = Object.freeze({ familyTag: "79", familyVersion: "1", family: "fractional", flow: "fractional-redemption", messageVersion: "legacy", lookupTables: 0 });
  const FRACTIONAL_ACTION_CONTRACTS = Object.freeze({
    "1": Object.freeze({ action: "initialize-fractional", ownerSchema: "fractional-redemption/79/1/1/initialize", driverRole: 0, equationNames: Object.freeze(["chain-derived Product foundation outcome width"]) }),
    "2": Object.freeze({ action: "redeem-fractional-internal-exact", ownerSchema: "fractional-redemption/79/1/2/redeem-internal-exact", driverRole: 12, equationNames: Object.freeze(["chain-derived internal Eggs burned", "chain-derived exact collateral payout"]) }),
    "3": Object.freeze({ action: "redeem-fractional-bearer-exact", ownerSchema: "fractional-redemption/79/1/3/redeem-bearer-exact", driverRole: 12, equationNames: Object.freeze(["holder-approved bearer Eggs burned", "chain-derived whole collateral payout", "chain-derived retained payout numerator"]) }),
    "4": Object.freeze({ action: "redeem-fractional-internal-credit", ownerSchema: "fractional-redemption/79/1/4/redeem-internal-credit", driverRole: 12, equationNames: Object.freeze(["holder-approved internal Eggs retired", "chain-derived whole collateral payout", "chain-derived retained payout numerator"]) }),
    "5": Object.freeze({ action: "redeem-fractional-bearer-credit", ownerSchema: "fractional-redemption/79/1/5/redeem-bearer-credit", driverRole: 12, equationNames: Object.freeze(["holder-approved bearer Eggs burned", "chain-derived whole collateral payout", "chain-derived retained payout numerator"]) }),
    "6": Object.freeze({ action: "transfer-fractional-credit", ownerSchema: "fractional-redemption/79/1/6/transfer-credit", driverRole: 13, equationNames: Object.freeze(["holder-approved credit numerator moved", "chain-derived whole credit payout"]) }),
    "7": Object.freeze({ action: "merge-fractional-credit", ownerSchema: "fractional-redemption/79/1/7/merge-credit", driverRole: 13, equationNames: Object.freeze(["holder-approved credit numerator moved", "chain-derived whole credit payout"]) }),
    "8": Object.freeze({ action: "close-fractional-zero-credit", ownerSchema: "fractional-redemption/79/1/8/close-zero-credit", driverRole: 13, equationNames: Object.freeze(["chain-derived zero-credit rent split"]) }),
    "9": Object.freeze({ action: "seal-fractional-claims-exhausted", ownerSchema: "fractional-redemption/79/1/9/seal-claims-exhausted", driverRole: 11, equationNames: Object.freeze(["chain-derived zero native claim supply"]) }),
    "10": Object.freeze({ action: "close-empty-fractional-ledger", ownerSchema: "fractional-redemption/79/1/10/close-empty-ledger", driverRole: 12, equationNames: Object.freeze(["chain-derived Product foundation outcome width"]) })
  });
  const directRole = (role, signer, writable) => Object.freeze({ role, signer, writable });
  const DIRECT_ACTION_CONTRACTS = Object.freeze({
    "2": Object.freeze({ action: "admit-direct-order", ownerSchema: "current-v1" }),
    "3": Object.freeze({ action: "cancel-direct-order", ownerSchema: "current-v1" }),
    "4": Object.freeze({ action: "freeze-direct-book", ownerSchema: "current-v1" }),
    "5": Object.freeze({ action: "submit-direct-candidate", ownerSchema: "current-v1" }),
    "6": Object.freeze({ action: "begin-direct-verification", ownerSchema: "current-v1" }),
    "7": Object.freeze({ action: "verify-direct-candidate", ownerSchema: "current-v1" }),
    "8": Object.freeze({ action: "finalize-direct-selection-current-v2", ownerSchema: "dragons-clutch/direct/finalize-selection-chain-material/v2" })
  });
  const DIRECT_RESERVATION_AUTHORITY_ROLES = Object.freeze([
    directRole("general-position-v3", false, true), directRole("general-position-replay-v3", false, true), directRole("realm", false, false),
    directRole("collateral-profile", false, false), directRole("collateral-policy", false, false), directRole("token-program", false, false),
    directRole("general-market-binding-v4", false, false), directRole("general-market-runtime", false, false), directRole("market-instance-v2", false, false)
  ]);
  const DIRECT_CANDIDATE_TAIL = Object.freeze([
    directRole("candidate-liveness-policy", false, false), directRole("candidate-compartment", false, true),
    directRole("keeper", true, true), directRole("candidate-immutable-payer", false, true)
  ]);
  const DEALER_VARIANT_CONTRACTS = Object.freeze([
    Object.freeze({ familyTag: "76", familyVersion: "1", localAction: "25", payloadDiscriminator: "8", name: "dealer-retire-active-facility-credit" }),
    Object.freeze({ familyTag: "76", familyVersion: "1", localAction: "25", payloadDiscriminator: "9", name: "dealer-retire-unused-future-credit" })
  ]);
  const dealerRole = (role, signer, writable) => Object.freeze({ role, signer, writable });
  const DEALER_COMMON_ROLES = Object.freeze([
    dealerRole("actor", true, true), dealerRole("policy", false, false), dealerRole("state-v3", false, true), dealerRole("facility-position-v3", false, true), dealerRole("facility-replay-v3", false, true), dealerRole("funded-dependencies-v2", false, false),
    dealerRole("dealer-liveness-schedule-v1", false, false), dealerRole("liveness-policy", false, false), dealerRole("liveness-source", false, false), dealerRole("liveness-candidate", false, false), dealerRole("liveness-clearing", false, false), dealerRole("liveness-settlement", false, false), dealerRole("liveness-resolution", false, false), dealerRole("liveness-retirement", false, true), dealerRole("liveness-recovery", false, false), dealerRole("liveness-receipt", false, true),
    dealerRole("liveness-payer", false, true), dealerRole("position-rent-payer", false, true), dealerRole("replay-rent-payer", false, true), dealerRole("obligation-rent-payer", false, true), dealerRole("neutral-lamport-sink", false, true), dealerRole("clock-sysvar", false, false), dealerRole("rent-sysvar", false, false), dealerRole("system-program", false, false), dealerRole("dealer-series-obligation-v2", false, true), dealerRole("product-market-root-v2", false, false), dealerRole("series-registry-v3", false, false),
    dealerRole("current-program", false, false), dealerRole("current-programdata", false, false), dealerRole("registry-release-v2", false, false), dealerRole("capability-profile-v4", false, false), dealerRole("series-market-link-v2", false, true), dealerRole("compiler-bundle-v6", false, false), dealerRole("attachment-v5", false, false), dealerRole("realm", false, false), dealerRole("collateral-profile-v2", false, false), dealerRole("collateral-policy-v2", false, false), dealerRole("collateral-token-program", false, false), dealerRole("collateral-token-programdata", false, false), dealerRole("market-binding-v2", false, false), dealerRole("market-runtime-v3", false, false), dealerRole("market-instance-v2", false, false), dealerRole("hoard-v2", false, false)
  ]);
  const DEALER_ACTIVE_TAIL = Object.freeze([dealerRole("claim-ledger-v3", false, true), dealerRole("resolution-v5", false, false), dealerRole("fractional-policy-v3", false, false), dealerRole("fractional-ledger-v1", false, true), dealerRole("facility-credit-v2", false, true)]);
  const DEALER_UNUSED_TAIL = Object.freeze([dealerRole("claim-ledger-v3", false, false), dealerRole("dealer-future-credit-funding-v1", false, true)]);
  const GROUP_ORDER = Object.freeze(["market", "product", "collateral", "source", "series", "candidate", "settlement", "liquidity", "recovery", "other"]);
  const GROUP_LABELS = Object.freeze({
    market: "Market",
    product: "Product",
    collateral: "Collateral & liabilities",
    source: "Source",
    series: "Series",
    candidate: "Candidate & clearing",
    settlement: "Settlement & positions",
    liquidity: "Covered liquidity",
    recovery: "Recovery",
    other: "Other release state"
  });
  const KIND_GROUPS = Object.freeze({
    "collateral-hoard-v2": "collateral",
    "collateral-claim-ledger-v3": "collateral",
    "collateral-resolution-v5": "collateral",
    "fractional-policy-v3": "collateral",
    "fractional-ledger-v1": "collateral",
    "fractional-credit-v2": "collateral",
    "fractional-credit-tombstone-v2": "collateral",
    "general-market-runtime": "market",
    "general-epoch": "market",
    "general-economic-domain": "market",
    "general-current-market-authority-v5": "market",
    "general-order-page-v5": "market",
    "general-reservation-v9": "settlement",
    "general-candidate-window": "market",
    "series-registry-v4": "product",
    "series-funding-v5": "series",
    "product-funding-quote-v6-schedule-v4": "product",
    "product-attachment-v6": "product",
    "compiled-product-series-bundle-v7": "product",
    "product-market-replay-v2-graph-v4": "product",
    "product-market-root-v3-graph-v4": "product",
    "series-market-link-v3": "series",
    "structured-claim-descriptor": "product",
    "product-capability-registry": "product",
    "product-capability-registry-v2": "product",
    "compiled-product-series-bundle": "product",
    "product-compiler-output": "product",
    "product-artifact": "product",
    "series-funding": "series",
    "source-release": "source",
    "source-head": "source",
    "source-open-raw-page": "source",
    "source-raw-page": "source",
    "source-window-work": "source",
    "source-window-seal": "source",
    "source-statistic-result": "source",
    "source-lineage": "source",
    "source-work-receipt": "source",
    "general-admission-node": "candidate",
    "general-candidate-feed-stage": "candidate",
    "general-candidate-feed": "candidate",
    "general-clear-work": "candidate",
    "general-epoch-budget": "candidate",
    "general-owner-settlement": "settlement",
    "general-owner-settlement-v5": "settlement",
    "general-settlement-receipt-v5": "settlement",
    "general-settlement-root-v1": "settlement",
    "general-owner-settlement-v3": "settlement",
    "owner-settlement-v3": "settlement",
    "general-settlement-cash-pot": "settlement",
    "general-final-pot": "settlement",
    "fee-selected-record": "settlement",
    "fee-owner-carry": "settlement",
    "fee-owner-finalization": "settlement",
    "fee-payer-allocation": "settlement",
    "fee-recipient-allocation": "settlement",
    "fee-treasury-ledger": "settlement",
    "liveness-policy": "settlement",
    "liveness-compartment": "settlement",
    "position-v3": "settlement",
    "replay-v3": "settlement",
    "dealer-policy-v1": "liquidity",
    "dealer-liveness-schedule-v1": "liquidity",
    "dealer-state-v2": "liquidity",
    "dealer-funded-dependencies-v2": "liquidity",
    "dealer-lp-page-v2": "liquidity",
    "dealer-lease-v2": "liquidity",
    "dealer-settlement-pot-v2": "liquidity",
    "dealer-epoch-binding-v2": "liquidity",
    "dealer-terminal-allocation-v1": "liquidity",
    "dealer-claim-work-v1": "liquidity",
    "dealer-root-tombstone-v2": "liquidity",
    "dealer-exit-ticket-v1": "liquidity",
    "dealer-action-receipt-v1": "liquidity",
    "dealer-replay": "liquidity",
    "failure-external-root": "recovery",
    "failure-market-root-v2": "recovery",
    "failure-liveness-policy": "recovery",
    "failure-recovery-compartment": "recovery",
    "failure-replay-tombstone": "recovery",
    "failure-interval-consensus-work-v1": "recovery",
    "failure-interval-consensus-replay-v1": "recovery"
  });
  const CURRENT_BINDING_PROJECTIONS = Object.freeze({
    "general-current-market-authority-v5": Object.freeze(["market-instance-v2", "product-market-root-v3-account"]),
    "series-registry-v4": Object.freeze(["series-plan-v5", "compiled-product-series-bundle-v7"]),
    "series-funding-v5": Object.freeze(["series-plan-v5", "compiled-product-series-bundle-v7"]),
    "product-funding-quote-v6-schedule-v4": Object.freeze(["series-funding-quote-v6", "market-foundation-schedule-v4"]),
    "product-attachment-v6": Object.freeze(["series-attachment-plan-v6", "series-funding-quote-v6"]),
    "compiled-product-series-bundle-v7": Object.freeze(["compiled-product-series-bundle-v7", "series-plan-v5"]),
    "product-market-replay-v2-graph-v4": Object.freeze(["market-instance-v2", "market-foundation-account-graph-v4"]),
    "product-market-root-v3-graph-v4": Object.freeze(["market-instance-v2", "market-foundation-account-graph-v4"]),
    "series-market-link-v3": Object.freeze(["series-plan-v5", "market-instance-v2"])
  });

  const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  const requirePlain = (value, name) => {
    if (!plain(value)) throw new Error(`${name} must be an object.`);
    return value;
  };
  const text = (value, name, maximum = 160) => {
    if (typeof value !== "string" || value.trim() !== value || value.length === 0 || value.length > maximum) throw new Error(`${name} must be nonempty, trimmed text no longer than ${maximum} characters.`);
    return value;
  };
  const decimal = (value, name, maximum = U64_MAX) => {
    if (typeof value !== "string" || !UINT.test(value)) throw new Error(`${name} must be a canonical decimal string.`);
    const parsed = BigInt(value);
    if (parsed > maximum) throw new Error(`${name} exceeds its exact integer width.`);
    return parsed;
  };
  const positiveDecimal = (value, name, maximum = U64_MAX) => {
    const parsed = decimal(value, name, maximum);
    if (parsed === 0n) throw new Error(`${name} must be positive.`);
    return parsed;
  };
  const hash32 = (value, name) => {
    if (typeof value !== "string" || !HASH32.test(value) || /^0+$/.test(value)) throw new Error(`${name} must be a nonzero lowercase SHA-256/32-byte hexadecimal identity.`);
    return value;
  };
  const encodeBase58 = (bytes) => {
    let value = 0n;
    for (const byte of bytes) value = value * 256n + BigInt(byte);
    let encoded = "";
    while (value > 0n) { encoded = BASE58_ALPHABET[Number(value % 58n)] + encoded; value /= 58n; }
    let leading = 0;
    while (leading < bytes.length && bytes[leading] === 0) leading += 1;
    return "1".repeat(leading) + (encoded || (leading === 0 ? "1" : ""));
  };
  const decodeBase58 = (value, name) => {
    if (typeof value !== "string" || value.length < 32 || value.length > 44) throw new Error(`${name} must be a canonical base58 Solana address.`);
    let decoded = 0n;
    for (const character of value) {
      const digit = BASE58_INDEX[character];
      if (digit === undefined) throw new Error(`${name} contains a non-base58 character.`);
      decoded = decoded * 58n + BigInt(digit);
    }
    const output = new Uint8Array(32);
    for (let index = 31; index >= 0; index -= 1) { output[index] = Number(decoded & 255n); decoded >>= 8n; }
    if (decoded !== 0n || encodeBase58(output) !== value) throw new Error(`${name} is not a canonical 32-byte base58 address.`);
    return output;
  };
  const address = (value, name) => {
    text(value, name, 44);
    decodeBase58(value, name);
    return value;
  };
  const nonzeroAddress = (value, name) => {
    const canonical = address(value, name);
    const decoded = decodeBase58(canonical, name);
    if (decoded.every((byte) => byte === 0)) throw new Error(`${name} must be a nonzero 32-byte base58 identity.`);
    return canonical;
  };
  const bool = (value, name) => {
    if (typeof value !== "boolean") throw new Error(`${name} must be boolean.`);
    return value;
  };
  const bindingProjection = (kind, primaryBinding, secondaryBinding) => {
    const labels = CURRENT_BINDING_PROJECTIONS[kind] || ["primary-semantic-binding", "secondary-semantic-binding"];
    return Object.freeze({
      primary: Object.freeze({ label: labels[0], value: primaryBinding }),
      secondary: Object.freeze({ label: labels[1], value: secondaryBinding }),
      authority: CURRENT_BINDING_PROJECTIONS[kind] ? "hostile-decoded-current-semantic-owner" : "hostile-decoded-account-codec"
    });
  };

  const validateIntentVariants = (raw, name, enabledIntents) => {
    if (!Array.isArray(raw) || raw.length > DEALER_VARIANT_CONTRACTS.length) throw new Error(`${name} is not a bounded current payload-variant set.`);
    const coarse = new Set(enabledIntents.map((intent) => `${intent.familyTag}:${intent.familyVersion}:${intent.localAction}`));
    const variants = raw.map((variant, index) => {
      requirePlain(variant, `${name}[${index}]`);
      const value = Object.freeze({
        familyTag: positiveDecimal(variant.familyTag, `${name}[${index}].familyTag`, 255n).toString(),
        familyVersion: positiveDecimal(variant.familyVersion, `${name}[${index}].familyVersion`, 255n).toString(),
        localAction: positiveDecimal(variant.localAction, `${name}[${index}].localAction`, 255n).toString(),
        payloadDiscriminator: positiveDecimal(variant.payloadDiscriminator, `${name}[${index}].payloadDiscriminator`, 255n).toString(),
        name: text(variant.name, `${name}[${index}].name`, 96)
      });
      const contract = DEALER_VARIANT_CONTRACTS.find((candidate) => candidate.familyTag === value.familyTag && candidate.familyVersion === value.familyVersion && candidate.localAction === value.localAction && candidate.payloadDiscriminator === value.payloadDiscriminator && candidate.name === value.name);
      if (!contract) throw new Error(`${name}[${index}] is not a closed current Dealer payload variant.`);
      if (coarse.has(`${value.familyTag}:${value.familyVersion}:${value.localAction}`)) throw new Error(`${name}[${index}] illegally promotes its disabled coarse Dealer coordinate.`);
      return value;
    });
    const keys = variants.map((variant) => `${variant.familyTag.padStart(3, "0")}:${variant.familyVersion.padStart(3, "0")}:${variant.localAction.padStart(3, "0")}:${variant.payloadDiscriminator.padStart(3, "0")}`);
    if (new Set(keys).size !== keys.length || keys.some((key, index) => index > 0 && keys[index - 1] >= key)) throw new Error(`${name} is duplicated or not in canonical coordinate/discriminator order.`);
    return Object.freeze(variants);
  };

  const validateIntentVariants = (raw, name, enabledIntents) => {
    if (!Array.isArray(raw) || raw.length > DEALER_VARIANT_CONTRACTS.length) throw new Error(`${name} is not a bounded current payload-variant set.`);
    const coarse = new Set(enabledIntents.map((intent) => `${intent.familyTag}:${intent.familyVersion}:${intent.localAction}`));
    const variants = raw.map((variant, index) => {
      requirePlain(variant, `${name}[${index}]`);
      const value = Object.freeze({
        familyTag: positiveDecimal(variant.familyTag, `${name}[${index}].familyTag`, 255n).toString(),
        familyVersion: positiveDecimal(variant.familyVersion, `${name}[${index}].familyVersion`, 255n).toString(),
        localAction: positiveDecimal(variant.localAction, `${name}[${index}].localAction`, 255n).toString(),
        payloadDiscriminator: positiveDecimal(variant.payloadDiscriminator, `${name}[${index}].payloadDiscriminator`, 255n).toString(),
        name: text(variant.name, `${name}[${index}].name`, 96)
      });
      const contract = DEALER_VARIANT_CONTRACTS.find((candidate) => candidate.familyTag === value.familyTag && candidate.familyVersion === value.familyVersion && candidate.localAction === value.localAction && candidate.payloadDiscriminator === value.payloadDiscriminator && candidate.name === value.name);
      if (!contract) throw new Error(`${name}[${index}] is not a closed current Dealer payload variant.`);
      if (coarse.has(`${value.familyTag}:${value.familyVersion}:${value.localAction}`)) throw new Error(`${name}[${index}] illegally promotes its disabled coarse Dealer coordinate.`);
      return value;
    });
    const keys = variants.map((variant) => `${variant.familyTag.padStart(3, "0")}:${variant.familyVersion.padStart(3, "0")}:${variant.localAction.padStart(3, "0")}:${variant.payloadDiscriminator.padStart(3, "0")}`);
    if (new Set(keys).size !== keys.length || keys.some((key, index) => index > 0 && keys[index - 1] >= key)) throw new Error(`${name} is duplicated or not in canonical coordinate/discriminator order.`);
    return Object.freeze(variants);
  };

  const boundedUrl = (value, name, schemes, allowQuery = false, preserveExact = false) => {
    text(value, name, 512);
    let parsed;
    try { parsed = new URL(value); } catch (_) { throw new Error(`${name} must be an absolute URL.`); }
    if (!schemes.includes(parsed.protocol)) throw new Error(`${name} uses a disallowed URL scheme.`);
    if (parsed.username || parsed.password || parsed.hash || (!allowQuery && parsed.search)) throw new Error(`${name} must not contain userinfo, a fragment, or a disallowed query.`);
    const loopback = parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost";
    if ((parsed.protocol === "http:" || parsed.protocol === "ws:") && !loopback) throw new Error(`${name} may use plaintext transport only on explicit loopback.`);
    return preserveExact ? value : parsed.toString().replace(/\/$/, "");
  };

  const validateConfiguration = (raw) => {
    requirePlain(raw, "configuration");
    const operatorUrl = boundedUrl(raw.operatorUrl, "operatord URL", ["http:", "https:"]);
    if (raw.commitment !== "finalized" && raw.commitment !== "processed") throw new Error("commitment must be finalized or processed.");
    requirePlain(raw.bounds, "bounds");
    const maximumAccounts = positiveDecimal(raw.bounds.maximumAccounts, "bounds.maximumAccounts", 4096n).toString();
    const maximumResponseBytes = positiveDecimal(raw.bounds.maximumResponseBytes, "bounds.maximumResponseBytes", 16_777_216n);
    if (maximumResponseBytes < 4096n) throw new Error("bounds.maximumResponseBytes must be at least 4096.");
    const timeoutMilliseconds = positiveDecimal(raw.bounds.timeoutMilliseconds, "bounds.timeoutMilliseconds", 30000n);
    if (timeoutMilliseconds < 250n) throw new Error("bounds.timeoutMilliseconds must be at least 250.");
    const maximumSlotLag = decimal(raw.bounds.maximumSlotLag, "bounds.maximumSlotLag", 1_000_000n).toString();
    return Object.freeze({
      schema: "dragons-clutch/browser-operatord-target/v3",
      authority: "explicit-user-selected-operatord-only",
      operatorUrl,
      rpcContact: "operatord-only; browser does not call validator RPC",
      commitment: raw.commitment,
      bounds: Object.freeze({ maximumAccounts, maximumResponseBytes: maximumResponseBytes.toString(), timeoutMilliseconds: timeoutMilliseconds.toString(), maximumSlotLag })
    });
  };

  const redactedConfiguration = (configuration) => Object.freeze({
    schema: configuration.schema,
    authority: configuration.authority,
    operatorUrl: configuration.operatorUrl,
    rpcContact: configuration.rpcContact,
    commitment: configuration.commitment,
    bounds: configuration.bounds,
    decoderSet: configuration.decoderSet || "not-acquired",
    clusterName: configuration.clusterName || "not-acquired",
    genesisHash: configuration.genesisHash || "not-acquired",
    clusterKey: configuration.clusterKey || "not-acquired",
    daemonProjection: configuration.release ? Object.freeze({
      decoderSet: configuration.decoderSet,
      clusterName: configuration.clusterName,
      genesisHash: configuration.genesisHash,
      clusterKey: configuration.clusterKey,
      rpcHttpEndpoint: configuration.rpcHttpEndpoint,
      rpcWebsocketEndpoint: configuration.rpcWebsocketEndpoint,
      release: configuration.release
    }) : "not-acquired"
  });

  const boundJsonShape = (value, name) => {
    let nodes = 0;
    const visit = (item, depth) => {
      nodes += 1;
      if (nodes > 100_000 || depth > 14) throw new Error(`${name} JSON exceeds browser shape bounds.`);
      if (typeof item === "string" && item.length > 8192) throw new Error(`${name} contains an oversized string.`);
      if (Array.isArray(item)) {
        if (item.length > 65_536) throw new Error(`${name} contains an oversized array.`);
        for (const child of item) visit(child, depth + 1);
      } else if (plain(item)) {
        const keys = Object.keys(item);
        if (keys.length > 128) throw new Error(`${name} contains an oversized object.`);
        for (const key of keys) {
          if (key.length > 128) throw new Error(`${name} contains an oversized property name.`);
          visit(item[key], depth + 1);
        }
      } else if (item !== null && !["string", "number", "boolean"].includes(typeof item)) {
        throw new Error(`${name} contains an unsupported JSON value.`);
      }
    };
    visit(value, 0);
    return value;
  };

  class BoundedGetReader {
    constructor(configuration, fetchFunction) {
      this.configuration = configuration;
      this.fetchFunction = fetchFunction;
      this.remaining = BigInt(configuration.bounds.maximumResponseBytes);
    }

    async get(path) {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), Number(this.configuration.bounds.timeoutMilliseconds));
      let response;
      try {
        response = await this.fetchFunction(`${this.configuration.operatorUrl}${path}`, {
          method: "GET",
          mode: "cors",
          credentials: "omit",
          cache: "no-store",
          redirect: "error",
          referrerPolicy: "no-referrer",
          headers: { Accept: "application/json" },
          signal: controller.signal
        });
      } finally {
        clearTimeout(timer);
      }
      if (!response.ok) throw new Error(`${path} returned HTTP ${response.status}.`);
      const contentType = response.headers.get("content-type") || "";
      if (!contentType.toLowerCase().startsWith("application/json")) throw new Error(`${path} did not return application/json.`);
      const declared = response.headers.get("content-length");
      if (declared !== null && decimal(declared, `${path} Content-Length`) > this.remaining) throw new Error(`${path} exceeds the remaining response-byte budget.`);
      const chunks = [];
      let length = 0n;
      if (response.body && typeof response.body.getReader === "function") {
        const reader = response.body.getReader();
        for (;;) {
          const item = await reader.read();
          if (item.done) break;
          length += BigInt(item.value.byteLength);
          if (length > this.remaining) {
            await reader.cancel();
            throw new Error(`${path} exceeded the remaining response-byte budget while reading.`);
          }
          chunks.push(item.value);
        }
      } else {
        const body = new Uint8Array(await response.arrayBuffer());
        length = BigInt(body.byteLength);
        if (length > this.remaining) throw new Error(`${path} exceeds the remaining response-byte budget.`);
        chunks.push(body);
      }
      this.remaining -= length;
      const bytes = new Uint8Array(Number(length));
      let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
      let parsed;
      try { parsed = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); } catch (_) { throw new Error(`${path} did not return valid UTF-8 JSON.`); }
      return boundJsonShape(parsed, path);
    }
  }

  const validateHealth = (raw, configuration) => {
    requirePlain(raw, "health");
    if (raw.status !== "ready" || raw.cluster !== configuration.clusterKey || raw.projectionAuthority !== "untrusted" || raw.signing !== false || raw.submission !== false) {
      throw new Error("operatord health does not match the selected cluster and read-only trust boundary.");
    }
    return Object.freeze({ status: "ready", cluster: raw.cluster, projectionAuthority: "untrusted", signing: false, submission: false });
  };

  const validateAcquisition = (raw, target) => {
    requirePlain(raw, "acquisition");
    if (raw.authority !== "untrusted read model") throw new Error("acquisition authority is not the expected untrusted read model.");
    if (bool(raw.authorityEligible, "acquisition.authorityEligible") !== false) throw new Error("operatord incorrectly marks an untrusted acquisition as authority-eligible.");
    const transportMode = text(raw.transportMode, "acquisition.transportMode", 80);
    if (transportMode !== "finalized-plus-processed-websocket") throw new Error("operatord is not serving the required finalized-plus-processed WebSocket transport.");
    const processedAvailable = bool(raw.processedAvailable, "acquisition.processedAvailable");
    requirePlain(raw.transportBinding, "acquisition.transportBinding");
    const binding = raw.transportBinding;
    requirePlain(binding.rpcHttpEndpoint, "transportBinding.rpcHttpEndpoint");
    requirePlain(binding.rpcWebsocketEndpoint, "transportBinding.rpcWebsocketEndpoint");
    const clusterName = text(binding.clusterName, "transportBinding.clusterName", 48);
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(clusterName)) throw new Error("daemon-projected cluster name contains unsupported characters.");
    const genesisHash = nonzeroAddress(binding.genesisHash, "transportBinding.genesisHash");
    const clusterKey = `${clusterName}:${genesisHash}`;
    const endpoint = (rawEndpoint, name) => {
      const redacted = text(rawEndpoint.redacted, `${name}.redacted`, 512);
      const separator = redacted.indexOf("://");
      const remainder = separator < 1 ? "" : redacted.slice(separator + 3);
      const boundaryCandidates = [remainder.indexOf("/"), remainder.indexOf("?")].filter((index) => index >= 0);
      const boundary = boundaryCandidates.length === 0 ? remainder.length : Math.min(...boundaryCandidates);
      const authority = remainder.slice(0, boundary);
      const suffix = remainder.slice(boundary);
      if (redacted.includes("@") || authority.length === 0 || !["", "/", "/<redacted>", "?<redacted>", "/?<redacted>", "/<redacted>?<redacted>"].includes(suffix)) throw new Error(`${name} leaks or malformedly represents endpoint credentials.`);
      return Object.freeze({ redacted, bindingSha256: hash32(rawEndpoint.bindingSha256, `${name}.bindingSha256`) });
    };
    const rpcHttpEndpoint = endpoint(binding.rpcHttpEndpoint, "transportBinding.rpcHttpEndpoint");
    const rpcWebsocketEndpoint = endpoint(binding.rpcWebsocketEndpoint, "transportBinding.rpcWebsocketEndpoint");
    if (binding.schema !== "dragons-clutch/operator-rpc-transport-binding/v3"
        || binding.verificationDisposition !== "last-complete-untrusted-http-release-bracket"
        || binding.authorityEligible !== false
        || text(binding.clusterKey, "transportBinding.clusterKey", 128) !== clusterKey
        || text(binding.decoderSet, "transportBinding.decoderSet", 128) !== DECODER_SET) {
      throw new Error("operatord transport binding is internally inconsistent or not the current decoder contract.");
    }
    if (!Array.isArray(binding.releases) || binding.releases.length !== 1) throw new Error("transportBinding must expose exactly one composed release.");
    const boundRelease = requirePlain(binding.releases[0], "transportBinding.release");
    const programId = nonzeroAddress(boundRelease.programId, "transportBinding.release.programId");
    const programData = nonzeroAddress(boundRelease.programData, "transportBinding.release.programData");
    if (programId === programData) throw new Error("daemon-projected Program and ProgramData identities alias.");
    const deploymentSlot = positiveDecimal(boundRelease.deploymentSlot, "transportBinding.release.deploymentSlot").toString();
    const elfSha256 = hash32(boundRelease.elfSha256, "transportBinding.release.elfSha256");
    const releaseManifestSha256 = hash32(boundRelease.releaseManifestSha256, "transportBinding.release.releaseManifestSha256");
    const capabilityProfileId = hash32(boundRelease.capabilityProfileId, "transportBinding.release.capabilityProfileId");
    if (typeof boundRelease.sourceCommit !== "string" || !COMMIT.test(boundRelease.sourceCommit)) throw new Error("daemon-projected source commit is not a full lowercase Git identity.");
    if (!Array.isArray(boundRelease.families) || boundRelease.families.length === 0 || boundRelease.families.length > 16) throw new Error("transportBinding.release.families is invalid.");
    const families = Object.freeze(boundRelease.families.map((family, index) => text(family, `transportBinding.release.families[${index}]`, 40)));
    if (new Set(families).size !== families.length) throw new Error("daemon-projected families are not unique.");
    if (!Array.isArray(boundRelease.enabledIntents) || boundRelease.enabledIntents.length > 256) throw new Error("transportBinding.release.enabledIntents is invalid.");
    const enabledIntents = Object.freeze(boundRelease.enabledIntents.map((intent, index) => {
      requirePlain(intent, `transportBinding.release.enabledIntents[${index}]`);
      const coordinate = Object.freeze({
        familyTag: positiveDecimal(intent.familyTag, `enabledIntents[${index}].familyTag`, 255n).toString(),
        familyVersion: positiveDecimal(intent.familyVersion, `enabledIntents[${index}].familyVersion`, 255n).toString(),
        localAction: decimal(intent.localAction, `enabledIntents[${index}].localAction`, 255n).toString()
      });
      if (index > 0) {
        const previous = boundRelease.enabledIntents[index - 1];
        const previousKey = [Number(previous.familyTag), Number(previous.familyVersion), Number(previous.localAction)];
        const key = [Number(coordinate.familyTag), Number(coordinate.familyVersion), Number(coordinate.localAction)];
        if (previousKey.join(".") === key.join(".") || previousKey[0] > key[0] || (previousKey[0] === key[0] && (previousKey[1] > key[1] || (previousKey[1] === key[1] && previousKey[2] >= key[2])))) throw new Error("daemon-projected enabled intents are not strictly canonical.");
      }
      return coordinate;
    }));
    const enabledIntentVariants = validateIntentVariants(boundRelease.enabledIntentVariants, "transportBinding.release.enabledIntentVariants", enabledIntents);
    const expectedReleaseKey = `${programId}:${deploymentSlot}:${elfSha256}:${releaseManifestSha256}`;
    if (text(boundRelease.releaseKey, "transportBinding.release.releaseKey", 320) !== expectedReleaseKey) throw new Error("daemon-projected release key does not bind its exact coordinates and manifest.");
    const release = Object.freeze({ programId, programData, deploymentSlot, elfSha256, releaseManifestSha256, sourceCommit: boundRelease.sourceCommit, capabilityProfileId, enabledIntents, enabledIntentVariants, families, releaseKey: expectedReleaseKey });
    const configuration = Object.freeze({
      ...target,
      schema: "dragons-clutch/browser-daemon-chain-projection/v3",
      authority: "untrusted-operatord-projection",
      decoderSet: DECODER_SET,
      clusterName,
      genesisHash,
      clusterKey,
      rpcHttpEndpoint,
      rpcWebsocketEndpoint,
      release
    });
    requirePlain(raw.processedSemantics, "acquisition.processedSemantics");
    if (raw.processedSemantics.finality !== "nonfinal-rollbackable"
        || raw.processedSemantics.authorityEligibility !== false
        || typeof raw.processedSemantics.websocketGenesis !== "string"
        || typeof raw.processedSemantics.accountRemoval !== "string") throw new Error("operatord does not expose the required genesis-bound, removal-aware, non-final, never-authoritative processed semantics.");
    const processedTransport = raw.processedTransport === null ? null : (() => {
      requirePlain(raw.processedTransport, "acquisition.processedTransport");
      return Object.freeze({
        phase: text(raw.processedTransport.phase, "processedTransport.phase", 80),
        available: bool(raw.processedTransport.available, "processedTransport.available"),
        websocketGenesisMatched: bool(raw.processedTransport.websocketGenesisMatched, "processedTransport.websocketGenesisMatched"),
        connectionGeneration: decimal(raw.processedTransport.connectionGeneration, "processedTransport.connectionGeneration").toString(),
        rollbackEpoch: decimal(raw.processedTransport.rollbackEpoch, "processedTransport.rollbackEpoch").toString(),
        reconnectAttempt: decimal(raw.processedTransport.reconnectAttempt, "processedTransport.reconnectAttempt").toString(),
        nextBackoffMilliseconds: decimal(raw.processedTransport.nextBackoffMilliseconds, "processedTransport.nextBackoffMilliseconds").toString(),
        reconnectIndexedVersionsWithdrawn: decimal(raw.processedTransport.reconnectIndexedVersionsWithdrawn, "processedTransport.reconnectIndexedVersionsWithdrawn").toString(),
        reconnectBufferedAccountsWithdrawn: decimal(raw.processedTransport.reconnectBufferedAccountsWithdrawn, "processedTransport.reconnectBufferedAccountsWithdrawn").toString(),
        deadSlotRollbacks: decimal(raw.processedTransport.deadSlotRollbacks, "processedTransport.deadSlotRollbacks").toString(),
        deadSlotIndexedVersionsWithdrawn: decimal(raw.processedTransport.deadSlotIndexedVersionsWithdrawn, "processedTransport.deadSlotIndexedVersionsWithdrawn").toString(),
        deadSlotBufferedAccountsWithdrawn: decimal(raw.processedTransport.deadSlotBufferedAccountsWithdrawn, "processedTransport.deadSlotBufferedAccountsWithdrawn").toString(),
        accountRemovalEvents: decimal(raw.processedTransport.accountRemovalEvents, "processedTransport.accountRemovalEvents").toString(),
        closedAccountRemovals: decimal(raw.processedTransport.closedAccountRemovals, "processedTransport.closedAccountRemovals").toString(),
        ownerChangedAccountRemovals: decimal(raw.processedTransport.ownerChangedAccountRemovals, "processedTransport.ownerChangedAccountRemovals").toString(),
        accountProjectionsWithdrawn: decimal(raw.processedTransport.accountProjectionsWithdrawn, "processedTransport.accountProjectionsWithdrawn").toString(),
        lastRollbackSlot: raw.processedTransport.lastRollbackSlot === null ? null : decimal(raw.processedTransport.lastRollbackSlot, "processedTransport.lastRollbackSlot").toString(),
        lastRemovedAccount: raw.processedTransport.lastRemovedAccount === null ? null : nonzeroAddress(raw.processedTransport.lastRemovedAccount, "processedTransport.lastRemovedAccount"),
        lastRemovalObservedOwner: raw.processedTransport.lastRemovalObservedOwner === null ? null : address(raw.processedTransport.lastRemovalObservedOwner, "processedTransport.lastRemovalObservedOwner"),
        lastRemovalKind: raw.processedTransport.lastRemovalKind === null ? null : (() => {
          if (raw.processedTransport.lastRemovalKind !== "closed" && raw.processedTransport.lastRemovalKind !== "owner-changed") throw new Error("processedTransport.lastRemovalKind is invalid.");
          return raw.processedTransport.lastRemovalKind;
        })(),
        lastError: raw.processedTransport.lastError === null ? null : text(raw.processedTransport.lastError, "processedTransport.lastError", 512)
      });
    })();
    if (processedTransport === null || processedTransport.available !== processedAvailable || (processedAvailable && !processedTransport.websocketGenesisMatched)) {
      throw new Error("operatord processed availability differs from its exact transport-generation state.");
    }
    if (target.commitment === "processed" && !processedAvailable) {
      throw new Error(`Processed projection is withdrawn: operatord reports ${processedTransport ? processedTransport.phase : transportMode} without a complete live subscription generation.`);
    }
    return Object.freeze({
      bootstrapComplete: bool(raw.bootstrapComplete, "acquisition.bootstrapComplete"),
      remainingScans: decimal(raw.remainingScans, "acquisition.remainingScans").toString(),
      remainingSubscriptionRegistrations: decimal(raw.remainingSubscriptionRegistrations, "acquisition.remainingSubscriptionRegistrations").toString(),
      activeSubscriptions: decimal(raw.activeSubscriptions, "acquisition.activeSubscriptions").toString(),
      pendingAccounts: decimal(raw.pendingAccounts, "acquisition.pendingAccounts").toString(),
      pendingAccountBytes: decimal(raw.pendingAccountBytes, "acquisition.pendingAccountBytes").toString(),
      pendingRoot: raw.pendingRoot === null ? null : decimal(raw.pendingRoot, "acquisition.pendingRoot").toString(),
      nextReceiveSequence: decimal(raw.nextReceiveSequence, "acquisition.nextReceiveSequence").toString(),
      authority: raw.authority,
      authorityEligible: false,
      transportMode,
      processedAvailable,
      processedTransport,
      transportBinding: Object.freeze({
        decoderSet: binding.decoderSet,
        rpcHttpEndpoint,
        rpcWebsocketEndpoint,
        releaseKey: configuration.release.releaseKey
      }),
      configuration
    });
  };

  const validateReleaseResponse = (raw, configuration) => {
    requirePlain(raw, "releases response");
    if (raw.cluster !== configuration.clusterKey || raw.authorityEligible !== false || !Array.isArray(raw.releases) || raw.releases.length > 256) throw new Error("release response does not match the selected cluster, trust boundary, or bounds.");
    const releases = raw.releases.map((release, index) => {
      requirePlain(release, `releases[${index}]`);
      if (!Array.isArray(release.families) || release.families.length === 0 || release.families.length > 16) throw new Error(`releases[${index}].families is invalid.`);
      const enabledIntents = Object.freeze(Array.isArray(release.enabledIntents) ? release.enabledIntents.map((intent, intentIndex) => {
        requirePlain(intent, `releases[${index}].enabledIntents[${intentIndex}]`);
        return Object.freeze({
          familyTag: positiveDecimal(intent.familyTag, `releases[${index}].enabledIntents[${intentIndex}].familyTag`, 255n).toString(),
          familyVersion: positiveDecimal(intent.familyVersion, `releases[${index}].enabledIntents[${intentIndex}].familyVersion`, 255n).toString(),
          localAction: decimal(intent.localAction, `releases[${index}].enabledIntents[${intentIndex}].localAction`, 255n).toString()
        });
      }) : (() => { throw new Error(`releases[${index}].enabledIntents is invalid.`); })());
      return Object.freeze({
        releaseKey: text(release.releaseKey, `releases[${index}].releaseKey`, 256),
        programId: address(release.programId, `releases[${index}].programId`),
        programData: address(release.programData, `releases[${index}].programData`),
        elfSha256: hash32(release.elfSha256, `releases[${index}].elfSha256`),
        deploymentSlot: positiveDecimal(release.deploymentSlot, `releases[${index}].deploymentSlot`).toString(),
        releaseManifestSha256: hash32(release.releaseManifestSha256, `releases[${index}].releaseManifestSha256`),
        capabilityProfileId: hash32(release.capabilityProfileId, `releases[${index}].capabilityProfileId`),
        sourceCommit: typeof release.sourceCommit === "string" && COMMIT.test(release.sourceCommit) ? release.sourceCommit : (() => { throw new Error(`releases[${index}].sourceCommit is invalid.`); })(),
        enabledIntents,
        enabledIntentVariants: validateIntentVariants(release.enabledIntentVariants, `releases[${index}].enabledIntentVariants`, enabledIntents),
        families: Object.freeze(release.families.map((family, familyIndex) => text(family, `releases[${index}].families[${familyIndex}]`, 40)))
      });
    });
    const selected = releases.find((release) => release.releaseKey === configuration.release.releaseKey);
    if (!selected) throw new Error("operatord release endpoint does not expose its acquisition-bound release key.");
    if (selected.programId !== configuration.release.programId || selected.programData !== configuration.release.programData || selected.elfSha256 !== configuration.release.elfSha256 || selected.deploymentSlot !== configuration.release.deploymentSlot || selected.releaseManifestSha256 !== configuration.release.releaseManifestSha256 || selected.capabilityProfileId !== configuration.release.capabilityProfileId || selected.sourceCommit !== configuration.release.sourceCommit || JSON.stringify(selected.enabledIntents) !== JSON.stringify(configuration.release.enabledIntents) || JSON.stringify(selected.enabledIntentVariants) !== JSON.stringify(configuration.release.enabledIntentVariants)) {
      throw new Error("operatord release endpoint differs from its acquisition transport binding.");
    }
    return Object.freeze({ cluster: raw.cluster, selected, observedReleaseCount: String(releases.length) });
  };

  const compareAddressBytes = (left, right) => {
    const leftBytes = decodeBase58(left, "canonical account address");
    const rightBytes = decodeBase58(right, "canonical account address");
    for (let index = 0; index < 32; index += 1) {
      if (leftBytes[index] !== rightBytes[index]) return leftBytes[index] - rightBytes[index];
    }
    return 0;
  };

  const validateSessionAccount = (raw, index, configuration) => {
    requirePlain(raw, `session.canonicalAccounts[${index}]`);
    requirePlain(raw.decode, `session.canonicalAccounts[${index}].decode`);
    if (raw.decode.status !== "canonical" && raw.decode.status !== "requires-context") throw new Error(`session.canonicalAccounts[${index}] has an unknown canonical codec disposition.`);
    const value = Object.freeze({
      address: nonzeroAddress(raw.address, `session.canonicalAccounts[${index}].address`),
      owner: nonzeroAddress(raw.owner, `session.canonicalAccounts[${index}].owner`),
      releaseKey: text(raw.releaseKey, `session.canonicalAccounts[${index}].releaseKey`, 320),
      lamports: decimal(raw.lamports, `session.canonicalAccounts[${index}].lamports`).toString(),
      rentEpoch: decimal(raw.rentEpoch, `session.canonicalAccounts[${index}].rentEpoch`).toString(),
      dataBytes: positiveDecimal(raw.dataBytes, `session.canonicalAccounts[${index}].dataBytes`).toString(),
      dataSha256: hash32(raw.dataSha256, `session.canonicalAccounts[${index}].dataSha256`),
      accountTag: decimal(raw.accountTag, `session.canonicalAccounts[${index}].accountTag`, 255n).toString(),
      accountVersion: decimal(raw.accountVersion, `session.canonicalAccounts[${index}].accountVersion`, 255n).toString(),
      family: text(raw.family, `session.canonicalAccounts[${index}].family`, 40),
      kind: text(raw.kind, `session.canonicalAccounts[${index}].kind`, 80),
      decode: Object.freeze({
        status: raw.decode.status,
        requirement: raw.decode.status === "requires-context" ? text(raw.decode.requirement, `session.canonicalAccounts[${index}].decode.requirement`, 240) : null
      }),
      generation: raw.generation === null ? null : decimal(raw.generation, `session.canonicalAccounts[${index}].generation`).toString(),
      primaryBinding: raw.primaryBinding === null ? null : hash32(raw.primaryBinding, `session.canonicalAccounts[${index}].primaryBinding`),
      secondaryBinding: raw.secondaryBinding === null ? null : hash32(raw.secondaryBinding, `session.canonicalAccounts[${index}].secondaryBinding`)
    });
    if (value.owner !== configuration.release.programId || value.releaseKey !== configuration.release.releaseKey) throw new Error(`session.canonicalAccounts[${index}] is not owned by the checked release.`);
    return value;
  };

  const validateRestartCursor = (raw, index) => {
    requirePlain(raw, `session.restart.cursors[${index}]`);
    requirePlain(raw.cursor, `session.restart.cursors[${index}].cursor`);
    if (!Array.isArray(raw.dependencies) || raw.dependencies.length > 128) throw new Error(`session.restart.cursors[${index}].dependencies is invalid.`);
    return Object.freeze({
      account: nonzeroAddress(raw.account, `session.restart.cursors[${index}].account`),
      releaseKey: text(raw.releaseKey, `session.restart.cursors[${index}].releaseKey`, 320),
      action: text(raw.action, `session.restart.cursors[${index}].action`, 80),
      dependencies: Object.freeze(raw.dependencies.map((dependency, dependencyIndex) => nonzeroAddress(dependency, `session.restart.cursors[${index}].dependencies[${dependencyIndex}]`))),
      cursor: Object.freeze({
        workflowId: hash32(raw.cursor.workflowId, `session.restart.cursors[${index}].cursor.workflowId`),
        lane: text(raw.cursor.lane, `session.restart.cursors[${index}].cursor.lane`, 48),
        generation: positiveDecimal(raw.cursor.generation, `session.restart.cursors[${index}].cursor.generation`).toString(),
        phase: decimal(raw.cursor.phase, `session.restart.cursors[${index}].cursor.phase`, 65535n).toString(),
        item: decimal(raw.cursor.item, `session.restart.cursors[${index}].cursor.item`).toString(),
        observedStateSha256: hash32(raw.cursor.observedStateSha256, `session.restart.cursors[${index}].cursor.observedStateSha256`)
      })
    });
  };

  const validateSession = (raw, configuration) => {
    requirePlain(raw, "read-only session manifest");
    if (raw.schema !== "dragons-clutch/operator-read-only-session-manifest/v1"
        || raw.status !== "ready"
        || raw.projectionAuthority !== "untrusted-canonical-codec-projection"
        || raw.authorityEligible !== false
        || raw.signing !== false
        || raw.submission !== false
        || raw.commitment !== "finalized") throw new Error("operatord session manifest has an unknown schema or dishonest authority boundary.");
    requirePlain(raw.transport, "session.transport");
    requirePlain(raw.transport.rpcHttpEndpoint, "session.transport.rpcHttpEndpoint");
    requirePlain(raw.transport.rpcWebsocketEndpoint, "session.transport.rpcWebsocketEndpoint");
    if (text(raw.transport.clusterName, "session.transport.clusterName", 48) !== configuration.clusterName
        || nonzeroAddress(raw.transport.genesisHash, "session.transport.genesisHash") !== configuration.genesisHash
        || text(raw.transport.clusterKey, "session.transport.clusterKey", 128) !== configuration.clusterKey
        || text(raw.transport.rpcHttpEndpoint.redacted, "session.transport.rpcHttpEndpoint.redacted", 512) !== configuration.rpcHttpEndpoint.redacted
        || hash32(raw.transport.rpcHttpEndpoint.bindingSha256, "session.transport.rpcHttpEndpoint.bindingSha256") !== configuration.rpcHttpEndpoint.bindingSha256
        || text(raw.transport.rpcWebsocketEndpoint.redacted, "session.transport.rpcWebsocketEndpoint.redacted", 512) !== configuration.rpcWebsocketEndpoint.redacted
        || hash32(raw.transport.rpcWebsocketEndpoint.bindingSha256, "session.transport.rpcWebsocketEndpoint.bindingSha256") !== configuration.rpcWebsocketEndpoint.bindingSha256) throw new Error("session transport differs from the explicit acquisition binding.");
    requirePlain(raw.release, "session.release");
    const release = raw.release;
    if (text(release.releaseKey, "session.release.releaseKey", 320) !== configuration.release.releaseKey
        || nonzeroAddress(release.programId, "session.release.programId") !== configuration.release.programId
        || nonzeroAddress(release.programData, "session.release.programData") !== configuration.release.programData
        || positiveDecimal(release.deploymentSlot, "session.release.deploymentSlot").toString() !== configuration.release.deploymentSlot
        || hash32(release.elfSha256, "session.release.elfSha256") !== configuration.release.elfSha256
        || hash32(release.releaseManifestSha256, "session.release.releaseManifestSha256") !== configuration.release.releaseManifestSha256
        || hash32(release.capabilityProfileId, "session.release.capabilityProfileId") !== configuration.release.capabilityProfileId
        || text(release.sourceCommit, "session.release.sourceCommit", 64) !== configuration.release.sourceCommit
        || text(release.decoderSet, "session.release.decoderSet", 128) !== configuration.decoderSet) throw new Error("session checked release/profile identity differs from acquisition.");
    if (!Array.isArray(release.enabledIntents) || release.enabledIntents.length !== configuration.release.enabledIntents.length) throw new Error("session checked release enabled-intent set differs from acquisition.");
    release.enabledIntents.forEach((intent, index) => {
      requirePlain(intent, `session.release.enabledIntents[${index}]`);
      const expected = configuration.release.enabledIntents[index];
      if (positiveDecimal(intent.familyTag, "session enabled family tag", 255n).toString() !== expected.familyTag || positiveDecimal(intent.familyVersion, "session enabled family version", 255n).toString() !== expected.familyVersion || positiveDecimal(intent.localAction, "session enabled local action", 255n).toString() !== expected.localAction) throw new Error("session checked release enabled-intent coordinate differs from acquisition.");
    });
    const enabledIntentVariants = validateIntentVariants(release.enabledIntentVariants, "session.release.enabledIntentVariants", configuration.release.enabledIntents);
    if (JSON.stringify(enabledIntentVariants) !== JSON.stringify(configuration.release.enabledIntentVariants)) throw new Error("session checked release payload-variant set differs from acquisition.");
    if (!Array.isArray(raw.canonicalAccounts) || BigInt(raw.canonicalAccounts.length) > BigInt(configuration.bounds.maximumAccounts)) throw new Error("session canonical account identities exceed the explicit browser bound.");
    const canonicalAccounts = Object.freeze(raw.canonicalAccounts.map((accountValue, index) => validateSessionAccount(accountValue, index, configuration)));
    if (new Set(canonicalAccounts.map((accountValue) => accountValue.address)).size !== canonicalAccounts.length) throw new Error("session canonical account identities contain duplicate addresses.");
    for (let index = 1; index < canonicalAccounts.length; index += 1) {
      if (compareAddressBytes(canonicalAccounts[index - 1].address, canonicalAccounts[index].address) >= 0) throw new Error("session canonical account identities are not in canonical address-byte order.");
    }
    requirePlain(raw.restart, "session.restart");
    if (raw.restart.semantics !== "reload every named account through its canonical codec and reauthenticate all joins before using a cursor"
        || raw.restart.identitySource !== "finalized onchain account bodies plus immutable checked release and RPC bindings"
        || decimal(raw.restart.accountCount, "session.restart.accountCount").toString() !== String(canonicalAccounts.length)
        || !Array.isArray(raw.restart.cursors)) throw new Error("session restart contract is malformed or not onchain-owned.");
    const cursors = Object.freeze(raw.restart.cursors.map((cursor, index) => validateRestartCursor(cursor, index)));
    if (decimal(raw.restart.cursorCount, "session.restart.cursorCount").toString() !== String(cursors.length)) throw new Error("session restart cursor count differs from its exact cursor set.");
    const addresses = new Set(canonicalAccounts.map((accountValue) => accountValue.address));
    for (const cursor of cursors) {
      if (cursor.releaseKey !== configuration.release.releaseKey) throw new Error("session restart cursor names another release.");
      if (!addresses.has(cursor.account) || cursor.dependencies.some((dependency) => !addresses.has(dependency))) throw new Error("session restart cursor names an identity not owned by a finalized canonical account decode.");
      if (new Set(cursor.dependencies).size !== cursor.dependencies.length) throw new Error("session restart cursor repeats a dependency identity.");
    }
    return Object.freeze({
      schema: raw.schema,
      status: raw.status,
      sessionId: hash32(raw.sessionId, "session.sessionId"),
      projectionAuthority: raw.projectionAuthority,
      authorityEligible: false,
      signing: false,
      submission: false,
      commitment: "finalized",
      transport: Object.freeze({ clusterName: configuration.clusterName, genesisHash: configuration.genesisHash, clusterKey: configuration.clusterKey, rpcHttpEndpoint: configuration.rpcHttpEndpoint, rpcWebsocketEndpoint: configuration.rpcWebsocketEndpoint }),
      release: Object.freeze({ ...configuration.release, enabledIntentVariants, decoderSet: configuration.decoderSet }),
      canonicalAccounts,
      restart: Object.freeze({ semantics: raw.restart.semantics, identitySource: raw.restart.identitySource, accountCount: String(canonicalAccounts.length), cursorCount: String(cursors.length), cursors })
    });
  };

  const validateBranch = (raw, name) => {
    requirePlain(raw, name);
    if (raw.kind === "finalized-scan") return Object.freeze({ kind: raw.kind, blockhash: null });
    if (raw.kind === "processed-fork") return Object.freeze({ kind: raw.kind, blockhash: text(raw.blockhash, `${name}.blockhash`, 96) });
    throw new Error(`${name}.kind is unknown.`);
  };

  const validateAccount = (raw, index, effectiveCommitment) => {
    requirePlain(raw, `accounts[${index}]`);
    const expectedDisposition = effectiveCommitment === "processed" ? "nonfinal-rollbackable" : "finalized-projection";
    if (raw.effectiveCommitment !== effectiveCommitment || raw.finalityDisposition !== expectedDisposition || raw.authorityEligible !== false) throw new Error(`accounts[${index}] has the wrong effective commitment or trust disposition.`);
    requirePlain(raw.decode, `accounts[${index}].decode`);
    if (raw.decode.status !== "canonical" && raw.decode.status !== "requires-context") throw new Error(`accounts[${index}] has an unknown decode status.`);
    const requirement = raw.decode.status === "requires-context" ? text(raw.decode.requirement, `accounts[${index}].decode.requirement`, 240) : null;
    const value = {
      address: address(raw.address, `accounts[${index}].address`),
      owner: address(raw.owner, `accounts[${index}].owner`),
      releaseKey: text(raw.releaseKey, `accounts[${index}].releaseKey`, 256),
      slot: decimal(raw.slot, `accounts[${index}].slot`).toString(),
      receiveSequence: decimal(raw.receiveSequence, `accounts[${index}].receiveSequence`).toString(),
      observedCommitment: raw.observedCommitment === "processed" || raw.observedCommitment === "finalized" ? raw.observedCommitment : (() => { throw new Error(`accounts[${index}].observedCommitment is invalid.`); })(),
      effectiveCommitment,
      lamports: decimal(raw.lamports, `accounts[${index}].lamports`).toString(),
      rentEpoch: decimal(raw.rentEpoch, `accounts[${index}].rentEpoch`).toString(),
      dataBytes: decimal(raw.dataBytes, `accounts[${index}].dataBytes`).toString(),
      dataSha256: hash32(raw.dataSha256, `accounts[${index}].dataSha256`),
      accountTag: decimal(raw.accountTag, `accounts[${index}].accountTag`, 255n).toString(),
      accountVersion: decimal(raw.accountVersion, `accounts[${index}].accountVersion`, 255n).toString(),
      family: text(raw.family, `accounts[${index}].family`, 40),
      kind: text(raw.kind, `accounts[${index}].kind`, 80),
      decode: Object.freeze({ status: raw.decode.status, requirement }),
      generation: raw.generation === null ? null : decimal(raw.generation, `accounts[${index}].generation`).toString(),
      primaryBinding: raw.primaryBinding === null ? null : hash32(raw.primaryBinding, `accounts[${index}].primaryBinding`),
      secondaryBinding: raw.secondaryBinding === null ? null : hash32(raw.secondaryBinding, `accounts[${index}].secondaryBinding`),
      branch: validateBranch(raw.branch, `accounts[${index}].branch`)
    };
    return Object.freeze({ ...value, bindingProjection: bindingProjection(value.kind, value.primaryBinding, value.secondaryBinding) });
  };

  const validateAccountsResponse = (raw, configuration) => {
    requirePlain(raw, "accounts response");
    const expectedDisposition = configuration.commitment === "processed" ? "nonfinal-rollbackable" : "finalized-projection";
    if (raw.cluster !== configuration.clusterKey || raw.effectiveCommitment !== configuration.commitment || raw.finalityDisposition !== expectedDisposition || raw.authorityEligible !== false || !Array.isArray(raw.accounts)) throw new Error("accounts response differs from the selected cluster/commitment/trust boundary.");
    if (BigInt(raw.accounts.length) > BigInt(configuration.bounds.maximumAccounts)) throw new Error("accounts response exceeds the explicit browser account bound.");
    const all = raw.accounts.map((accountValue, index) => validateAccount(accountValue, index, configuration.commitment));
    const selected = all.filter((accountValue) => accountValue.releaseKey === configuration.release.releaseKey);
    if (selected.some((accountValue) => accountValue.owner !== configuration.release.programId)) throw new Error("A selected-release account owner differs from the selected program.");
    return Object.freeze({ selected: Object.freeze(selected), ignoredOtherReleases: String(all.length - selected.length) });
  };

  const joinSessionAccounts = (session, finalizedResponse) => {
    if (session.canonicalAccounts.length !== finalizedResponse.selected.length) throw new Error("session account identity count differs from the finalized canonical account endpoint.");
    const finalized = new Map(finalizedResponse.selected.map((accountValue) => [accountValue.address, accountValue]));
    for (const identity of session.canonicalAccounts) {
      const accountValue = finalized.get(identity.address);
      if (!accountValue || accountValue.branch.kind !== "finalized-scan") throw new Error("session identity is absent from the finalized canonical account endpoint.");
      for (const field of ["owner", "releaseKey", "lamports", "rentEpoch", "dataBytes", "dataSha256", "accountTag", "accountVersion", "family", "kind", "generation", "primaryBinding", "secondaryBinding"]) {
        if (accountValue[field] !== identity[field]) throw new Error(`session identity ${identity.address} differs from finalized canonical account field ${field}.`);
      }
      if (accountValue.decode.status !== identity.decode.status || accountValue.decode.requirement !== identity.decode.requirement) throw new Error(`session identity ${identity.address} differs from its finalized canonical decode disposition.`);
    }
  };

  const validateForks = (raw) => {
    requirePlain(raw, "fork response");
    if (raw.authorityEligible !== false || raw.processedTopology !== true || !Array.isArray(raw.frozenSlots) || raw.frozenSlots.length > 65_536 || !Array.isArray(raw.deadSlots) || raw.deadSlots.length > 65_536 || !Array.isArray(raw.nodes) || raw.nodes.length > 65_536) throw new Error("fork response trust boundary or arrays are invalid.");
    const finalizedRoot = raw.finalizedRoot === null ? null : (() => {
      requirePlain(raw.finalizedRoot, "forks.finalizedRoot");
      return Object.freeze({ slot: decimal(raw.finalizedRoot.slot, "forks.finalizedRoot.slot").toString(), blockhash: text(raw.finalizedRoot.blockhash, "forks.finalizedRoot.blockhash", 96) });
    })();
    const frozenSlots = Object.freeze(raw.frozenSlots.map((slot, index) => decimal(slot, `forks.frozenSlots[${index}]`).toString()));
    const deadSlots = Object.freeze(raw.deadSlots.map((slot, index) => decimal(slot, `forks.deadSlots[${index}]`).toString()));
    if (new Set(frozenSlots).size !== frozenSlots.length || new Set(deadSlots).size !== deadSlots.length) throw new Error("fork response contains duplicate frozen or dead slots.");
    const nodes = Object.freeze(raw.nodes.map((node, index) => {
      requirePlain(node, `forks.nodes[${index}]`);
      return Object.freeze({
        slot: decimal(node.slot, `forks.nodes[${index}].slot`).toString(),
        parentSlot: decimal(node.parentSlot, `forks.nodes[${index}].parentSlot`).toString(),
        blockhash: text(node.blockhash, `forks.nodes[${index}].blockhash`, 96),
        previousBlockhash: text(node.previousBlockhash, `forks.nodes[${index}].previousBlockhash`, 96),
        receiveSequence: decimal(node.receiveSequence, `forks.nodes[${index}].receiveSequence`).toString()
      });
    }));
    if (new Set(nodes.map((node) => `${node.slot}:${node.blockhash}`)).size !== nodes.length) throw new Error("fork response contains duplicate slot/blockhash nodes.");
    return Object.freeze({ finalizedRoot, frozenSlots, deadSlots, nodes });
  };

  const validateKeeper = (raw, configuration) => {
    requirePlain(raw, "keeper response");
    if (raw.effectiveCommitment !== configuration.commitment || !Array.isArray(raw.actions) || raw.actions.length > 4096) throw new Error("keeper response differs from the selected commitment or bounds.");
    if (raw.authorityEligible !== false) throw new Error("keeper projection must never claim authority eligibility.");
    if (configuration.commitment === "processed" && raw.actions.length !== 0) throw new Error("processed keeper construction must remain disabled.");
    const actions = raw.actions.map((actionValue, index) => {
      requirePlain(actionValue, `keeper.actions[${index}]`);
      requirePlain(actionValue.cursor, `keeper.actions[${index}].cursor`);
      if (!Array.isArray(actionValue.dependencies) || actionValue.dependencies.length > 128) throw new Error(`keeper.actions[${index}].dependencies is invalid.`);
      const observedCommitment = actionValue.observedCommitment;
      if (observedCommitment !== "processed" && observedCommitment !== "finalized") throw new Error(`keeper.actions[${index}].observedCommitment is invalid.`);
      if (actionValue.effectiveCommitment !== configuration.commitment) throw new Error(`keeper.actions[${index}].effectiveCommitment is invalid.`);
      return Object.freeze({
        account: address(actionValue.account, `keeper.actions[${index}].account`),
        releaseKey: text(actionValue.releaseKey, `keeper.actions[${index}].releaseKey`, 256),
        action: text(actionValue.action, `keeper.actions[${index}].action`, 80),
        accountSlot: decimal(actionValue.accountSlot, `keeper.actions[${index}].accountSlot`).toString(),
        observedCommitment,
        effectiveCommitment: actionValue.effectiveCommitment,
        branch: validateBranch(actionValue.branch, `keeper.actions[${index}].branch`),
        dependencies: Object.freeze(actionValue.dependencies.map((dependency, dependencyIndex) => address(dependency, `keeper.actions[${index}].dependencies[${dependencyIndex}]`))),
        cursor: Object.freeze({
          workflowId: hash32(actionValue.cursor.workflowId, `keeper.actions[${index}].cursor.workflowId`),
          lane: text(actionValue.cursor.lane, `keeper.actions[${index}].cursor.lane`, 48),
          generation: positiveDecimal(actionValue.cursor.generation, `keeper.actions[${index}].cursor.generation`).toString(),
          phase: decimal(actionValue.cursor.phase, `keeper.actions[${index}].cursor.phase`, 65535n).toString(),
          item: decimal(actionValue.cursor.item, `keeper.actions[${index}].cursor.item`).toString(),
          observedStateSha256: hash32(actionValue.cursor.observedStateSha256, `keeper.actions[${index}].cursor.observedStateSha256`)
        })
      });
    }).filter((actionValue) => actionValue.releaseKey === configuration.release.releaseKey);
    return Object.freeze(actions);
  };

  const executionReleaseKey = (value, manifestSha256, name) => {
    text(value, name, 320);
    const fields = value.split(":");
    if (fields.length !== 4) throw new Error(`${name} does not contain exact Program/deployment/ELF/manifest coordinates.`);
    const parsed = Object.freeze({
      programId: nonzeroAddress(fields[0], `${name}.programId`),
      deploymentSlot: positiveDecimal(fields[1], `${name}.deploymentSlot`).toString(),
      elfSha256: hash32(fields[2], `${name}.elfSha256`),
      releaseManifestSha256: hash32(fields[3], `${name}.releaseManifestSha256`)
    });
    if (parsed.releaseManifestSha256 !== manifestSha256) throw new Error(`${name} differs from its exact execution manifest digest.`);
    return Object.freeze({ ...parsed, releaseKey: value });
  };

  const validateActionReleaseAdmission = (raw, coordinate, callable, configuration, index) => {
    requirePlain(raw, `actions[${index}].releaseAdmission`);
    if (raw.enabled !== true || raw.releaseKey !== configuration.release.releaseKey || raw.capabilityProfileId !== configuration.release.capabilityProfileId) throw new Error("action release admission differs from the checked indexed release.");
    if (!callable) return Object.freeze({ scope: "indexed-release-coordinate-only", executionReleaseKey: null, driverReleaseKey: configuration.release.releaseKey, executionReleaseManifestSha256: null });
    const structured = coordinate.familyTag === STRUCTURED_COORDINATE.familyTag && coordinate.familyVersion === STRUCTURED_COORDINATE.familyVersion && coordinate.family === STRUCTURED_COORDINATE.family;
    const source = coordinate.familyTag === SOURCE_COORDINATE.familyTag && coordinate.familyVersion === SOURCE_COORDINATE.familyVersion && coordinate.family === SOURCE_COORDINATE.family;
    const direct = coordinate.familyTag === DIRECT_COORDINATE.familyTag && coordinate.familyVersion === DIRECT_COORDINATE.familyVersion && coordinate.family === DIRECT_COORDINATE.family && DIRECT_ACTION_CONTRACTS[coordinate.localAction] !== undefined;
    const fractional = coordinate.familyTag === FRACTIONAL_COORDINATE.familyTag && coordinate.familyVersion === FRACTIONAL_COORDINATE.familyVersion && coordinate.family === FRACTIONAL_COORDINATE.family && FRACTIONAL_ACTION_CONTRACTS[coordinate.localAction] !== undefined;
    if (!structured && !source && !direct && !fractional) throw new Error("callable action lacks a current browser release-composition contract.");
    const expectedScope = structured ? "structured-composite-wrapper-execution-base-driver-v1" : "single-release-execution-and-driver-v1";
    const manifestSha256 = hash32(raw.executionReleaseManifestSha256, `actions[${index}].releaseAdmission.executionReleaseManifestSha256`);
    const execution = executionReleaseKey(raw.executionReleaseKey, manifestSha256, `actions[${index}].releaseAdmission.executionReleaseKey`);
    const driverReleaseKey = text(raw.driverReleaseKey, `actions[${index}].releaseAdmission.driverReleaseKey`, 320);
    if (raw.scope !== expectedScope || driverReleaseKey !== configuration.release.releaseKey) throw new Error("action release admission has the wrong execution/driver scope.");
    if (structured && execution.releaseKey === driverReleaseKey) throw new Error("Structured action aliases its disjoint wrapper execution and base driver releases.");
    if ((source || direct || fractional) && (execution.releaseKey !== configuration.release.releaseKey || manifestSha256 !== configuration.release.releaseManifestSha256)) throw new Error("single-release action does not use its one checked execution/driver release.");
    return Object.freeze({ scope: expectedScope, executionReleaseKey: execution.releaseKey, driverReleaseKey, executionReleaseManifestSha256: manifestSha256 });
  };

  const validateDealerVariantVerdict = (row, coordinate, payloadVariant, accountRoles, configuration, index) => {
    if (row.releaseAdmission.enabled !== true
        || row.releaseAdmission.scope !== "payload-discriminator-only"
        || row.releaseAdmission.coarseCoordinateEnabled !== false
        || row.releaseAdmission.releaseKey !== configuration.release.releaseKey
        || row.releaseAdmission.capabilityProfileId !== configuration.release.capabilityProfileId
        || row.stateSelection !== null
        || row.semanticOwnerConstructor !== "chain-derived-dealer-terminal-v1") {
      throw new Error("Dealer payload-variant verdict promotes a coarse tuple or differs from its checked release.");
    }
    const projectedCallable = bool(row.callable, `actions[${index}].callable`);
    if (!projectedCallable) {
      if (row.verdict !== "unavailable" || row.transactionDraft !== null || !Array.isArray(row.signerRequirements) || row.signerRequirements.length !== 0 || typeof row.freshnessDisposition !== "string") throw new Error("unavailable Dealer payload variant carries executable-looking material.");
      return Object.freeze({ coordinate, payloadVariant, accountRoles, callable: false, projectedCallable: false, verdict: row.verdict, reason: text(row.reason, `actions[${index}].reason`, 512), stateSelection: null, transactionDraft: null, signerRequirements: Object.freeze([]), freshnessDisposition: null });
    }
    const expectedRoles = [...DEALER_COMMON_ROLES, ...(payloadVariant.payloadDiscriminator === "8" ? DEALER_ACTIVE_TAIL : DEALER_UNUSED_TAIL)];
    if (accountRoles.length !== expectedRoles.length || accountRoles.some((role, roleIndex) => role.role !== expectedRoles[roleIndex].role || role.signer !== expectedRoles[roleIndex].signer || role.writable !== expectedRoles[roleIndex].writable || role.address === null || role.identityDisposition !== "semantic-owner-derived-and-bound-to-draft")) throw new Error("callable Dealer payload variant differs from its exact 48/45-role target contract.");
    requirePlain(row.observationSet, `actions[${index}].observationSet`);
    if (row.observationSet.schema !== "dragons-clutch/operator/dealer-terminal-observation-set/v1" || !Array.isArray(row.observationSet.accounts) || row.observationSet.accounts.length !== expectedRoles.length) throw new Error("Dealer payload variant lacks its exact finalized observation set.");
    const observationSet = Object.freeze({
      schema: row.observationSet.schema,
      payloadDiscriminator: positiveDecimal(row.observationSet.payloadDiscriminator, `actions[${index}].observationSet.payloadDiscriminator`, 255n).toString(),
      authorityStateSha256: hash32(row.observationSet.authorityStateSha256, `actions[${index}].observationSet.authorityStateSha256`),
      chainStateSha256: hash32(row.observationSet.chainStateSha256, `actions[${index}].observationSet.chainStateSha256`),
      collateralCatalogReceiptId: hash32(row.observationSet.collateralCatalogReceiptId, `actions[${index}].observationSet.collateralCatalogReceiptId`),
      lookupTableStateSha256: hash32(row.observationSet.lookupTableStateSha256, `actions[${index}].observationSet.lookupTableStateSha256`),
      accounts: Object.freeze(row.observationSet.accounts.map((observation, observationIndex) => {
        requirePlain(observation, `actions[${index}].observationSet.accounts[${observationIndex}]`);
        if (decimal(observation.index, `actions[${index}].observationSet.accounts[${observationIndex}].index`).toString() !== String(observationIndex)) throw new Error("Dealer observations are not in exact semantic-role order.");
        const value = {
          index: String(observationIndex),
          address: nonzeroAddress(observation.address, `actions[${index}].observationSet.accounts[${observationIndex}].address`),
          observedSlot: positiveDecimal(observation.observedSlot, `actions[${index}].observationSet.accounts[${observationIndex}].observedSlot`).toString(),
          disposition: observation.disposition
        };
        if (value.address !== accountRoles[observationIndex].address) throw new Error("Dealer observation identity differs from its exact semantic role.");
        const mustBeAbsent = observationIndex === 15;
        if (mustBeAbsent) {
          if (observation.disposition !== "finalized-absent" || observation.owner !== null || observation.lamports !== null || observation.executable !== null || observation.rentEpoch !== null || observation.dataSha256 !== null || observation.releaseKey !== null) throw new Error("Dealer liveness receipt creation target is not one finalized exact absence.");
          return Object.freeze({ ...value, owner: null, lamports: null, executable: null, rentEpoch: null, dataSha256: null, releaseKey: null });
        }
        if (observation.disposition !== "finalized-present") throw new Error("Dealer required role is not one finalized present account observation.");
        return Object.freeze({
          ...value,
          owner: nonzeroAddress(observation.owner, `actions[${index}].observationSet.accounts[${observationIndex}].owner`),
          lamports: decimal(observation.lamports, `actions[${index}].observationSet.accounts[${observationIndex}].lamports`).toString(),
          executable: bool(observation.executable, `actions[${index}].observationSet.accounts[${observationIndex}].executable`),
          rentEpoch: decimal(observation.rentEpoch, `actions[${index}].observationSet.accounts[${observationIndex}].rentEpoch`).toString(),
          dataSha256: hash32(observation.dataSha256, `actions[${index}].observationSet.accounts[${observationIndex}].dataSha256`),
          releaseKey: text(observation.releaseKey, `actions[${index}].observationSet.accounts[${observationIndex}].releaseKey`, 320)
        });
      }))
    });
    if (observationSet.payloadDiscriminator !== payloadVariant.payloadDiscriminator) throw new Error("Dealer observation set differs from its exact target discriminator.");
    requirePlain(row.transactionDraft, `actions[${index}].transactionDraft`);
    const draft = row.transactionDraft;
    if (row.verdict !== "callable-unsigned-draft"
        || draft.schema !== "dragons-clutch/operator-canonical-action-material/v1"
        || draft.constructionSchema !== "dragons-clutch/operator/unsigned-protocol-transaction/v3"
        || hash32(draft.releaseManifestSha256, "Dealer draft release manifest") !== configuration.release.releaseManifestSha256
        || hash32(draft.capabilityProfileId, "Dealer draft capability profile") !== configuration.release.capabilityProfileId
        || text(draft.executionReleaseKey, "Dealer draft execution release key", 320) !== configuration.release.releaseKey
        || text(draft.driverReleaseKey, "Dealer draft driver release key", 320) !== configuration.release.releaseKey
        || draft.messageVersion !== "v0"
        || draft.recentBlockhash !== null
        || draft.hasRecentBlockhash !== false
        || draft.signed !== false
        || draft.submitted !== false
        || draft.reloadAuthoritativeAccounts !== true) throw new Error("callable Dealer payload variant violates its unsigned release boundary.");
    const driverAccount = nonzeroAddress(draft.driverAccount, "Dealer draft driver account");
    const driverAccountSlot = decimal(draft.driverAccountSlot, "Dealer draft driver account slot").toString();
    if (driverAccount !== observationSet.accounts[2].address || driverAccountSlot !== observationSet.accounts[2].observedSlot) throw new Error("Dealer draft driver differs from the exact state-v3 observation.");
    const transactionHex = text(draft.serializedTransactionHex, "Dealer serialized transaction", 2464);
    if (!HEX_BYTES.test(transactionHex) || decimal(draft.serializedBytes, "Dealer serialized transaction bytes", 1232n).toString() !== String(transactionHex.length / 2)) throw new Error("Dealer serialized transaction is not bounded canonical hexadecimal bytes.");
    if (!Array.isArray(draft.addressLookupTables) || draft.addressLookupTables.length !== 1) throw new Error("Dealer target-8/9 material does not use exactly one authenticated lookup table.");
    const lookup = draft.addressLookupTables[0];
    requirePlain(lookup, "Dealer draft lookup table");
    const writableAddresses = decimal(lookup.writableAddresses, "Dealer draft lookup writable addresses", 256n);
    const readonlyAddresses = decimal(lookup.readonlyAddresses, "Dealer draft lookup readonly addresses", 256n);
    if (writableAddresses + readonlyAddresses === 0n || writableAddresses + readonlyAddresses > 256n) throw new Error("Dealer lookup table has an invalid exact address count.");
    const addressLookupTables = Object.freeze([Object.freeze({ account: nonzeroAddress(lookup.account, "Dealer draft lookup account"), observedSlot: positiveDecimal(lookup.observedSlot, "Dealer draft lookup observed slot").toString(), stateSha256: hash32(lookup.stateSha256, "Dealer draft lookup state"), writableAddresses: writableAddresses.toString(), readonlyAddresses: readonlyAddresses.toString() })]);
    if (addressLookupTables[0].stateSha256 !== observationSet.lookupTableStateSha256 || hash32(draft.authorityStateSha256, "Dealer draft authority state") !== observationSet.authorityStateSha256) throw new Error("Dealer observation set is not bound to the draft authority and lookup-table state.");
    if (!Array.isArray(draft.actions) || draft.actions.length !== 1 || draft.actions[0] !== payloadVariant.name || !Array.isArray(draft.flows) || draft.flows.length !== 1 || draft.flows[0] !== "dealer-facility-terminal" || !Array.isArray(draft.semanticOwners) || draft.semanticOwners.length !== 1 || !Array.isArray(draft.registryBindings) || draft.registryBindings.length !== 1 || !Array.isArray(draft.runtimeAdmissions) || draft.runtimeAdmissions.length !== 1 || draft.runtimeAdmissions[0] !== "payload-variant-release-bound-enabled" || !Array.isArray(draft.exactEquations) || draft.exactEquations.length === 0) throw new Error("Dealer draft is not one exact payload-variant terminal action.");
    const owner = draft.semanticOwners[0];
    requirePlain(owner, "Dealer draft semantic owner");
    if (owner.package !== "clutch-dealer-runtime-contract" || owner.schema !== "dragons-clutch/dealer-terminal-retire/action25/targets8-9/v1" || hash32(owner.releaseSha256, "Dealer semantic owner release") !== configuration.release.elfSha256) throw new Error("Dealer draft semantic owner differs from the checked target-8/9 owner.");
    const binding = draft.registryBindings[0];
    requirePlain(binding, "Dealer draft registry binding");
    if (positiveDecimal(binding.familyTag, "Dealer binding family tag", 255n).toString() !== "76" || positiveDecimal(binding.familyVersion, "Dealer binding family version", 255n).toString() !== "1" || positiveDecimal(binding.localAction, "Dealer binding local action", 255n).toString() !== "25" || binding.allocationStatus !== "frozen" || decimal(binding.centralAction, "Dealer binding central action", 255n).toString() !== "25") throw new Error("Dealer draft promotes or changes its exact allocated coordinate.");
    draft.exactEquations.forEach((equation, equationIndex) => { requirePlain(equation, `Dealer exactEquations[${equationIndex}]`); requirePlain(equation.unit, `Dealer exactEquations[${equationIndex}].unit`); text(equation.name, `Dealer exactEquations[${equationIndex}].name`, 200); if (decimal(equation.left, "Dealer exact equation left") !== decimal(equation.right, "Dealer exact equation right")) throw new Error("Dealer exact-integer equation is unbalanced."); });
    if (!Array.isArray(row.signerRequirements) || row.signerRequirements.length > 64) throw new Error("Dealer signer projection exceeds the browser bound.");
    const signerRequirements = Object.freeze(row.signerRequirements.map((requirement, signerIndex) => {
      requirePlain(requirement, `actions[${index}].signerRequirements[${signerIndex}]`);
      if (!Array.isArray(requirement.semanticRoles) || requirement.semanticRoles.length === 0 || requirement.signaturePresent !== false || requirement.keyAccess !== false) throw new Error("Dealer signer projection implies signature or key access.");
      return Object.freeze({ address: nonzeroAddress(requirement.address, `actions[${index}].signerRequirements[${signerIndex}].address`), semanticRoles: Object.freeze(requirement.semanticRoles.map((role, roleIndex) => text(role, `actions[${index}].signerRequirements[${signerIndex}].semanticRoles[${roleIndex}]`, 64))), signaturePresent: false, keyAccess: false });
    }));
    if (new Set(signerRequirements.map((requirement) => requirement.address)).size !== signerRequirements.length) throw new Error("Dealer signer projection repeats an identity.");
    const feePayer = nonzeroAddress(draft.feePayer, "Dealer draft fee payer");
    const roleSigners = new Set(accountRoles.filter((role) => role.signer).map((role) => role.address));
    if (feePayer !== accountRoles[0].address || signerRequirements.some((requirement) => !roleSigners.has(requirement.address) && requirement.address !== feePayer) || !signerRequirements.some((requirement) => requirement.address === feePayer && requirement.semanticRoles.includes("transaction-fee-payer"))) throw new Error("Dealer signer requirements differ from the exact actor/payer roles.");
    requirePlain(row.freshnessDisposition, `actions[${index}].freshnessDisposition`);
    const observedSlot = positiveDecimal(row.freshnessDisposition.observedSlot, `actions[${index}].freshnessDisposition.observedSlot`);
    const validBeforeSlot = positiveDecimal(row.freshnessDisposition.validBeforeSlot, `actions[${index}].freshnessDisposition.validBeforeSlot`);
    const maximumValiditySlots = positiveDecimal(row.freshnessDisposition.maximumValiditySlots, `actions[${index}].freshnessDisposition.maximumValiditySlots`);
    if (validBeforeSlot <= observedSlot || validBeforeSlot - observedSlot > maximumValiditySlots || addressLookupTables.some((table) => BigInt(table.observedSlot) > observedSlot) || observationSet.accounts.some((observation) => BigInt(observation.observedSlot) > observedSlot) || row.freshnessDisposition.recentBlockhash !== "absent; a launcher must reacquire state before adding one" || typeof row.freshnessDisposition.beforeSigning !== "string" || typeof row.freshnessDisposition.afterSubmission !== "string") throw new Error("Dealer payload-variant freshness boundary is invalid.");
    return Object.freeze({
      coordinate,
      payloadVariant,
      accountRoles,
      observationSet,
      callable: true,
      projectedCallable: true,
      verdict: row.verdict,
      reason: text(row.reason, `actions[${index}].reason`, 512),
      stateSelection: null,
      transactionDraft: Object.freeze({ ...draft, draftId: hash32(draft.draftId, "Dealer draft ID"), authorityStateSha256: hash32(draft.authorityStateSha256, "Dealer draft authority state"), driverAccount, driverAccountSlot, feePayer, addressLookupTables, serializedTransactionHex: transactionHex }),
      signerRequirements,
      freshnessDisposition: Object.freeze({ observedSlot: observedSlot.toString(), validBeforeSlot: validBeforeSlot.toString(), maximumValiditySlots: maximumValiditySlots.toString() })
    });
  };

  const directRolesMatch = (actual, expected) => actual.length === expected.length && actual.every((role, roleIndex) => {
    const contract = expected[roleIndex];
    return role.role === contract.role && role.signer === contract.signer && role.writable === contract.writable && role.address !== null && role.identityDisposition === "semantic-owner-derived-and-bound-to-draft";
  });

  const directRoleContract = (localAction, roles) => {
    const rootPrefix = [directRole("direct-root", false, true), directRole("direct-replay", false, true)];
    if (localAction === "2") {
      const fixed = [
        ...rootPrefix, directRole("fresh-direct-reservation", false, true), directRole("actor-payer", true, true), ...DIRECT_RESERVATION_AUTHORITY_ROLES,
        directRole("system-program", false, false), directRole("rent-sysvar", false, false), directRole("clock-sysvar", false, false),
        directRole("compiler-bundle-v6", false, false), directRole("market-genesis-v2", false, false), directRole("price-grid", false, false)
      ];
      if (directRolesMatch(roles, fixed)) return "reservation-without-peer";
      if (directRolesMatch(roles, [...fixed, directRole("root-owned-peer-reservation", false, false)])) return "reservation-with-peer";
      return null;
    }
    if (localAction === "3") {
      const fixed = [
        ...rootPrefix, directRole("direct-reservation", false, true), directRole("actor-payer", true, true), ...DIRECT_RESERVATION_AUTHORITY_ROLES,
        directRole("market-genesis-v2", false, false), directRole("neutral-lamport-sink", false, true), directRole("clock-sysvar", false, false)
      ];
      return directRolesMatch(roles, fixed) ? "reservation-cancel" : null;
    }
    if (localAction === "4") {
      const prefix = [
        ...rootPrefix, directRole("fresh-direct-selection", false, true), directRole("selection-rent-payer", true, true),
        directRole("system-program", false, false), directRole("rent-sysvar", false, false), directRole("clock-sysvar", false, false),
        directRole("compiler-bundle-v6", false, false), directRole("native-claim-basis", false, false), directRole("price-measure-policy", false, false),
        directRole("market-genesis-v2", false, false), directRole("price-grid", false, false)
      ];
      for (let reservationCount = 0; reservationCount <= 2; reservationCount += 1) {
        const expected = [...prefix, ...Array.from({ length: reservationCount }, () => directRole("root-owned-reservation", false, false)), ...DIRECT_CANDIDATE_TAIL];
        if (directRolesMatch(roles, expected)) return `freeze-${reservationCount}-reservations`;
      }
      return null;
    }
    if (localAction === "5") {
      const fixed = [
        ...rootPrefix, directRole("direct-selection", false, true), directRole("clock-sysvar", false, false),
        directRole("candidate-submitter", true, true), directRole("system-program", false, false)
      ];
      if (directRolesMatch(roles, fixed)) return "submit-without-eviction";
      if (directRolesMatch(roles, [...fixed, directRole("evicted-bond-refund-owner", false, true)])) return "submit-with-eviction";
      return null;
    }
    if (localAction === "6" || localAction === "7") {
      const expected = [...rootPrefix, directRole("direct-selection", false, true), directRole("clock-sysvar", false, false), ...DIRECT_CANDIDATE_TAIL];
      return directRolesMatch(roles, expected) ? (localAction === "6" ? "begin-verification" : "verify-candidate") : null;
    }
    if (localAction === "8") {
      const prefix = [
        directRole("direct-root-v2", false, true), directRole("direct-action-replay-v1", false, true),
        directRole("direct-selection-v1", false, true), directRole("clock-sysvar", false, false)
      ];
      for (let refundCount = 1; refundCount <= 3; refundCount += 1) {
        const expected = [...prefix, ...Array.from({ length: refundCount }, () => directRole("candidate-bond-refund-owner", false, true)),
          directRole("candidate-liveness-policy", false, false), directRole("candidate-liveness-account", false, true),
          directRole("keeper", true, true), directRole("candidate-liveness-payer", false, true)];
        if (directRolesMatch(roles, expected)) return `nonempty-${refundCount}-refunds`;
      }
      const noCandidate = [
        directRole("direct-root-v2", false, true), directRole("direct-action-replay-v1", false, true), directRole("direct-selection-v1", false, true),
        directRole("realm-v1", false, false), directRole("collateral-profile-v2", false, false), directRole("collateral-policy-v2", false, false),
        directRole("collateral-token-program", false, false), directRole("general-market-binding-v4", false, false), directRole("general-market-runtime-v3", false, false),
        directRole("market-instance-v2-artifact", false, false), directRole("market-genesis-profile-v2-artifact", false, false), directRole("clock-sysvar", false, false),
        directRole("direct-reservation-v1", false, true), directRole("general-position-v3", false, true), directRole("general-position-replay-v3", false, true),
        directRole("direct-reservation-v1", false, true), directRole("general-position-v3", false, true), directRole("general-position-replay-v3", false, true),
        directRole("candidate-liveness-policy", false, false), directRole("candidate-liveness-account", false, true),
        directRole("keeper", true, true), directRole("candidate-liveness-payer", false, true)
      ];
      return directRolesMatch(roles, noCandidate) ? "no-candidate-two-endpoints" : null;
    }
    return null;
  };

  const validateDirectMaterialContract = (row, coordinate, accountRoles, stateSelection, transactionDraft, configuration, index) => {
    const contract = DIRECT_ACTION_CONTRACTS[coordinate.localAction];
    if (!contract || coordinate.action !== contract.action || row.semanticOwnerConstructor !== (coordinate.localAction === "8" ? contract.ownerSchema : "clutch-direct-market-runtime/current-v1")) throw new Error("Direct verdict is generic, legacy, or differs from the current action2-8 constructor contract.");
    const branch = directRoleContract(coordinate.localAction, accountRoles);
    if (branch === null) throw new Error("Direct verdict differs from its exact current account-role grammar.");
    if (stateSelection.action !== contract.action || stateSelection.account !== accountRoles[0].address || transactionDraft.driverAccount !== accountRoles[0].address || transactionDraft.driverAccountSlot !== stateSelection.accountSlot) throw new Error("Direct driver, finalized selection, and exact root role do not agree.");
    if (stateSelection.dependencies.some((dependency) => !accountRoles.some((role) => role.address === dependency))) throw new Error("Direct finalized selection names a dependency outside its exact account tuple.");
    if (transactionDraft.messageVersion !== "legacy" || transactionDraft.addressLookupTables.length !== 0 || transactionDraft.flows[0] !== DIRECT_COORDINATE.flow || transactionDraft.actions[0] !== contract.action) throw new Error("Direct material is not one current legacy transaction with no lookup table.");
    const owner = transactionDraft.semanticOwners[0];
    if (owner.package !== "clutch-direct-market-runtime" || owner.schema !== contract.ownerSchema) throw new Error("Direct material differs from its exact current semantic owner.");
    const binding = transactionDraft.registryBindings[0];
    if (decimal(binding.centralAction, "Direct central action", 255n).toString() !== coordinate.localAction) throw new Error("Direct material does not retain its exact allocated central action.");
    const expectedEquationNames = {
      "2": [["reservation rent principal plus immutable prefund", "Position reserved-cash subset increases by exact order reserve"], ["reservation rent principal plus immutable prefund", "Position Egg debit equals exact Reservation escrow"]],
      "3": [["reservation principal refund plus neutral surplus", "cancel releases the exact reserved-cash subset"], ["reservation principal refund plus neutral surplus", "cancel restores exact escrowed Eggs"]],
      "4": [["selection rent principal plus immutable prefund", "candidate prepaid work conservation"]],
      "5": [["selection retained-bond principal conservation"]],
      "6": [["candidate work principal conservation"]],
      "7": [["candidate work principal conservation"]],
      "8": branch.startsWith("nonempty-")
        ? [["selection-bond-principal-refunded-exactly", "candidate-work-capital-disposition-exactly"]]
        : [["no-candidate-selection-bond-principal-is-zero", "candidate-work-capital-disposition-exactly"]]
    }[coordinate.localAction];
    const equationNames = transactionDraft.exactEquations.map((equation) => equation.name);
    if (!expectedEquationNames.some((expected) => expected.length === equationNames.length && expected.every((name, equationIndex) => name === equationNames[equationIndex]))) throw new Error("Direct material differs from its exact current conservation equations.");
    let symbolicPostcondition = null;
    if (coordinate.localAction !== "8") {
      if (row.symbolicPostcondition !== null) throw new Error("Direct actions2-7 unexpectedly carry an action8 execution-Clock postcondition.");
    } else {
      requirePlain(row.symbolicPostcondition, `actions[${index}].symbolicPostcondition`);
      if (row.symbolicPostcondition.executionClock !== "hostile SBF Clock slot; absent from the unsigned payload"
          || row.symbolicPostcondition.lamports !== "exact signed deltas from actual execution prebalances"
          || row.symbolicPostcondition.confirmation !== "realize exact semantic successors from the confirmed execution slot; compare deltas to the actual execution pre/post witness"
          || !Array.isArray(row.symbolicPostcondition.writableAccounts)) throw new Error("Direct action8 lacks its exact symbolic execution-Clock postcondition.");
      const writableAccounts = Object.freeze(row.symbolicPostcondition.writableAccounts.map((value, writableIndex) => nonzeroAddress(value, `actions[${index}].symbolicPostcondition.writableAccounts[${writableIndex}]`)));
      if (new Set(writableAccounts).size !== writableAccounts.length) throw new Error("Direct action8 symbolic postcondition repeats a writable identity.");
      for (let writableIndex = 1; writableIndex < writableAccounts.length; writableIndex += 1) {
        if (compareAddressBytes(writableAccounts[writableIndex - 1], writableAccounts[writableIndex]) >= 0) throw new Error("Direct action8 symbolic writable identities are not canonical.");
      }
      const roleWritable = [...new Set(accountRoles.filter((role) => role.writable).map((role) => role.address))].sort(compareAddressBytes);
      if (roleWritable.length !== writableAccounts.length || roleWritable.some((addressValue, writableIndex) => addressValue !== writableAccounts[writableIndex])) throw new Error("Direct action8 symbolic postcondition differs from its exact writable role set.");
      symbolicPostcondition = Object.freeze({ contractId: hash32(row.symbolicPostcondition.contractId, `actions[${index}].symbolicPostcondition.contractId`), executionClock: row.symbolicPostcondition.executionClock, lamports: row.symbolicPostcondition.lamports, writableAccounts, confirmation: row.symbolicPostcondition.confirmation });
    }
    return Object.freeze({ schema: "dragons-clutch/browser/direct-chain-material-contract/v2", branch, symbolicPostcondition });
  };

  const validateFractionalMaterialContract = (row, coordinate, accountRoles, stateSelection, transactionDraft, signerRequirements, configuration) => {
    const contract = FRACTIONAL_ACTION_CONTRACTS[coordinate.localAction];
    const common = [
      ["realm", false, false], ["profile", false, false], ["collateral-policy", false, false], ["collateral-token-program", false, false],
      ["market-binding-v2", false, false], ["market-runtime-v3", false, false], ["market-instance-preimage-v2", false, false], ["hoard-v2", false, false],
      ["claim-ledger-v3", false, true], ["resolution-v5", false, false], ["fractional-policy-v3", false, false], ["fractional-ledger-v1", false, true]
    ];
    let expected;
    let geometry = "fixed";
    let holderChoice = null;
    let derivedFacts = null;
    let payerIndex = null;
    if (coordinate.localAction === "1" || coordinate.localAction === "10") {
      const outcomeCount = accountRoles.length - 32;
      if (outcomeCount < 2 || outcomeCount > 16) throw new Error("Fractional lifecycle material does not have the exact 32+N reduced Product-foundation geometry.");
      const core = Array.from({ length: 15 }, (_, index) => ["foundation-core", false, index === 0 || index === 4 || index === 11 || index === 12]);
      const outcomes = Array.from({ length: outcomeCount }, () => ["foundation-outcome-mint", false, false]);
      const auxiliary = Array.from({ length: 17 }, (_, index) => ["lifecycle-authority", false, coordinate.localAction === "10" && (index === 15 || index === 16)]);
      expected = [...core, ...outcomes, ...auxiliary];
      geometry = `${coordinate.localAction === "1" ? "initialize" : "close-empty-ledger"}-reduced-foundation-32+N(${outcomeCount})-${accountRoles.length}-roles`;
      requirePlain(transactionDraft.exactEquations[0].unit, "Fractional lifecycle outcome-width unit");
      const equationOutcome = decimal(transactionDraft.exactEquations[0].unit.outcome, "Fractional lifecycle equation outcome", 15n);
      const equationWidth = decimal(transactionDraft.exactEquations[0].left, "Fractional lifecycle outcome width", 16n);
      if (transactionDraft.exactEquations[0].unit.kind !== "egg-atoms" || equationOutcome !== 0n || equationWidth !== BigInt(outcomeCount)) throw new Error("Fractional lifecycle material does not bind its chain-derived Product outcome width.");
      if (stateSelection.cursor.lane !== FRACTIONAL_COORDINATE.flow || stateSelection.cursor.phase !== coordinate.localAction || (coordinate.localAction === "1" ? stateSelection.cursor.item !== "0" : BigInt(stateSelection.cursor.item) === 0n)) throw new Error("Fractional lifecycle selection does not carry its exact scheduler lane, phase, and sequence.");
      const dependencies = [...new Set(accountRoles.map((role) => role.address).filter((identity) => identity !== accountRoles[contract.driverRole].address))].sort(compareAddressBytes);
      if (dependencies.length !== stateSelection.dependencies.length || dependencies.some((identity, index) => identity !== stateSelection.dependencies[index])) throw new Error("Fractional lifecycle restart cursor differs from the complete reduced-frame dependency index.");
      derivedFacts = Object.freeze({
        kind: "fractional-lifecycle",
        outcomeCount: String(outcomeCount),
        driver: coordinate.localAction === "1" ? "market-lifecycle-root-v3" : "fractional-ledger-v1",
        sequence: stateSelection.cursor.item,
        authority: "reduced-finalized-frame-derived-complete-product-foundation-graph-and-release-join"
      });
    } else if (coordinate.localAction === "2") {
      expected = [["claimant", true, false], ...common, ["position-v3", false, true], ["general-replay-v3", false, true]];
    } else if (coordinate.localAction === "9") {
      expected = common;
    } else if (coordinate.localAction === "8") {
      const payerAlias = accountRoles.length === 18 && accountRoles[0].address === accountRoles[14].address;
      expected = [
        ["claimant", true, payerAlias], ...common, ["fractional-credit-v2", false, true],
        ["credit-rent-payer", payerAlias, true], ["market-lifecycle-root-v2", false, false], ["neutral-sink", false, true], ["rent-sysvar", false, false]
      ];
    } else if (coordinate.localAction === "3" || coordinate.localAction === "5") {
      const creditIndex = coordinate.localAction === "5" ? accountRoles.findIndex((role, roleIndex) => roleIndex >= 23 && role.role === "fractional-credit-v2") : -1;
      const outcomeCount = coordinate.localAction === "3" ? accountRoles.length - 21 : creditIndex - 21;
      const creation = coordinate.localAction === "5" && creditIndex >= 0 && accountRoles.length === creditIndex + 6;
      if (outcomeCount < 2 || outcomeCount > 16 || (coordinate.localAction === "3" && accountRoles.length !== 21 + outcomeCount) || (coordinate.localAction === "5" && (creditIndex < 23 || (!creation && accountRoles.length !== creditIndex + 4)))) throw new Error("Fractional bearer material has invalid dynamic outcome/credit geometry.");
      payerIndex = creation ? creditIndex + 4 : null;
      const payerAlias = payerIndex !== null && accountRoles[0].address === accountRoles[payerIndex].address;
      const selectedOutcomeRoles = accountRoles.slice(21, 21 + outcomeCount).map((role, outcome) => role.role === "outcome-mint" && role.writable ? outcome : null).filter((outcome) => outcome !== null);
      if (selectedOutcomeRoles.length !== 1) throw new Error("Fractional bearer material does not select exactly one writable outcome mint.");
      const selectedOutcome = selectedOutcomeRoles[0];
      const prefix = [
        ["claimant", true, payerAlias], ["realm", false, false], ["profile", false, false], ["collateral-policy", false, false], ["collateral-token-program", false, false],
        ["market-binding-v2", false, false], ["market-runtime-v3", false, false], ["market-instance-preimage-v2", false, false], ["hoard-v2", false, true],
        ["claim-ledger-v3", false, true], ["resolution-v5", false, false], ["fractional-policy-v3", false, false], ["fractional-ledger-v1", false, true],
        ["collateral-mint", false, false], ["collateral-destination", false, true], ["hoard-authority", false, false], ["hoard-token", false, true],
        ["outcome-token-program", false, false], ["outcome-token-programdata", false, false], ["bearer-source", false, true], ["collateral-token-programdata", false, false]
      ];
      const outcomes = Array.from({ length: outcomeCount }, (_, outcome) => ["outcome-mint", false, outcome === selectedOutcome]);
      const suffix = coordinate.localAction === "5" ? [
        ["fractional-credit-v2", false, true], ["market-lifecycle-root-v2", false, false], ["neutral-lamport-sink", false, false], ["rent-sysvar", false, false],
        ...(creation ? [["credit-funding-payer", true, true], ["system-program", false, false]] : [])
      ] : [];
      expected = [...prefix, ...outcomes, ...suffix];
      geometry = coordinate.localAction === "3" ? `bearer-exact-21+${outcomeCount}` : creation ? `bearer-credit-27+${outcomeCount}` : `bearer-credit-25+${outcomeCount}`;
      requirePlain(transactionDraft.exactEquations[0].unit, "Fractional holder Eggs unit");
      const equationOutcome = decimal(transactionDraft.exactEquations[0].unit.outcome, "Fractional holder outcome", 15n).toString();
      const quantity = positiveDecimal(transactionDraft.exactEquations[0].left, "Fractional holder quantity").toString();
      if (transactionDraft.exactEquations[0].unit.kind !== "egg-atoms" || equationOutcome !== String(selectedOutcome)) throw new Error("Fractional holder outcome differs from its selected chain-derived mint and exact burn equation.");
      holderChoice = Object.freeze({
        schema: "dragons-clutch/browser/fractional-holder-intent/v1",
        kind: coordinate.localAction === "3" ? "redeem-bearer-exact" : "redeem-bearer-credit",
        claimant: accountRoles[0].address,
        bearerSource: accountRoles[19].address,
        outcome: String(selectedOutcome),
        quantity,
        collateralDestination: accountRoles[14].address,
        fundingPayer: payerIndex === null ? null : accountRoles[payerIndex].address,
        disposition: "holder-approved-choices-reauthenticated-against-finalized-chain-derived-material"
      });
    } else if (coordinate.localAction === "6" || coordinate.localAction === "7") {
      const internal = accountRoles.length >= 21 && accountRoles[16].role === "payout-and-lifecycle-role" && accountRoles[16].writable && accountRoles[17].writable;
      const external = accountRoles.length >= 24 && accountRoles[16].role === "payout-and-lifecycle-role" && !accountRoles[16].writable && accountRoles[17].writable && accountRoles[19].writable;
      const fixedCount = internal ? 21 : external ? 24 : 0;
      const creation = fixedCount !== 0 && accountRoles.length === fixedCount + 2;
      if (fixedCount === 0 || (!creation && accountRoles.length !== fixedCount)) throw new Error("Fractional credit movement has invalid internal/external payout geometry.");
      payerIndex = creation ? fixedCount : null;
      const payerAliasSource = payerIndex !== null && accountRoles[0].address === accountRoles[payerIndex].address;
      const payerAliasDestination = payerIndex !== null && accountRoles[1].address === accountRoles[payerIndex].address;
      const base = [
        ["source-claimant", true, payerAliasSource], ["destination-claimant", true, payerAliasDestination], ["realm", false, false], ["profile", false, false],
        ["collateral-policy", false, false], ["collateral-token-program", false, false], ["market-binding-v2", false, false], ["market-runtime-v3", false, false],
        ["market-instance-preimage-v2", false, false], ["hoard-v2", false, true], ["claim-ledger-v3", false, true], ["resolution-v5", false, false],
        ["fractional-policy-v3", false, false], ["fractional-ledger-v1", false, true], ["source-credit-v2", false, true], ["destination-credit-v2", false, true]
      ];
      const payoutRoles = Array.from({ length: internal ? 5 : 8 }, (_, payoutIndex) => {
        const absolute = 16 + payoutIndex;
        return ["payout-and-lifecycle-role", false, internal ? absolute === 16 || absolute === 17 : absolute === 17 || absolute === 19];
      });
      expected = [...base, ...payoutRoles, ...(creation ? [["credit-funding-payer", true, true], ["system-program", false, false]] : [])];
      geometry = `${coordinate.localAction === "6" ? "transfer" : "merge"}-${internal ? "internal" : "external"}-${creation ? "credit-create-or-reopen" : "live-credit"}-${accountRoles.length}-roles`;
      requirePlain(transactionDraft.exactEquations[0].unit, "Fractional moved numerator unit");
      if (transactionDraft.exactEquations[0].unit.kind !== "price-units") throw new Error("Fractional credit movement is not expressed in exact scaled numerator units.");
      const moved = positiveDecimal(transactionDraft.exactEquations[0].left, "Fractional moved numerator").toString();
      holderChoice = Object.freeze({
        schema: "dragons-clutch/browser/fractional-holder-intent/v1",
        kind: coordinate.localAction === "6" ? "transfer-credit" : "merge-credit",
        sourceClaimant: accountRoles[0].address,
        sourceCredit: accountRoles[14].address,
        destinationClaimant: accountRoles[1].address,
        numerator: coordinate.localAction === "6" ? moved : null,
        payout: Object.freeze({ kind: internal ? "internal-position" : "external-collateral", target: accountRoles[internal ? 16 : 17].address }),
        fundingPayer: payerIndex === null ? null : accountRoles[payerIndex].address,
        disposition: "holder-approved-choices-reauthenticated-against-finalized-chain-derived-material"
      });
    } else if (coordinate.localAction === "4") {
      const creation = accountRoles.length === 21;
      if (!creation && accountRoles.length !== 19) throw new Error("Fractional internal-credit material is not one exact live/fresh/reopen role shape.");
      payerIndex = creation ? 19 : null;
      const payerAlias = payerIndex !== null && accountRoles[0].address === accountRoles[payerIndex].address;
      expected = [
        ["claimant", true, payerAlias], ["realm", false, false], ["profile", false, false], ["collateral-policy", false, false],
        ["collateral-token-program", false, false], ["market-binding-v2", false, false], ["market-runtime-v3", false, false], ["market-instance-preimage-v2", false, false],
        ["hoard-v2", false, true], ["claim-ledger-v3", false, true], ["resolution-v5", false, false], ["fractional-policy-v3", false, false],
        ["fractional-ledger-v1", false, true], ["position-v3", false, true], ["general-replay-v3", false, true], ["fractional-credit-v2", false, true],
        ["market-lifecycle-root-v2", false, false], ["neutral-lamport-sink", false, false], ["rent-sysvar", false, false],
        ...(creation ? [["credit-funding-payer", true, true], ["system-program", false, false]] : [])
      ];
      geometry = creation ? "internal-credit-fresh-or-reopen-21-roles" : "internal-credit-live-19-roles";
      requirePlain(transactionDraft.exactEquations[0].unit, "Fractional internal Eggs unit");
      const outcome = decimal(transactionDraft.exactEquations[0].unit.outcome, "Fractional internal outcome", 15n).toString();
      const quantity = positiveDecimal(transactionDraft.exactEquations[0].left, "Fractional internal quantity").toString();
      requirePlain(transactionDraft.exactEquations[1].unit, "Fractional internal payout unit");
      requirePlain(transactionDraft.exactEquations[2].unit, "Fractional internal retained-numerator unit");
      if (transactionDraft.exactEquations[0].unit.kind !== "egg-atoms" || transactionDraft.exactEquations[1].unit.kind !== "collateral-atoms" || transactionDraft.exactEquations[2].unit.kind !== "price-units") throw new Error("Fractional internal holder choice and derived payout facts use the wrong exact integer units.");
      holderChoice = Object.freeze({
        schema: "dragons-clutch/browser/fractional-holder-intent/v1",
        kind: "redeem-internal-credit",
        claimant: accountRoles[0].address,
        position: accountRoles[13].address,
        outcome,
        quantity,
        fundingPayer: payerIndex === null ? null : accountRoles[payerIndex].address,
        disposition: "holder-approved-choices-reauthenticated-against-finalized-chain-derived-material"
      });
      derivedFacts = Object.freeze({
        credit: accountRoles[15].address,
        creditAdmission: creation ? "fresh-or-reopened-chain-state" : "live-chain-state",
        wholeCollateralPayout: decimal(transactionDraft.exactEquations[1].left, "Fractional internal collateral payout").toString(),
        retainedPayoutNumerator: decimal(transactionDraft.exactEquations[2].left, "Fractional internal retained numerator").toString()
      });
    } else {
      throw new Error("Fractional action lacks a landed exact browser material contract.");
    }
    const exactRoles = expected.map(([role, signer, writable]) => directRole(role, signer, writable));
    if (!contract || coordinate.action !== contract.action || row.semanticOwnerConstructor !== `clutch-fractional-redemption-runtime/${contract.ownerSchema}` || !directRolesMatch(accountRoles, exactRoles)) throw new Error("Fractional verdict differs from its exact landed chain-derived material contract.");
    const loaderAlias = (left, right) => (left === 4 && right === 17) || (left === 18 && right === 20);
    if (coordinate.localAction === "1" || coordinate.localAction === "10") {
      const lifecycleAux = accountRoles.length - 17;
      if ((accountRoles[lifecycleAux + 3].address === accountRoles[lifecycleAux + 5].address) !== (accountRoles[lifecycleAux + 4].address === accountRoles[lifecycleAux + 6].address)) throw new Error("Fractional lifecycle collateral/claim loader aliases are not correlated.");
    }
    if ((coordinate.localAction === "3" || coordinate.localAction === "5") && ((accountRoles[4].address === accountRoles[17].address) !== (accountRoles[18].address === accountRoles[20].address))) throw new Error("Fractional bearer loader program/programdata aliases are not correlated.");
    for (let left = 0; left < accountRoles.length; left += 1) {
      for (let right = left + 1; right < accountRoles.length; right += 1) {
        const lifecycleAux = accountRoles.length - 17;
        const permitted = ((coordinate.localAction === "1" || coordinate.localAction === "10") && ((left === lifecycleAux + 3 && right === lifecycleAux + 5) || (left === lifecycleAux + 4 && right === lifecycleAux + 6)))
          || (coordinate.localAction === "8" && left === 0 && right === 14)
          || ((coordinate.localAction === "3" || coordinate.localAction === "5") && loaderAlias(left, right))
          || (coordinate.localAction === "5" && payerIndex !== null && left === 0 && right === payerIndex)
          || ((coordinate.localAction === "6" || coordinate.localAction === "7") && payerIndex !== null && (left === 0 || left === 1) && right === payerIndex)
          || (coordinate.localAction === "4" && payerIndex !== null && left === 0 && right === payerIndex);
        if (accountRoles[left].address === accountRoles[right].address && !permitted) throw new Error("Fractional exact tuple contains an unowned account alias.");
      }
    }
    const driver = accountRoles[contract.driverRole];
    if (stateSelection.action !== contract.action || stateSelection.account !== driver.address || transactionDraft.driverAccount !== driver.address || transactionDraft.driverAccountSlot !== stateSelection.accountSlot || stateSelection.dependencies.some((dependency) => !accountRoles.some((role) => role.address === dependency))) throw new Error("Fractional finalized selection, exact driver role, and dependency tuple do not agree.");
    if (transactionDraft.messageVersion !== "legacy" || transactionDraft.addressLookupTables.length !== 0 || transactionDraft.flows[0] !== FRACTIONAL_COORDINATE.flow || transactionDraft.actions[0] !== contract.action) throw new Error("Fractional material is not one current legacy transaction with no lookup table.");
    const owner = transactionDraft.semanticOwners[0];
    if (owner.package !== "clutch-fractional-redemption-runtime" || owner.schema !== contract.ownerSchema || owner.releaseSha256 !== configuration.release.elfSha256) throw new Error("Fractional material differs from its checked current semantic owner release.");
    const binding = transactionDraft.registryBindings[0];
    if (decimal(binding.centralAction, "Fractional central action", 255n).toString() !== coordinate.localAction) throw new Error("Fractional material does not retain its exact allocated central action.");
    const equationNames = transactionDraft.exactEquations.map((equation) => equation.name);
    if (equationNames.length !== contract.equationNames.length || equationNames.some((name, equationIndex) => name !== contract.equationNames[equationIndex])) throw new Error("Fractional material differs from its exact chain-derived conservation equations.");
    const expectedSignerRoles = new Map();
    for (const role of accountRoles.filter((value) => value.signer)) {
      const labels = expectedSignerRoles.get(role.address) || [];
      labels.push(role.role);
      expectedSignerRoles.set(role.address, labels);
    }
    const feeLabels = expectedSignerRoles.get(transactionDraft.feePayer) || [];
    feeLabels.push("transaction-fee-payer");
    expectedSignerRoles.set(transactionDraft.feePayer, feeLabels);
    if (signerRequirements.length !== expectedSignerRoles.size || signerRequirements.some((requirement) => {
      const labels = expectedSignerRoles.get(requirement.address);
      return labels === undefined || labels.length !== requirement.semanticRoles.length || labels.some((label, labelIndex) => label !== requirement.semanticRoles[labelIndex]);
    })) throw new Error("Fractional signer projection differs from claimant and explicit transaction-fee-payer facts.");
    if (row.symbolicPostcondition !== null) throw new Error("Fractional current material unexpectedly carries a Direct execution-Clock contract.");
    return Object.freeze({
      schema: "dragons-clutch/browser/fractional-chain-material-contract/v1",
      provenance: "all-account-identities-metas-payload-and-amounts-derived-from-finalized-chain-state",
      geometry,
      holderChoice,
      derivedFacts,
      holderChoiceDisposition: holderChoice === null ? "none-chain-derived-lifecycle-or-permissionless-maintenance-action" : "irreducible-holder-choices-only-no-account-meta-authority"
    });
  };

  const validateActionCapabilities = (raw, configuration, session) => {
    requirePlain(raw, "action capabilities");
    if (raw.schema !== "dragons-clutch/operator-action-capability-set/v1"
        || raw.status !== "ready"
        || raw.commitment !== "finalized"
        || raw.projectionAuthority !== "untrusted-release-and-canonical-codec-projection"
        || raw.signing !== false
        || raw.submission !== false
        || hash32(raw.sessionId, "actions.sessionId") !== session.sessionId
        || text(raw.releaseKey, "actions.releaseKey", 320) !== configuration.release.releaseKey
        || hash32(raw.capabilityProfileId, "actions.capabilityProfileId") !== configuration.release.capabilityProfileId) {
      throw new Error("action capabilities do not bind the finalized canonical session and checked release.");
    }
    requirePlain(raw.freshness, "actions.freshness");
    if (raw.freshness.recentBlockhash !== "absent-by-contract"
        || raw.freshness.feePayer !== "must-be-explicit-in-server-constructed-draft"
        || raw.freshness.validBeforeSlot !== "must-be-derived-from-a-fresh-clock-observation"
        || typeof raw.freshness.beforeSigning !== "string"
        || typeof raw.freshness.afterSubmission !== "string") {
      throw new Error("action capability freshness/reacquisition contract is incomplete.");
    }
    if (!Array.isArray(raw.actions) || raw.actions.length > 256 || !Array.isArray(configuration.release.enabledIntentVariants)) throw new Error("action capability or payload-variant set exceeds the browser bound.");
    const enabled = new Set(configuration.release.enabledIntents.map((intent) => `${intent.familyTag}:${intent.familyVersion}:${intent.localAction}`));
    const enabledVariants = new Set(configuration.release.enabledIntentVariants.map((variant) => `${variant.familyTag}:${variant.familyVersion}:${variant.localAction}:${variant.payloadDiscriminator}:${variant.name}`));
    const seen = new Set();
    const seenVariants = new Set();
    const actions = raw.actions.map((row, index) => {
      requirePlain(row, `actions[${index}]`);
      requirePlain(row.coordinate, `actions[${index}].coordinate`);
      requirePlain(row.releaseAdmission, `actions[${index}].releaseAdmission`);
      const coordinate = Object.freeze({
        familyTag: positiveDecimal(row.coordinate.familyTag, `actions[${index}].coordinate.familyTag`, 255n).toString(),
        familyVersion: positiveDecimal(row.coordinate.familyVersion, `actions[${index}].coordinate.familyVersion`, 255n).toString(),
        localAction: positiveDecimal(row.coordinate.localAction, `actions[${index}].coordinate.localAction`, 255n).toString(),
        family: text(row.coordinate.family, `actions[${index}].coordinate.family`, 32),
        action: text(row.coordinate.action, `actions[${index}].coordinate.action`, 96)
      });
      const key = `${coordinate.familyTag}:${coordinate.familyVersion}:${coordinate.localAction}`;
      let payloadVariant = null;
      if (row.payloadVariant !== undefined) {
        requirePlain(row.payloadVariant, `actions[${index}].payloadVariant`);
        payloadVariant = validateIntentVariants([{ familyTag: coordinate.familyTag, familyVersion: coordinate.familyVersion, localAction: coordinate.localAction, payloadDiscriminator: row.payloadVariant.discriminator, name: row.payloadVariant.name }], `actions[${index}].payloadVariant`, configuration.release.enabledIntents)[0];
      }
      if (payloadVariant === null) {
        if (!enabled.has(key) || seen.has(key)) throw new Error("action verdict is absent from, or duplicated within, the checked release enabled-intent set.");
        seen.add(key);
      } else {
        const variantKey = `${key}:${payloadVariant.payloadDiscriminator}:${payloadVariant.name}`;
        if (!enabledVariants.has(variantKey) || seenVariants.has(variantKey) || enabled.has(key)) throw new Error("Dealer payload-variant verdict is absent from its exact enabled variant set or promotes the coarse coordinate.");
        seenVariants.add(variantKey);
      }
      if (!Array.isArray(row.accountRoles) || row.accountRoles.length > 128 || !Array.isArray(row.signerRequirements)) throw new Error("action role/signer projection is invalid.");
      const accountRoles = Object.freeze(row.accountRoles.map((role, roleIndex) => {
        requirePlain(role, `actions[${index}].accountRoles[${roleIndex}]`);
        if (decimal(role.index, `actions[${index}].accountRoles[${roleIndex}].index`).toString() !== String(roleIndex)) throw new Error("action account roles are not in exact semantic-owner order.");
        return Object.freeze({
          index: String(roleIndex),
          role: text(role.role, `actions[${index}].accountRoles[${roleIndex}].role`, 64),
          writable: bool(role.writable, `actions[${index}].accountRoles[${roleIndex}].writable`),
          signer: bool(role.signer, `actions[${index}].accountRoles[${roleIndex}].signer`),
          address: role.address === null ? null : nonzeroAddress(role.address, `actions[${index}].accountRoles[${roleIndex}].address`),
          identityDisposition: text(role.identityDisposition, `actions[${index}].accountRoles[${roleIndex}].identityDisposition`, 160)
        });
      }));
      if (payloadVariant !== null) return validateDealerVariantVerdict(row, coordinate, payloadVariant, accountRoles, configuration, index);
      const callable = bool(row.callable, `actions[${index}].callable`);
      const releaseAdmission = validateActionReleaseAdmission(row.releaseAdmission, coordinate, callable, configuration, index);
      if (!callable && (row.verdict !== "unavailable" || row.transactionDraft !== null || row.signerRequirements.length !== 0)) throw new Error("unavailable action carries executable-looking transaction or signer material.");
      if (!callable) {
        const stateSelection = row.stateSelection === null ? null : validateFinalizedStateSelection(row.stateSelection, session, index);
        if (typeof row.freshnessDisposition !== "string") throw new Error("unavailable action must carry an explicit non-draft freshness disposition.");
        return Object.freeze({ coordinate, releaseAdmission, accountRoles, callable: false, verdict: row.verdict, reason: text(row.reason, `actions[${index}].reason`, 512), stateSelection, transactionDraft: null, signerRequirements: Object.freeze([]), freshnessDisposition: null });
      }
      if (row.verdict !== "callable-unsigned-draft" || row.stateSelection === null) throw new Error("callable action lacks an exact finalized state selection.");
      const stateSelection = validateFinalizedStateSelection(row.stateSelection, session, index);
      if (accountRoles.length === 0 || accountRoles.some((role) => role.address === null || role.identityDisposition !== "semantic-owner-derived-and-bound-to-draft")) throw new Error("callable action has unresolved account-role identity.");
      const signerRequirements = Object.freeze(row.signerRequirements.map((requirement, signerIndex) => {
        requirePlain(requirement, `actions[${index}].signerRequirements[${signerIndex}]`);
        if (!Array.isArray(requirement.semanticRoles) || requirement.semanticRoles.length === 0 || requirement.signaturePresent !== false || requirement.keyAccess !== false) throw new Error("callable signer requirement implies signature or key access.");
        return Object.freeze({
          address: nonzeroAddress(requirement.address, `actions[${index}].signerRequirements[${signerIndex}].address`),
          semanticRoles: Object.freeze(requirement.semanticRoles.map((role, roleIndex) => text(role, `actions[${index}].signerRequirements[${signerIndex}].semanticRoles[${roleIndex}]`, 64))),
          signaturePresent: false,
          keyAccess: false
        });
      }));
      if (new Set(signerRequirements.map((requirement) => requirement.address)).size !== signerRequirements.length) throw new Error("callable signer requirements repeat an identity.");
      const roleSigners = new Set(accountRoles.filter((role) => role.signer).map((role) => role.address));
      const fractional = coordinate.familyTag === FRACTIONAL_COORDINATE.familyTag && coordinate.familyVersion === FRACTIONAL_COORDINATE.familyVersion && coordinate.family === FRACTIONAL_COORDINATE.family && FRACTIONAL_ACTION_CONTRACTS[coordinate.localAction] !== undefined;
      const projectedFeePayer = nonzeroAddress(row.transactionDraft.feePayer, "projected draft fee payer");
      if (signerRequirements.some((requirement) => !roleSigners.has(requirement.address) && !(fractional && requirement.address === projectedFeePayer)) || [...roleSigners].some((signer) => !signerRequirements.some((requirement) => requirement.address === signer))) throw new Error("callable signer requirements differ from exact signer roles and any explicitly separate Fractional fee payer.");
      const transactionDraft = validateCanonicalTransactionDraft(row.transactionDraft, configuration, releaseAdmission, coordinate, stateSelection, accountRoles, signerRequirements, index);
      const directContract = coordinate.familyTag === DIRECT_COORDINATE.familyTag && coordinate.familyVersion === DIRECT_COORDINATE.familyVersion && coordinate.family === DIRECT_COORDINATE.family
        ? validateDirectMaterialContract(row, coordinate, accountRoles, stateSelection, transactionDraft, configuration, index)
        : null;
      const fractionalContract = fractional ? validateFractionalMaterialContract(row, coordinate, accountRoles, stateSelection, transactionDraft, signerRequirements, configuration) : null;
      requirePlain(row.freshnessDisposition, `actions[${index}].freshnessDisposition`);
      const observedSlot = positiveDecimal(row.freshnessDisposition.observedSlot, `actions[${index}].freshnessDisposition.observedSlot`);
      const validBeforeSlot = positiveDecimal(row.freshnessDisposition.validBeforeSlot, `actions[${index}].freshnessDisposition.validBeforeSlot`);
      const maximumValiditySlots = positiveDecimal(row.freshnessDisposition.maximumValiditySlots, `actions[${index}].freshnessDisposition.maximumValiditySlots`);
      if (validBeforeSlot <= observedSlot || validBeforeSlot - observedSlot > maximumValiditySlots || observedSlot < BigInt(stateSelection.accountSlot) || row.freshnessDisposition.recentBlockhash !== "absent; a launcher must reacquire state before adding one" || typeof row.freshnessDisposition.beforeSigning !== "string" || typeof row.freshnessDisposition.afterSubmission !== "string") throw new Error("callable action freshness boundary is invalid.");
      if (transactionDraft.addressLookupTables.some((lookup) => BigInt(lookup.observedSlot) > observedSlot)) throw new Error("callable action uses a lookup-table observation newer than its freshness observation.");
      return Object.freeze({ coordinate, releaseAdmission, accountRoles, callable: true, verdict: row.verdict, reason: text(row.reason, `actions[${index}].reason`, 512), stateSelection, transactionDraft, signerRequirements, freshnessDisposition: Object.freeze({ observedSlot: observedSlot.toString(), validBeforeSlot: validBeforeSlot.toString(), maximumValiditySlots: maximumValiditySlots.toString() }), directContract, fractionalContract });
    });
    if (seen.size !== enabled.size || seenVariants.size !== enabledVariants.size) throw new Error("operatord omitted a checked release-enabled coordinate or payload variant from its action verdict set.");
    return Object.freeze({ schema: raw.schema, sessionId: session.sessionId, actions: Object.freeze(actions), freshness: Object.freeze({ ...raw.freshness }) });
  };

  const validateFinalizedStateSelection = (raw, session, index) => {
    requirePlain(raw, `actions[${index}].stateSelection`);
    requirePlain(raw.cursor, `actions[${index}].stateSelection.cursor`);
    if (!Array.isArray(raw.dependencies) || raw.dependencies.length > 128 || raw.releaseKey !== session.release.releaseKey || raw.observedCommitment !== "finalized" || raw.effectiveCommitment !== "finalized") throw new Error("callable state selection is not finalized and release-bound.");
    const value = Object.freeze({
      account: nonzeroAddress(raw.account, `actions[${index}].stateSelection.account`),
      releaseKey: raw.releaseKey,
      action: text(raw.action, `actions[${index}].stateSelection.action`, 80),
      accountSlot: decimal(raw.accountSlot, `actions[${index}].stateSelection.accountSlot`).toString(),
      observedCommitment: "finalized",
      effectiveCommitment: "finalized",
      branch: validateBranch(raw.branch, `actions[${index}].stateSelection.branch`),
      dependencies: Object.freeze(raw.dependencies.map((dependency, dependencyIndex) => nonzeroAddress(dependency, `actions[${index}].stateSelection.dependencies[${dependencyIndex}]`))),
      cursor: Object.freeze({
        workflowId: hash32(raw.cursor.workflowId, `actions[${index}].stateSelection.cursor.workflowId`),
        lane: text(raw.cursor.lane, `actions[${index}].stateSelection.cursor.lane`, 48),
        generation: positiveDecimal(raw.cursor.generation, `actions[${index}].stateSelection.cursor.generation`).toString(),
        phase: decimal(raw.cursor.phase, `actions[${index}].stateSelection.cursor.phase`, 65535n).toString(),
        item: decimal(raw.cursor.item, `actions[${index}].stateSelection.cursor.item`).toString(),
        observedStateSha256: hash32(raw.cursor.observedStateSha256, `actions[${index}].stateSelection.cursor.observedStateSha256`)
      })
    });
    if (value.branch.kind !== "finalized-scan" || new Set([value.account, ...value.dependencies]).size !== value.dependencies.length + 1) throw new Error("action state selection is forked or repeats a named account identity.");
    const restart = session.restart.cursors.find((candidate) => candidate.account === value.account && candidate.action === value.action && candidate.cursor.workflowId === value.cursor.workflowId && candidate.cursor.lane === value.cursor.lane && candidate.cursor.generation === value.cursor.generation && candidate.cursor.phase === value.cursor.phase && candidate.cursor.item === value.cursor.item && candidate.cursor.observedStateSha256 === value.cursor.observedStateSha256);
    if (!restart || restart.dependencies.length !== value.dependencies.length || restart.dependencies.some((dependency, dependencyIndex) => dependency !== value.dependencies[dependencyIndex])) throw new Error("callable state selection differs from the attached onchain-derived restart cursor.");
    return value;
  };

  const validateCanonicalTransactionDraft = (raw, configuration, releaseAdmission, coordinate, selection, roles, signers, index) => {
    requirePlain(raw, `actions[${index}].transactionDraft`);
    if (raw.schema !== "dragons-clutch/operator-canonical-action-material/v1" || raw.constructionSchema !== "dragons-clutch/operator/unsigned-protocol-transaction/v3" || hash32(raw.releaseManifestSha256, "draft release manifest") !== releaseAdmission.executionReleaseManifestSha256 || text(raw.executionReleaseKey, "draft execution release key", 320) !== releaseAdmission.executionReleaseKey || hash32(raw.capabilityProfileId, "draft capability profile") !== configuration.release.capabilityProfileId || nonzeroAddress(raw.driverAccount, "draft driver account") !== selection.account || decimal(raw.driverAccountSlot, "draft driver account slot").toString() !== selection.accountSlot || text(raw.driverReleaseKey, "draft driver release key", 320) !== releaseAdmission.driverReleaseKey || releaseAdmission.driverReleaseKey !== selection.releaseKey || hash32(raw.authorityStateSha256, "draft authority state") !== selection.cursor.observedStateSha256 || raw.recentBlockhash !== null || raw.hasRecentBlockhash !== false || raw.signed !== false || raw.submitted !== false || raw.reloadAuthoritativeAccounts !== true) throw new Error("callable transaction draft violates its composite release/construction boundary.");
    const feePayer = nonzeroAddress(raw.feePayer, "draft fee payer");
    const fractional = coordinate.familyTag === FRACTIONAL_COORDINATE.familyTag && coordinate.familyVersion === FRACTIONAL_COORDINATE.familyVersion && coordinate.family === FRACTIONAL_COORDINATE.family && FRACTIONAL_ACTION_CONTRACTS[coordinate.localAction] !== undefined;
    if (!signers.some((requirement) => requirement.address === feePayer && requirement.semanticRoles.includes("transaction-fee-payer")) || (!fractional && !roles.some((role) => role.address === feePayer && role.signer))) throw new Error("draft fee payer is not the exact signer role or explicitly separate Fractional transaction payer.");
    const transactionHex = text(raw.serializedTransactionHex, "serialized transaction", 2464);
    if (!HEX_BYTES.test(transactionHex) || decimal(raw.serializedBytes, "serialized transaction bytes", 1232n).toString() !== String(transactionHex.length / 2)) throw new Error("serialized transaction encoding or byte count is invalid.");
    if (!Array.isArray(raw.actions) || raw.actions.length !== 1 || raw.actions[0] !== coordinate.action || !Array.isArray(raw.flows) || raw.flows.length !== 1 || !Array.isArray(raw.semanticOwners) || raw.semanticOwners.length !== 1 || !Array.isArray(raw.registryBindings) || raw.registryBindings.length !== 1 || !Array.isArray(raw.runtimeAdmissions) || raw.runtimeAdmissions.length !== 1 || raw.runtimeAdmissions[0] !== "release-bound-enabled" || !Array.isArray(raw.exactEquations) || raw.exactEquations.length === 0 || !Array.isArray(raw.addressLookupTables)) throw new Error("callable draft is not one exact release-admitted semantic-owner action.");
    const flowContract = [SOURCE_COORDINATE, STRUCTURED_COORDINATE, DIRECT_COORDINATE, FRACTIONAL_COORDINATE].find((contract) => contract.familyTag === coordinate.familyTag && contract.familyVersion === coordinate.familyVersion && contract.family === coordinate.family) || null;
    if (flowContract === null) throw new Error("callable draft belongs to a family without a current browser semantic-owner contract.");
    if (raw.flows[0] !== flowContract.flow || raw.messageVersion !== flowContract.messageVersion || raw.addressLookupTables.length !== flowContract.lookupTables) throw new Error("callable draft differs from its current family transport contract.");
    const addressLookupTables = Object.freeze(raw.addressLookupTables.map((lookup, lookupIndex) => {
      requirePlain(lookup, `draft.addressLookupTables[${lookupIndex}]`);
      const writableAddresses = decimal(lookup.writableAddresses, `draft.addressLookupTables[${lookupIndex}].writableAddresses`, 256n);
      const readonlyAddresses = decimal(lookup.readonlyAddresses, `draft.addressLookupTables[${lookupIndex}].readonlyAddresses`, 256n);
      if (writableAddresses + readonlyAddresses === 0n || writableAddresses + readonlyAddresses > 256n) throw new Error("draft lookup-table projection has an invalid exact address count.");
      return Object.freeze({
        account: nonzeroAddress(lookup.account, `draft.addressLookupTables[${lookupIndex}].account`),
        observedSlot: positiveDecimal(lookup.observedSlot, `draft.addressLookupTables[${lookupIndex}].observedSlot`).toString(),
        stateSha256: hash32(lookup.stateSha256, `draft.addressLookupTables[${lookupIndex}].stateSha256`),
        writableAddresses: writableAddresses.toString(),
        readonlyAddresses: readonlyAddresses.toString()
      });
    }));
    const binding = raw.registryBindings[0];
    requirePlain(binding, "draft registry binding");
    if (positiveDecimal(binding.familyTag, "draft binding family tag", 255n).toString() !== coordinate.familyTag || positiveDecimal(binding.familyVersion, "draft binding family version", 255n).toString() !== coordinate.familyVersion || positiveDecimal(binding.localAction, "draft binding local action", 255n).toString() !== coordinate.localAction || binding.allocationStatus !== "frozen") throw new Error("draft registry binding differs from the checked coordinate.");
    if ((coordinate.family === "structured-claim" && binding.centralAction !== null) || ((coordinate.family === "source" || coordinate.family === "direct" || coordinate.family === "fractional") && decimal(binding.centralAction, "draft central action", 255n).toString() !== coordinate.localAction)) throw new Error("draft central action binding differs from its family-owned current contract.");
    raw.semanticOwners.forEach((owner, ownerIndex) => { requirePlain(owner, `draft.semanticOwners[${ownerIndex}]`); text(owner.package, "semantic owner package", 160); text(owner.schema, "semantic owner schema", 160); hash32(owner.releaseSha256, "semantic owner release"); });
    raw.exactEquations.forEach((equation, equationIndex) => { requirePlain(equation, `draft.exactEquations[${equationIndex}]`); requirePlain(equation.unit, `draft.exactEquations[${equationIndex}].unit`); text(equation.name, "exact equation name", 200); const left = decimal(equation.left, "exact equation left"); const right = decimal(equation.right, "exact equation right"); if (left !== right) throw new Error("draft exact-integer equation is unbalanced."); });
    return Object.freeze({ ...raw, draftId: hash32(raw.draftId, "draft ID"), feePayer, addressLookupTables, serializedTransactionHex: transactionHex });
  };

  const maximumSlot = (values) => values.reduce((maximum, value) => value > maximum ? value : maximum, 0n);
  const accountGroup = (kind) => {
    if (KIND_GROUPS[kind]) return KIND_GROUPS[kind];
    if (kind.startsWith("source-")) return "source";
    if (kind.startsWith("dealer-")) return "liquidity";
    if (kind.startsWith("failure-")) return "recovery";
    return "other";
  };

  const actionInspectionDisposition = (action, accountByAddress, tipSlot, requestedCommitment, maximumSlotLag) => {
    if (requestedCommitment !== "finalized") {
      return Object.freeze({
        eligible: false,
        kind: "nonfinal-view-refused",
        reason: "The visible account projection is processed and rollbackable; switch to finalized and reacquire the complete exact tuple.",
        observedAccounts: "0",
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition === null ? null : action.freshnessDisposition.validBeforeSlot
      });
    }
    if (!action.callable) {
      return Object.freeze({
        eligible: false,
        kind: action.projectedCallable ? "payload-variant-browser-contract-unavailable" : action.stateSelection === null ? "missing-finalized-selection" : "exact-tuple-unavailable",
        reason: action.reason,
        observedAccounts: action.stateSelection === null ? "0" : String(1 + action.stateSelection.dependencies.length),
        staleAccounts: "0",
        validBeforeSlot: action.projectedDraft ? action.projectedDraft.validBeforeSlot : null
      });
    }
    if (action.payloadVariant) {
      const stale = action.observationSet.accounts.filter((observation) => tipSlot >= BigInt(observation.observedSlot) && tipSlot - BigInt(observation.observedSlot) > maximumSlotLag);
      if (stale.length !== 0) {
        return Object.freeze({ eligible: false, kind: "stale-finalized-dealer-tuple", reason: `${stale.length} exact Dealer role observation(s) exceed the explicit maximum slot lag.`, observedAccounts: String(action.observationSet.accounts.length), staleAccounts: String(stale.length), validBeforeSlot: action.freshnessDisposition.validBeforeSlot });
      }
      if (tipSlot >= BigInt(action.freshnessDisposition.validBeforeSlot)) {
        return Object.freeze({ eligible: false, kind: "dealer-draft-freshness-expired", reason: `The projected tip ${tipSlot} has reached the Dealer draft's exclusive valid-before slot ${action.freshnessDisposition.validBeforeSlot}.`, observedAccounts: String(action.observationSet.accounts.length), staleAccounts: "0", validBeforeSlot: action.freshnessDisposition.validBeforeSlot });
      }
      return Object.freeze({ eligible: true, kind: "finalized-exact-dealer-variant-inspectable", reason: `Exact target-${action.payloadVariant.payloadDiscriminator} release admission, ${action.observationSet.accounts.length}-role finalized observation set, v0 lookup table, signer roles, and unsigned material agree for inspection.`, observedAccounts: String(action.observationSet.accounts.length), staleAccounts: "0", validBeforeSlot: action.freshnessDisposition.validBeforeSlot });
    }
    const selection = action.stateSelection;
    const namedAddresses = [selection.account, ...selection.dependencies];
    const missing = namedAddresses.filter((identity) => !accountByAddress.has(identity));
    if (missing.length !== 0) {
      return Object.freeze({
        eligible: false,
        kind: "missing-finalized-observation",
        reason: `${missing.length} semantic-owner account observation(s) are absent from the attached finalized session.`,
        observedAccounts: String(namedAddresses.length - missing.length),
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    const observations = namedAddresses.map((identity) => accountByAddress.get(identity));
    const driver = observations[0];
    if (driver.slot !== selection.accountSlot) {
      return Object.freeze({
        eligible: false,
        kind: "finalized-driver-observation-changed",
        reason: `The selected driver was observed at slot ${selection.accountSlot}, but the joined finalized account body is now at slot ${driver.slot}.`,
        observedAccounts: String(observations.length),
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    const stale = observations.filter((accountValue) => accountValue.stale);
    if (stale.length !== 0) {
      return Object.freeze({
        eligible: false,
        kind: "stale-finalized-observation",
        reason: `${stale.length} named finalized account observation(s) exceed the explicit maximum slot lag.`,
        observedAccounts: String(observations.length),
        staleAccounts: String(stale.length),
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    if (tipSlot >= BigInt(action.freshnessDisposition.validBeforeSlot)) {
      return Object.freeze({
        eligible: false,
        kind: "draft-freshness-expired",
        reason: `The projected tip ${tipSlot} has reached the draft's exclusive valid-before slot ${action.freshnessDisposition.validBeforeSlot}.`,
        observedAccounts: String(observations.length),
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    if (action.directContract) {
      return Object.freeze({
        eligible: true,
        kind: "finalized-exact-direct-current-inspectable",
        reason: `Current Direct ${action.coordinate.localAction} ${action.directContract.branch} material has an exact role grammar, single checked release, finalized driver/dependency cursor, balanced conservation equations, and no browser-authored authority input.`,
        observedAccounts: String(observations.length),
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    if (action.fractionalContract) {
      const holderDisposition = action.fractionalContract.holderChoice === null
        ? "this action asks for no holder-authored semantic input"
        : "only the displayed claimant/source/outcome/quantity/destination/funding choices are holder-approved; every account meta and derived field remains chain-owned";
      return Object.freeze({
        eligible: true,
        kind: "finalized-exact-fractional-chain-material-inspectable",
        reason: `Fractional action ${action.coordinate.localAction} has exact finalized identities, metas, amount equations, replay cursor, release, and unsigned bytes; ${holderDisposition}.`,
        observedAccounts: String(observations.length),
        staleAccounts: "0",
        validBeforeSlot: action.freshnessDisposition.validBeforeSlot
      });
    }
    return Object.freeze({
      eligible: true,
      kind: "finalized-exact-tuple-inspectable",
      reason: "Checked release, exact finalized driver/dependencies, freshness bound, and canonical unsigned material agree for inspection.",
      observedAccounts: String(observations.length),
      staleAccounts: "0",
      validBeforeSlot: action.freshnessDisposition.validBeforeSlot
    });
  };

  const deriveSnapshot = (configuration, session, actionCapabilities, health, acquisition, releases, accountResponse, keeperActions, forks, remainingResponseBytes) => {
    const accounts = accountResponse.selected;
    const candidateSlots = forks.nodes.map((node) => BigInt(node.slot)).concat(accounts.map((accountValue) => BigInt(accountValue.slot)));
    for (const action of actionCapabilities.actions) {
      if (action.stateSelection !== null) candidateSlots.push(BigInt(action.stateSelection.accountSlot));
      if (action.freshnessDisposition !== null) candidateSlots.push(BigInt(action.freshnessDisposition.observedSlot));
    }
    if (forks.finalizedRoot) candidateSlots.push(BigInt(forks.finalizedRoot.slot));
    const tipSlot = maximumSlot(candidateSlots);
    const finalizedSlot = forks.finalizedRoot ? BigInt(forks.finalizedRoot.slot) : null;
    const frozen = new Set(forks.frozenSlots);
    const dead = new Set(forks.deadSlots);
    const maximumLag = BigInt(configuration.bounds.maximumSlotLag);
    const annotated = accounts.map((accountValue) => {
      const lag = tipSlot >= BigInt(accountValue.slot) ? tipSlot - BigInt(accountValue.slot) : 0n;
      let forkState = "finalized-scan";
      if (accountValue.branch.kind === "processed-fork") {
        const matching = forks.nodes.find((node) => node.slot === accountValue.slot && node.blockhash === accountValue.branch.blockhash);
        forkState = dead.has(accountValue.slot) ? "dead-fork" : !matching ? "unidentified-fork" : frozen.has(accountValue.slot) ? "processed-frozen" : "processed-unfrozen";
      }
      return Object.freeze({ ...accountValue, slotLag: lag.toString(), stale: lag > maximumLag, forkState, group: accountGroup(accountValue.kind) });
    });
    const groups = Object.freeze(Object.fromEntries(GROUP_ORDER.map((name) => [name, Object.freeze(annotated.filter((accountValue) => accountValue.group === name))])));
    const accountByAddress = new Map(annotated.map((accountValue) => [accountValue.address, accountValue]));
    const inspectedActions = Object.freeze(actionCapabilities.actions.map((action) => Object.freeze({
      ...action,
      inspection: actionInspectionDisposition(action, accountByAddress, tipSlot, configuration.commitment, maximumLag)
    })));
    const inspectedActionCapabilities = Object.freeze({ ...actionCapabilities, actions: inspectedActions });
    const familySet = new Set(releases.selected.families);
    const successorCapabilities = inspectedActions.map((action) => Object.freeze({
      surface: "successor-action",
      family: action.coordinate.family,
      label: `${action.coordinate.familyTag}/${action.coordinate.familyVersion}/${action.coordinate.localAction}${action.payloadVariant ? `/${action.payloadVariant.payloadDiscriminator}` : ""} · ${action.payloadVariant ? action.payloadVariant.name : action.coordinate.action}`,
      indexedByRelease: action.stateSelection !== null || Boolean(action.observationSet),
      allocationStatus: action.inspection.eligible ? "inspectable" : "refused",
      enabled: action.inspection.eligible,
      reason: action.inspection.reason
    }));
    const productIndexed = familySet.has("series") || annotated.some((accountValue) => accountValue.group === "product" || accountValue.group === "series");
    const ownerV3Indexed = familySet.has("position-v3") || familySet.has("replay-v3") || annotated.some((accountValue) => accountValue.kind === "position-v3" || accountValue.kind === "replay-v3" || accountValue.kind.startsWith("general-owner-settlement"));
    const capabilities = Object.freeze([
      ...successorCapabilities,
      Object.freeze({
        surface: "joined-runtime",
        family: "product-registration",
        label: "Product / Series registration",
        indexedByRelease: productIndexed,
        allocationStatus: "not-authenticated",
        enabled: false,
        reason: productIndexed
          ? "Product or Series state is visible, but compiler output is only an untrusted proposal and operatord exposes no release-bound Product registration capability verdict."
          : "No Product or Series decoder/state is visible for the selected release, and operatord exposes no release-bound Product registration capability verdict."
      }),
      Object.freeze({
        surface: "joined-runtime",
        family: "owner-position-v3",
        label: "Owner / Position V3 lifecycle",
        indexedByRelease: ownerV3Indexed,
        allocationStatus: "not-authenticated",
        enabled: false,
        reason: ownerV3Indexed
          ? "Position V3, Replay V3, or owner-settlement state is visible, but codec presence is not an authenticated action-level capability verdict for this release."
          : "No Position V3, Replay V3, or owner-settlement decoder/state is visible, and no action-level capability verdict is exposed for this release."
      })
    ]);
    const unsafeForkAccounts = annotated.filter((accountValue) => accountValue.forkState === "dead-fork" || accountValue.forkState === "unidentified-fork").length;
    const staleAccounts = annotated.filter((accountValue) => accountValue.stale).length;
    const snapshot = {
      schema: "dragons-clutch/operatord-chain-projection/v1",
      authority: "untrusted-projection",
      session,
      configuration: redactedConfiguration(configuration),
      health,
      acquisition,
      release: Object.freeze({
        observed: releases.selected,
        declaredManifestSha256: configuration.release.releaseManifestSha256,
        declaredSourceCommit: configuration.release.sourceCommit,
        declaredCapabilityProfileId: configuration.release.capabilityProfileId,
        manifestSourceCapabilityAuthentication: "daemon reports offline checker + sealed deployment + measured ELF join; browser treats this as an untrusted projection"
      }),
      finality: Object.freeze({
        requestedCommitment: configuration.commitment,
        projectedTipSlot: tipSlot.toString(),
        finalizedRootSlot: finalizedSlot === null ? null : finalizedSlot.toString(),
        maximumAcceptedSlotLag: configuration.bounds.maximumSlotLag,
        staleAccountCount: String(staleAccounts),
        unsafeForkAccountCount: String(unsafeForkAccounts),
        authorityEligible: false,
        processedTransport: acquisition.processedTransport,
        forkNodeCount: String(forks.nodes.length),
        frozenSlotCount: String(forks.frozenSlots.length),
        deadSlotCount: String(forks.deadSlots.length)
      }),
      acquisitionBounds: Object.freeze({ ...configuration.bounds, remainingResponseBytes }),
      accountCounts: Object.freeze({ selectedRelease: String(annotated.length), ignoredOtherReleases: accountResponse.ignoredOtherReleases }),
      accounts: Object.freeze(annotated),
      groups,
      groupLabels: GROUP_LABELS,
      keeperActions,
      actionCapabilities: inspectedActionCapabilities,
      capabilities,
      forks
    };
    Object.defineProperty(snapshot, "sourceConfiguration", { value: configuration, enumerable: false });
    return Object.freeze(snapshot);
  };

  const acquire = async (target, fetchFunction = root.fetch.bind(root)) => {
    const reader = new BoundedGetReader(target, fetchFunction);
    const acquisition = validateAcquisition(await reader.get("/v1/acquisition"), target);
    const configuration = acquisition.configuration;
    const session = validateSession(await reader.get("/v1/session"), configuration);
    const actionCapabilities = validateActionCapabilities(await reader.get("/v1/actions"), configuration, session);
    const healthRaw = await reader.get("/v1/health");
    const health = validateHealth(healthRaw, configuration);
    const releases = validateReleaseResponse(await reader.get("/v1/releases"), configuration);
    const accounts = validateAccountsResponse(await reader.get(`/v1/accounts?commitment=${configuration.commitment}`), configuration);
    const finalizedConfiguration = configuration.commitment === "finalized" ? configuration : Object.freeze({ ...configuration, commitment: "finalized" });
    const finalizedAccounts = configuration.commitment === "finalized" ? accounts : validateAccountsResponse(await reader.get("/v1/accounts?commitment=finalized"), finalizedConfiguration);
    joinSessionAccounts(session, finalizedAccounts);
    const keeper = validateKeeper(await reader.get(`/v1/keeper/next?commitment=${configuration.commitment}`), configuration);
    const forks = validateForks(await reader.get("/v1/forks"));
    const endSession = validateSession(await reader.get("/v1/session"), configuration);
    const endAcquisition = validateAcquisition(await reader.get("/v1/acquisition"), target);
    if (endAcquisition.configuration.clusterKey !== configuration.clusterKey
        || endAcquisition.configuration.release.releaseKey !== configuration.release.releaseKey
        || endAcquisition.configuration.rpcHttpEndpoint.bindingSha256 !== configuration.rpcHttpEndpoint.bindingSha256
        || endAcquisition.configuration.rpcWebsocketEndpoint.bindingSha256 !== configuration.rpcWebsocketEndpoint.bindingSha256) {
      throw new Error("Daemon chain/release/endpoint binding changed during acquisition; reacquire instead of mixing generations.");
    }
    if (configuration.commitment === "processed"
        && (endAcquisition.processedTransport.connectionGeneration !== acquisition.processedTransport.connectionGeneration
          || endAcquisition.processedTransport.rollbackEpoch !== acquisition.processedTransport.rollbackEpoch)) {
      throw new Error("Processed transport generation or rollback epoch changed during acquisition; reacquire a coherent non-final projection.");
    }
    if (endSession.sessionId !== session.sessionId) throw new Error("Finalized canonical session identity changed during acquisition; reacquire instead of persisting a mixed restart view.");
    if (actionCapabilities.sessionId !== endSession.sessionId) throw new Error("Action capability set belongs to a stale finalized session; reacquire from the beginning.");
    return deriveSnapshot(configuration, endSession, actionCapabilities, health, endAcquisition, releases, accounts, keeper, forks, reader.remaining.toString());
  };

  root.GlassChainClient = Object.freeze({ validateConfiguration, redactedConfiguration, validateSession, validateActionCapabilities, acquire, deriveSnapshot, decodeBase58, encodeBase58, groupOrder: GROUP_ORDER, groupLabels: GROUP_LABELS });
})(typeof globalThis === "object" ? globalThis : this);
