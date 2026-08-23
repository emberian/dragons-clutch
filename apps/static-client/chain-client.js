/*
 * Bounded GET-only client for the operatord fork-aware index projection.
 *
 * It never calls a validator RPC directly. URLs, release coordinates, and all
 * returned state are untrusted inputs checked against explicit user-selected
 * bounds before they become a visible projection.
 */
(function (root) {
  "use strict";

  const BUILDER = root.GlassSuccessorBuilder;
  const REGISTRY = root.GlassSuccessorRegistry;
  const UINT = /^(0|[1-9][0-9]*)$/;
  const HASH32 = /^[0-9a-f]{64}$/;
  const COMMIT = /^[0-9a-f]{40}$/;
  const U64_MAX = (1n << 64n) - 1n;
  const GROUP_ORDER = Object.freeze(["market", "product", "source", "series", "candidate", "settlement", "liquidity", "recovery", "other"]);
  const GROUP_LABELS = Object.freeze({
    market: "Market",
    product: "Product",
    source: "Source",
    series: "Series",
    candidate: "Candidate & clearing",
    settlement: "Settlement & positions",
    liquidity: "Covered liquidity",
    recovery: "Recovery",
    other: "Other release state"
  });
  const KIND_GROUPS = Object.freeze({
    "general-market-runtime": "market",
    "general-epoch": "market",
    "general-economic-domain": "market",
    "general-market-binding": "market",
    "general-candidate-window": "market",
    "series-registry": "product",
    "structured-claim-descriptor": "product",
    "product-capability-registry": "product",
    "product-capability-registry-v2": "product",
    "compiled-product-series-bundle": "product",
    "product-compiler-output": "product",
    "product-artifact": "product",
    "series-funding": "series",
    "general-admission-node": "candidate",
    "general-candidate-feed-stage": "candidate",
    "general-candidate-feed": "candidate",
    "general-clear-work": "candidate",
    "general-selected-candidate": "candidate",
    "general-epoch-budget": "candidate",
    "general-owner-settlement": "settlement",
    "general-owner-settlement-v3": "settlement",
    "owner-settlement-v3": "settlement",
    "general-settlement-cash-pot": "settlement",
    "general-final-pot": "settlement",
    "fee-selected-record": "settlement",
    "fee-owner-carry": "settlement",
    "fee-payer-allocation": "settlement",
    "fee-recipient-allocation": "settlement",
    "fee-treasury-ledger": "settlement",
    "liveness-policy": "settlement",
    "liveness-compartment": "settlement",
    "position-v3": "settlement",
    "replay-v3": "settlement"
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
  const address = (value, name) => {
    text(value, name, 44);
    BUILDER.decodeBase58(value, name);
    return value;
  };
  const nonzeroAddress = (value, name) => {
    const canonical = address(value, name);
    const decoded = BUILDER.decodeBase58(canonical, name);
    if (decoded.every((byte) => byte === 0)) throw new Error(`${name} must be a nonzero 32-byte base58 identity.`);
    return canonical;
  };
  const bool = (value, name) => {
    if (typeof value !== "boolean") throw new Error(`${name} must be boolean.`);
    return value;
  };

  const boundedUrl = (value, name, schemes) => {
    text(value, name, 512);
    let parsed;
    try { parsed = new URL(value); } catch (_) { throw new Error(`${name} must be an absolute URL.`); }
    if (!schemes.includes(parsed.protocol)) throw new Error(`${name} uses a disallowed URL scheme.`);
    if (parsed.username || parsed.password || parsed.hash || parsed.search) throw new Error(`${name} must not contain credentials, a query, or a fragment.`);
    const loopback = parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost";
    if ((parsed.protocol === "http:" || parsed.protocol === "ws:") && !loopback) throw new Error(`${name} may use plaintext transport only on explicit loopback.`);
    return parsed.toString().replace(/\/$/, "");
  };

  const validateConfiguration = (raw) => {
    requirePlain(raw, "configuration");
    const operatorUrl = boundedUrl(raw.operatorUrl, "operatord URL", ["http:", "https:"]);
    const clusterName = text(raw.clusterName, "cluster name", 48);
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(clusterName)) throw new Error("cluster name contains unsupported characters.");
    const genesisHash = nonzeroAddress(raw.genesisHash, "genesis hash");
    const rpcHttpUrl = boundedUrl(raw.rpcHttpUrl, "validator RPC HTTP URL", ["http:", "https:"]);
    const rpcWebsocketUrl = boundedUrl(raw.rpcWebsocketUrl, "validator RPC WebSocket URL", ["ws:", "wss:"]);
    if (raw.commitment !== "finalized" && raw.commitment !== "processed") throw new Error("commitment must be finalized or processed.");
    requirePlain(raw.release, "release");
    const programId = nonzeroAddress(raw.release.programId, "release.programId");
    const programData = nonzeroAddress(raw.release.programData, "release.programData");
    if (programId === programData) throw new Error("Program and ProgramData addresses must be different.");
    const deploymentSlot = positiveDecimal(raw.release.deploymentSlot, "release.deploymentSlot").toString();
    const elfSha256 = hash32(raw.release.elfSha256, "release.elfSha256");
    const releaseManifestSha256 = hash32(raw.release.releaseManifestSha256, "release.releaseManifestSha256");
    const capabilityProfileId = hash32(raw.release.capabilityProfileId, "release.capabilityProfileId");
    if (typeof raw.release.sourceCommit !== "string" || !COMMIT.test(raw.release.sourceCommit)) throw new Error("release.sourceCommit must be a full lowercase git commit identity.");
    requirePlain(raw.bounds, "bounds");
    const maximumAccounts = positiveDecimal(raw.bounds.maximumAccounts, "bounds.maximumAccounts", 4096n).toString();
    const maximumResponseBytes = positiveDecimal(raw.bounds.maximumResponseBytes, "bounds.maximumResponseBytes", 16_777_216n);
    if (maximumResponseBytes < 4096n) throw new Error("bounds.maximumResponseBytes must be at least 4096.");
    const timeoutMilliseconds = positiveDecimal(raw.bounds.timeoutMilliseconds, "bounds.timeoutMilliseconds", 30000n);
    if (timeoutMilliseconds < 250n) throw new Error("bounds.timeoutMilliseconds must be at least 250.");
    const maximumSlotLag = decimal(raw.bounds.maximumSlotLag, "bounds.maximumSlotLag", 1_000_000n).toString();
    const clusterKey = `${clusterName}:${genesisHash}`;
    const releaseKey = `${programId}:${deploymentSlot}:${elfSha256}`;
    return Object.freeze({
      schema: "dragons-clutch/browser-chain-target/v1",
      authority: "explicit-user-selection",
      operatorUrl,
      clusterName,
      genesisHash,
      clusterKey,
      rpcHttpUrl,
      rpcWebsocketUrl,
      rpcContact: "operatord-only; browser does not call validator RPC",
      commitment: raw.commitment,
      release: Object.freeze({ programId, programData, deploymentSlot, elfSha256, releaseManifestSha256, sourceCommit: raw.release.sourceCommit, capabilityProfileId, releaseKey }),
      bounds: Object.freeze({ maximumAccounts, maximumResponseBytes: maximumResponseBytes.toString(), timeoutMilliseconds: timeoutMilliseconds.toString(), maximumSlotLag })
    });
  };

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

  const validateAcquisition = (raw, configuration) => {
    requirePlain(raw, "acquisition");
    if (raw.authority !== "untrusted read model") throw new Error("acquisition authority is not the expected untrusted read model.");
    if (bool(raw.authorityEligible, "acquisition.authorityEligible") !== false) throw new Error("operatord incorrectly marks an untrusted acquisition as authority-eligible.");
    const transportMode = text(raw.transportMode, "acquisition.transportMode", 80);
    if (transportMode !== "finalized-plus-processed-websocket") throw new Error("operatord is not serving the required finalized-plus-processed WebSocket transport.");
    const processedAvailable = bool(raw.processedAvailable, "acquisition.processedAvailable");
    requirePlain(raw.transportBinding, "acquisition.transportBinding");
    const binding = raw.transportBinding;
    if (binding.schema !== "dragons-clutch/operator-rpc-transport-binding/v1"
        || binding.verificationDisposition !== "last-complete-untrusted-http-release-bracket"
        || binding.authorityEligible !== false
        || text(binding.clusterName, "transportBinding.clusterName", 48) !== configuration.clusterName
        || nonzeroAddress(binding.genesisHash, "transportBinding.genesisHash") !== configuration.genesisHash
        || text(binding.clusterKey, "transportBinding.clusterKey", 128) !== configuration.clusterKey
        || boundedUrl(binding.rpcHttpUrl, "transportBinding.rpcHttpUrl", ["http:", "https:"]) !== configuration.rpcHttpUrl
        || boundedUrl(binding.rpcWebsocketUrl, "transportBinding.rpcWebsocketUrl", ["ws:", "wss:"]) !== configuration.rpcWebsocketUrl) {
      throw new Error("operatord transport coordinates differ from the exact selected HTTP+WebSocket RPC target.");
    }
    if (!Array.isArray(binding.releases) || binding.releases.length === 0 || binding.releases.length > 256) throw new Error("transportBinding.releases is invalid.");
    const boundRelease = binding.releases.find((release) => plain(release) && release.releaseKey === configuration.release.releaseKey);
    if (!boundRelease
        || nonzeroAddress(boundRelease.programId, "transportBinding.release.programId") !== configuration.release.programId
        || nonzeroAddress(boundRelease.programData, "transportBinding.release.programData") !== configuration.release.programData
        || positiveDecimal(boundRelease.deploymentSlot, "transportBinding.release.deploymentSlot").toString() !== configuration.release.deploymentSlot
        || hash32(boundRelease.elfSha256, "transportBinding.release.elfSha256") !== configuration.release.elfSha256
        || !Array.isArray(boundRelease.families)) {
      throw new Error("operatord release coordinates differ from the exact selected Program/ProgramData/slot/ELF target.");
    }
    requirePlain(raw.processedSemantics, "acquisition.processedSemantics");
    if (raw.processedSemantics.finality !== "nonfinal-rollbackable" || raw.processedSemantics.authorityEligibility !== false) throw new Error("operatord does not expose the required non-final, never-authoritative processed semantics.");
    const processedTransport = raw.processedTransport === null ? null : (() => {
      requirePlain(raw.processedTransport, "acquisition.processedTransport");
      return Object.freeze({
        phase: text(raw.processedTransport.phase, "processedTransport.phase", 80),
        available: bool(raw.processedTransport.available, "processedTransport.available"),
        connectionGeneration: decimal(raw.processedTransport.connectionGeneration, "processedTransport.connectionGeneration").toString(),
        rollbackEpoch: decimal(raw.processedTransport.rollbackEpoch, "processedTransport.rollbackEpoch").toString(),
        reconnectAttempt: decimal(raw.processedTransport.reconnectAttempt, "processedTransport.reconnectAttempt").toString(),
        nextBackoffMilliseconds: decimal(raw.processedTransport.nextBackoffMilliseconds, "processedTransport.nextBackoffMilliseconds").toString(),
        reconnectIndexedVersionsWithdrawn: decimal(raw.processedTransport.reconnectIndexedVersionsWithdrawn, "processedTransport.reconnectIndexedVersionsWithdrawn").toString(),
        reconnectBufferedAccountsWithdrawn: decimal(raw.processedTransport.reconnectBufferedAccountsWithdrawn, "processedTransport.reconnectBufferedAccountsWithdrawn").toString(),
        deadSlotRollbacks: decimal(raw.processedTransport.deadSlotRollbacks, "processedTransport.deadSlotRollbacks").toString(),
        deadSlotIndexedVersionsWithdrawn: decimal(raw.processedTransport.deadSlotIndexedVersionsWithdrawn, "processedTransport.deadSlotIndexedVersionsWithdrawn").toString(),
        deadSlotBufferedAccountsWithdrawn: decimal(raw.processedTransport.deadSlotBufferedAccountsWithdrawn, "processedTransport.deadSlotBufferedAccountsWithdrawn").toString(),
        lastRollbackSlot: raw.processedTransport.lastRollbackSlot === null ? null : decimal(raw.processedTransport.lastRollbackSlot, "processedTransport.lastRollbackSlot").toString(),
        lastError: raw.processedTransport.lastError === null ? null : text(raw.processedTransport.lastError, "processedTransport.lastError", 512)
      });
    })();
    if (processedTransport === null || processedTransport.available !== processedAvailable) {
      throw new Error("operatord processed availability differs from its exact transport-generation state.");
    }
    if (configuration.commitment === "processed" && !processedAvailable) {
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
      transportBinding: Object.freeze({ rpcHttpUrl: configuration.rpcHttpUrl, rpcWebsocketUrl: configuration.rpcWebsocketUrl, releaseKey: configuration.release.releaseKey })
    });
  };

  const validateReleaseResponse = (raw, configuration) => {
    requirePlain(raw, "releases response");
    if (raw.cluster !== configuration.clusterKey || raw.authorityEligible !== false || !Array.isArray(raw.releases) || raw.releases.length > 256) throw new Error("release response does not match the selected cluster, trust boundary, or bounds.");
    const releases = raw.releases.map((release, index) => {
      requirePlain(release, `releases[${index}]`);
      if (!Array.isArray(release.families) || release.families.length === 0 || release.families.length > 16) throw new Error(`releases[${index}].families is invalid.`);
      return Object.freeze({
        releaseKey: text(release.releaseKey, `releases[${index}].releaseKey`, 256),
        programId: address(release.programId, `releases[${index}].programId`),
        programData: address(release.programData, `releases[${index}].programData`),
        elfSha256: hash32(release.elfSha256, `releases[${index}].elfSha256`),
        deploymentSlot: positiveDecimal(release.deploymentSlot, `releases[${index}].deploymentSlot`).toString(),
        families: Object.freeze(release.families.map((family, familyIndex) => text(family, `releases[${index}].families[${familyIndex}]`, 40)))
      });
    });
    const selected = releases.find((release) => release.releaseKey === configuration.release.releaseKey);
    if (!selected) throw new Error("operatord does not expose the explicitly selected release key.");
    if (selected.programId !== configuration.release.programId || selected.programData !== configuration.release.programData || selected.elfSha256 !== configuration.release.elfSha256 || selected.deploymentSlot !== configuration.release.deploymentSlot) {
      throw new Error("operatord release coordinates differ from the explicit program/ProgramData/slot/ELF selection.");
    }
    return Object.freeze({ cluster: raw.cluster, selected, observedReleaseCount: String(releases.length) });
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
    return Object.freeze({
      address: address(raw.address, `accounts[${index}].address`),
      owner: address(raw.owner, `accounts[${index}].owner`),
      releaseKey: text(raw.releaseKey, `accounts[${index}].releaseKey`, 256),
      slot: decimal(raw.slot, `accounts[${index}].slot`).toString(),
      observedCommitment: raw.observedCommitment === "processed" || raw.observedCommitment === "finalized" ? raw.observedCommitment : (() => { throw new Error(`accounts[${index}].observedCommitment is invalid.`); })(),
      effectiveCommitment,
      lamports: decimal(raw.lamports, `accounts[${index}].lamports`).toString(),
      rentEpoch: decimal(raw.rentEpoch, `accounts[${index}].rentEpoch`).toString(),
      dataBytes: decimal(raw.dataBytes, `accounts[${index}].dataBytes`).toString(),
      dataSha256: hash32(raw.dataSha256, `accounts[${index}].dataSha256`),
      family: text(raw.family, `accounts[${index}].family`, 40),
      kind: text(raw.kind, `accounts[${index}].kind`, 80),
      decode: Object.freeze({ status: raw.decode.status, requirement }),
      generation: raw.generation === null ? null : decimal(raw.generation, `accounts[${index}].generation`).toString(),
      primaryBinding: raw.primaryBinding === null ? null : hash32(raw.primaryBinding, `accounts[${index}].primaryBinding`),
      secondaryBinding: raw.secondaryBinding === null ? null : hash32(raw.secondaryBinding, `accounts[${index}].secondaryBinding`),
      branch: validateBranch(raw.branch, `accounts[${index}].branch`)
    });
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

  const maximumSlot = (values) => values.reduce((maximum, value) => value > maximum ? value : maximum, 0n);
  const accountGroup = (kind) => {
    if (KIND_GROUPS[kind]) return KIND_GROUPS[kind];
    if (kind.startsWith("source-")) return "source";
    if (kind.startsWith("dealer-")) return "liquidity";
    if (kind.startsWith("failure-")) return "recovery";
    return "other";
  };

  const deriveSnapshot = (configuration, health, acquisition, releases, accountResponse, keeperActions, forks, remainingResponseBytes) => {
    const accounts = accountResponse.selected;
    const candidateSlots = forks.nodes.map((node) => BigInt(node.slot)).concat(accounts.map((accountValue) => BigInt(accountValue.slot)));
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
    const familySet = new Set(releases.selected.families);
    const successorCapabilities = Object.values(REGISTRY.families).map((familyValue) => Object.freeze({
      surface: "successor-family",
      family: familyValue.name,
      label: familyValue.label,
      indexedByRelease: familySet.has(familyValue.operatorFamily),
      allocationStatus: familyValue.allocationStatus,
      enabled: false,
      reason: familyValue.disabledReason || (familySet.has(familyValue.operatorFamily)
        ? "The selected release declares this decoder family, but the central allocation is ReservedDisabled and operatord exposes no release-bound capability admission."
        : `The selected release does not declare the ${familyValue.operatorFamily} decoder family, and the central allocation is ReservedDisabled.`)
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
    return Object.freeze({
      schema: "dragons-clutch/operatord-chain-projection/v1",
      authority: "untrusted-projection",
      configuration,
      health,
      acquisition,
      release: Object.freeze({
        observed: releases.selected,
        declaredManifestSha256: configuration.release.releaseManifestSha256,
        declaredSourceCommit: configuration.release.sourceCommit,
        declaredCapabilityProfileId: configuration.release.capabilityProfileId,
        manifestSourceCapabilityAuthentication: "not exposed by current operatord read API"
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
      capabilities,
      forks
    });
  };

  const acquire = async (configuration, fetchFunction = root.fetch.bind(root)) => {
    const reader = new BoundedGetReader(configuration, fetchFunction);
    const healthRaw = await reader.get("/v1/health");
    const health = validateHealth(healthRaw, configuration);
    const acquisition = validateAcquisition(await reader.get("/v1/acquisition"), configuration);
    const releases = validateReleaseResponse(await reader.get("/v1/releases"), configuration);
    const accounts = validateAccountsResponse(await reader.get(`/v1/accounts?commitment=${configuration.commitment}`), configuration);
    const keeper = validateKeeper(await reader.get(`/v1/keeper/next?commitment=${configuration.commitment}`), configuration);
    const forks = validateForks(await reader.get("/v1/forks"));
    const endAcquisition = validateAcquisition(await reader.get("/v1/acquisition"), configuration);
    if (configuration.commitment === "processed"
        && (endAcquisition.processedTransport.connectionGeneration !== acquisition.processedTransport.connectionGeneration
          || endAcquisition.processedTransport.rollbackEpoch !== acquisition.processedTransport.rollbackEpoch)) {
      throw new Error("Processed transport generation or rollback epoch changed during acquisition; reacquire a coherent non-final projection.");
    }
    return deriveSnapshot(configuration, health, endAcquisition, releases, accounts, keeper, forks, reader.remaining.toString());
  };

  root.GlassChainClient = Object.freeze({ validateConfiguration, acquire, deriveSnapshot, groupOrder: GROUP_ORDER, groupLabels: GROUP_LABELS });
})(typeof globalThis === "object" ? globalThis : this);
