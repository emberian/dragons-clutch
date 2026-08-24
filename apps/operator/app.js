/* Canonical read-only operatord attachment. No fallback and no persistence. */

const HASH32 = /^[0-9a-f]{64}$/;
const ADDRESS = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const HEX_BYTES = /^(?:[0-9a-f]{2})+$/;
const DECODER_SET = "dragons-clutch/canonical-account-decoders/v4-source-work-schedule";
const $ = (id) => document.getElementById(id);
const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value);
const requiredText = (value, name, pattern, maximum = 512) => {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || (pattern && !pattern.test(value))) throw new Error(`${name} is invalid.`);
  return value;
};
const decimal = (value, name) => requiredText(value, name, DECIMAL, 40);
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
  if (!Array.isArray(raw.release.enabledIntents) || raw.release.enabledIntents.length > 256) throw new Error("session release enabled-intent set is invalid.");
  const enabledIntents = Object.freeze(raw.release.enabledIntents.map((intent, index) => {
    object(intent, `session.release.enabledIntents[${index}]`);
    const coordinate = Object.freeze({ familyTag: decimal(intent.familyTag, "enabled family tag"), familyVersion: decimal(intent.familyVersion, "enabled family version"), localAction: decimal(intent.localAction, "enabled local action") });
    if (index > 0) {
      const previous = raw.release.enabledIntents[index - 1];
      const before = [BigInt(previous.familyTag), BigInt(previous.familyVersion), BigInt(previous.localAction)];
      const after = [BigInt(coordinate.familyTag), BigInt(coordinate.familyVersion), BigInt(coordinate.localAction)];
      const ordered = before[0] < after[0] || (before[0] === after[0] && (before[1] < after[1] || (before[1] === after[1] && before[2] < after[2])));
      if (!ordered) throw new Error("session release enabled-intent coordinates are not strictly ordered.");
    }
    return coordinate;
  }));
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
  return Object.freeze({ sessionId: hash(raw.sessionId, "session.sessionId"), transport, release: Object.freeze({ ...release, enabledIntents }), accounts, cursors });
};

