/* Canonical read-only operatord attachment. No fallback and no persistence. */

const HASH32 = /^[0-9a-f]{64}$/;
const ADDRESS = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const DECODER_SET = "dragons-clutch/canonical-account-decoders/v3-general-no-keeper-no-selected-candidate";
const $ = (id) => document.getElementById(id);
const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value);
const requiredText = (value, name, pattern, maximum = 512) => {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || (pattern && !pattern.test(value))) throw new Error(`${name} is invalid.`);
  return value;
};
const decimal = (value, name) => requiredText(value, name, DECIMAL, 24);
const hash = (value, name) => requiredText(value, name, HASH32, 64);
const addressBytes = (value, name) => {
  requiredText(value, name, ADDRESS, 44);
  let integer = 0n;
  for (const character of value) {
    const digit = BASE58.indexOf(character);
    if (digit < 0) throw new Error(`${name} is not base58.`);
    integer = integer * 58n + BigInt(digit);
  }
  const significant = [];
  while (integer > 0n) { significant.unshift(Number(integer & 255n)); integer >>= 8n; }
  let leading = 0;
  while (leading < value.length && value[leading] === "1") leading += 1;
  if (leading + significant.length !== 32) throw new Error(`${name} is not an exact 32-byte Solana identity.`);
  return Uint8Array.from([...new Array(leading).fill(0), ...significant]);
};
const address = (value, name) => {
  const bytes = addressBytes(value, name);
  if (bytes.every((byte) => byte === 0)) throw new Error(`${name} must be nonzero.`);
  return value;
};
const object = (value, name) => {
  if (!plain(value)) throw new Error(`${name} must be an object.`);
  return value;
};
const boundedUrl = (value) => {
  let parsed;
  try { parsed = new URL(value); } catch (_) { throw new Error("operatord URL must be absolute."); }
  if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password || parsed.search || parsed.hash) throw new Error("operatord URL has a disallowed scheme or component.");
  const loopback = parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost";
  if (parsed.protocol === "http:" && !loopback) throw new Error("Plain HTTP is allowed only for an explicit loopback operatord.");
  return parsed.toString().replace(/\/$/, "");
};

const endpoint = (value, name) => {
  object(value, name);
  const redacted = requiredText(value.redacted, `${name}.redacted`, null);
  const separator = redacted.indexOf("://");
  const remainder = separator < 1 ? "" : redacted.slice(separator + 3);
  const boundaryCandidates = [remainder.indexOf("/"), remainder.indexOf("?")].filter((index) => index >= 0);
  const boundary = boundaryCandidates.length === 0 ? remainder.length : Math.min(...boundaryCandidates);
  const authority = remainder.slice(0, boundary);
  const suffix = remainder.slice(boundary);
  if (redacted.includes("@") || authority.length === 0 || !["", "/", "/<redacted>", "?<redacted>", "/?<redacted>", "/<redacted>?<redacted>"].includes(suffix)) throw new Error(`${name} is not credential-redacted.`);
  return Object.freeze({ redacted, bindingSha256: hash(value.bindingSha256, `${name}.bindingSha256`) });
};

