/*
 * Glass offline client. This file deliberately has no network, wallet, RPC,
 * signing, or submission capability.
 *
 * It holds no copy of the release data. manifest.json and terms.json are
 * mirrored once into embedded-data.js (regenerate with `npm run embed`), and
 * the test `embedded_static_data_equals_reviewed_manifest_and_terms` fails the
 * build if that mirror drifts from the reviewed files. Nothing here re-states a
 * digest, note, or binding as a second literal.
 */
(function () {
  "use strict";

  const EMBEDDED = globalThis.GlassEmbeddedData;
  const MANIFEST = EMBEDDED && EMBEDDED.manifest;
  const TERMS = EMBEDDED && EMBEDDED.terms;

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

  // SHA-256 through Web Crypto or nothing. An insecure context (including a
  // plain file:// open) has no Web Crypto, and a cheap non-cryptographic
  // checksum shown under a SHA-256 label would be worse than an honest "not
  // recomputed here": it looks like verification and is not.
  const subtleAvailable = () => Boolean(globalThis.crypto && globalThis.crypto.subtle && globalThis.TextEncoder);
  const sha256 = async (text) => {
    const bytes = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
    return `sha256:${Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  };
  const short = (value, width = 26) => value && value.length > width ? `${value.slice(0, width - 1)}…` : (value || "—");

  // Set when a locally checkable integrity claim fails. The page then refuses
  // to compose intents instead of presenting a binding it cannot stand behind.
  let integrityFault = null;

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

  const buildIntent = (kind, account, amount, binding, termsDigest) => {
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
      termsVersion: TERMS.termsVersion,
      termsDigest: termsDigest || TERMS.digest,
      authorization: {
        wallet: "not-connected",
        signer: "none",
        signature: null,
        submission: "disabled"
      },
      postcondition: "No chain state changes. This object is a local preview only."
    };
  };

  // The digest check is asynchronous, so composition stays closed until it
  // settles. An early submit must not slip past a check that has not run.
  const settleTermsCheck = () => {
    $("build-intent").disabled = Boolean(integrityFault);
    if (!integrityFault) return;
    const error = $("form-error");
    error.textContent = `Refusing to compose an intent: ${integrityFault}`;
    error.hidden = false;
  };

  const setDigestStatus = (text, tone) => {
    const status = $("digest-status");
    status.textContent = text;
    status.className = `digest-status ${tone}`;
  };

  const renderTerms = async () => {
    const declared = TERMS.digest;
    $("terms-digest").textContent = declared;
    $("terms-json").textContent = JSON.stringify(TERMS.canonicalTerms, null, 2);
    $("term-template").textContent = TERMS.canonicalTerms.templateId;
    $("term-outcomes").textContent = `${TERMS.canonicalTerms.outcomeCount} / ${TERMS.canonicalTerms.maxOutcomes}`;
    $("term-rounding").textContent = TERMS.canonicalTerms.rounding;
    $("term-redemption").textContent = TERMS.canonicalTerms.redemption;
    $("terms-semantics").textContent = TERMS.semanticsNote;

    if (MANIFEST.terms && MANIFEST.terms.digest !== declared) {
      integrityFault = "manifest.json and terms.json declare different terms digests.";
      setDigestStatus(integrityFault, "bad");
    } else if (!subtleAvailable()) {
      setDigestStatus("Declared digest shown as published. Web Crypto is unavailable in this context, so it was not recomputed here — verify terms.json yourself.", "unknown");
    } else {
      const computed = await sha256(canonicalJson(TERMS.canonicalTerms));
      if (computed === declared) {
        setDigestStatus("Recomputed locally from the displayed canonical terms; matches the declared digest.", "good");
      } else {
        integrityFault = `Declared terms digest does not match the locally recomputed ${computed}.`;
        setDigestStatus(integrityFault, "bad");
      }
    }
    if (integrityFault) $("intent-state").textContent = "refused · integrity fault";
    settleTermsCheck();
    return declared;
  };

  const renderIntent = (event) => {
    event.preventDefault();
    const error = $("form-error");
    error.hidden = true;
    try {
      if (integrityFault) throw new Error(`Refusing to compose an intent: ${integrityFault}`);
      const intent = buildIntent($("intent-kind").value, $("account-ref").value.trim(), $("amount").value.trim(), selectedBinding());
      $("intent-json").textContent = JSON.stringify(intent, null, 2);
      $("intent-state").textContent = "local object created · unsigned";
      $("copy-intent").disabled = false;
      window.__lastIntentPreview = JSON.stringify(intent, null, 2);
    } catch (buildError) {
      error.textContent = buildError.message;
      error.hidden = false;
      error.focus();
      $("intent-state").textContent = "refused";
      $("copy-intent").disabled = true;
    }
  };

  const copyText = async (text, button, label) => {
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
    button.setAttribute("aria-label", `${label} copied`);
    window.setTimeout(() => {
      button.textContent = original;
      button.setAttribute("aria-label", label);
    }, 1100);
  };

  const refuseToRender = (message) => {
    const error = $("form-error");
    if (error) {
      error.textContent = message;
      error.hidden = false;
    }
    const digest = $("terms-digest");
    if (digest) digest.textContent = "unavailable";
    const status = $("digest-status");
    if (status) {
      status.textContent = message;
      status.className = "digest-status bad";
    }
    const state = $("intent-state");
    if (state) state.textContent = "refused · no embedded data";
    const button = $("build-intent");
    if (button) button.disabled = true;
  };

  const init = () => {
    if (!MANIFEST || !TERMS) {
      refuseToRender("embedded-data.js did not load, so no release data is available. This page shows nothing rather than guessing a binding.");
      return;
    }
    populateSelect($("cluster-select"), MANIFEST.clusters, (entry) => `${entry.label} · ${entry.status}`);
    populateSelect($("program-select"), MANIFEST.programs, (entry) => `${entry.label} · ${entry.status}`);
    populateSelect($("profile-select"), MANIFEST.profiles, (entry) => entry.label);
    $("build-intent").disabled = true;
    ["cluster-select", "program-select", "profile-select"].forEach((id) => $(id).addEventListener("change", renderBinding));
    $("intent-form").addEventListener("submit", renderIntent);
    $("copy-intent").addEventListener("click", () => copyText(window.__lastIntentPreview, $("copy-intent"), "Copy intent JSON"));
    $("copy-digest").addEventListener("click", () => copyText($("terms-digest").textContent, $("copy-digest"), "Copy terms digest"));
    renderBinding();
    renderTerms().catch((checkError) => {
      integrityFault = `the terms digest check did not complete (${checkError.message}).`;
      setDigestStatus(integrityFault, "bad");
      settleTermsCheck();
    });
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init);
  else init();

  // Expose pure local construction for a future test harness, never a signer.
  window.StaticClientOffline = Object.freeze({ buildIntent, canonicalJson, canonicalize });
})();
