/*
 * Regenerate embedded-data.js from the reviewed manifest.json and terms.json.
 *
 * The page must work from file:// with `default-src 'none'` (no fetch, no
 * connect-src), so the reviewed JSON is mirrored into a classic script instead
 * of being loaded at runtime. This generator is a convenience for repairing
 * drift; it is never required to serve the page. The equality gate that makes
 * the mirror trustworthy is the test
 * `embedded_static_data_equals_reviewed_manifest_and_terms`.
 *
 * Usage: npm run embed
 */
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => JSON.parse(fs.readFileSync(path.join(root, name), "utf8"));

const indent = (json, pad) => json.split("\n").join(`\n${pad}`);
const manifest = indent(JSON.stringify(read("manifest.json"), null, 2), "  ");
const terms = indent(JSON.stringify(read("terms.json"), null, 2), "  ");

const source = `/*
 * GENERATED MIRROR — do not hand-edit. Regenerate with \`npm run embed\`.
 *
 * Verbatim copies of the reviewed manifest.json and terms.json in this
 * directory. They are embedded rather than fetched because the page must run
 * from file:// under a \`default-src 'none'\` policy that permits no network
 * connection at all. Equality with the reviewed files is enforced by the test
 * \`embedded_static_data_equals_reviewed_manifest_and_terms\`; a drifted mirror
 * fails \`npm test\` instead of quietly displaying a different binding.
 *
 * This file contains data only. It has no network, wallet, RPC, signing, or
 * submission capability, and it must never acquire one.
 */
(function (root) {
  "use strict";

  var MANIFEST = ${manifest};

  var TERMS = ${terms};

  root.GlassEmbeddedData = Object.freeze({ manifest: MANIFEST, terms: TERMS });
})(typeof globalThis === "object" ? globalThis : this);
`;

fs.writeFileSync(path.join(root, "embedded-data.js"), source);
console.log(`embedded-data.js regenerated from manifest.json and terms.json (terms ${read("terms.json").digest})`);
