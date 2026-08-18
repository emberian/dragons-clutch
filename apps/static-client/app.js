/*
 * Glass offline client. This file deliberately has no network, wallet, RPC,
 * signing, or submission capability. Keep the data below in lockstep with the
 * checked-in manifest.json and terms.json until a reproducible build emits it.
 */
(function () {
  "use strict";

  const MANIFEST = {
    application: { name: "Glass / Dragon's Clutch", version: "0.1.0-offline" },
    releaseIdentity: {
      sourceCommit: "UNBOUND-OFFLINE-SNAPSHOT",
      bundleSha256: "UNPUBLISHED-BUNDLE-DIGEST",
      ipfsCid: null
    },
    clusters: [
      { id: "mainnet-beta", label: "Solana Mainnet Beta", status: "unavailable", note: "No RPC endpoint is embedded or contacted by this build." },
      { id: "devnet", label: "Solana Devnet", status: "unavailable", note: "No devnet deployment is checked by this build." },
      { id: "localnet", label: "Local validator", status: "unavailable", note: "A local validator is optional future infrastructure, not started here." }
    ],
    programs: [
      { key: "clutch", label: "Dragon's Clutch protocol", programId: null, status: "not-deployed", note: "No program ID or ELF release has been checked." }
    ],
    profiles: [
      { id: "synthetic-six-decimal", label: "Synthetic six-decimal Realm", status: "offline-fixture", mint: "SYNTHETIC-MINT-NOT-ONCHAIN", note: "A local shape-only fixture. It has no collateral value and no chain identity." },
      { id: "dregg-reference", label: "DREGG reference Realm", status: "reference-only-unchecked", mint: "XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump", note: "Reference only; no deployment or account read is asserted." }
    ]
  };

  const TERMS = {
    schemaVersion: "dragon-clutch.terms.v0",
    termsVersion: "terms-v0-offline-sample",
    canonicalTerms: {
      collateralProfile: "synthetic-six-decimal",
      feeAtoms: 0,
      maxOutcomes: 16,
      outcomeCount: 2,
      rounding: "exact-scaled-integer-floor-at-final-payout-boundary",
      sourceAdapter: "none-offline-fixture",
      sourceVersion: "unbound",
      state: "inspection-only",
      templateId: "offline-sample-template-v0",
      window: { endUnixSeconds: 0, startUnixSeconds: 0 }
    }
  };

  const $ = (id) => document.getElementById(id);
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

  // Synchronous fallback keeps the local file:// experience useful. A secure
  // host additionally gets a real SHA-256 comparison through Web Crypto.
  const fallbackDigest = (text) => {
    let hash = 2166136261;
    for (let i = 0; i < text.length; i += 1) {
      hash ^= text.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    return `fnv1a32:${(hash >>> 0).toString(16).padStart(8, "0")}`;
  };
  const localDigest = async (text) => {
    if (globalThis.crypto && globalThis.crypto.subtle && globalThis.TextEncoder) {
      const bytes = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
      return `sha256:${Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    }
    return fallbackDigest(text);
  };
  const short = (value, width = 26) => value && value.length > width ? `${value.slice(0, width - 1)}…` : (value || "—");

  const populateSelect = (select, values, valueLabel) => {
    values.forEach((entry) => {
      const option = document.createElement("option");
      option.value = entry.id || entry.key;
      option.textContent = valueLabel(entry);
      select.appendChild(option);
    });
  };

  const selectedBinding = () => ({
    cluster: MANIFEST.clusters.find((entry) => entry.id === $("cluster-select").value),
    program: MANIFEST.programs.find((entry) => entry.key === $("program-select").value),
    profile: MANIFEST.profiles.find((entry) => entry.id === $("profile-select").value)
  });

  const renderBinding = () => {
    const binding = selectedBinding();
    $("cluster-note").textContent = binding.cluster.note;
    $("program-note").textContent = binding.program.note;
    $("profile-note").textContent = `${binding.profile.status} · ${short(binding.profile.mint, 31)}`;
    $("binding-status-title").textContent = "No executable chain binding";
    $("binding-status-copy").textContent = `${binding.cluster.label}, ${binding.program.label}, and ${binding.profile.label} are labels only until checked release data exists.`;
  };

  const buildIntent = (kind, account, amount, binding) => {
    const normalizedAmount = amount === "" ? null : amount;
    if (normalizedAmount !== null && !/^[0-9]+$/.test(normalizedAmount)) throw new Error("Amount must be an exact non-negative integer in atoms.");
    if (account.length > 160) throw new Error("Account reference is too long for a local intent.");
    return {
      schema: "dragon-clutch.transaction-intent.v0",
      mode: "offline-inspection-only",
      intent: kind,
      target: {
        cluster: binding.cluster.id,
        programKey: binding.program.key,
        programId: binding.program.programId,
        profile: binding.profile.id,
        accountReference: account || null
      },
      quantities: { collateralAtoms: normalizedAmount },
      termsDigest: "sha256:a21f6cbb1ab3b06afc7c8625f3388835843edb17c48173e8fb57df8b7e0dd8e8",
      authorization: {
        wallet: "not-connected",
        signer: "none",
        signature: null,
        submission: "disabled"
      },
      postcondition: "No chain state changes. This object is a local preview only."
    };
  };

  const renderTerms = async () => {
    const canonical = canonicalJson(TERMS.canonicalTerms);
    const digest = await localDigest(canonical);
    $("terms-digest").textContent = digest;
    $("terms-json").textContent = JSON.stringify(TERMS.canonicalTerms, null, 2);
    $("term-template").textContent = TERMS.canonicalTerms.templateId;
    $("term-outcomes").textContent = `${TERMS.canonicalTerms.outcomeCount} / ${TERMS.canonicalTerms.maxOutcomes}`;
    $("term-rounding").textContent = "floor at payout";
    return digest;
  };

  const renderIntent = (event) => {
    event.preventDefault();
    const error = $("form-error");
    error.hidden = true;
    try {
      const intent = buildIntent($("intent-kind").value, $("account-ref").value.trim(), $("amount").value.trim(), selectedBinding());
      $("intent-json").textContent = JSON.stringify(intent, null, 2);
      $("intent-state").textContent = "local object created · unsigned";
      $("copy-intent").disabled = false;
      window.__lastIntentPreview = JSON.stringify(intent, null, 2);
    } catch (buildError) {
      error.textContent = buildError.message;
      error.hidden = false;
      $("intent-state").textContent = "refused";
      $("copy-intent").disabled = true;
    }
  };

  const copyText = async (text, button) => {
    if (navigator.clipboard && window.isSecureContext) await navigator.clipboard.writeText(text);
    else {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    const original = button.textContent;
    button.textContent = "Copied";
    window.setTimeout(() => { button.textContent = original; }, 1100);
  };

  const init = () => {
    populateSelect($("cluster-select"), MANIFEST.clusters, (entry) => `${entry.label} · unavailable`);
    populateSelect($("program-select"), MANIFEST.programs, (entry) => `${entry.label} · not deployed`);
    populateSelect($("profile-select"), MANIFEST.profiles, (entry) => entry.label);
    ["cluster-select", "program-select", "profile-select"].forEach((id) => $(id).addEventListener("change", renderBinding));
    $("intent-form").addEventListener("submit", renderIntent);
    $("copy-intent").addEventListener("click", () => copyText(window.__lastIntentPreview, $("copy-intent")));
    $("copy-digest").addEventListener("click", () => copyText($("terms-digest").textContent, $("copy-digest")));
    renderBinding();
    renderTerms();
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init);
  else init();

  // Expose pure local construction for a future test harness, never a signer.
  window.StaticClientOffline = Object.freeze({ buildIntent, canonicalJson, fallbackDigest });
})();
