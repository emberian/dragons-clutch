/*
 * Glass offline client. This file deliberately has no network, wallet, RPC,
 * signing, or submission capability. It holds no copy of reviewed release
 * data: manifest.json and terms.json are mirrored once into embedded-data.js.
 */
(function () {
  "use strict";

  const EMBEDDED = globalThis.GlassEmbeddedData;
  const MANIFEST = EMBEDDED && EMBEDDED.manifest;
  const TERMS = EMBEDDED && EMBEDDED.terms;
  const $ = (id) => document.getElementById(id);
  const EVIDENCE = Object.freeze({
    LOCAL_FIXTURE: Object.freeze({ label: "LOCAL FIXTURE", className: "local-chip" }),
    PROVED_MODEL: Object.freeze({ label: "PROVED-MODEL", className: "evidence-chip" }),
    CHECKED_RUST_SUBSET: Object.freeze({ label: "CHECKED-RUST-SUBSET", className: "evidence-chip" }),
    CHECKED_FINITE: Object.freeze({ label: "CHECKED-FINITE", className: "evidence-chip" }),
    HOST_TESTED: Object.freeze({ label: "HOST-TESTED", className: "evidence-chip" }),
    SBF_EXECUTED: Object.freeze({ label: "SBF-EXECUTED", className: "evidence-chip" }),
    MODEL_ONLY: Object.freeze({ label: "MODEL-ONLY", className: "proposed-chip" }),
    PROPOSED: Object.freeze({ label: "PROPOSED", className: "proposed-chip" }),
    IN_FLIGHT: Object.freeze({ label: "IN-FLIGHT", className: "proposed-chip" }),
    STOP: Object.freeze({ label: "STOP", className: "stop-chip" }),
    UNAVAILABLE: Object.freeze({ label: "UNAVAILABLE", className: "stop-chip" })
  });
  const EVIDENCE_KINDS = new Set(Object.keys(EVIDENCE));
  const ARTIFACT_KINDS = new Set(["file-sha256", "elf-sha256", "source-revision", "unaccepted-worktree", "unavailable"]);
  const DISPOSITIONS = new Set(["inspect-only", "blocked", "unavailable", "not-released"]);
  const ACTIONS = new Set(["local-preview", "none"]);
  const BOUNDARY_CATEGORIES = new Set(["trust", "semantic", "release", "lifecycle", "evidence"]);
  const DISPLAY_SCHEMA = "dragon-clutch.glass-display-snapshot.v1";
  const MAX = Object.freeze({ id: 80, label: 120, text: 900, path: 240, locator: 180, items: 48, lifecycle: 20, basis: 12, fixtures: 12, boundaries: 24 });
  let integrityFault = null;
  let lastIntent = null;

  const canonicalize = (value) => {
    if (Array.isArray(value)) return value.map(canonicalize);
    if (value && typeof value === "object") {
      return Object.keys(value).sort().reduce((out, key) => {
        out[key] = canonicalize(value[key]);
        return out;
      }, {});
    }
    return value;
  };
  const canonicalJson = (value) => JSON.stringify(canonicalize(value));
  const create = (name, className, text) => {
    const element = document.createElement(name);
    if (className) element.className = className;
    if (text !== undefined) element.textContent = text;
    return element;
  };
  const reset = (element) => element.replaceChildren();
  const definition = (term, description) => {
    const pair = create("div");
    pair.append(create("dt", null, term), create("dd", null, description));
    return pair;
  };
  const formatEvidence = (kind) => EVIDENCE[kind];
  const badge = (kind) => {
    const formatted = formatEvidence(kind);
    return create("span", `evidence-chip ${formatted.className}`, formatted.label);
  };
  const own = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
  const plainRecord = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  const exactKeys = (value, keys) => plainRecord(value) && Object.keys(value).length === keys.length && keys.every((key) => own(value, key));
  const string = (value, limit) => typeof value === "string" && value.length > 0 && value.length <= limit;
  const id = (value) => string(value, MAX.id) && /^[a-z0-9][a-z0-9-]*$/.test(value);
  const commit = (value) => typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
  const digest = (value) => typeof value === "string" && /^sha256:[0-9a-f]{64}$/.test(value);
  const hash = (value) => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
  const localPath = (value) => string(value, MAX.path) && /^[A-Za-z0-9._/-]+$/.test(value) && !value.startsWith("/") && !value.includes("..") && !value.includes("//");
  const array = (value, maximum) => Array.isArray(value) && value.length > 0 && value.length <= maximum;
  const unique = (values) => new Set(values).size === values.length;
  const frozenCopy = (value) => {
    if (Array.isArray(value)) return Object.freeze(value.map(frozenCopy));
    if (plainRecord(value)) {
      const copy = {};
      Object.keys(value).forEach((key) => { copy[key] = frozenCopy(value[key]); });
      return Object.freeze(copy);
    }
    return value;
  };
  const result = (ok, value) => ok ? Object.freeze({ ok: true, value: frozenCopy(value) }) : Object.freeze({ ok: false, reason: value });

  const validateArtifact = (value) => exactKeys(value, ["kind", "value", "path"])
    && ARTIFACT_KINDS.has(value.kind)
    && string(value.value, MAX.text)
    && (value.path === null || localPath(value.path));
  const validateIdentity = (value) => exactKeys(value, ["sourceCommit", "artifact"])
    && commit(value.sourceCommit) && validateArtifact(value.artifact);
  const validateSourceRef = (value) => exactKeys(value, ["repositoryPath", "locator"])
    && localPath(value.repositoryPath) && string(value.locator, MAX.locator);
  const validateSubject = (value) => exactKeys(value, ["id", "label"])
    && id(value.id) && string(value.label, MAX.label);
  const validateRefs = (value, maximum) => array(value, maximum) && value.every(id) && unique(value);
  const validateEvidence = (value) => exactKeys(value, ["id", "kind", "subject", "scope", "sourceRef", "negativeBoundary", "identity"])
    && id(value.id) && EVIDENCE_KINDS.has(value.kind) && validateSubject(value.subject)
    && string(value.scope, MAX.text) && validateSourceRef(value.sourceRef)
    && string(value.negativeBoundary, MAX.text) && validateIdentity(value.identity);
  const validateLifecycle = (value) => exactKeys(value, ["id", "label", "statement", "evidenceRefs", "prerequisiteIds", "boundaryRefs", "disposition", "action"])
    && id(value.id) && string(value.label, MAX.label) && string(value.statement, MAX.text)
    && validateRefs(value.evidenceRefs, MAX.items) && Array.isArray(value.prerequisiteIds) && value.prerequisiteIds.length <= MAX.lifecycle && value.prerequisiteIds.every(id) && unique(value.prerequisiteIds)
    && validateRefs(value.boundaryRefs, MAX.boundaries) && DISPOSITIONS.has(value.disposition) && ACTIONS.has(value.action)
    && ((value.disposition === "inspect-only") === (value.action === "local-preview"));
  const validateBasis = (value) => exactKeys(value, ["id", "aspect", "nativeDegreeZero", "nativeSmoothDegreesOneToThree", "categoricalCompatibilityLowering", "boundaryRefs"])
    && id(value.id) && string(value.aspect, MAX.label) && string(value.nativeDegreeZero, MAX.text)
    && string(value.nativeSmoothDegreesOneToThree, MAX.text) && string(value.categoricalCompatibilityLowering, MAX.text)
    && validateRefs(value.boundaryRefs, MAX.boundaries);
  const validateFixture = (value) => exactKeys(value, ["id", "label", "localPath", "fileSha256", "producer", "evidenceRefs", "notChainState", "provenanceBoundary"])
    && id(value.id) && string(value.label, MAX.label) && localPath(value.localPath) && hash(value.fileSha256)
    && string(value.producer, MAX.label) && validateRefs(value.evidenceRefs, MAX.items)
    && value.notChainState === true && string(value.provenanceBoundary, MAX.text);
  const validateBoundary = (value) => exactKeys(value, ["id", "title", "category", "text", "evidenceRefs"])
    && id(value.id) && string(value.title, MAX.label) && BOUNDARY_CATEGORIES.has(value.category)
    && string(value.text, MAX.text) && validateRefs(value.evidenceRefs, MAX.items);
  const cycleFree = (nodes) => {
    const byId = new Map(nodes.map((node) => [node.id, node]));
    const visiting = new Set();
    const visited = new Set();
    const visit = (nodeId) => {
      if (visited.has(nodeId)) return true;
      if (visiting.has(nodeId)) return false;
      visiting.add(nodeId);
      const node = byId.get(nodeId);
      const valid = node.prerequisiteIds.every(visit);
      visiting.delete(nodeId);
      visited.add(nodeId);
      return valid;
    };
    return nodes.every((node) => visit(node.id));
  };

  const validateDisplaySnapshot = (raw, context) => {
    const manifest = context && context.manifest;
    const terms = context && context.terms;
    if (!plainRecord(manifest) || !plainRecord(terms) || !plainRecord(raw)) return result(false, "reviewed data is missing or not a plain record");
    if (!exactKeys(raw, ["schema", "schemaVersion", "snapshotIdentity", "termsBinding", "evidence", "lifecycle", "basisComparison", "localFixtures", "boundaries"])) return result(false, "display snapshot has an unknown or missing top-level key");
    if (raw.schema !== DISPLAY_SCHEMA || raw.schemaVersion !== 1) return result(false, "display snapshot schema is not recognized");
    if (!exactKeys(raw.snapshotIdentity, ["reviewedTreeCommit", "releaseBinding", "releaseSourceCommit"])) return result(false, "snapshot identity is malformed");
    if (!commit(raw.snapshotIdentity.reviewedTreeCommit) || raw.snapshotIdentity.releaseBinding !== "unbound-offline-snapshot" || raw.snapshotIdentity.releaseSourceCommit !== null) return result(false, "snapshot identity does not describe an unbound reviewed snapshot");
    if (!exactKeys(raw.termsBinding, ["termsVersion", "digest"]) || !string(raw.termsBinding.termsVersion, MAX.label) || !digest(raw.termsBinding.digest)) return result(false, "terms binding is malformed");
    if (!plainRecord(manifest.terms) || raw.termsBinding.digest !== manifest.terms.digest || raw.termsBinding.digest !== terms.digest || raw.termsBinding.termsVersion !== manifest.terms.termsVersion || raw.termsBinding.termsVersion !== terms.termsVersion) return result(false, "terms binding does not match reviewed manifest and terms data");
    if (!exactKeys(manifest.application, ["name", "version", "releaseStatus", "official"]) || !string(manifest.application.name, MAX.label) || !string(manifest.application.version, MAX.label) || !string(manifest.application.releaseStatus, MAX.label) || manifest.application.official !== false || !/offline.*prototype/i.test(manifest.application.releaseStatus)) return result(false, "manifest application boundary is malformed");
    if (!exactKeys(manifest.releaseIdentity, ["sourceRepository", "sourceCommit", "bundleSha256", "ipfsCid", "githubPagesMirror", "manifestSha256"]) || !string(manifest.releaseIdentity.sourceRepository, MAX.label) || !/^UNBOUND/.test(manifest.releaseIdentity.sourceCommit) || !string(manifest.releaseIdentity.bundleSha256, MAX.text) || !string(manifest.releaseIdentity.manifestSha256, MAX.text) || manifest.releaseIdentity.ipfsCid !== null || manifest.releaseIdentity.githubPagesMirror !== null) return result(false, "manifest release boundary is not offline and unbound");
    if (!exactKeys(manifest.terms, ["path", "termsVersion", "digestAlgorithm", "digestScope", "digest"]) || !localPath(manifest.terms.path) || !string(manifest.terms.termsVersion, MAX.label) || manifest.terms.digestAlgorithm !== "sha256" || !string(manifest.terms.digestScope, MAX.text) || !digest(manifest.terms.digest)) return result(false, "manifest terms declaration is malformed");
    if (!plainRecord(terms.canonicalTerms) || !string(terms.termsVersion, MAX.label) || !digest(terms.digest) || !string(terms.semanticsNote, MAX.text) || !string(terms.warning, MAX.text)) return result(false, "terms fixture is malformed");
    if (!plainRecord(manifest.capabilities) || ["rpcReads", "walletConnection", "transactionSigning", "transactionSubmission", "backgroundWork"].some((key) => manifest.capabilities[key] !== false)) return result(false, "manifest declares a browser chain capability");
    if (!array(raw.evidence, MAX.items) || !raw.evidence.every(validateEvidence) || !unique(raw.evidence.map((item) => item.id))) return result(false, "evidence records are malformed or duplicate");
    if (!array(raw.lifecycle, MAX.lifecycle) || !raw.lifecycle.every(validateLifecycle) || !unique(raw.lifecycle.map((item) => item.id))) return result(false, "lifecycle records are malformed or duplicate");
    if (!array(raw.basisComparison, MAX.basis) || !raw.basisComparison.every(validateBasis) || !unique(raw.basisComparison.map((item) => item.id))) return result(false, "basis comparison records are malformed or duplicate");
    if (!array(raw.localFixtures, MAX.fixtures) || !raw.localFixtures.every(validateFixture) || !unique(raw.localFixtures.map((item) => item.id))) return result(false, "local fixture records are malformed or duplicate");
    if (!array(raw.boundaries, MAX.boundaries) || !raw.boundaries.every(validateBoundary) || !unique(raw.boundaries.map((item) => item.id))) return result(false, "boundary records are malformed or duplicate");
    const evidenceIds = new Set(raw.evidence.map((item) => item.id));
    const boundaryIds = new Set(raw.boundaries.map((item) => item.id));
    const lifecycleIds = new Set(raw.lifecycle.map((item) => item.id));
    const refsExist = (refs, known) => refs.every((reference) => known.has(reference));
    if (!raw.lifecycle.every((item) => refsExist(item.evidenceRefs, evidenceIds) && refsExist(item.boundaryRefs, boundaryIds) && refsExist(item.prerequisiteIds, lifecycleIds))) return result(false, "lifecycle has a dangling reference");
    if (!raw.basisComparison.every((item) => refsExist(item.boundaryRefs, boundaryIds)) || !raw.localFixtures.every((item) => refsExist(item.evidenceRefs, evidenceIds)) || !raw.boundaries.every((item) => refsExist(item.evidenceRefs, evidenceIds))) return result(false, "display snapshot has a dangling evidence or boundary reference");
    const evidenceById = new Map(raw.evidence.map((item) => [item.id, item]));
    if (raw.lifecycle.some((item) => item.evidenceRefs.some((reference) => evidenceById.get(reference).kind === "IN_FLIGHT"))) return result(false, "in-flight evidence cannot create a lifecycle node");
    if (!cycleFree(raw.lifecycle)) return result(false, "lifecycle prerequisites contain a cycle");
    return result(true, raw);
  };

  const deriveDisplayView = (validated) => {
    if (!validated || !validated.ok) return Object.freeze({ mode: "unavailable", headline: "Evidence snapshot unavailable", cards: Object.freeze([]), actionsDisabled: true });
    const snapshot = validated.value;
    const evidenceById = new Map(snapshot.evidence.map((item) => [item.id, item]));
    const boundariesById = new Map(snapshot.boundaries.map((item) => [item.id, item]));
    const byIds = (ids) => ids.map((item) => evidenceById.get(item));
    const railIds = ["bspline-lean-model", "bspline-finite-bridge", "native-point-resolution", "release-evidence-stop"];
    const rail = railIds.map((item) => evidenceById.get(item)).filter(Boolean);
    const lifecycle = snapshot.lifecycle.map((node) => Object.freeze({ ...node, evidence: Object.freeze(byIds(node.evidenceRefs)), boundaries: Object.freeze(node.boundaryRefs.map((item) => boundariesById.get(item))) }));
    const decorateBoundary = (item) => Object.freeze({ ...item, evidence: Object.freeze(byIds(item.evidenceRefs)) });
    const currentBoundaries = snapshot.boundaries.filter((item) => item.category !== "evidence").map(decorateBoundary);
    const worktreeBoundaries = snapshot.boundaries.filter((item) => item.category === "evidence").map(decorateBoundary);
    return Object.freeze({
      mode: "ready",
      headline: "One basis. Exact state claims.",
      cards: Object.freeze(rail),
      rail: Object.freeze(rail),
      lifecycle: Object.freeze(lifecycle),
      basisComparison: snapshot.basisComparison,
      boundaries: Object.freeze(currentBoundaries),
      worktreeBoundaries: Object.freeze(worktreeBoundaries),
      localFixtures: snapshot.localFixtures,
      evidence: snapshot.evidence,
      snapshot
    });
  };

  const renderEvidenceRail = (items) => {
    const target = $("evidence-rail");
    reset(target);
    for (const item of items) {
      const article = create("article", "rail-item");
      const header = create("div", "rail-item-header");
      header.append(badge(item.kind), create("h3", null, item.subject.label));
      const boundary = create("p", "rail-boundary");
      boundary.append(create("strong", null, "Does not establish: "), document.createTextNode(item.negativeBoundary));
      article.append(header, create("p", "rail-fact", item.scope), boundary);
      target.append(article);
    }
  };
  const renderBasisComparison = (items) => {
    const target = $("basis-table-body");
    reset(target);
    for (const item of items) {
      const row = create("tr");
      const heading = create("th", null, item.aspect);
      heading.scope = "row";
      row.append(heading, create("td", null, item.nativeDegreeZero), create("td", null, item.nativeSmoothDegreesOneToThree), create("td", null, item.categoricalCompatibilityLowering));
      target.append(row);
    }
  };
  const evidenceSummary = (items) => items.map((item) => `${formatEvidence(item.kind).label} · ${item.subject.label}`).join("; ");
  const renderLifecycle = (items) => {
    const target = $("evidence-path");
    reset(target);
    items.forEach((item, index) => {
      const entry = create("li", "path-step");
      entry.append(create("span", "path-number", String(index + 1).padStart(2, "0")));
      const body = create("div", "path-body");
      const heading = create("div", "path-heading");
      heading.append(create("h3", null, item.label));
      body.append(heading, create("p", null, item.statement), create("p", "boundary-copy", evidenceSummary(item.evidence)));
      entry.append(body);
      target.append(entry);
    });
  };
  const renderBoundaries = (items, targetId) => {
    const target = $(targetId);
    reset(target);
    for (const item of items) {
      const article = create("article", targetId === "roadmap-list" ? "roadmap-card" : "boundary-card");
      const header = create("div", "card-heading");
      const chips = create("div", "evidence-chip-list");
      item.evidence.forEach((evidence) => chips.append(badge(evidence.kind)));
      header.append(create("h3", null, item.title), chips);
      article.append(header, create("p", null, item.text));
      target.append(article);
    }
  };
  const renderReleaseIdentity = (snapshot) => {
    const target = $("release-identity");
    const identity = MANIFEST.releaseIdentity;
    reset(target);
    target.append(definition("Application", MANIFEST.application.name), definition("Release", `${MANIFEST.application.version} · ${MANIFEST.application.releaseStatus}`), definition("Bundle SHA-256", identity.bundleSha256), definition("IPFS CID", identity.ipfsCid || "not assigned"), definition("Release source", identity.sourceCommit), definition("Reviewed tree anchor", snapshot.snapshotIdentity.reviewedTreeCommit));
    $("release-status").textContent = MANIFEST.application.releaseStatus;
    $("snapshot-provenance").textContent = `Reviewed documentation snapshot: ${snapshot.snapshotIdentity.releaseBinding}. The tree anchor is evidence provenance only, not a release source commit, deployment identity, or official-client claim.`;
  };
  const renderFixtures = (items) => {
    const target = $("evidence-vocabulary");
    reset(target);
    for (const item of items) target.append(definition(item.label, item.provenanceBoundary));
    const termsFixture = items.find((item) => item.localPath === "terms.json");
    $("fixture-inspector-note").textContent = termsFixture.provenanceBoundary;
    $("terms-fixture-status").textContent = formatEvidence("LOCAL_FIXTURE").label;
  };

  const subtleAvailable = () => Boolean(globalThis.crypto && globalThis.crypto.subtle && globalThis.TextEncoder);
  const sha256 = async (text) => {
    const bytes = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
    return `sha256:${Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  };
  const setDigestStatus = (text, tone) => {
    const status = $("digest-status");
    status.textContent = text;
    status.className = `digest-status ${tone}`;
  };
  const settleTermsCheck = () => {
    $("build-intent").disabled = Boolean(integrityFault);
    if (integrityFault) setFormError(`Refusing to render a local description: ${integrityFault}`);
  };
  const renderTerms = async () => {
    const declared = TERMS.digest;
    $("terms-digest").textContent = declared;
    $("terms-json").textContent = JSON.stringify(TERMS.canonicalTerms, null, 2);
    $("manifest-json").textContent = JSON.stringify(MANIFEST, null, 2);
    $("term-template").textContent = TERMS.canonicalTerms.templateId;
    $("term-outcomes").textContent = `${TERMS.canonicalTerms.outcomeCount} / ${TERMS.canonicalTerms.maxOutcomes}`;
    $("term-rounding").textContent = TERMS.canonicalTerms.rounding;
    $("term-redemption").textContent = TERMS.canonicalTerms.redemption;
    $("terms-semantics").textContent = TERMS.semanticsNote;
    if (MANIFEST.terms.digest !== declared) {
      integrityFault = "manifest.json and terms.json declare different terms digests.";
      setDigestStatus(integrityFault, "bad");
    } else if (!subtleAvailable()) {
      setDigestStatus("Declared fixture digest shown as published. Web Crypto is unavailable in this context, so it was not recomputed here — verify terms.json yourself.", "unknown");
    } else {
      const computed = await sha256(canonicalJson(TERMS.canonicalTerms));
      if (computed === declared) setDigestStatus("Recomputed locally from bundled canonical terms; matches the declared fixture digest.", "good");
      else {
        integrityFault = `Declared terms digest does not match the locally recomputed ${computed}.`;
        setDigestStatus(integrityFault, "bad");
      }
    }
    if (integrityFault) $("intent-state").textContent = "refused · integrity fault";
    settleTermsCheck();
  };

  const buildIntent = (kind, account, amount) => {
    const normalizedAmount = amount === "" ? null : amount;
    if (normalizedAmount !== null && !/^[0-9]+$/.test(normalizedAmount)) throw new Error("Collateral atoms must be an exact non-negative integer.");
    if (account.length > 160) throw new Error("External reference is too long for a local description.");
    return { schema: "dragon-clutch.transaction-intent.v0", mode: "offline-inspection-only", intent: kind, target: { externalReference: account || null }, quantities: { collateralAtoms: normalizedAmount }, termsVersion: TERMS.termsVersion, termsDigest: TERMS.digest, authorization: { wallet: "not-connected", signer: "none", signature: null, submission: "disabled" }, postcondition: "No chain state changes. This object is a local preview only." };
  };
  const setFormError = (message) => {
    const error = $("form-error");
    if (!error) return;
    error.textContent = message;
    error.hidden = false;
    error.focus();
  };
  const renderIntent = (event) => {
    event.preventDefault();
    $("form-error").hidden = true;
    try {
      if (integrityFault) throw new Error(`Refusing to render a local description: ${integrityFault}`);
      lastIntent = JSON.stringify(buildIntent($("intent-kind").value, $("account-ref").value.trim(), $("amount").value.trim()), null, 2);
      $("intent-json").textContent = lastIntent;
      $("intent-state").textContent = "local object created · unsigned";
      $("copy-intent").disabled = false;
    } catch (error) {
      setFormError(error.message);
      $("intent-state").textContent = "refused";
      $("copy-intent").disabled = true;
    }
  };
  const copyText = async (text, button, label) => {
    if (!text) throw new Error("There is no local text to copy yet.");
    if (navigator.clipboard && window.isSecureContext) await navigator.clipboard.writeText(text);
    else {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.className = "copy-area";
      document.body.appendChild(area);
      area.select();
      const copied = document.execCommand("copy");
      area.remove();
      if (!copied) throw new Error("The browser did not allow the local copy action.");
    }
    const original = button.textContent;
    button.textContent = "Copied";
    button.setAttribute("aria-label", `${label} copied`);
    window.setTimeout(() => { button.textContent = original; button.setAttribute("aria-label", label); }, 1100);
  };
  const handleCopy = async (text, button, label) => {
    try { await copyText(text, button, label); } catch (error) { setFormError(`Copy unavailable: ${error.message}`); }
  };
  const renderUnavailable = (reason) => {
    const button = $("build-intent");
    if (button) button.disabled = true;
    const main = $("main");
    reset(main);
    const section = create("section", "unavailable-state");
    section.append(create("p", "eyebrow", "Offline evidence lens"), create("h1", null, "Evidence snapshot unavailable"), create("p", null, reason));
    main.append(section);
  };
  const init = () => {
    const validated = validateDisplaySnapshot(MANIFEST && MANIFEST.displaySnapshot, { manifest: MANIFEST, terms: TERMS });
    const view = deriveDisplayView(validated);
    if (view.mode === "unavailable") {
      renderUnavailable(validated.reason || view.headline);
      return;
    }
    renderEvidenceRail(view.rail);
    renderBasisComparison(view.basisComparison);
    renderLifecycle(view.lifecycle);
    renderBoundaries(view.boundaries, "boundary-ledger");
    renderBoundaries(view.worktreeBoundaries, "roadmap-list");
    renderReleaseIdentity(view.snapshot);
    renderFixtures(view.localFixtures);
    $("build-intent").disabled = true;
    $("intent-form").addEventListener("submit", renderIntent);
    $("copy-intent").addEventListener("click", () => handleCopy(lastIntent, $("copy-intent"), "Copy intent JSON"));
    $("copy-digest").addEventListener("click", () => handleCopy($("terms-digest").textContent, $("copy-digest"), "Copy terms digest"));
    renderTerms().catch((error) => {
      integrityFault = `the terms digest check did not complete (${error.message}).`;
      setDigestStatus(integrityFault, "bad");
      settleTermsCheck();
    });
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init);
  else init();

  window.StaticClientOffline = Object.freeze({ buildIntent, canonicalJson, canonicalize, validateDisplaySnapshot, deriveDisplayView, formatEvidence });
})();