const validateManifest = (raw) => {
  object(raw, "session manifest");
  if (raw.schema !== "dragons-clutch/operator-read-only-session-manifest/v1" || raw.status !== "ready" || raw.projectionAuthority !== "untrusted-canonical-codec-projection" || raw.authorityEligible !== false || raw.signing !== false || raw.submission !== false || raw.commitment !== "finalized") throw new Error("operatord did not return the canonical read-only session contract.");
  object(raw.transport, "session.transport");
  object(raw.release, "session.release");
  object(raw.restart, "session.restart");
  const transport = Object.freeze({
    clusterName: requiredText(raw.transport.clusterName, "session.transport.clusterName", /^[A-Za-z0-9][A-Za-z0-9._-]*$/, 48),
    genesisHash: address(raw.transport.genesisHash, "session.transport.genesisHash"),
    clusterKey: requiredText(raw.transport.clusterKey, "session.transport.clusterKey", null, 128),
    rpcHttpEndpoint: endpoint(raw.transport.rpcHttpEndpoint, "session.transport.rpcHttpEndpoint"),
    rpcWebsocketEndpoint: endpoint(raw.transport.rpcWebsocketEndpoint, "session.transport.rpcWebsocketEndpoint")
  });
  if (transport.clusterKey !== `${transport.clusterName}:${transport.genesisHash}`) throw new Error("session cluster key does not bind its genesis hash.");
  const release = Object.freeze({
    releaseKey: requiredText(raw.release.releaseKey, "session.release.releaseKey", null, 320),
    programId: address(raw.release.programId, "session.release.programId"),
    programData: address(raw.release.programData, "session.release.programData"),
    deploymentSlot: decimal(raw.release.deploymentSlot, "session.release.deploymentSlot"),
    elfSha256: hash(raw.release.elfSha256, "session.release.elfSha256"),
    releaseManifestSha256: hash(raw.release.releaseManifestSha256, "session.release.releaseManifestSha256"),
    capabilityProfileId: hash(raw.release.capabilityProfileId, "session.release.capabilityProfileId"),
    sourceCommit: requiredText(raw.release.sourceCommit, "session.release.sourceCommit", /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/, 64),
    decoderSet: requiredText(raw.release.decoderSet, "session.release.decoderSet", null, 160)
  });
  if (release.decoderSet !== DECODER_SET) throw new Error("session release names an unsupported canonical decoder set.");
  if (release.programId === release.programData) throw new Error("Program and ProgramData identities alias.");
  const expectedReleaseKey = `${release.programId}:${release.deploymentSlot}:${release.elfSha256}:${release.releaseManifestSha256}`;
  if (release.releaseKey !== expectedReleaseKey) throw new Error("session release key does not bind its exact checked coordinates.");
  if (!Array.isArray(raw.canonicalAccounts) || raw.canonicalAccounts.length > 4096) throw new Error("session canonical account set is invalid or exceeds the operator bound.");
  const seen = new Set();
  let previousAddress = null;
  const accounts = Object.freeze(raw.canonicalAccounts.map((row, index) => {
    object(row, `session.canonicalAccounts[${index}]`);
    object(row.decode, `session.canonicalAccounts[${index}].decode`);
    if (!["canonical", "requires-context"].includes(row.decode.status)) throw new Error(`session.canonicalAccounts[${index}] has an unknown decode disposition.`);
    const account = Object.freeze({
      address: address(row.address, `session.canonicalAccounts[${index}].address`),
      owner: address(row.owner, `session.canonicalAccounts[${index}].owner`),
      releaseKey: requiredText(row.releaseKey, `session.canonicalAccounts[${index}].releaseKey`, null, 320),
      lamports: decimal(row.lamports, `session.canonicalAccounts[${index}].lamports`),
      rentEpoch: decimal(row.rentEpoch, `session.canonicalAccounts[${index}].rentEpoch`),
      dataBytes: decimal(row.dataBytes, `session.canonicalAccounts[${index}].dataBytes`),
      dataSha256: hash(row.dataSha256, `session.canonicalAccounts[${index}].dataSha256`),
      accountTag: decimal(row.accountTag, `session.canonicalAccounts[${index}].accountTag`),
      accountVersion: decimal(row.accountVersion, `session.canonicalAccounts[${index}].accountVersion`),
      family: requiredText(row.family, `session.canonicalAccounts[${index}].family`, null, 40),
      kind: requiredText(row.kind, `session.canonicalAccounts[${index}].kind`, null, 80),
      generation: row.generation === null ? null : decimal(row.generation, `session.canonicalAccounts[${index}].generation`),
      decode: Object.freeze({ status: row.decode.status, requirement: row.decode.status === "requires-context" ? requiredText(row.decode.requirement, `session.canonicalAccounts[${index}].decode.requirement`, null, 240) : null })
    });
    if (account.owner !== release.programId || account.releaseKey !== release.releaseKey || seen.has(account.address)) throw new Error("session contains a duplicate or foreign canonical account identity.");
    const bytes = addressBytes(account.address, `session.canonicalAccounts[${index}].address`);
    if (previousAddress && previousAddress.every((byte, byteIndex) => byte === bytes[byteIndex])) throw new Error("session canonical account identities are duplicated.");
    if (previousAddress) {
      const firstDifference = previousAddress.findIndex((byte, byteIndex) => byte !== bytes[byteIndex]);
      if (firstDifference >= 0 && previousAddress[firstDifference] > bytes[firstDifference]) throw new Error("session canonical account identities are not in address-byte order.");
    }
    previousAddress = bytes;
    seen.add(account.address);
    return account;
  }));
  if (raw.restart.semantics !== "reload every named account through its canonical codec and reauthenticate all joins before using a cursor" || raw.restart.identitySource !== "finalized onchain account bodies plus immutable checked release and RPC bindings" || decimal(raw.restart.accountCount, "session.restart.accountCount") !== String(accounts.length) || !Array.isArray(raw.restart.cursors) || decimal(raw.restart.cursorCount, "session.restart.cursorCount") !== String(raw.restart.cursors.length)) throw new Error("session restart fields are not the canonical onchain-owned contract.");
  const cursors = Object.freeze(raw.restart.cursors.map((row, index) => {
    object(row, `session.restart.cursors[${index}]`);
    object(row.cursor, `session.restart.cursors[${index}].cursor`);
    if (!Array.isArray(row.dependencies) || row.releaseKey !== release.releaseKey) throw new Error(`session.restart.cursors[${index}] is not release-bound.`);
    const cursor = Object.freeze({
      account: address(row.account, `session.restart.cursors[${index}].account`),
      action: requiredText(row.action, `session.restart.cursors[${index}].action`, null, 80),
      workflowId: hash(row.cursor.workflowId, `session.restart.cursors[${index}].cursor.workflowId`),
      lane: requiredText(row.cursor.lane, `session.restart.cursors[${index}].cursor.lane`, null, 48),
      generation: decimal(row.cursor.generation, `session.restart.cursors[${index}].cursor.generation`),
      phase: decimal(row.cursor.phase, `session.restart.cursors[${index}].cursor.phase`),
      item: decimal(row.cursor.item, `session.restart.cursors[${index}].cursor.item`),
      observedStateSha256: hash(row.cursor.observedStateSha256, `session.restart.cursors[${index}].cursor.observedStateSha256`),
      dependencies: Object.freeze(row.dependencies.map((value, dependencyIndex) => address(value, `session.restart.cursors[${index}].dependencies[${dependencyIndex}]`)))
    });
    if (!seen.has(cursor.account) || cursor.dependencies.some((dependency) => !seen.has(dependency)) || new Set(cursor.dependencies).size !== cursor.dependencies.length) throw new Error("restart cursor refers to a noncanonical or repeated account identity.");
    return cursor;
  }));
  return Object.freeze({ sessionId: hash(raw.sessionId, "session.sessionId"), transport, release, accounts, cursors });
};