const validateActions = (raw, manifest) => {
  object(raw, "action capability set");
  if (raw.schema !== "dragons-clutch/operator-action-capability-set/v1" || raw.status !== "ready" || raw.commitment !== "finalized" || raw.projectionAuthority !== "untrusted-release-and-canonical-codec-projection" || raw.signing !== false || raw.submission !== false) throw new Error("operatord did not return the canonical action capability contract.");
  if (hash(raw.sessionId, "actions.sessionId") !== manifest.sessionId || requiredText(raw.releaseKey, "actions.releaseKey", null, 320) !== manifest.release.releaseKey || hash(raw.capabilityProfileId, "actions.capabilityProfileId") !== manifest.release.capabilityProfileId) throw new Error("action verdicts do not belong to the attached session and checked release.");
  object(raw.freshness, "actions.freshness");
  if (raw.freshness.recentBlockhash !== "absent-by-contract" || raw.freshness.feePayer !== "must-be-explicit-in-server-constructed-draft" || raw.freshness.validBeforeSlot !== "must-be-derived-from-a-fresh-clock-observation" || typeof raw.freshness.beforeSigning !== "string" || typeof raw.freshness.afterSubmission !== "string") throw new Error("action freshness contract is incomplete.");
  if (!Array.isArray(raw.actions) || raw.actions.length > 256) throw new Error("action capability set exceeds the browser bound.");
  const coordinates = new Set();
  const enabled = new Set(manifest.release.enabledIntents.map((coordinate) => `${coordinate.familyTag}/${coordinate.familyVersion}/${coordinate.localAction}`));
  const actions = Object.freeze(raw.actions.map((row, index) => {
    object(row, `actions[${index}]`); object(row.coordinate, `actions[${index}].coordinate`); object(row.releaseAdmission, `actions[${index}].releaseAdmission`);
    const coordinate = Object.freeze({
      familyTag: decimal(row.coordinate.familyTag, `actions[${index}].coordinate.familyTag`),
      familyVersion: decimal(row.coordinate.familyVersion, `actions[${index}].coordinate.familyVersion`),
      localAction: decimal(row.coordinate.localAction, `actions[${index}].coordinate.localAction`),
      family: requiredText(row.coordinate.family, `actions[${index}].coordinate.family`, null, 32),
      action: requiredText(row.coordinate.action, `actions[${index}].coordinate.action`, null, 96)
    });
    const coordinateKey = `${coordinate.familyTag}/${coordinate.familyVersion}/${coordinate.localAction}`;
    if (!enabled.has(coordinateKey) || coordinates.has(coordinateKey)) throw new Error("action capability set contains a disabled or repeated exact coordinate.");
    coordinates.add(coordinateKey);
    if (row.releaseAdmission.enabled !== true || row.releaseAdmission.releaseKey !== manifest.release.releaseKey || row.releaseAdmission.capabilityProfileId !== manifest.release.capabilityProfileId) throw new Error("action verdict is not admitted by the attached checked release.");
    if (!Array.isArray(row.accountRoles) || row.accountRoles.length > 64 || !Array.isArray(row.signerRequirements)) throw new Error("action role or signer projection is invalid.");
    const roles = Object.freeze(row.accountRoles.map((role, roleIndex) => {
      object(role, `actions[${index}].accountRoles[${roleIndex}]`);
      if (decimal(role.index, `actions[${index}].accountRoles[${roleIndex}].index`) !== String(roleIndex) || typeof role.writable !== "boolean" || typeof role.signer !== "boolean") throw new Error("action roles are not in exact semantic-owner order.");
      return Object.freeze({ role: requiredText(role.role, "account role", null, 64), writable: role.writable, signer: role.signer, address: role.address === null ? null : address(role.address, "account role address"), identityDisposition: requiredText(role.identityDisposition, "account role disposition", null, 128) });
    }));
    if (row.callable === false) {
      if (row.verdict !== "unavailable" || row.transactionDraft !== null || row.signerRequirements.length !== 0 || typeof row.reason !== "string") throw new Error("an unavailable action carries executable-looking transaction material.");
      return Object.freeze({ coordinate, roles, callable: false, reason: requiredText(row.reason, "action reason", null, 512), cursor: row.stateSelection, transactionDraft: null, signerRequirements: Object.freeze([]) });
    }
    if (row.callable !== true || row.verdict !== "callable-unsigned-draft" || row.stateSelection === null) throw new Error("a callable action lacks an exact canonical state selection.");
    const selection = object(row.stateSelection, `actions[${index}].stateSelection`);
    const cursor = object(selection.cursor, `actions[${index}].stateSelection.cursor`);
    const restart = manifest.cursors.find((candidate) => candidate.account === selection.account && candidate.action === selection.action && candidate.workflowId === cursor.workflowId && candidate.generation === cursor.generation && candidate.phase === cursor.phase && candidate.item === cursor.item && candidate.observedStateSha256 === cursor.observedStateSha256);
    object(selection.branch, `actions[${index}].stateSelection.branch`);
    if (!Array.isArray(selection.dependencies) || !restart || selection.releaseKey !== manifest.release.releaseKey || selection.observedCommitment !== "finalized" || selection.effectiveCommitment !== "finalized" || selection.branch.kind !== "finalized-scan" || selection.dependencies.length !== restart.dependencies.length || selection.dependencies.some((dependency, dependencyIndex) => dependency !== restart.dependencies[dependencyIndex])) throw new Error("callable action state selection is not the attached finalized restart cursor.");
    const accountSlot = BigInt(decimal(selection.accountSlot, "callable state-selection account slot"));
    if (roles.length === 0 || roles.some((role) => role.address === null || role.identityDisposition !== "semantic-owner-derived-and-bound-to-draft")) throw new Error("callable action has unresolved or noncanonical account roles.");
    const signerRequirements = Object.freeze(row.signerRequirements.map((requirement, signerIndex) => {
      object(requirement, `actions[${index}].signerRequirements[${signerIndex}]`);
      if (!Array.isArray(requirement.semanticRoles) || requirement.semanticRoles.length === 0 || requirement.signaturePresent !== false || requirement.keyAccess !== false) throw new Error("callable signer requirement implies key or signature access.");
      return Object.freeze({ address: address(requirement.address, "signer address"), semanticRoles: Object.freeze(requirement.semanticRoles.map((role) => requiredText(role, "signer semantic role", null, 64))), signaturePresent: false, keyAccess: false });
    }));
    if (new Set(signerRequirements.map((requirement) => requirement.address)).size !== signerRequirements.length) throw new Error("callable signer requirements repeat an identity.");
    const requiredByRoles = new Set(roles.filter((role) => role.signer).map((role) => role.address));
    if (signerRequirements.some((requirement) => !requiredByRoles.has(requirement.address)) || [...requiredByRoles].some((signer) => !signerRequirements.some((requirement) => requirement.address === signer))) throw new Error("callable signer requirements differ from exact signer account roles.");
    const draft = validateTransactionDraft(row.transactionDraft, manifest, coordinate, selection, roles, signerRequirements, index);
    object(row.freshnessDisposition, `actions[${index}].freshnessDisposition`);
    const observedSlot = BigInt(decimal(row.freshnessDisposition.observedSlot, "draft observed slot"));
    const validBeforeSlot = BigInt(decimal(row.freshnessDisposition.validBeforeSlot, "draft valid-before slot"));
    const maximumValiditySlots = BigInt(decimal(row.freshnessDisposition.maximumValiditySlots, "draft maximum validity slots"));
    if (observedSlot === 0n || observedSlot < accountSlot || validBeforeSlot <= observedSlot || validBeforeSlot - observedSlot > maximumValiditySlots || row.freshnessDisposition.recentBlockhash !== "absent; a launcher must reacquire state before adding one" || typeof row.freshnessDisposition.beforeSigning !== "string" || typeof row.freshnessDisposition.afterSubmission !== "string") throw new Error("callable action freshness boundary is invalid.");
    return Object.freeze({ coordinate, roles, callable: true, reason: requiredText(row.reason, "action reason", null, 512), cursor: selection, transactionDraft: draft, signerRequirements });
  }));
  if (coordinates.size !== enabled.size) throw new Error("operatord omitted a checked release-enabled coordinate.");
  return actions;
};

