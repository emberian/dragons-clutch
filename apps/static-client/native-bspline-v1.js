/*
 * Dependency-free inspection SDK for Dragon's Clutch native B-spline V1.
 *
 * This module performs no RPC, wallet, signing, or submission. It projects a
 * self-certifying TermsAccount into the same BasisSpec fields used by native
 * resolution, structurally verifies a compiler certificate, and composes the
 * live BeginArtifact -> nine WriteArtifact -> SealArtifact -> CreateMarket
 * route. It never lowers a smooth basis to degree-zero categories. A
 * certificate still requires the Rust compiler's exact recompile check before
 * its analytic claim is trusted.
 */
(function (root) {
  "use strict";

  const BASIS_MAGIC = ascii("DCBASV01");
  const CERT_MAGIC = ascii("DCSHCV01");
  const BASIS_DOMAIN = ascii("dragons-clutch/basis-spec/v1");
  const CERT_DOMAIN = ascii("dragons-clutch/shape-certificate/v1");
  const TERMS_DOMAIN = ascii("dragons-clutch/terms/v2");
  const BASIS_BYTES = 304;
  const CERT_FIXED_BYTES = 456;
  const TERMS_BYTES = 1656;
  const TERMS_BODY_START = 34;
  const TERMS_BODY_BYTES = 1620;
  const MAX_OUTCOMES = 16;
  const MAX_KNOTS = 16;
  const MAX_RATIONAL_INTEGER_BYTES = 4096;
  const MAX_CERTIFICATE_BYTES = 256 * 1024;
  const ARTIFACT_CHUNK_BYTES = 192;
  const TERMS_WRITE_COUNT = 9;
  const MIN_UPLOAD_LIFETIME_SLOTS = 8n;
  const MAX_UPLOAD_LIFETIME_SLOTS = 432000n;
  const MAX_VALUE = 1000000000000000000000000n;
  const U64_MAX = (1n << 64n) - 1n;
  const U128_MAX = (1n << 128n) - 1n;

  function ascii(text) {
    return Uint8Array.from(text, (character) => character.charCodeAt(0));
  }

  function bytes(value, name = "bytes") {
    if (value instanceof Uint8Array) return value;
    if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    if (value instanceof ArrayBuffer) return new Uint8Array(value);
    throw new TypeError(`${name} must be a byte array.`);
  }

  function equal(left, right) {
    if (left.length !== right.length) return false;
    let difference = 0;
    for (let index = 0; index < left.length; index += 1) difference |= left[index] ^ right[index];
    return difference === 0;
  }

  function concat(...parts) {
    const length = parts.reduce((sum, part) => sum + part.length, 0);
    const output = new Uint8Array(length);
    let at = 0;
    for (const part of parts) {
      output.set(part, at);
      at += part.length;
    }
    return output;
  }

  function hex(value) {
    return Array.from(bytes(value), (byte) => byte.toString(16).padStart(2, "0")).join("");
  }

  function fromHex(value, length, name) {
    if (typeof value !== "string" || !/^[0-9a-f]+$/.test(value) || value.length !== length * 2) {
      throw new Error(`${name} must be exactly ${length} lowercase-hex bytes.`);
    }
    const output = new Uint8Array(length);
    for (let index = 0; index < length; index += 1) output[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
    if (output.every((byte) => byte === 0)) throw new Error(`${name} may not be the reserved zero identity.`);
    return output;
  }

  function canonicalUnsigned(value, maximum, name) {
    if (typeof value !== "string" || !/^(?:0|[1-9][0-9]*)$/.test(value)) {
      throw new Error(`${name} must be a canonical unsigned decimal string.`);
    }
    const parsed = BigInt(value);
    if (parsed > maximum) throw new Error(`${name} exceeds its frozen integer width.`);
    return parsed;
  }

  function safeU8(value, name) {
    if (!Number.isInteger(value) || value < 0 || value > 255) throw new Error(`${name} must be a u8.`);
    return value;
  }

  function readLe(input, offset, width) {
    let value = 0n;
    for (let index = width - 1; index >= 0; index -= 1) value = (value << 8n) | BigInt(input[offset + index]);
    return value;
  }

  function writeLe(output, offset, value, width) {
    let remaining = value;
    for (let index = 0; index < width; index += 1) {
      output[offset + index] = Number(remaining & 255n);
      remaining >>= 8n;
    }
    if (remaining !== 0n) throw new Error("Integer does not fit the selected width.");
  }

  function readU16(input, offset) {
    return Number(readLe(input, offset, 2));
  }

  function assertZero(input, start, end, name) {
    for (let index = start; index < end; index += 1) {
      if (input[index] !== 0) throw new Error(`${name} is not canonical zero padding.`);
    }
  }

  async function domainDigest(domain, payload) {
    if (!root.crypto || !root.crypto.subtle) throw new Error("Web Crypto SHA-256 is unavailable; no digest was claimed.");
    const result = await root.crypto.subtle.digest("SHA-256", concat(domain, bytes(payload)));
    return new Uint8Array(result);
  }

  function normalizeBasisSpec(spec) {
    if (!spec || typeof spec !== "object" || Array.isArray(spec)) throw new Error("basisSpec must be an object.");
    const outcomeCount = safeU8(spec.outcomeCount, "basisSpec.outcomeCount");
    const degree = safeU8(spec.degree, "basisSpec.degree");
    if (degree > 3) throw new Error("basisSpec.degree must be in 0..=3.");
    if (outcomeCount < 2 || outcomeCount > MAX_OUTCOMES) throw new Error("basisSpec.outcomeCount must be in 2..=16.");
    if (!Array.isArray(spec.knots)) throw new Error("basisSpec.knots must be the active knot array.");
    const knotCount = spec.knots.length;
    if (knotCount < 1 || knotCount > MAX_KNOTS) throw new Error("basisSpec.knots has an invalid active length.");
    const expectedKnots = degree === 0 ? outcomeCount - 1 : outcomeCount + 1 - degree;
    if (knotCount !== expectedKnots || (degree >= 1 && knotCount < 2)) throw new Error("basisSpec knot/outcome/degree count relation is invalid.");
    const denominator = canonicalUnsigned(spec.denominator, U64_MAX, "basisSpec.denominator");
    if (denominator === 0n) throw new Error("basisSpec.denominator may not be zero.");
    const domainMax = canonicalUnsigned(spec.domainMax, U128_MAX, "basisSpec.domainMax");
    const knots = spec.knots.map((knot, index) => canonicalUnsigned(knot, U128_MAX, `basisSpec.knots[${index}]`));
    let largestGap = 0n;
    for (let index = 0; index < knots.length; index += 1) {
      if (knots[index] > domainMax || (index === 0 && degree === 0 && knots[index] === 0n)) throw new Error("basisSpec has a knot outside its admitted domain.");
      if (index > 0) {
        if (knots[index] <= knots[index - 1]) throw new Error("basisSpec active knots must be strictly increasing.");
        const gap = knots[index] - knots[index - 1];
        if (gap > largestGap) largestGap = gap;
      }
    }
    const uniformLog2Spacing = spec.uniformLog2Spacing === null ? 255 : safeU8(spec.uniformLog2Spacing, "basisSpec.uniformLog2Spacing");
    if (uniformLog2Spacing === 255) {
      if (degree >= 2) throw new Error("degree two and three require a uniform power-of-two spacing declaration.");
    } else {
      if (uniformLog2Spacing >= 128) throw new Error("basisSpec.uniformLog2Spacing must be below 128.");
      const gap = 1n << BigInt(uniformLog2Spacing);
      for (let index = 1; index < knots.length; index += 1) {
        if (knots[index] - knots[index - 1] !== gap) throw new Error("basisSpec uniform spacing contradicts its knots.");
      }
    }
    if (spec.edgePolicy !== "clamp" && spec.edgePolicy !== "refuse") throw new Error("basisSpec.edgePolicy is not recognized.");
    if (degree === 0 && spec.edgePolicy !== "clamp") throw new Error("native degree zero is exhaustive and clamp-only.");
    if (degree >= 1) {
      let operand;
      if (degree === 1) operand = largestGap - 1n;
      else if (degree === 2) operand = largestGap * largestGap * 2n;
      else operand = largestGap * largestGap * largestGap * 6n;
      if (denominator * operand >= (1n << 127n)) throw new Error("basisSpec exceeds the freeze-time arithmetic bound.");
    }
    return {
      outcomeCount,
      degree,
      uniformLog2Spacing,
      denominator,
      domainMax,
      edgePolicy: spec.edgePolicy,
      knots
    };
  }

  function encodeBasisSpec(spec) {
    const value = normalizeBasisSpec(spec);
    const output = new Uint8Array(BASIS_BYTES);
    output.set(BASIS_MAGIC, 0);
    writeLe(output, 8, 1n, 2);
    writeLe(output, 10, 1n, 2);
    output[12] = 1;
    output[13] = value.outcomeCount;
    output[14] = value.degree;
    output[15] = value.knots.length;
    output[16] = value.uniformLog2Spacing;
    output[17] = value.edgePolicy === "clamp" ? 1 : 2;
    writeLe(output, 24, value.denominator, 8);
    writeLe(output, 32, value.domainMax, 16);
    value.knots.forEach((knot, index) => writeLe(output, 48 + index * 16, knot, 16));
    return output;
  }

  function decodeBasisSpec(input) {
    const value = bytes(input, "basis bytes");
    if (value.length !== BASIS_BYTES) throw new Error(`Basis artifact must be exactly ${BASIS_BYTES} bytes.`);
    if (!equal(value.slice(0, 8), BASIS_MAGIC) || readU16(value, 8) !== 1 || readU16(value, 10) !== 1 || value[12] !== 1) {
      throw new Error("Unknown basis magic, version, evaluator, or native semantic tag.");
    }
    assertZero(value, 18, 24, "Basis reserved suffix");
    const knotCount = value[15];
    const knots = [];
    for (let index = 0; index < MAX_KNOTS; index += 1) {
      const knot = readLe(value, 48 + index * 16, 16);
      if (index < knotCount) knots.push(knot.toString());
      else if (knot !== 0n) throw new Error("Basis inactive knots are not canonical zero padding.");
    }
    const decoded = {
      semanticMode: "native-bspline",
      outcomeCount: value[13],
      degree: value[14],
      uniformLog2Spacing: value[16] === 255 ? null : value[16],
      denominator: readLe(value, 24, 8).toString(),
      domainMax: readLe(value, 32, 16).toString(),
      edgePolicy: value[17] === 1 ? "clamp" : value[17] === 2 ? "refuse" : "unknown",
      knots
    };
    if (decoded.edgePolicy === "unknown" || !equal(encodeBasisSpec(decoded), value)) throw new Error("Basis bytes are not the unique canonical encoding.");
    return decoded;
  }

  async function digestBasisSpec(input) {
    const byteInput = ArrayBuffer.isView(input) || input instanceof ArrayBuffer;
    const canonical = byteInput ? bytes(input, "basis bytes") : encodeBasisSpec(input);
    if (byteInput) decodeBasisSpec(canonical);
    return domainDigest(BASIS_DOMAIN, canonical);
  }

  async function inspectTermsAccount(input) {
    const value = bytes(input, "TermsAccount bytes");
    if (value.length !== TERMS_BYTES) throw new Error(`TermsAccount must be exactly ${TERMS_BYTES} bytes.`);
    if (value[0] !== 10 || value[1] !== 3) throw new Error("TermsAccount tag/version is not canonical V3.");
    if (value[TERMS_BYTES - 1] !== 0) throw new Error("TermsAccount flags must be zero.");
    const storedDigest = value.slice(2, 34);
    if (storedDigest.every((byte) => byte === 0)) throw new Error("TermsAccount uses the reserved zero digest.");
    const body = value.slice(TERMS_BODY_START, TERMS_BODY_START + TERMS_BODY_BYTES);
    const computedDigest = await domainDigest(TERMS_DOMAIN, body);
    if (!equal(storedDigest, computedDigest)) throw new Error("TermsAccount digest does not recompute from its exact body.");
    const basisOffset = TERMS_BODY_START;
    for (const [name, start] of [["realm", 0], ["profile", 32], ["feed", 64]]) {
      if (value.slice(basisOffset + start, basisOffset + start + 32).every((byte) => byte === 0)) {
        throw new Error(`TermsAccount ${name} uses the reserved zero identity.`);
      }
    }
    const outcomeCount = value[basisOffset + 128];
    const payoutCount = value[basisOffset + 129];
    if (payoutCount < 1 || payoutCount > 8) throw new Error("TermsAccount payout count is invalid.");
    const denominator = readLe(value, basisOffset + 130, 8);
    const edgePolicyId = value[basisOffset + 1271];
    const degree = value[basisOffset + 1272];
    const knotCount = value[basisOffset + 1273];
    const uniform = value[basisOffset + 1274];
    assertZero(value, basisOffset + 1276, basisOffset + 1277, "Terms resolution reserved byte");
    assertZero(value, basisOffset + 1613, basisOffset + 1620, "Terms body reserved suffix");
    const knots = [];
    for (let index = 0; index < MAX_KNOTS; index += 1) {
      const knot = readLe(value, basisOffset + 1349 + index * 16, 16);
      if (index < knotCount) knots.push(knot.toString());
      else if (knot !== 0n) throw new Error("TermsAccount inactive knots are not zero.");
    }
    const basisSpec = decodeBasisSpec(encodeBasisSpec({
      outcomeCount,
      degree,
      uniformLog2Spacing: uniform === 255 ? null : uniform,
      denominator: denominator.toString(),
      domainMax: MAX_VALUE.toString(),
      edgePolicy: edgePolicyId === 1 ? "clamp" : edgePolicyId === 2 ? "refuse" : "unknown",
      knots
    }));
    return Object.freeze({
      termsDigest: hex(storedDigest),
      realm: hex(value.slice(basisOffset, basisOffset + 32)),
      profile: hex(value.slice(basisOffset + 32, basisOffset + 64)),
      feed: hex(value.slice(basisOffset + 64, basisOffset + 96)),
      basisSpec,
      validationScope: "self-certifying-digest-and-native-basis-projection; runtime policy admission remains on-chain"
    });
  }

  function gcd(left, right) {
    let a = left;
    let b = right;
    while (b !== 0n) [a, b] = [b, a % b];
    return a;
  }

  function parseRational(value, state) {
    if (state.at + 4 > value.length) throw new Error("Certificate rational header is truncated.");
    const numeratorLength = readU16(value, state.at);
    const denominatorLength = readU16(value, state.at + 2);
    state.at += 4;
    if (numeratorLength > MAX_RATIONAL_INTEGER_BYTES || denominatorLength < 1 || denominatorLength > MAX_RATIONAL_INTEGER_BYTES) {
      throw new Error("Certificate rational length is invalid.");
    }
    if (state.at + numeratorLength + denominatorLength > value.length) throw new Error("Certificate rational body is truncated.");
    const numeratorBytes = value.slice(state.at, state.at + numeratorLength);
    state.at += numeratorLength;
    const denominatorBytes = value.slice(state.at, state.at + denominatorLength);
    state.at += denominatorLength;
    if ((numeratorLength > 0 && numeratorBytes[0] === 0) || denominatorBytes[0] === 0) throw new Error("Certificate rational integer has leading zeroes.");
    const numerator = numeratorLength === 0 ? 0n : BigInt(`0x${hex(numeratorBytes)}`);
    const denominator = BigInt(`0x${hex(denominatorBytes)}`);
    if (denominator === 0n || gcd(numerator, denominator) !== 1n) throw new Error("Certificate rational is zero-denominator or unreduced.");
    if (numerator === 0n && (numeratorLength !== 0 || denominator !== 1n)) throw new Error("Certificate zero rational is not canonical 0/1.");
    return `${numerator}/${denominator}`;
  }

  function inspectShape(value, offset) {
    const tag = value[offset];
    assertZero(value, offset + 1, offset + 8, "Shape reserved prefix");
    const a = readLe(value, offset + 8, 16);
    const b = readLe(value, offset + 24, 16);
    const c = readLe(value, offset + 40, 16);
    const height = readLe(value, offset + 56, 8);
    const names = [null, "hard-range", "upper-tail", "lower-tail", "triangle", "capped-call", "capped-put", "gaussian"];
    if (!names[tag]) throw new Error("Certificate shape tag is not recognized.");
    if (((tag === 2 || tag === 3) && (b !== 0n || c !== 0n)) || ((tag === 1 || tag === 5 || tag === 6 || tag === 7) && c !== 0n)) {
      throw new Error("Certificate shape inactive coordinates are not zero.");
    }
    return Object.freeze({ family: names[tag], a: a.toString(), b: b.toString(), c: c.toString(), height: height.toString() });
  }

  async function inspectShapeCertificate(input) {
    const value = bytes(input, "shape certificate bytes");
    if (value.length > MAX_CERTIFICATE_BYTES || value.length < CERT_FIXED_BYTES) throw new Error("Shape certificate length is outside the host bound.");
    if (!equal(value.slice(0, 8), CERT_MAGIC)) throw new Error("Shape certificate magic is unknown.");
    for (const offset of [8, 10, 12, 14]) if (readU16(value, offset) !== 1) throw new Error("Shape certificate version is not recognized.");
    if (value[16] !== 1) throw new Error("Only native B-spline certificates are accepted; compatibility lowering is refused.");
    if (![1, 2].includes(value[17]) || value[18] < 1 || value[18] > 5) throw new Error("Shape certificate compiler enum is invalid.");
    const termsDigest = value.slice(20, 52);
    if (termsDigest.every((byte) => byte === 0)) throw new Error("Shape certificate Terms digest is zero.");
    const storedBasisDigest = value.slice(52, 84);
    const basisBytes = value.slice(84, 84 + BASIS_BYTES);
    const basisSpec = decodeBasisSpec(basisBytes);
    const computedBasisDigest = await digestBasisSpec(basisBytes);
    if (!equal(storedBasisDigest, computedBasisDigest)) throw new Error("Shape certificate basis digest does not recompute.");
    const shape = inspectShape(value, 84 + BASIS_BYTES);
    const rationalCount = readU16(value, 452);
    if (rationalCount !== basisSpec.outcomeCount + 10) throw new Error("Shape certificate rational count is invalid.");
    assertZero(value, 454, 456, "Shape certificate reserved suffix");
    const state = { at: CERT_FIXED_BYTES };
    const rationals = [];
    for (let index = 0; index < rationalCount; index += 1) rationals.push(parseRational(value, state));
    if (state.at !== value.length) throw new Error("Shape certificate has trailing bytes.");
    return Object.freeze({
      schema: "dragon-clutch.native-shape-certificate.v1",
      semanticMode: "native-bspline",
      termsDigest: hex(termsDigest),
      basisDigest: hex(storedBasisDigest),
      basisSpec,
      shape,
      spanStatus: value[17] === 1 ? "exact-in-span" : "certified-approximation",
      constructionId: value[18],
      subdivisionDepth: value[19],
      rationalCount,
      rationals,
      verificationScope: "canonical-structure-and-digests-only; exact Rust compiler recompile still required"
    });
  }

  async function digestShapeCertificate(input) {
    await inspectShapeCertificate(input);
    return domainDigest(CERT_DOMAIN, bytes(input));
  }

  function uploadWindow(observedCurrentSlot, expiresSlot) {
    const current = canonicalUnsigned(observedCurrentSlot, U64_MAX, "observedCurrentSlot");
    const expiry = canonicalUnsigned(expiresSlot, U64_MAX, "expiresSlot");
    const lifetime = expiry - current;
    if (lifetime < MIN_UPLOAD_LIFETIME_SLOTS || lifetime > MAX_UPLOAD_LIFETIME_SLOTS) {
      throw new Error("Artifact expiry is outside the runtime's 8..=432000 slot lifetime bound.");
    }
    return { current, expiry };
  }

  function encodeBeginTermsArtifactIntent({ realm, termsDigest, expiresSlot }) {
    const output = new Uint8Array(77);
    output.set(Uint8Array.of(18, 3, 3), 0);
    output.set(fromHex(realm, 32, "realm"), 3);
    output.set(fromHex(termsDigest, 32, "termsDigest"), 35);
    writeLe(output, 67, BigInt(TERMS_BYTES), 2);
    writeLe(output, 69, canonicalUnsigned(expiresSlot, U64_MAX, "expiresSlot"), 8);
    return output;
  }

  function encodeWriteTermsArtifactIntent({ realm, termsDigest, cursor, chunk }) {
    const source = bytes(chunk, "Terms artifact chunk");
    if (source.length < 1 || source.length > ARTIFACT_CHUNK_BYTES) throw new Error("Terms artifact chunk length must be in 1..=192.");
    const cursorValue = canonicalUnsigned(cursor, 65535n, "cursor");
    const output = new Uint8Array(263);
    output.set(Uint8Array.of(19, 3, 3), 0);
    output.set(fromHex(realm, 32, "realm"), 3);
    output.set(fromHex(termsDigest, 32, "termsDigest"), 35);
    writeLe(output, 67, cursorValue, 2);
    writeLe(output, 69, BigInt(source.length), 2);
    output.set(source, 71);
    return output;
  }

  function encodeSealTermsArtifactIntent({ realm, termsDigest }) {
    const output = new Uint8Array(69);
    output.set(Uint8Array.of(20, 3, 3), 0);
    output.set(fromHex(realm, 32, "realm"), 3);
    output.set(fromHex(termsDigest, 32, "termsDigest"), 35);
    writeLe(output, 67, BigInt(TERMS_BYTES), 2);
    return output;
  }

  function buildTermsArtifactUploadPlan({ termsAccountBytes, realm, termsDigest, observedCurrentSlot, expiresSlot }) {
    const termsBytes = bytes(termsAccountBytes, "TermsAccount bytes");
    if (termsBytes.length !== TERMS_BYTES) throw new Error(`TermsAccount must be exactly ${TERMS_BYTES} bytes.`);
    const window = uploadWindow(observedCurrentSlot, expiresSlot);
    const beginIntent = encodeBeginTermsArtifactIntent({ realm, termsDigest, expiresSlot: window.expiry.toString() });
    const writeIntents = [];
    for (let cursor = 0; cursor < TERMS_BYTES; cursor += ARTIFACT_CHUNK_BYTES) {
      writeIntents.push(encodeWriteTermsArtifactIntent({
        realm,
        termsDigest,
        cursor: String(cursor),
        chunk: termsBytes.slice(cursor, Math.min(cursor + ARTIFACT_CHUNK_BYTES, TERMS_BYTES))
      }));
    }
    if (writeIntents.length !== TERMS_WRITE_COUNT || readU16(writeIntents[8], 69) !== 120) throw new Error("Internal Terms chunk geometry mismatch.");
    const sealIntent = encodeSealTermsArtifactIntent({ realm, termsDigest });
    const plan = Object.freeze({
      observedCurrentSlot: window.current.toString(),
      expiresSlot: window.expiry.toString(),
      beginIntent,
      writeIntents: Object.freeze(writeIntents),
      sealIntent
    });
    verifyTermsArtifactUploadPlan({ termsAccountBytes: termsBytes, realm, termsDigest, plan });
    return plan;
  }

  function verifyTermsArtifactUploadPlan({ termsAccountBytes, realm, termsDigest, plan }) {
    const source = bytes(termsAccountBytes, "TermsAccount bytes");
    if (source.length !== TERMS_BYTES || !plan || !Array.isArray(plan.writeIntents)) throw new Error("Terms artifact plan shape is invalid.");
    const window = uploadWindow(plan.observedCurrentSlot, plan.expiresSlot);
    const realmBytes = fromHex(realm, 32, "realm");
    const digestBytes = fromHex(termsDigest, 32, "termsDigest");
    const begin = bytes(plan.beginIntent, "BeginArtifact intent");
    if (begin.length !== 77 || !equal(begin.slice(0, 3), Uint8Array.of(18, 3, 3))
      || !equal(begin.slice(3, 35), realmBytes) || !equal(begin.slice(35, 67), digestBytes)
      || readU16(begin, 67) !== TERMS_BYTES || readLe(begin, 69, 8) !== window.expiry) {
      throw new Error("BeginArtifact intent is not bound to this Terms upload.");
    }
    if (plan.writeIntents.length !== TERMS_WRITE_COUNT) throw new Error("Terms upload must contain exactly nine writes.");
    const reconstructed = new Uint8Array(TERMS_BYTES);
    for (let index = 0; index < TERMS_WRITE_COUNT; index += 1) {
      const intent = bytes(plan.writeIntents[index], `WriteArtifact intent ${index}`);
      const expectedCursor = index * ARTIFACT_CHUNK_BYTES;
      const expectedLength = Math.min(ARTIFACT_CHUNK_BYTES, TERMS_BYTES - expectedCursor);
      if (intent.length !== 263 || !equal(intent.slice(0, 3), Uint8Array.of(19, 3, 3))
        || !equal(intent.slice(3, 35), realmBytes) || !equal(intent.slice(35, 67), digestBytes)
        || readU16(intent, 67) !== expectedCursor || readU16(intent, 69) !== expectedLength) {
        throw new Error(`WriteArtifact intent ${index} has the wrong binding, cursor, or chunk length.`);
      }
      assertZero(intent, 71 + expectedLength, 263, `WriteArtifact intent ${index} wire suffix`);
      reconstructed.set(intent.slice(71, 71 + expectedLength), expectedCursor);
    }
    if (!equal(reconstructed, source)) throw new Error("Terms artifact writes do not reconstruct the exact TermsAccount bytes.");
    const seal = bytes(plan.sealIntent, "SealArtifact intent");
    if (seal.length !== 69 || !equal(seal.slice(0, 3), Uint8Array.of(20, 3, 3))
      || !equal(seal.slice(3, 35), realmBytes) || !equal(seal.slice(35, 67), digestBytes)
      || readU16(seal, 67) !== TERMS_BYTES) {
      throw new Error("SealArtifact intent is not bound to this Terms upload.");
    }
    return true;
  }

  function encodeCreateMarketIntent({ realm, profile, marketNonce, outcomeCount, termsDigest, feed }) {
    const nonce = canonicalUnsigned(marketNonce, U64_MAX, "marketNonce");
    const count = safeU8(outcomeCount, "outcomeCount");
    if (count < 2 || count > MAX_OUTCOMES) throw new Error("outcomeCount must be in 2..=16.");
    const output = new Uint8Array(139);
    output[0] = 1;
    output[1] = 3;
    output.set(fromHex(realm, 32, "realm"), 2);
    output.set(fromHex(profile, 32, "profile"), 34);
    writeLe(output, 66, nonce, 8);
    output[74] = count;
    output.set(fromHex(termsDigest, 32, "termsDigest"), 75);
    output.set(fromHex(feed, 32, "feed"), 107);
    return output;
  }

  async function buildMarketCreationPreview({
    termsAccountBytes,
    shapeCertificateBytes,
    marketNonce,
    observedCurrentSlot,
    expiresSlot
  }) {
    const terms = await inspectTermsAccount(termsAccountBytes);
    const certificate = await inspectShapeCertificate(shapeCertificateBytes);
    const basisBytes = encodeBasisSpec(terms.basisSpec);
    const basisDigest = await digestBasisSpec(basisBytes);
    if (certificate.termsDigest !== terms.termsDigest) throw new Error("Shape certificate is bound to a different Terms digest.");
    if (certificate.basisDigest !== hex(basisDigest) || !equal(encodeBasisSpec(certificate.basisSpec), basisBytes)) {
      throw new Error("Shape certificate is bound to a different native BasisSpec.");
    }
    const termsUpload = buildTermsArtifactUploadPlan({
      termsAccountBytes,
      realm: terms.realm,
      termsDigest: terms.termsDigest,
      observedCurrentSlot,
      expiresSlot
    });
    const createMarketIntent = encodeCreateMarketIntent({
      realm: terms.realm,
      profile: terms.profile,
      marketNonce,
      outcomeCount: terms.basisSpec.outcomeCount,
      termsDigest: terms.termsDigest,
      feed: terms.feed
    });
    return Object.freeze({
      schema: "dragon-clutch.native-bspline-market-creation.v1",
      mode: "offline-inspection-only",
      semanticMode: "native-bspline",
      termsDigest: terms.termsDigest,
      basisSpec: terms.basisSpec,
      basisSpecBytes: hex(basisBytes),
      basisSpecDigest: hex(basisDigest),
      shapeCertificateDigest: hex(await digestShapeCertificate(shapeCertificateBytes)),
      observedCurrentSlot: termsUpload.observedCurrentSlot,
      expiresSlot: termsUpload.expiresSlot,
      termsArtifactIntentBytes: [
        hex(termsUpload.beginIntent),
        ...termsUpload.writeIntents.map(hex),
        hex(termsUpload.sealIntent)
      ],
      createMarketIntentBytes: hex(createMarketIntent),
      authorization: { wallet: "not-connected", signer: "none", signature: null, submission: "disabled" },
      warning: "Compiler semantics require an exact Rust recompile; these unsigned bytes do not authorize or submit anything."
    });
  }

  root.DragonsClutchNativeBsplineV1 = Object.freeze({
    BASIS_BYTES,
    CERT_FIXED_BYTES,
    TERMS_BYTES,
    buildMarketCreationPreview,
    buildTermsArtifactUploadPlan,
    decodeBasisSpec,
    digestBasisSpec,
    digestShapeCertificate,
    encodeBasisSpec,
    encodeBeginTermsArtifactIntent,
    encodeCreateMarketIntent,
    encodeSealTermsArtifactIntent,
    encodeWriteTermsArtifactIntent,
    hex,
    inspectShapeCertificate,
    inspectTermsAccount,
    verifyTermsArtifactUploadPlan
  });
})(typeof globalThis === "object" ? globalThis : this);