const field = (term, value) => {
  const wrapper = document.createElement("div"); wrapper.className = "field";
  const dt = document.createElement("dt"); dt.textContent = term;
  const dd = document.createElement("dd"); dd.textContent = value;
  wrapper.append(dt, dd); return wrapper;
};

const render = (manifest) => {
  $("session-state").textContent = "CANONICAL FINALIZED SESSION ATTACHED";
  $("session-id").textContent = manifest.sessionId;
  $("status").replaceChildren(Object.assign(document.createElement("span"), { className: "live", textContent: "attached" }), Object.assign(document.createElement("span"), { textContent: "read-only" }));
  $("identity-fields").replaceChildren(
    field("Session", manifest.sessionId), field("Cluster", manifest.transport.clusterKey),
    field("RPC HTTP binding", `${manifest.transport.rpcHttpEndpoint.redacted} · ${manifest.transport.rpcHttpEndpoint.bindingSha256}`),
    field("RPC WebSocket binding", `${manifest.transport.rpcWebsocketEndpoint.redacted} · ${manifest.transport.rpcWebsocketEndpoint.bindingSha256}`),
    field("Program / ProgramData", `${manifest.release.programId} / ${manifest.release.programData}`),
    field("Release manifest", manifest.release.releaseManifestSha256), field("Capability profile", manifest.release.capabilityProfileId),
    field("ELF / deployment slot", `${manifest.release.elfSha256} / ${manifest.release.deploymentSlot}`), field("Decoder set", manifest.release.decoderSet)
  );
  $("identity-card").hidden = false;
  $("account-count").textContent = `${manifest.accounts.length} identities`;
  $("account-rows").replaceChildren(...manifest.accounts.map((account) => {
    const row = document.createElement("tr");
    for (const value of [account.kind, account.address, account.lamports, account.dataBytes, `${account.accountTag} / ${account.accountVersion}`, account.generation ?? "none", account.decode.status]) {
      const cell = document.createElement("td"); cell.textContent = value; row.append(cell);
    }
    return row;
  }));
  $("accounts-card").hidden = false;
  $("cursor-count").textContent = `${manifest.cursors.length} hints · all disabled`;
  const empty = Object.assign(document.createElement("p"), { className: "muted", textContent: "No finalized onchain-owned restart cursor is presently available." });
  $("cursor-rows").replaceChildren(...(manifest.cursors.length ? manifest.cursors.map((cursor) => {
    const row = document.createElement("div"); row.className = "callout";
    const label = document.createElement("code"); label.textContent = `${cursor.lane} / ${cursor.action} / generation ${cursor.generation} / ${cursor.phase}:${cursor.item}`;
    const disabled = document.createElement("button"); disabled.type = "button"; disabled.disabled = true; disabled.textContent = "Execution unavailable";
    row.append(label, disabled); return row;
  }) : [empty]));
  $("cursors-card").hidden = false;
  $("attach-error").textContent = "Attached. This remains an untrusted projection; restart requires fresh canonical decoding.";
  $("attach-error").className = "callout callout-ok";
};

$("attach-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("identity-card").hidden = true; $("accounts-card").hidden = true; $("cursors-card").hidden = true;
  $("session-state").textContent = "ATTACHING"; $("session-id").textContent = "session unavailable";
  try {
    const base = boundedUrl($("operator-url").value.trim());
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 10000);
    let response;
    try { response = await fetch(`${base}/v1/session`, { method: "GET", mode: "cors", credentials: "omit", cache: "no-store", redirect: "error", referrerPolicy: "no-referrer", headers: { Accept: "application/json" }, signal: controller.signal }); } finally { clearTimeout(timer); }
    if (!response.ok) throw new Error(`/v1/session is unavailable (HTTP ${response.status}).`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.byteLength === 0 || bytes.byteLength > 2_097_152) throw new Error("session response violates the 2 MiB browser bound.");
    render(validateManifest(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes))));
  } catch (error) {
    $("session-state").textContent = "NO CANONICAL SESSION ATTACHED";
    $("status").replaceChildren(Object.assign(document.createElement("span"), { className: "dead", textContent: "detached" }), Object.assign(document.createElement("span"), { textContent: "unavailable" }));
    $("attach-error").className = "callout callout-warn";
    $("attach-error").textContent = error instanceof Error ? error.message : "Canonical session attachment failed.";
  }
});

export { validateManifest };