const validateTransactionDraft = (raw, manifest, coordinate, selection, roles, signers, actionIndex) => {
  object(raw, `actions[${actionIndex}].transactionDraft`);
  if (raw.schema !== "dragons-clutch/operator-canonical-action-material/v1" || raw.constructionSchema !== "dragons-clutch/operator/unsigned-protocol-transaction/v3" || raw.releaseManifestSha256 !== manifest.release.releaseManifestSha256 || raw.capabilityProfileId !== manifest.release.capabilityProfileId || raw.driverAccount !== selection.account || raw.recentBlockhash !== null || raw.hasRecentBlockhash !== false || raw.signed !== false || raw.submitted !== false || raw.reloadAuthoritativeAccounts !== true) throw new Error("callable transaction draft violates its construction/release boundary.");
  const feePayer = address(raw.feePayer, "transaction fee payer");
  if (!signers.some((requirement) => requirement.address === feePayer && requirement.semanticRoles.includes("transaction-fee-payer")) || !roles.some((role) => role.address === feePayer && role.signer)) throw new Error("transaction fee payer is not the exact signer role.");
  hash(raw.draftId, "transaction draft ID");
  const bytes = requiredText(raw.serializedTransactionHex, "serialized transaction", HEX_BYTES, 2464);
  if (decimal(raw.serializedBytes, "serialized transaction bytes") !== String(bytes.length / 2)) throw new Error("serialized transaction byte count is inconsistent.");
  if (!Array.isArray(raw.actions) || raw.actions.length !== 1 || !Array.isArray(raw.flows) || raw.flows.length !== 1 || raw.flows[0] !== "source-plane-v3" || !Array.isArray(raw.semanticOwners) || raw.semanticOwners.length !== 1 || !Array.isArray(raw.registryBindings) || raw.registryBindings.length !== 1 || !Array.isArray(raw.runtimeAdmissions) || raw.runtimeAdmissions.length !== 1 || raw.runtimeAdmissions[0] !== "release-bound-enabled" || !Array.isArray(raw.exactEquations) || raw.exactEquations.length === 0) throw new Error("callable transaction draft is not one exact admitted Source action.");
  const binding = object(raw.registryBindings[0], "transaction registry binding");
  if (binding.familyTag !== coordinate.familyTag || binding.familyVersion !== coordinate.familyVersion || binding.localAction !== coordinate.localAction || binding.allocationStatus !== "frozen") throw new Error("transaction registry binding differs from the release-enabled coordinate.");
  raw.semanticOwners.forEach((owner, ownerIndex) => { object(owner, `semanticOwners[${ownerIndex}]`); requiredText(owner.package, "semantic owner package", null, 160); requiredText(owner.schema, "semantic owner schema", null, 160); hash(owner.releaseSha256, "semantic owner release"); });
  raw.exactEquations.forEach((equation, equationIndex) => { object(equation, `exactEquations[${equationIndex}]`); object(equation.unit, `exactEquations[${equationIndex}].unit`); requiredText(equation.name, "exact equation name", null, 200); if (decimal(equation.left, "exact equation left") !== decimal(equation.right, "exact equation right")) throw new Error("transaction exact-integer equation is unbalanced."); });
  return Object.freeze({ ...raw, draftId: raw.draftId, feePayer, serializedTransactionHex: bytes });
};

