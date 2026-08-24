/* Chain-attached, read-only, unsigned Glass application. */
(function (root) {
  "use strict";

  const CHAIN = root.GlassChainClient;
  const COMPILER = root.GlassCompilerProposal;
  const state = { configuration: null, snapshot: null, compilerProposal: null, construction: null };
  let compilerRevision = 0;
  const $ = (id) => document.getElementById(id);
  const create = (tag, className, value) => {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (value !== undefined) node.textContent = value;
    return node;
  };
  const reset = (node) => node.replaceChildren();
  const definition = (term, value) => {
    const row = create("div");
    row.append(create("dt", null, term), create("dd", "mono", value === null || value === undefined ? "not exposed" : String(value)));
    return row;
  };
  const setError = (id, message) => {
    const node = $(id);
    node.textContent = message;
    node.hidden = false;
  };
  const clearError = (id) => {
    const node = $(id);
    node.textContent = "";
    node.hidden = true;
  };
  const canonicalize = (value) => Array.isArray(value) ? value.map(canonicalize) : value && typeof value === "object"
    ? Object.keys(value).sort().reduce((output, key) => { output[key] = canonicalize(value[key]); return output; }, {})
    : value;
  const canonicalJson = (value) => JSON.stringify(canonicalize(value));
  const digest = async (bytes) => {
    if (!root.crypto || !root.crypto.subtle) return null;
    const output = await root.crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(output), (byte) => byte.toString(16).padStart(2, "0")).join("");
  };
  const short = (value, head = 8, tail = 6) => value && value.length > head + tail + 2 ? `${value.slice(0, head)}…${value.slice(-tail)}` : value;

  const readConfigurationForm = () => ({
    operatorUrl: $("operator-url").value.trim(),
    commitment: $("commitment").value,
    bounds: {
      maximumAccounts: $("maximum-accounts").value.trim(),
      maximumResponseBytes: $("maximum-response-bytes").value.trim(),
      timeoutMilliseconds: $("timeout-milliseconds").value.trim(),
      maximumSlotLag: $("maximum-slot-lag").value.trim()
    }
  });

  const renderConfiguration = (configuration) => {
    state.configuration = configuration;
    state.snapshot = null;
    state.compilerProposal = null;
    state.construction = null;
    const status = $("configuration-status");
    status.className = "status-panel incomplete";
    status.textContent = "Explicit operatord target recorded. Cluster, RPC identity, decoder set, and release remain unknown until bounded acquisition.";
    $("configuration-json").textContent = JSON.stringify(CHAIN.redactedConfiguration(configuration), null, 2);
    $("read-chain").disabled = false;
    $("export-configuration").disabled = false;
    resetProjection("No daemon-projected chain release has been acquired from this operatord.");
    resetCompiler("waiting for pure Rust compiler output");
  };

  const resetCompiler = (status) => {
    state.compilerProposal = null;
    $("compiler-status").textContent = status;
    reset($("compiler-identities"));
    reset($("compiler-bounds"));
    $("compiler-output").textContent = "No compiler proposal bound.\n\nThe Rust compiler is not reimplemented in this page.\nRegistration remains authority.";
    $("copy-compiler-output").disabled = true;
  };

  const resetProjection = (message) => {
    const status = $("projection-status");
    status.className = "status-panel incomplete";
    status.textContent = message;
    reset($("release-identity"));
    reset($("observation-metrics"));
    reset($("capability-grid"));
    reset($("state-groups"));
    $("snapshot-json").textContent = "No chain projection loaded.";
    $("copy-snapshot").disabled = true;
    const keeper = $("keeper-action");
    reset(keeper);
    const option = create("option", null, "Manual explicit construction (no keeper cursor)");
    option.value = "";
    keeper.append(option);
    keeper.disabled = true;
    $("build-workflow").disabled = true;
  };

  const metric = (label, value, disposition) => {
    const card = create("section", `metric ${disposition || ""}`.trim());
    card.append(create("span", "metric-label", label), create("strong", "mono", value));
    return card;
  };

  const renderRelease = (snapshot) => {
    const target = $("release-identity");
    reset(target);
    const release = snapshot.release.observed;
    target.append(
      definition("Read-only session", snapshot.session.sessionId),
      definition("Restart identity source", snapshot.session.restart.identitySource),
      definition("Finalized restart accounts / cursors", `${snapshot.session.restart.accountCount} / ${snapshot.session.restart.cursorCount}`),
      definition("Cluster identity", snapshot.configuration.clusterKey),
      definition("Canonical decoder set", snapshot.configuration.decoderSet),
      definition("Program", release.programId),
      definition("ProgramData", release.programData),
      definition("Deployment slot", release.deploymentSlot),
      definition("ELF SHA-256", release.elfSha256),
      definition("Release-manifest SHA-256", snapshot.release.declaredManifestSha256),
      definition("Source commit", snapshot.release.declaredSourceCommit),
      definition("Capability profile", snapshot.release.declaredCapabilityProfileId),
      definition("Decoded families", release.families.join(", ")),
      definition("Manifest/capability authentication", snapshot.release.manifestSourceCapabilityAuthentication)
    );
  };

  const renderMetrics = (snapshot) => {
    const target = $("observation-metrics");
    reset(target);
    const finality = snapshot.finality;
    target.append(
      metric("Commitment", finality.requestedCommitment, finality.requestedCommitment === "finalized" ? "good" : "warn"),
      metric("Canonical session", short(snapshot.session.sessionId), "good"),
      metric("Authority eligibility", "false — projection only", "warn"),
      metric("Projected tip slot", finality.projectedTipSlot),
      metric("Finalized root", finality.finalizedRootSlot || "not observed", finality.finalizedRootSlot ? "good" : "warn"),
      metric("Selected accounts", snapshot.accountCounts.selectedRelease),
      metric("Stale by policy", finality.staleAccountCount, finality.staleAccountCount === "0" ? "good" : "warn"),
      metric("Unsafe fork rows", finality.unsafeForkAccountCount, finality.unsafeForkAccountCount === "0" ? "good" : "bad"),
      metric("Bootstrap", snapshot.acquisition.bootstrapComplete ? "complete" : "incomplete", snapshot.acquisition.bootstrapComplete ? "good" : "warn"),
      metric("Pending accounts", snapshot.acquisition.pendingAccounts, snapshot.acquisition.pendingAccounts === "0" ? "good" : "warn"),
      metric("Processed transport", finality.processedTransport ? `${finality.processedTransport.phase} · generation ${finality.processedTransport.connectionGeneration} · rollback epoch ${finality.processedTransport.rollbackEpoch} · WS genesis ${finality.processedTransport.websocketGenesisMatched ? "matched" : "unmatched"}` : "not configured", finality.requestedCommitment === "processed" ? "warn" : ""),
      metric("Processed removals", finality.processedTransport ? `${finality.processedTransport.accountProjectionsWithdrawn} withdrawn / ${finality.processedTransport.accountRemovalEvents} observed` : "0", finality.processedTransport && finality.processedTransport.accountProjectionsWithdrawn !== "0" ? "warn" : ""),
      metric("Response budget left", `${snapshot.acquisitionBounds.remainingResponseBytes} bytes`)
    );
  };

  const renderCapabilities = (snapshot) => {
    const target = $("capability-grid");
    reset(target);
    for (const capability of snapshot.capabilities) {
      const card = create("section", `capability-card ${capability.enabled ? "" : "disabled"}`.trim());
      const heading = create("div", "card-heading");
      heading.append(create("h3", null, capability.label), create("span", `chip ${capability.enabled ? "" : "disabled-chip"}`.trim(), capability.allocationStatus));
      card.append(heading, create("p", null, capability.reason));
      const footer = create("p", "micro");
      footer.textContent = capability.indexedByRelease ? "Relevant decoder or projected state is visible for this release." : "Relevant decoder and projected state are absent from this bounded release view.";
      card.append(footer);
      target.append(card);
    }
  };

  const accountCard = (account) => {
    const warning = account.stale || account.forkState === "dead-fork" || account.forkState === "unidentified-fork";
    const row = create("article", `account-row ${warning ? "account-warning" : ""}`.trim());
    const heading = create("div", "account-heading");
    heading.append(create("strong", null, account.kind), create("span", "chip", account.forkState));
    row.append(heading);
    const facts = create("dl", "compact-facts");
    facts.append(
      definition("Address", account.address),
      definition("Slot / lag", `${account.slot} / ${account.slotLag}`),
      definition("Lamports", account.lamports),
      definition("Data bytes", account.dataBytes),
      definition("Codec tag / version", `${account.accountTag} / ${account.accountVersion}`),
      definition("Generation", account.generation),
      definition(account.bindingProjection.primary.label, account.bindingProjection.primary.value),
      definition(account.bindingProjection.secondary.label, account.bindingProjection.secondary.value),
      definition("Projection authority", account.bindingProjection.authority),
      definition("Body SHA-256", account.dataSha256),
      definition("Decode", account.decode.status === "requires-context" ? `requires context: ${account.decode.requirement}` : "canonical")
    );
    row.append(facts);
    return row;
  };

  const renderState = (snapshot) => {
    const target = $("state-groups");
    reset(target);
    for (const groupName of CHAIN.groupOrder) {
      if (groupName === "other" && snapshot.groups[groupName].length === 0) continue;
      const section = create("section", "state-family");
      const heading = create("div", "state-family-heading");
      const title = create("div");
      title.append(create("p", "eyebrow", "Untrusted indexed accounts"), create("h3", null, snapshot.groupLabels[groupName]));
      heading.append(title, create("span", "count mono", snapshot.groups[groupName].length.toString()));
      section.append(heading);
      const rows = create("div", "account-list");
      if (snapshot.groups[groupName].length === 0) {
        rows.append(create("p", "empty-state", "No selected-release account of this family is present in the current bounded projection. This is not proof of global absence."));
      } else {
        for (const account of snapshot.groups[groupName]) rows.append(accountCard(account));
      }
      section.append(rows);
      target.append(section);
    }
  };

  const renderKeeper = (snapshot) => {
    const select = $("keeper-action");
    reset(select);
    const actions = snapshot.actionCapabilities.actions;
    const callable = actions.filter((action) => action.callable);
    const empty = create("option", null, callable.length === 0
      ? `${actions.length} release-authenticated verdict(s); no canonical draft is callable`
      : "Select one server-constructed canonical transaction draft");
    empty.value = "";
    select.append(empty);
    actions.forEach((action, index) => {
      const option = create("option", null, `${action.coordinate.familyTag}/${action.coordinate.familyVersion}/${action.coordinate.localAction} · ${action.coordinate.action}${action.callable ? "" : ` — unavailable: ${action.reason}`}`);
      option.value = String(index);
      option.disabled = !action.callable;
      select.append(option);
    });
    select.disabled = snapshot.finality.requestedCommitment === "processed" || callable.length === 0;
    $("build-workflow").disabled = true;
    $("workflow-status").textContent = callable.length === 0 ? "no callable canonical draft" : "select a canonical draft";
  };

  const renderSnapshot = async (snapshot) => {
    state.configuration = snapshot.sourceConfiguration;
    state.snapshot = snapshot;
    state.construction = null;
    $("configuration-status").textContent = `Daemon projects ${snapshot.configuration.clusterKey} and release ${short(snapshot.release.observed.releaseKey)}. This remains untrusted browser state.`;
    $("configuration-json").textContent = JSON.stringify(CHAIN.redactedConfiguration(state.configuration), null, 2);
    const status = $("projection-status");
    const unsafe = snapshot.finality.unsafeForkAccountCount !== "0";
    const stale = snapshot.finality.staleAccountCount !== "0";
    status.className = `status-panel ${unsafe ? "bad" : stale || !snapshot.acquisition.bootstrapComplete ? "incomplete" : "ready"}`;
    const processedWarning = snapshot.finality.requestedCommitment === "processed" ? " This view is non-final, rollbackable, never authority-eligible, and cannot construct keeper workflows." : "";
    status.textContent = `Attached read-only session ${short(snapshot.session.sessionId)} with ${snapshot.session.restart.accountCount} finalized canonical restart identities and ${snapshot.session.restart.cursorCount} onchain-derived cursors. Loaded ${snapshot.accountCounts.selectedRelease} selected-release accounts at ${snapshot.finality.requestedCommitment} commitment. This is an untrusted operatord projection, not onchain authority.${processedWarning}${unsafe ? " Dead or unidentified fork rows are present." : ""}${stale ? " The configured staleness policy is exceeded." : ""}`;
    renderRelease(snapshot);
    renderMetrics(snapshot);
    renderCapabilities(snapshot);
    renderState(snapshot);
    renderKeeper(snapshot);
    const serializable = { ...snapshot, groups: undefined, groupLabels: undefined };
    $("snapshot-json").textContent = JSON.stringify(serializable, null, 2);
    $("copy-snapshot").disabled = false;
    const snapshotDigest = await digest(new TextEncoder().encode(canonicalJson(serializable)));
    if (state.snapshot === snapshot) {
      $("snapshot-digest").textContent = snapshotDigest ? `Local canonical projection SHA-256: ${snapshotDigest}` : "Web Crypto unavailable; no local projection digest was computed.";
    }
  };

  const actionForSelection = () => {
    if (!state.snapshot || $("keeper-action").value === "") return null;
    const index = Number.parseInt($("keeper-action").value, 10);
    return state.snapshot.actionCapabilities.actions[index] || null;
  };

  const parseJsonField = (id, label) => {
    try { return JSON.parse($(id).value); } catch (_) { throw new Error(`${label} is not valid JSON.`); }
  };

  const prepareCompilerRequest = async () => {
    const configuration = state.configuration;
    const revision = compilerRevision;
    if (!configuration || !configuration.release) throw new Error("Acquire the daemon-projected checked release before binding compiler output.");
    const definitionValue = COMPILER.validateDefinition(parseJsonField("compiler-definition", "Exact rational payoff definition"));
    const compilerReleaseSha256 = $("compiler-release-sha256").value.trim();
    const request = COMPILER.buildRequest(
      compilerReleaseSha256,
      configuration.release.programId,
      definitionValue,
      parseJsonField("compiler-bundle-inputs", "Canonical Product/Series bundle inputs"),
      $("compiler-exact-market-search").value.trim() === "" ? null : parseJsonField("compiler-exact-market-search", "Exact market search")
    );
    const inputCanonicalSha256 = await digest(new TextEncoder().encode(definitionValue.canonicalJson));
    const requestCanonicalSha256 = await digest(new TextEncoder().encode(COMPILER.canonicalJson(request)));
    if (inputCanonicalSha256 === null || requestCanonicalSha256 === null) throw new Error("Web Crypto SHA-256 is unavailable; the page refuses to bind compiler output without cryptographic input joins.");
    if (state.configuration !== configuration || compilerRevision !== revision) throw new Error("Compiler target or inputs changed while their exact request identity was being prepared.");
    return Object.freeze({ configuration, revision, definitionValue, compilerReleaseSha256, request, inputCanonicalSha256, requestCanonicalSha256 });
  };

  const bindCompilerProposal = async (rawProposal = null, prepared = null) => {
    const context = prepared || await prepareCompilerRequest();
    if (state.configuration !== context.configuration || compilerRevision !== context.revision) throw new Error("Compiler target or inputs changed while the proposal was in flight.");
    const proposal = COMPILER.validateProposal(
      rawProposal || parseJsonField("compiler-proposal", "operatord/CLI compiler proposal"),
      context.requestCanonicalSha256,
      context.inputCanonicalSha256,
      context.compilerReleaseSha256,
      context.definitionValue,
      context.request
    );
    const profileId = proposal.compiledProductSeriesBundleV7.identities.capabilityProfileId;
    if (profileId !== context.configuration.release.capabilityProfileId) {
      throw new Error("Compiler output capabilityProfileId differs from the daemon-projected checked release profile.");
    }

    state.compilerProposal = Object.freeze({
      definition: Object.freeze({ canonicalJson: context.definitionValue.canonicalJson, canonicalSha256: context.inputCanonicalSha256 }),
      requestCanonicalSha256: context.requestCanonicalSha256,
      proposal,
      authority: "untrusted-proposal; onchain registration remains authority"
    });
    const identities = $("compiler-identities");
    reset(identities);
    identities.append(
      definition("Payoff classification", proposal.classification),
      definition("Span status", proposal.spanStatus),
      definition("Input canonical SHA-256", proposal.inputCanonicalSha256),
      definition("Whole request canonical SHA-256", proposal.requestCanonicalSha256),
      definition("Configured compiler release SHA-256", proposal.compilerReleaseSha256),
      definition("Product Terms ID", proposal.productTermsId),
      definition("Native basis ID / bytes", `${proposal.nativeClaimBasis.id} / ${proposal.nativeClaimBasis.byteLength}`),
      definition("Certificate ID / bytes", proposal.certificate ? `${proposal.certificate.id} / ${proposal.certificate.byteLength}` : "none — categorical basis is semantic owner"),
      definition("Certification subdivision depth", proposal.subdivisionDepth),
      definition("BundleV7 ID / bytes", `${proposal.compiledProductSeriesBundleV7.id} / ${proposal.compiledProductSeriesBundleV7.byteLength}`),
      definition("BundleV7 artifact kind / PDA", `${proposal.compiledProductSeriesBundleV7.artifact.kind} / ${proposal.compiledProductSeriesBundleV7.artifact.pda}`),
      definition("Exact market outcome / coverage", proposal.exactMarket ? `${proposal.exactMarket.outcome} / ${proposal.exactMarket.coverage}` : "not requested"),
      definition("Exact certificate / work manifest", proposal.exactMarket ? `${proposal.exactMarket.certificate ? proposal.exactMarket.certificate.outputId : "none"} / ${proposal.exactMarket.workManifest.id}` : "not requested"),
      definition("Capability profile join", profileId),
      definition("Registration authority", "false — every body and join must be recomputed onchain")
    );
    const bounds = $("compiler-bounds");
    reset(bounds);
    if (proposal.bounds.length === 0) {
      bounds.append(create("p", "empty-state", "Exact output: no analytic approximation bounds apply."));
    } else {
      for (const bound of proposal.bounds) {
        const card = create("section", "bound-card");
        card.append(create("span", "metric-label", bound.name), create("strong", "mono", `${bound.value.numerator} / ${bound.value.denominator}`));
        bounds.append(card);
      }
    }
    $("compiler-status").textContent = proposal.spanStatus === "certified-approximation" ? "certified approximation · registration pending" : "exact in named representation · registration pending";
    $("compiler-output").textContent = JSON.stringify(state.compilerProposal, null, 2);
    $("copy-compiler-output").disabled = false;
  };

  const compilePayoff = async () => {
    const prepared = await prepareCompilerRequest();
    const proposal = await COMPILER.compileRemote(
      prepared.configuration.operatorUrl,
      prepared.request,
      prepared.configuration.bounds.maximumResponseBytes,
      prepared.configuration.bounds.timeoutMilliseconds
    );
    await bindCompilerProposal(proposal, prepared);
  };

  const buildWorkflow = async () => {
    if (!state.configuration || !state.snapshot) throw new Error("Acquire a chain projection before constructing a workflow node.");
    if (state.snapshot.sourceConfiguration !== state.configuration) throw new Error("The acquired projection belongs to a different explicit configuration; acquire again before constructing.");
    if (state.snapshot.finality.requestedCommitment === "processed") throw new Error("Processed observations are rollbackable and never authority-eligible; switch to finalized and reacquire before constructing a workflow.");
    const action = actionForSelection();
    if (!action || !action.callable || !action.transactionDraft) throw new Error("No release-authenticated, state-callable server transaction draft is selected. Browser-authored protocol material is forbidden.");
    const output = action.transactionDraft;
    state.construction = output;
    $("workflow-output").textContent = JSON.stringify(output, null, 2);
    $("workflow-status").textContent = "canonical unsigned draft inspected · blockhash/signing/submission absent";
    $("copy-workflow").disabled = false;
  };

  const copy = async (value, button) => {
    await navigator.clipboard.writeText(value);
    const previous = button.textContent;
    button.textContent = "Copied";
    setTimeout(() => { button.textContent = previous; }, 1100);
  };

  const initialize = () => {
    resetProjection("No configuration or account projection loaded. Nothing is inferred from fixtures or defaults.");
    resetCompiler("waiting for pure Rust compiler output");
    $("configuration-form").addEventListener("submit", (event) => {
      event.preventDefault();
      clearError("configuration-error");
      try { renderConfiguration(CHAIN.validateConfiguration(readConfigurationForm())); } catch (error) { setError("configuration-error", error.message); }
    });
    $("read-chain").addEventListener("click", async () => {
      clearError("configuration-error");
      if (!state.configuration) return setError("configuration-error", "Apply an explicit configuration first.");
      const button = $("read-chain");
      button.disabled = true;
      button.textContent = "Reading bounded endpoints…";
      const configuration = state.configuration;
      try {
        const snapshot = await CHAIN.acquire(configuration);
        if (state.configuration !== configuration) throw new Error("The explicit configuration changed while acquisition was in flight.");
        await renderSnapshot(snapshot);
      } catch (error) { resetProjection(`Acquisition refused: ${error.message}`); setError("configuration-error", error.message); }
      finally { button.disabled = false; button.textContent = "Read bounded operatord state"; }
    });
    $("export-configuration").addEventListener("click", () => { if (state.configuration) copy(JSON.stringify(CHAIN.redactedConfiguration(state.configuration), null, 2), $("export-configuration")); });
    $("copy-snapshot").addEventListener("click", () => { if (state.snapshot) copy($("snapshot-json").textContent, $("copy-snapshot")); });
    for (const id of ["compiler-release-sha256", "compiler-definition", "compiler-bundle-inputs", "compiler-exact-market-search"]) {
      $(id).addEventListener("input", () => { compilerRevision += 1; });
    }
    $("compiler-form").addEventListener("submit", async (event) => {
      event.preventDefault();
      clearError("compiler-error");
      const button = $("compile-product-exact-market");
      button.disabled = true;
      button.textContent = "Compiling through bounded endpoint…";
      try { await compilePayoff(); } catch (error) { resetCompiler("proposal refused"); setError("compiler-error", error.message); }
      finally { button.disabled = false; button.textContent = "Compile through selected operatord"; }
    });
    $("bind-compiler-proposal").addEventListener("click", async () => {
      clearError("compiler-error");
      try { await bindCompilerProposal(); } catch (error) { resetCompiler("proposal refused"); setError("compiler-error", error.message); }
    });
    $("copy-compiler-output").addEventListener("click", () => { if (state.compilerProposal) copy($("compiler-output").textContent, $("copy-compiler-output")); });
    $("workflow-form").addEventListener("submit", async (event) => {
      event.preventDefault();
      clearError("workflow-error");
      try { await buildWorkflow(); } catch (error) { setError("workflow-error", error.message); }
    });
    $("keeper-action").addEventListener("change", () => {
      const action = actionForSelection();
      const callable = Boolean(action && action.callable && action.transactionDraft);
      $("build-workflow").disabled = !callable;
      $("workflow-status").textContent = callable
        ? "canonical unsigned draft available for inspection"
        : "no callable canonical draft selected";
    });
    $("copy-workflow").addEventListener("click", () => { if (state.construction) copy($("workflow-output").textContent, $("copy-workflow")); });
  };

  root.GlassChainApp = Object.freeze({ canonicalJson, readConfigurationForm });
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", initialize, { once: true });
  else initialize();
})(typeof globalThis === "object" ? globalThis : this);
