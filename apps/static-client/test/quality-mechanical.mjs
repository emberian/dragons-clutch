import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
const html = read("index.html");
const css = read("styles.css");
const app = read("app.js");

test("document_has_landmarks_one_h1_and_resolved_local_navigation", () => {
  assert.equal((html.match(/<main\b/g) || []).length, 1);
  assert.equal((html.match(/<h1\b/g) || []).length, 1);
  assert.match(html, /class="skip-link" href="#main"/);
  const ids = Array.from(html.matchAll(/\bid="([^"]+)"/g), (match) => match[1]);
  assert.equal(new Set(ids).size, ids.length, "HTML ids must be unique");
  for (const target of Array.from(html.matchAll(/href="#([^"]+)"/g), (match) => match[1])) assert.ok(ids.includes(target), `missing #${target}`);
});

test("all_form_controls_are_inside_labels_and_buttons_have_types", () => {
  const controls = (html.match(/<(?:input|select|textarea)\b/g) || []).length;
  const labelsWithControls = (html.match(/<label\b[^>]*>[\s\S]*?<(?:input|select|textarea)\b[\s\S]*?<\/label>/g) || []).length;
  assert.equal(labelsWithControls, controls);
  for (const button of html.match(/<button\b[^>]*>/g) || []) assert.match(button, /\btype="(?:button|submit|reset)"/);
});

test("app_element_ids_resolve_to_html", () => {
  const ids = new Set(Array.from(html.matchAll(/\bid="([^"]+)"/g), (match) => match[1]));
  const addressed = new Set(Array.from(app.matchAll(/\$\("([^"]+)"\)/g), (match) => match[1]));
  assert.ok(addressed.size > 25);
  for (const id of addressed) assert.ok(ids.has(id), `app.js addresses missing #${id}`);
});

test("assets_are_local_and_code_is_external", () => {
  assert.doesNotMatch(html, /\bon[a-z]+\s*=|\bstyle\s*=/i);
  assert.doesNotMatch(html, /<(?:script|style)\b[^>]*>[\s\S]*?\S[\s\S]*?<\/(?:script|style)>/i);
  for (const reference of html.matchAll(/(?:src|href)="([^"]+)"/g)) assert.doesNotMatch(reference[1], /^(?:[a-z][a-z\d+.-]*:|\/\/)/i);
  assert.doesNotMatch(css, /url\s*\(\s*["']?(?:https?:|\/\/)/i);
});

test("meta_csp_admits_only_explicit_read_transport_and_header_only_claims_stay_in_serving_note", () => {
  const policy = /<meta http-equiv="Content-Security-Policy" content="([^"]+)">/.exec(html)?.[1];
  assert.ok(policy);
  assert.match(policy, /default-src 'none'/);
  assert.match(policy, /connect-src 'self' https: http:\/\/127\.0\.0\.1:\* http:\/\/localhost:\*/);
  assert.match(policy, /script-src 'self'/);
  for (const directive of ["frame-ancestors", "sandbox", "report-to", "report-uri"]) assert.doesNotMatch(policy, new RegExp(directive));
  const serving = read("SERVING.md");
  assert.match(serving, /frame-ancestors/);
  assert.match(serving, /X-Content-Type-Options: nosniff/);
  assert.match(serving, /Referrer-Policy/);
});

test("responsive_accessibility_modes_are_present", () => {
  assert.match(css, /@media\s*\(max-width:\s*680px\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.match(css, /@media\s*\(forced-colors:\s*active\)/);
  assert.match(css, /:focus-visible/);
});
