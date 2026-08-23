/*
 * Dependency-free counterpart of the Rust outer ProtocolTransactionBuilder.
 *
 * It accepts bytes from their semantic owner and compiles one legacy Solana
 * transaction with a zero recent blockhash and zero signatures. It owns no
 * action payload codec, wallet, keypair, blockhash acquisition, signing, or
 * submission path.
 */
(function (root) {
  "use strict";

  const UINT = /^(0|[1-9][0-9]*)$/;
  const HEX32 = /^[0-9a-f]{64}$/;
  const HEX = /^(?:[0-9a-f]{2})*$/;
  const U64_MAX = (1n << 64n) - 1n;
  const U128_MAX = (1n << 128n) - 1n;
  const MAX_SUCCESSOR_PAYLOAD_BYTES = 399;
  const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  const BASE58_INDEX = Object.freeze(Object.fromEntries(Array.from(BASE58_ALPHABET, (character, index) => [character, index])));
  const FLOW_FAMILIES = Object.freeze({
    "market-epoch-creation": Object.freeze(["general"]),
    "source-plane-v3": Object.freeze(["source"]),
    "general-v2-candidate": Object.freeze(["general"]),
    "general-v2-settlement": Object.freeze(["general"]),
    "general-v2-fees": Object.freeze(["general"]),
    "direct-egg-settlement": Object.freeze(["general"]),
    "product-series": Object.freeze(["series"]),
    "structured-claim": Object.freeze(["structured"]),
    "dealer-liquidity": Object.freeze(["dealer"]),
    "keeper-settlement": Object.freeze(["general"]),
    "recovery-retirement": Object.freeze(["general", "recovery"])
  });

  const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  const requirePlain = (value, name) => {
    if (!plain(value)) throw new Error(`${name} must be an object.`);
    return value;
  };
  const requireText = (value, name, maximum = 128) => {
    if (typeof value !== "string" || value.trim() !== value || value.length === 0 || value.length > maximum) {
      throw new Error(`${name} must be nonempty, trimmed text no longer than ${maximum} characters.`);
    }
    return value;
  };
  const decimal = (value, name, maximum) => {
    if (typeof value !== "string" || !UINT.test(value)) throw new Error(`${name} must be a canonical decimal string.`);
    const parsed = BigInt(value);
    if (parsed > maximum) throw new Error(`${name} exceeds its exact integer width.`);
    return parsed;
  };
  const hex32 = (value, name) => {
    if (typeof value !== "string" || !HEX32.test(value) || /^0+$/.test(value)) throw new Error(`${name} must be a nonzero lowercase 32-byte hexadecimal identity.`);
    return value;
  };
  const byteHex = (value, name, maximumBytes = MAX_SUCCESSOR_PAYLOAD_BYTES) => {
    if (typeof value !== "string" || !HEX.test(value)) throw new Error(`${name} must be lowercase, even-length hexadecimal bytes.`);
    if (value.length / 2 > maximumBytes) throw new Error(`${name} exceeds ${maximumBytes} bytes.`);
    const output = [];
    for (let index = 0; index < value.length; index += 2) output.push(Number.parseInt(value.slice(index, index + 2), 16));
    return output;
  };

  const encodeBase58 = (bytes) => {
    let value = 0n;
    for (const byte of bytes) value = value * 256n + BigInt(byte);
    let encoded = "";
    while (value > 0n) {
      const remainder = Number(value % 58n);
      encoded = BASE58_ALPHABET[remainder] + encoded;
      value /= 58n;
    }
    let leading = 0;
    while (leading < bytes.length && bytes[leading] === 0) leading += 1;
    return "1".repeat(leading) + (encoded || (leading === 0 ? "1" : ""));
  };

  const decodeBase58 = (value, name = "address") => {
    if (typeof value !== "string" || value.length < 32 || value.length > 44) throw new Error(`${name} must be a canonical base58 Solana address.`);
    let decoded = 0n;
    for (const character of value) {
      const digit = BASE58_INDEX[character];
      if (digit === undefined) throw new Error(`${name} contains a non-base58 character.`);
      decoded = decoded * 58n + BigInt(digit);
    }
    const output = new Uint8Array(32);
    for (let index = 31; index >= 0; index -= 1) {
      output[index] = Number(decoded & 0xffn);
      decoded >>= 8n;
    }
    if (decoded !== 0n || encodeBase58(output) !== value) throw new Error(`${name} is not a canonical 32-byte base58 address.`);
    return output;
  };

  const compareBytes = (left, right) => {
    for (let index = 0; index < left.length; index += 1) {
      if (left[index] !== right[index]) return left[index] - right[index];
    }
    return 0;
  };
  const bytesHex = (bytes) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  const append = (output, values) => { for (const value of values) output.push(value); };
  const shortVector = (value) => {
    if (!Number.isInteger(value) || value < 0 || value > 0xffff) throw new Error("Solana short-vector length exceeds u16.");
    const output = [];
    let remaining = value;
    do {
      let byte = remaining & 0x7f;
      remaining >>= 7;
      if (remaining > 0) byte |= 0x80;
      output.push(byte);
    } while (remaining > 0);
    return output;
  };
  const base64 = (bytes) => {
    const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let output = "";
    for (let index = 0; index < bytes.length; index += 3) {
      const a = bytes[index];
      const hasB = index + 1 < bytes.length;
      const hasC = index + 2 < bytes.length;
      const b = hasB ? bytes[index + 1] : 0;
      const c = hasC ? bytes[index + 2] : 0;
      output += alphabet[a >> 2];
      output += alphabet[((a & 3) << 4) | (b >> 4)];
      output += hasB ? alphabet[((b & 15) << 2) | (c >> 6)] : "=";
      output += hasC ? alphabet[c & 63] : "=";
    }
    return output;
  };

  const exactKeys = (value, expected, name) => {
    requirePlain(value, name);
    const keys = Object.keys(value);
    if (keys.length !== expected.length || expected.some((key) => !keys.includes(key))) throw new Error(`${name} must contain exactly ${expected.join(", ")}.`);
    return value;
  };
  const nonzeroAddress = (value, name) => {
    requireText(value, name, 44);
    const decoded = decodeBase58(value, name);
    if (decoded.every((byte) => byte === 0)) throw new Error(`${name} must be a nonzero address.`);
    return value;
  };
  const normalizeUnit = (raw, name) => {
    requirePlain(raw, name);
    switch (raw.kind) {
      case "lamports":
        exactKeys(raw, ["kind"], name);
        return Object.freeze({ kind: raw.kind });
      case "collateral-atoms":
      case "fee-atoms":
      case "wrapper-atoms":
        exactKeys(raw, ["kind", "mint"], name);
        return Object.freeze({ kind: raw.kind, mint: nonzeroAddress(raw.mint, `${name}.mint`) });
      case "price-units": {
        exactKeys(raw, ["kind", "scale"], name);
        const scale = decimal(raw.scale, `${name}.scale`, U64_MAX);
        if (scale === 0n) throw new Error(`${name}.scale must be positive.`);
        return Object.freeze({ kind: raw.kind, scale: scale.toString() });
      }
      case "egg-atoms":
        exactKeys(raw, ["kind", "marketId", "outcome"], name);
        return Object.freeze({
          kind: raw.kind,
          marketId: hex32(raw.marketId, `${name}.marketId`),
          outcome: decimal(raw.outcome, `${name}.outcome`, 255n).toString()
        });
      default:
        throw new Error(`${name}.kind is not a canonical exact-integer unit.`);
    }
  };

  const normalizeEquation = (raw, index, instructionIndex) => {
    exactKeys(raw, ["name", "unit", "left", "right"], `instructions[${instructionIndex}].equations[${index}]`);
    const name = requireText(raw.name, `equation[${index}].name`, 96);
    const unit = normalizeUnit(raw.unit, `equation[${index}].unit`);
    const left = decimal(raw.left, `equation[${index}].left`, U128_MAX);
    const right = decimal(raw.right, `equation[${index}].right`, U128_MAX);
    if (left !== right) throw new Error(`Equation ${JSON.stringify(name)} is not exactly balanced.`);
    return Object.freeze({ name, unit, left: left.toString(), right: right.toString() });
  };

  const normalizeOwner = (raw, instructionIndex) => {
    exactKeys(raw, ["package", "schema", "releaseSha256"], `instructions[${instructionIndex}].semanticOwner`);
    return Object.freeze({
      package: requireText(raw.package, "semanticOwner.package", 120),
      schema: requireText(raw.schema, "semanticOwner.schema", 160),
      releaseSha256: hex32(raw.releaseSha256, "semanticOwner.releaseSha256")
    });
  };

  const normalizeInstruction = (raw, index, programId) => {
    requirePlain(raw, `instructions[${index}]`);
    const expectedInstructionKeys = ["flow", "family", "familyTag", "familyVersion", "localAction", "payloadHex", "semanticOwner", "accounts", "requiredSigners", "equations"];
    if (raw.actionName !== undefined) expectedInstructionKeys.push("actionName");
    exactKeys(raw, expectedInstructionKeys, `instructions[${index}]`);
    const flow = requireText(raw.flow, `instructions[${index}].flow`, 64);
    if (!FLOW_FAMILIES[flow]) throw new Error(`instructions[${index}].flow is not a canonical outer-builder flow.`);
    const familyName = requireText(raw.family, `instructions[${index}].family`, 24);
    if (!FLOW_FAMILIES[flow].includes(familyName)) throw new Error(`${familyName} does not own the ${flow} flow.`);
    const localActionValue = decimal(raw.localAction, `instructions[${index}].localAction`, 255n);
    if (localActionValue === 0n) throw new Error("localAction must be positive.");
    const familyTag = decimal(raw.familyTag, `instructions[${index}].familyTag`, 255n);
    const familyVersion = decimal(raw.familyVersion, `instructions[${index}].familyVersion`, 255n);
    if (familyTag === 0n || familyVersion === 0n) throw new Error("familyTag and familyVersion must be positive bytes.");
    const actionName = raw.actionName === undefined ? null : requireText(raw.actionName, `instructions[${index}].actionName`, 96);
    const coordinate = Object.freeze({
      family: familyName,
      tag: Number(familyTag),
      version: Number(familyVersion),
      localAction: Number(localActionValue),
      actionName,
      source: "explicit-semantic-owner-draft; not runtime capability admission"
    });
    const payload = byteHex(raw.payloadHex, `instructions[${index}].payloadHex`);
    const data = Uint8Array.from([coordinate.tag, coordinate.version, coordinate.localAction, ...payload]);
    if (!Array.isArray(raw.accounts) || raw.accounts.length > 64) throw new Error(`instructions[${index}].accounts must be an array of at most 64 metas.`);
    const seen = new Set();
    const accounts = raw.accounts.map((meta, metaIndex) => {
      exactKeys(meta, ["address", "isSigner", "isWritable"], `instructions[${index}].accounts[${metaIndex}]`);
      const address = requireText(meta.address, `account meta ${metaIndex}.address`, 44);
      const addressBytes = decodeBase58(address, `account meta ${metaIndex}.address`);
      if (address === programId) throw new Error("The invoked program cannot also be supplied as an account meta.");
      if (seen.has(address)) throw new Error(`Instruction ${index} aliases account ${address}.`);
      seen.add(address);
      if (typeof meta.isSigner !== "boolean" || typeof meta.isWritable !== "boolean") throw new Error(`Account meta ${metaIndex} must declare boolean isSigner and isWritable.`);
      return Object.freeze({ address, addressBytes, isSigner: meta.isSigner, isWritable: meta.isWritable });
    });
    if (!Array.isArray(raw.requiredSigners)) throw new Error(`instructions[${index}].requiredSigners must be an array.`);
    const signerSet = new Set();
    const requiredSigners = raw.requiredSigners.map((address, signerIndex) => {
      requireText(address, `requiredSigners[${signerIndex}]`, 44);
      decodeBase58(address, `requiredSigners[${signerIndex}]`);
      if (signerSet.has(address)) throw new Error(`Instruction ${index} repeats required signer ${address}.`);
      signerSet.add(address);
      return address;
    });
    for (const meta of accounts) {
      if (meta.isSigner && !signerSet.has(meta.address)) throw new Error(`Signer meta ${meta.address} is absent from requiredSigners.`);
      if (!meta.isSigner && signerSet.has(meta.address)) throw new Error(`Required signer ${meta.address} is not a signer meta.`);
    }
    if (!Array.isArray(raw.equations) || raw.equations.length === 0 || raw.equations.length > 64) throw new Error(`instructions[${index}].equations must contain 1..64 exact equations.`);
    const equations = raw.equations.map((equation, equationIndex) => normalizeEquation(equation, equationIndex, index));
    return Object.freeze({
      flow,
      actionName,
      semanticOwner: normalizeOwner(raw.semanticOwner, index),
      coordinate,
      accounts: Object.freeze(accounts),
      requiredSigners: Object.freeze(requiredSigners),
      equations: Object.freeze(equations),
      data
    });
  };

  const compileMessage = (payer, payerBytes, programId, programBytes, instructions) => {
    const keyMap = new Map();
    const mergeKey = (address, bytes, isSigner, isWritable, isInvoked) => {
      const key = bytesHex(bytes);
      const previous = keyMap.get(key);
      keyMap.set(key, previous ? {
        address,
        bytes,
        isSigner: previous.isSigner || isSigner,
        isWritable: previous.isWritable || isWritable,
        isInvoked: previous.isInvoked || isInvoked
      } : { address, bytes, isSigner, isWritable, isInvoked });
    };
    mergeKey(programId, programBytes, false, false, true);
    for (const instruction of instructions) {
      for (const meta of instruction.accounts) mergeKey(meta.address, meta.addressBytes, meta.isSigner, meta.isWritable, false);
    }
    mergeKey(payer, payerBytes, true, true, false);
    const payerKey = bytesHex(payerBytes);
    const rest = Array.from(keyMap.entries()).filter(([key]) => key !== payerKey).map(([, value]) => value);
    const sort = (values) => values.sort((left, right) => compareBytes(left.bytes, right.bytes));
    const writableSigners = sort(rest.filter((key) => key.isSigner && key.isWritable));
    const readonlySigners = sort(rest.filter((key) => key.isSigner && !key.isWritable));
    const writableUnsigned = sort(rest.filter((key) => !key.isSigner && key.isWritable));
    const readonlyUnsigned = sort(rest.filter((key) => !key.isSigner && !key.isWritable));
    const accountKeys = [{ address: payer, bytes: payerBytes, isSigner: true, isWritable: true, isInvoked: false }, ...writableSigners, ...readonlySigners, ...writableUnsigned, ...readonlyUnsigned];
    if (accountKeys.length > 256) throw new Error("Compiled Solana message exceeds 256 account keys.");
    const keyIndexes = new Map(accountKeys.map((key, index) => [key.address, index]));
    const compiled = instructions.map((instruction) => ({
      programIdIndex: keyIndexes.get(programId),
      accountIndexes: instruction.accounts.map((meta) => keyIndexes.get(meta.address)),
      data: instruction.data
    }));
    const header = Object.freeze({
      numRequiredSignatures: 1 + writableSigners.length + readonlySigners.length,
      numReadonlySignedAccounts: readonlySigners.length,
      numReadonlyUnsignedAccounts: readonlyUnsigned.length
    });
    const message = [header.numRequiredSignatures, header.numReadonlySignedAccounts, header.numReadonlyUnsignedAccounts];
    append(message, shortVector(accountKeys.length));
    for (const key of accountKeys) append(message, key.bytes);
    append(message, new Uint8Array(32));
    append(message, shortVector(compiled.length));
    for (const instruction of compiled) {
      message.push(instruction.programIdIndex);
      append(message, shortVector(instruction.accountIndexes.length));
      append(message, instruction.accountIndexes);
      append(message, shortVector(instruction.data.length));
      append(message, instruction.data);
    }
    return Object.freeze({ header, accountKeys: Object.freeze(accountKeys), compiled: Object.freeze(compiled), bytes: Uint8Array.from(message) });
  };

  const build = (raw, configuration, transportLimit) => {
    exactKeys(raw, ["payer", "instructions"], "construction draft");
    requirePlain(configuration, "release configuration");
    requirePlain(configuration.release, "release configuration.release");
    const programId = requireText(configuration.release.programId, "release programId", 44);
    const programBytes = decodeBase58(programId, "release programId");
    if (programBytes.every((byte) => byte === 0)) throw new Error("release programId must be nonzero.");
    const payer = requireText(raw.payer, "payer", 44);
    const payerBytes = decodeBase58(payer, "payer");
    if (payerBytes.every((byte) => byte === 0)) throw new Error("payer must be nonzero.");
    if (payer === programId) throw new Error("Payer and invoked program must be different addresses.");
    const packetLimit = decimal(transportLimit, "packet limit", 65535n);
    if (packetLimit === 0n) throw new Error("Packet limit must be positive.");
    if (!Array.isArray(raw.instructions) || raw.instructions.length === 0 || raw.instructions.length > 16) throw new Error("construction draft must contain 1..16 instructions.");
    const instructions = raw.instructions.map((instruction, index) => normalizeInstruction(instruction, index, programId));
    const explicitSignerSet = new Set([payer]);
    for (const instruction of instructions) {
      for (const signer of instruction.requiredSigners) {
        const represented = signer === payer || instruction.accounts.some((meta) => meta.address === signer && meta.isSigner);
        if (!represented) throw new Error(`Required signer ${signer} has no signer account role.`);
        explicitSignerSet.add(signer);
      }
    }
    const message = compileMessage(payer, payerBytes, programId, programBytes, instructions);
    const messageSigners = message.accountKeys.slice(0, message.header.numRequiredSignatures).map((key) => key.address);
    for (const signer of messageSigners) {
      if (!explicitSignerSet.has(signer)) throw new Error(`Compiled signer ${signer} is absent from explicit requiredSigners.`);
    }
    const transaction = [];
    append(transaction, shortVector(message.header.numRequiredSignatures));
    append(transaction, new Uint8Array(message.header.numRequiredSignatures * 64));
    append(transaction, message.bytes);
    if (BigInt(transaction.length) > packetLimit) throw new Error(`Serialized unsigned transaction is ${transaction.length} bytes, exceeding packet limit ${packetLimit}.`);
    const transactionBytes = Uint8Array.from(transaction);
    const uniqueFlows = [];
    for (const instruction of instructions) if (!uniqueFlows.includes(instruction.flow)) uniqueFlows.push(instruction.flow);
    const allEquations = instructions.flatMap((instruction) => instruction.equations);
    const release = Object.freeze({
      clusterKey: configuration.clusterKey,
      programId,
      programData: configuration.release.programData,
      deploymentSlot: configuration.release.deploymentSlot,
      elfSha256: configuration.release.elfSha256,
      releaseManifestSha256: configuration.release.releaseManifestSha256,
      sourceCommit: configuration.release.sourceCommit,
      capabilityProfileId: configuration.release.capabilityProfileId,
      sourceProfile: configuration.release.sourceProfile,
      registeredSourceReleaseCount: configuration.release.registeredSourceReleaseCount
    });
    return Object.freeze({
      schema: "dragons-clutch/operator/unsigned-protocol-transaction/v4",
      authority: "local-construction-from-explicit-semantic-owner-material",
      release,
      flows: Object.freeze(uniqueFlows),
      actions: Object.freeze(instructions.map((instruction) => instruction.actionName)),
      semanticOwners: Object.freeze(instructions.map((instruction) => instruction.semanticOwner)),
      instructionCoordinates: Object.freeze(instructions.map((instruction) => Object.freeze({
        family: instruction.coordinate.family,
        familyTag: String(instruction.coordinate.tag),
        familyVersion: String(instruction.coordinate.version),
        localAction: String(instruction.coordinate.localAction),
        actionName: instruction.coordinate.actionName,
        source: instruction.coordinate.source
      }))),
      runtimeCapability: "not-authenticated",
      disabledReason: "No release-authenticated runtime capability verdict was supplied; explicit draft coordinates are construction material only.",
      requiredSigners: Object.freeze(messageSigners),
      exactEquations: Object.freeze(allEquations),
      serializedTransactionEncoding: "base64",
      serializedTransaction: base64(transactionBytes),
      serializedTransactionHex: bytesHex(transactionBytes),
      serializedTransactionBytes: String(transactionBytes.length),
      packetLimitBytes: packetLimit.toString(),
      message: Object.freeze({
        version: "legacy",
        header: Object.freeze({
          numRequiredSignatures: String(message.header.numRequiredSignatures),
          numReadonlySignedAccounts: String(message.header.numReadonlySignedAccounts),
          numReadonlyUnsignedAccounts: String(message.header.numReadonlyUnsignedAccounts)
        }),
        accountKeys: Object.freeze(message.accountKeys.map((key) => key.address)),
        recentBlockhash: "11111111111111111111111111111111",
        instructionCount: String(instructions.length)
      }),
      hasRecentBlockhash: false,
      signed: false,
      submitted: false
    });
  };

  root.GlassSuccessorBuilder = Object.freeze({ build, decodeBase58, encodeBase58, bytesHex });
})(typeof globalThis === "object" ? globalThis : this);
