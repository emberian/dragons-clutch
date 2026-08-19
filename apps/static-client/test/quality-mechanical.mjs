/*
 * Redesign-resistant quality gates for the static client.
 *
 * These checks deliberately inspect structure and security boundaries rather
 * than layout, class names, or the exact arrangement of the page. The reviewed
 * data mirror remains covered by test/smoke.mjs; this file does not duplicate
 * that equality test.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");

const html = read("index.html");
const css = read("styles.css");
const manifest = JSON.parse(read("manifest.json"));

const browserScripts = ["app.js", "embedded-data.js", "native-bspline-v1.js"];
const browserSources = browserScripts.map((name) => [name, read(name)]);

// This is intentionally a small HTML tokenizer, not a DOM implementation.
// The quality gates need tag names and attributes only, and keeping the test
// dependency-free makes it runnable from a checkout or file:// copy.
const tokens = (source) => {
  const output = [];
  const tokenPattern = /<!--[\s\S]*?-->|<![^>]*>|<\/?[A-Za-z][^>]*>/g;
  for (const match of source.matchAll(tokenPattern)) {
    const raw = match[0];
    if (raw.startsWith("<!--") || raw.startsWith("<!")) continue;
    const opening = /^<\s*([A-Za-z][\w:-]*)\b([\s\S]*?)>$/i.exec(raw);
    const closing = /^<\s*\/\s*([A-Za-z][\w:-]*)\s*>$/i.exec(raw);
    if (closing) {
      output.push({ raw, name: closing[1].toLowerCase(), closing: true, index: match.index });
      continue;
    }
    if (!opening) continue;
    output.push({
      raw,
      name: opening[1].toLowerCase(),
      attrs: parseAttributes(opening[2]),
      closing: false,
      index: match.index
    });
  }
  return output;
};

function parseAttributes(source) {
  const attrs = new Map();
  const attrPattern = /([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g;
  for (const match of source.matchAll(attrPattern)) {
    const name = match[1].toLowerCase();
    if (name === "/") continue;
    attrs.set(name, match[2] ?? match[3] ?? match[4] ?? "");
  }
  return attrs;
}

const htmlTokens = tokens(html);
const openingTags = (name) => htmlTokens.filter((token) => !token.closing && (!name || token.name === name));
const attributes = (token) => token.attrs || new Map();

const countMatches = (source, pattern) => Array.from(source.matchAll(pattern)).length;

const stripJavaScriptComments = (source) => source
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .replace(/(^|[^:])\/\/.*$/gm, "$1");

const visibleText = html
  .replace(/<!--[\s\S]*?-->/g, " ")
  .replace(/<script\b[\s\S]*?<\/script\s*>/gi, " ")
  .replace(/<style\b[\s\S]*?<\/style\s*>/gi, " ")
  .replace(/<[^>]*>/g, " ")
  .replace(/\s+/g, " ");

test("html_has_one_main_heading_and_a_working_skip_target", () => {
  assert.equal(openingTags("main").length, 1, "the document should have one main landmark");
  assert.equal(openingTags("h1").length, 1, "the document should have one primary heading");

  const ids = new Set(openingTags().flatMap((token) => {
    const id = attributes(token).get("id");
    return id ? [id] : [];
  }));
  const skipLinks = openingTags("a").filter((token) => {
    const attrs = attributes(token);
    return (attrs.get("class") || "").split(/\s+/).includes("skip-link");
  });
  assert.equal(skipLinks.length, 1, "the page should expose one skip link");
  assert.equal(attributes(skipLinks[0]).get("href"), "#main", "the skip link target should be #main");
  assert.ok(ids.has("main"), "the skip link target must exist");
});

test("html_ids_are_unique_and_id_references_resolve", () => {
  const seen = new Map();
  for (const token of openingTags()) {
    const id = attributes(token).get("id");
    if (!id) continue;
    assert.doesNotMatch(id, /\s/, `${token.name} id must not contain whitespace`);
    assert.ok(!seen.has(id), `duplicate id: ${id}`);
    seen.set(id, token);
  }

  for (const token of openingTags()) {
    const attrs = attributes(token);
    for (const attribute of ["aria-describedby", "aria-labelledby", "aria-controls", "aria-owns", "aria-activedescendant"]) {
      const value = attrs.get(attribute);
      if (!value) continue;
      for (const id of value.split(/\s+/).filter(Boolean)) {
        assert.ok(seen.has(id), `${token.name} ${attribute} references missing #${id}`);
      }
    }
  }
});

test("labels_reference_controls_and_controls_are_labelled", () => {
  const controls = openingTags().filter((token) => ["input", "select", "textarea"].includes(token.name));
  const labels = [];
  const labelPattern = /<label\b([^>]*)>([\s\S]*?)<\/label\s*>/gi;
  for (const match of html.matchAll(labelPattern)) {
    const opening = parseAttributes(match[1]);
    const target = opening.get("for");
    const start = match.index;
    const end = start + match[0].length;
    const descendants = controls.filter((control) => control.index > start && control.index < end);
    if (target) {
      const targetControl = controls.find((control) => attributes(control).get("id") === target);
      assert.ok(targetControl, `label for="${target}" must reference a form control`);
      labels.push({ start, end, targetControl });
    } else {
      assert.ok(descendants.length > 0, "a label without for must contain its form control");
      labels.push({ start, end, descendants });
    }
  }

  for (const control of controls) {
    const id = attributes(control).get("id");
    const associated = labels.some((label) => label.descendants?.includes(control) || label.targetControl === control);
    assert.ok(associated, `${control.name}${id ? `#${id}` : ""} must have an associated label`);
  }
});

test("buttons_have_explicit_native_types_and_images_have_alt_text", () => {
  for (const button of openingTags("button")) {
    const type = attributes(button).get("type");
    assert.ok(type, "every button must declare type explicitly");
    assert.match(type, /^(?:button|submit|reset)$/i, `unsupported button type: ${type}`);
  }
  for (const image of openingTags("img")) {
    assert.ok(attributes(image).has("alt"), "every image must declare alt, including decorative images (alt=\"\")");
  }
});

test("html_has_no_inline_code_or_external_asset_references", () => {
  assert.equal(openingTags("style").length, 0, "styles must be in the local stylesheet");
  assert.equal(countMatches(html, /\bon[a-z]+\s*=/gi), 0, "inline event handlers are not allowed");
  assert.equal(countMatches(html, /\bstyle\s*=/gi), 0, "inline style attributes are not allowed");

  for (const match of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/gi)) {
    assert.equal(match[2].trim(), "", "a script tag with src must not carry an inline fallback body");
  }
  for (const script of openingTags("script")) {
    const attrs = attributes(script);
    assert.ok(attrs.has("src"), "scripts must be external local files, not inline code");
    assert.doesNotMatch(attrs.get("src"), /^(?:[a-z][a-z\d+.-]*:|\/\/)/i, `non-local script: ${attrs.get("src")}`);
  }

  for (const token of openingTags()) {
    const attrs = attributes(token);
    for (const attribute of ["src", "href"]) {
      const value = attrs.get(attribute);
      if (!value) continue;
      assert.doesNotMatch(value, /^(?:[a-z][a-z\d+.-]*:|\/\/)/i, `non-local ${attribute}: ${value}`);
    }
  }
});

test("browser_sources_do_not_use_network_capable_apis_or_urls", () => {
  const forbiddenApis = /\b(?:fetch|XMLHttpRequest|WebSocket|EventSource|SharedWorker|BroadcastChannel|Worker)\b|\bnavigator\.(?:sendBeacon|serviceWorker)\b|\b(?:location\.(?:assign|replace)|window\.open)\b|\bimport\s*\(/;
  for (const [name, source] of browserSources) {
    const executable = stripJavaScriptComments(source);
    assert.doesNotMatch(executable, forbiddenApis, `${name} must remain offline`);
    assert.doesNotMatch(executable, /\b(?:https?:)?\/\//i, `${name} must not embed a network URL`);
    assert.doesNotMatch(executable, /document\.createElement\s*\(\s*["']script["']/i, `${name} must not inject scripts`);
  }

  assert.doesNotMatch(css, /url\s*\(\s*["']?(?:https?:|\/\/)/i, "styles must not load remote assets");
});

test("deployment_identity_is_explicitly_unpublished_and_not_a_placeholder_claim", () => {
  assert.equal(manifest.application.official, false, "the static client must not claim official status");
  assert.match(manifest.application.releaseStatus, /offline|prototype|unpublished/i);

  const identity = manifest.releaseIdentity;
  for (const field of ["ipfsCid", "githubPagesMirror"]) assert.equal(identity[field], null, `${field} must remain unassigned`);
  for (const field of ["sourceCommit", "bundleSha256", "manifestSha256"]) {
    assert.match(identity[field], /UNBOUND|UNPUBLISHED/i, `${field} must remain visibly unpublished`);
  }
  for (const cluster of manifest.clusters) {
    assert.equal(cluster.endpoint, null, `${cluster.id} must not embed an RPC endpoint`);
    assert.equal(cluster.status, "unavailable", `${cluster.id} must not be presented as deployed`);
  }
  for (const program of manifest.programs) {
    assert.equal(program.programId, null, `${program.key} must not claim a deployed program id`);
    assert.equal(program.deploymentManifest, null, `${program.key} must not claim a deployment manifest`);
  }
});

test("offline_capability_labels_use_the_exact_declared_wording", () => {
  for (const phrase of ["OFFLINE BUILD", "No RPC at startup", "No wallet auto-connect", "No signing or submission"]) {
    assert.equal(countMatches(visibleText, new RegExp(`\\b${phrase}\\b`, "g")), 1, `capability wording must remain exact: ${phrase}`);
  }
  assert.equal(manifest.capabilities.rpcReads, false);
  assert.equal(manifest.capabilities.walletConnection, false);
  assert.equal(manifest.capabilities.transactionSigning, false);
  assert.equal(manifest.capabilities.transactionSubmission, false);
});

const hasMotion = /(?:animation(?:-name)?|transition(?:-duration)?|scroll-behavior)\s*:/i.test(css);
if (hasMotion) {
  test("motion_rules_have_a_reduced_motion_target", () => {
    assert.match(css, /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/i);
  });
} else {
  test.todo("TODO: add a prefers-reduced-motion target when the stylesheet introduces motion");
}

const hasForcedColors = /@media\s*\(\s*forced-colors\s*:/i.test(css);
if (hasForcedColors) {
  test("forced_colors_rules_have_a_forced_colors_target", () => {
    assert.match(css, /@media\s*\(\s*forced-colors\s*:\s*active\s*\)/i);
  });
} else {
  test.todo("TODO: add forced-colors targets if the stylesheet adds high-contrast-specific styling");
}
