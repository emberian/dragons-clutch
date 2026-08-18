import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
const manifest = JSON.parse(read("manifest.json"));
const terms = JSON.parse(read("terms.json"));

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

assert.equal(manifest.schemaVersion, "dragon-clutch.static-release-manifest.v0");
assert.ok(Array.isArray(manifest.clusters) && manifest.clusters.length >= 3);
assert.ok(manifest.clusters.every((cluster) => cluster.status === "unavailable" && cluster.endpoint === null));
assert.ok(manifest.programs.length > 0 && manifest.programs.every((program) => program.programId === null));
assert.ok(manifest.profiles.some((profile) => profile.id === "synthetic-six-decimal"));
assert.equal(manifest.capabilities.walletConnection, false);
assert.equal(manifest.capabilities.rpcReads, false);
assert.equal(manifest.capabilities.transactionSigning, false);
assert.equal(manifest.capabilities.transactionSubmission, false);

const digest = `sha256:${createHash("sha256").update(JSON.stringify(canonicalize(terms.canonicalTerms))).digest("hex")}`;
assert.equal(terms.digest, digest);
assert.equal(manifest.terms.digest, digest);

const app = read("app.js");
const html = read("index.html");
assert.match(html, /Content-Security-Policy/);
assert.match(html, /default-src 'none'/);
assert.match(html, /script-src 'self'/);
assert.match(app, /mode:\s*["']offline-inspection-only["']/);
assert.match(app, /submission:\s*["']disabled["']/);
assert.doesNotMatch(app, /window\.solana|window\.phantom|new\s+WebSocket|\bfetch\s*\(/);
assert.doesNotMatch(app, /signTransaction|signAllTransactions|sendRawTransaction/);

for (const file of ["index.html", "styles.css", "app.js", "manifest.json", "terms.json"]) {
  assert.ok(fs.statSync(path.join(root, file)).size > 0, `${file} should not be empty`);
}

console.log(`static-client smoke: ok (${digest})`);