const field = (term, value) => {
  const wrapper = document.createElement("div"); wrapper.className = "field";
  const dt = document.createElement("dt"); dt.textContent = term;
  const dd = document.createElement("dd"); dd.textContent = value;
  wrapper.append(dt, dd); return wrapper;
};

const render = (manifest, actions) => {
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
  const callable = actions.filter((action) => action.callable);
  $("cursor-count").textContent = `${actions.length} release-enabled · ${callable.length} callable`;
  const empty = Object.assign(document.createElement("p"), { className: "muted", textContent: "The checked release enables no successor action coordinates." });
  $("cursor-rows").replaceChildren(...(actions.length ? actions.map((action) => {
    const row = document.createElement("div"); row.className = "callout";
    const label = document.createElement("code"); label.textContent = `${action.coordinate.familyTag}/${action.coordinate.familyVersion}/${action.coordinate.localAction} · ${action.coordinate.family}/${action.coordinate.action}`;
    const reason = document.createElement("span"); reason.className = "muted"; reason.textContent = action.reason;
    const control = document.createElement("button"); control.type = "button"; control.disabled = !action.callable; control.textContent = action.callable ? "Inspect canonical draft" : "Unavailable";
    const detail = document.createElement("pre"); detail.className = "mono"; detail.hidden = true;
    if (action.callable) control.addEventListener("click", () => { detail.textContent = JSON.stringify({ transactionDraft: action.transactionDraft, signerRequirements: action.signerRequirements }, null, 2); detail.hidden = !detail.hidden; });
    row.append(label, reason, control, detail); return row;
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
    let startResponse; let actionsResponse; let endResponse;
    try {
      startResponse = await fetch(`${base}/v1/session`, { method: "GET", mode: "cors", credentials: "omit", cache: "no-store", redirect: "error", referrerPolicy: "no-referrer", headers: { Accept: "application/json" }, signal: controller.signal });
      actionsResponse = await fetch(`${base}/v1/actions`, { method: "GET", mode: "cors", credentials: "omit", cache: "no-store", redirect: "error", referrerPolicy: "no-referrer", headers: { Accept: "application/json" }, signal: controller.signal });
      endResponse = await fetch(`${base}/v1/session`, { method: "GET", mode: "cors", credentials: "omit", cache: "no-store", redirect: "error", referrerPolicy: "no-referrer", headers: { Accept: "application/json" }, signal: controller.signal });
    } finally { clearTimeout(timer); }
    if (!startResponse.ok || !actionsResponse.ok || !endResponse.ok) throw new Error(`canonical session/action bracket is unavailable (HTTP ${startResponse.status}/${actionsResponse.status}/${endResponse.status}).`);
    const decodeResponse = async (response, name) => { const bytes = new Uint8Array(await response.arrayBuffer()); if (bytes.byteLength === 0 || bytes.byteLength > 2_097_152) throw new Error(`${name} response violates the 2 MiB browser bound.`); return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); };
    const start = validateManifest(await decodeResponse(startResponse, "start session"));
    const actions = validateActions(await decodeResponse(actionsResponse, "actions"), start);
    const end = validateManifest(await decodeResponse(endResponse, "end session"));
    if (start.sessionId !== end.sessionId) throw new Error("canonical session changed while action verdicts were acquired; reacquire from the beginning.");
    render(start, actions);
  } catch (error) {
    $("session-state").textContent = "NO CANONICAL SESSION ATTACHED";
    $("status").replaceChildren(Object.assign(document.createElement("span"), { className: "dead", textContent: "detached" }), Object.assign(document.createElement("span"), { textContent: "unavailable" }));
    $("attach-error").className = "callout callout-warn";
    $("attach-error").textContent = error instanceof Error ? error.message : "Canonical session attachment failed.";
  }
});

export { validateActions, validateManifest };
