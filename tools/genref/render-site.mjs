#!/usr/bin/env node
// tools/genref/render-site.mjs -- assemble the public Pages artifact:
// a hand-authored landing page, HTML renderings of the generated reference
// (docs/reference) and the guides (docs/guides), the repository README, and
// optionally the static export of the real frontend.
//
//   node tools/genref/render-site.mjs --out DIR [--app DIST_CLIENT_DIR] \
//        [--repo-url URL]
//
// Zero dependencies, deliberately: the gen-1 microsite's discipline was "no
// build step, no external asset or network dependency", and this keeps it.
// The Markdown converter below covers exactly the constructs the reference
// generator emits and the guides use (headings, paragraphs, lists with
// continuations, tables with escaped pipes, fenced code, blockquotes, inline
// code/bold/italic/links). Anything else passes through as escaped text --
// visible, never dropped. Every rendered page is link-checked at the end:
// a relative link must resolve inside the artifact, and a link to a
// repository file that is not part of the site is rewritten to the
// repository URL instead of being left to 404.
//
// The artifact's layout:
//   index.html            the landing page (authored in this file)
//   readme.html           the repository README, rendered
//   reference/…           docs/reference rendered (same tree shape)
//   guides/…              docs/guides rendered
//   app/…                 the frontend's static export, copied verbatim
//   style.css             one stylesheet
//
// The app export is built with root-absolute asset URLs and therefore
// expects the site to be SERVED AT A DOMAIN ROOT (vinext 1.0.0-beta.3's
// basePath is incompatible with output:'export' -- its export prerenderer
// requests un-prefixed paths and skips every route). The landing page states
// this rather than hiding it.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..", "..");

const argv = process.argv.slice(2);
let outDir = null;
let appDir = null;
let repoUrl = "https://github.com/emberian/dragons-clutch/blob/main/dclutch";
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "--out") outDir = argv[++i];
  else if (argv[i] === "--app") appDir = argv[++i];
  else if (argv[i] === "--repo-url") repoUrl = argv[++i];
  else {
    console.error(`render-site: unknown argument ${argv[i]}`);
    process.exit(2);
  }
}
if (!outDir) {
  console.error("render-site: --out DIR is required");
  process.exit(2);
}
outDir = path.resolve(outDir);

// ----------------------------------------------------------- what we render

// repo-relative .md -> site-relative .html
const renderSet = new Map();
renderSet.set("README.md", "readme.html");
for (const base of ["docs/reference", "docs/guides"]) {
  const walk = (dir) => {
    for (const e of fs.readdirSync(path.join(REPO, dir)).sort()) {
      const rel = path.posix.join(dir, e);
      const abs = path.join(REPO, rel);
      if (fs.statSync(abs).isDirectory()) walk(rel);
      else if (e.endsWith(".md")) {
        renderSet.set(
          rel,
          rel.replace(/^docs\//, "").replace(/\.md$/, ".html"),
        );
      }
    }
  };
  walk(base);
}

// --------------------------------------------------------------- inline md

function escapeHtml(s) {
  return s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

// Resolve a Markdown link target found in sourceRel (repo-relative md path)
// into an href usable from that page's rendered location.
function resolveHref(target, sourceRel) {
  if (/^[a-z][a-z0-9+.-]*:/.test(target) || target.startsWith("#")) {
    return target; // absolute URL or fragment
  }
  const [pathPart, frag] = target.split("#");
  const sourceDirRepo = path.posix.dirname(sourceRel);
  const resolvedRepo = path.posix.normalize(
    path.posix.join(sourceDirRepo, pathPart),
  );
  const suffix = frag ? `#${frag}` : "";
  if (renderSet.has(resolvedRepo)) {
    const fromDir = path.posix.dirname(renderSet.get(sourceRel));
    const to = renderSet.get(resolvedRepo);
    return path.posix.relative(fromDir, to) + suffix;
  }
  // Not part of the site: send the reader to the repository.
  return `${repoUrl}/${resolvedRepo}${suffix}`;
}

function inlineMd(text, sourceRel) {
  // Tokenize inline code into placeholders first, so nothing inside
  // backticks is styled AND so bold/link syntax that *contains* a code span
  // still pairs up across it ("**`X` -- title.**", "[`doc.md`](doc.md)").
  const codes = [];
  let s = text.replace(/`([^`]*)`/g, (_m, c) => {
    codes.push(`<code>${escapeHtml(c)}</code>`);
    return `\u0000${codes.length - 1}\u0000`;
  });
  s = escapeHtml(s);
  // links: [text](target)
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_m, txt, href) => {
    return `<a href="${resolveHref(href, sourceRel)}">${txt}</a>`;
  });
  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[\s(])\*([^*\s][^*]*)\*(?=[\s).,;:!?]|$)/g, "$1<em>$2</em>");
  s = s.replace(/\u0000(\d+)\u0000/g, (_m, idx) => codes[Number(idx)]);
  return s;
}

// Split a table row on unescaped pipes; unescape \| inside cells.
function splitRow(line) {
  const cells = [];
  let cur = "";
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === "\\" && line[i + 1] === "|") {
      cur += "|";
      i++;
    } else if (c === "|") {
      cells.push(cur);
      cur = "";
    } else {
      cur += c;
    }
  }
  cells.push(cur);
  // leading/trailing empties from the outer pipes
  if (cells.length && cells[0].trim() === "") cells.shift();
  if (cells.length && cells[cells.length - 1].trim() === "") cells.pop();
  return cells.map((c) => c.trim());
}

// ---------------------------------------------------------------- block md

function mdToHtml(md, sourceRel) {
  const lines = md.split("\n");
  const out = [];
  let i = 0;
  // Skip a leading HTML comment block (the @generated header).
  while (i < lines.length && lines[i].trim() === "") i++;
  if (i < lines.length && lines[i].startsWith("<!--")) {
    while (i < lines.length && !lines[i].includes("-->")) i++;
    i++;
  }

  const isTableSep = (l) => /^\|?[\s:|-]+\|?$/.test(l) && l.includes("-");

  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "") {
      i++;
      continue;
    }
    // fenced code
    if (line.startsWith("```")) {
      const buf = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) {
        buf.push(lines[i]);
        i++;
      }
      i++; // closing fence
      out.push(`<pre><code>${escapeHtml(buf.join("\n"))}</code></pre>`);
      continue;
    }
    // heading
    const h = line.match(/^(#{1,6}) (.*)$/);
    if (h) {
      const level = h[1].length;
      const text = inlineMd(h[2], sourceRel);
      const id = h[2]
        .toLowerCase()
        .replace(/`/g, "")
        .replace(/[^a-z0-9\s-]/g, "")
        .trim()
        .replace(/\s+/g, "-");
      out.push(`<h${level} id="${id}">${text}</h${level}>`);
      i++;
      continue;
    }
    // table
    if (line.startsWith("|") && i + 1 < lines.length && isTableSep(lines[i + 1])) {
      const headers = splitRow(line);
      i += 2;
      const rows = [];
      while (i < lines.length && lines[i].startsWith("|")) {
        rows.push(splitRow(lines[i]));
        i++;
      }
      const th = headers
        .map((c) => `<th>${inlineMd(c, sourceRel)}</th>`)
        .join("");
      const trs = rows
        .map(
          (r) =>
            `<tr>${r.map((c) => `<td>${inlineMd(c, sourceRel)}</td>`).join("")}</tr>`,
        )
        .join("\n");
      out.push(
        `<div class="tablewrap"><table><thead><tr>${th}</tr></thead><tbody>\n${trs}\n</tbody></table></div>`,
      );
      continue;
    }
    // blockquote
    if (line.startsWith(">")) {
      const buf = [];
      while (i < lines.length && lines[i].startsWith(">")) {
        buf.push(lines[i].replace(/^> ?/, ""));
        i++;
      }
      out.push(
        `<blockquote>${mdToHtml(buf.join("\n"), sourceRel)}</blockquote>`,
      );
      continue;
    }
    // list (unordered or ordered), items may have indented continuations
    const listStart = line.match(/^(-|\d+\.) /);
    if (listStart) {
      const ordered = listStart[1] !== "-";
      const items = [];
      while (i < lines.length) {
        const m = lines[i].match(/^(-|\d+\.) (.*)$/);
        if (!m) break;
        const item = [m[2]];
        i++;
        while (
          i < lines.length &&
          (lines[i].match(/^\s+\S/) || lines[i].trim() === "")
        ) {
          if (lines[i].trim() === "") {
            // blank inside a list ends it only if the next line is not indented
            if (!(i + 1 < lines.length && lines[i + 1].match(/^\s+\S/))) break;
            item.push("");
          } else {
            item.push(lines[i].trim());
          }
          i++;
        }
        items.push(item.join(" ").replace(/\s{2,}/g, " ").trim());
      }
      const tag = ordered ? "ol" : "ul";
      out.push(
        `<${tag}>\n${items
          .map((it) => `<li>${inlineMd(it, sourceRel)}</li>`)
          .join("\n")}\n</${tag}>`,
      );
      continue;
    }
    // paragraph: join until blank/structural line
    const buf = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !lines[i].startsWith("#") &&
      !lines[i].startsWith("```") &&
      !lines[i].startsWith("|") &&
      !lines[i].startsWith(">") &&
      !lines[i].match(/^(-|\d+\.) /)
    ) {
      buf.push(lines[i]);
      i++;
    }
    out.push(`<p>${inlineMd(buf.join(" "), sourceRel)}</p>`);
  }
  return out.join("\n");
}

// ------------------------------------------------------------------ chrome

const STYLE = `:root { color-scheme: light dark;
  --bg: #ffffff; --fg: #1a1d21; --muted: #5c6570; --line: #d9dee3;
  --code-bg: #f2f4f6; --accent: #7a4dbe; --warn-bg: #fff7e6; --warn-line: #e8c576; }
@media (prefers-color-scheme: dark) { :root {
  --bg: #14161a; --fg: #e6e4df; --muted: #98a0a8; --line: #2c3138;
  --code-bg: #1d2126; --accent: #b394e6; --warn-bg: #2a2415; --warn-line: #6d5a2a; } }
* { box-sizing: border-box; }
body { background: var(--bg); color: var(--fg); margin: 0;
  font: 16px/1.6 -apple-system, "Segoe UI", system-ui, sans-serif; }
main { max-width: 46rem; margin: 0 auto; padding: 2rem 1.25rem 4rem; }
h1, h2, h3, h4 { line-height: 1.25; margin: 1.8em 0 0.5em; }
h1 { margin-top: 0.4em; }
a { color: var(--accent); }
code { background: var(--code-bg); padding: 0.1em 0.35em; border-radius: 4px;
  font: 0.875em/1.5 ui-monospace, "SF Mono", Menlo, monospace; }
pre { background: var(--code-bg); padding: 0.9rem 1rem; border-radius: 8px;
  overflow-x: auto; }
pre code { background: none; padding: 0; font-size: 0.8125rem; }
.tablewrap { overflow-x: auto; margin: 1rem 0; }
table { border-collapse: collapse; font-size: 0.875rem; min-width: 100%; }
th, td { border: 1px solid var(--line); padding: 0.35rem 0.6rem;
  text-align: left; vertical-align: top; }
th { background: var(--code-bg); }
blockquote { border-left: 3px solid var(--line); margin: 1rem 0;
  padding: 0.1rem 0 0.1rem 1rem; color: var(--muted); }
nav.crumbs { font-size: 0.875rem; color: var(--muted); margin-bottom: 1.5rem; }
nav.crumbs a { color: var(--muted); }
.unreleased { background: var(--warn-bg); border: 1px solid var(--warn-line);
  border-radius: 8px; padding: 0.75rem 1rem; font-size: 0.9375rem;
  margin: 1.25rem 0; }
footer { max-width: 46rem; margin: 0 auto; padding: 0 1.25rem 2.5rem;
  color: var(--muted); font-size: 0.8125rem; border-top: 1px solid var(--line);
  padding-top: 1rem; }
.cards { display: grid; gap: 0.9rem; grid-template-columns:
  repeat(auto-fit, minmax(15rem, 1fr)); margin: 1.5rem 0; padding: 0; }
.cards li { list-style: none; border: 1px solid var(--line); border-radius: 8px;
  padding: 0.9rem 1rem; }
.cards li strong { display: block; margin-bottom: 0.25rem; }
`;

function page({ title, body, depth, crumbs }) {
  const root = "../".repeat(depth);
  const crumbHtml = crumbs
    ? `<nav class="crumbs">${crumbs}</nav>`
    : `<nav class="crumbs"><a href="${root}index.html">dClutch</a></nav>`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<link rel="stylesheet" href="${root}style.css">
</head>
<body>
<main>
${crumbHtml}
${body}
</main>
<footer>dClutch is unreleased: no deployment on any cluster, no release,
nothing value-bearing. This page is published by manual dispatch and
describes local execution evidence. <a href="${repoUrl.replace(/\/blob\/.*$/, "")}">Repository</a>.</footer>
</body>
</html>
`;
}

const LANDING_BODY = `<h1>dClutch</h1>
<p>A Solana protocol for <strong>fully collateralized, liquidation-free
claims over bounded objective states</strong>. A market partitions an
objective outcome domain into exact cells; claims are minted only against
collateral already segregated in the market's Hoard; resolution consumes an
authenticated, release-bound source observation, never a discretionary
resolver. No leverage, no liquidation, no path where a claim outgrows the
collateral behind it.</p>
<div class="unreleased"><strong>Unreleased.</strong> Nothing here is deployed
to any cluster; there is no release, no official deployment, no live market,
and nothing value-bearing. Everything below describes locally executed
software, labeled at exactly the evidence level it reaches. This site is
published by manual dispatch only.</div>
<ul class="cards">
<li><strong><a href="readme.html">The README</a></strong>
What exists today: the seven programs, the open market, the census, the
evidence ladder.</li>
<li><strong><a href="reference/README.html">Protocol reference</a></strong>
Generated from the protocol's own authorities: routes and their execution
status, every refusal code with its meaning, compute budgets, ABI tables,
the decision index.</li>
<li><strong><a href="guides/README.html">Guides</a></strong>
Trader, operator, and reader: what a claim is, how to run a market, and how
to audit the whole thing.</li>
<li><strong><a href="app/index.html">The application</a></strong>
The real frontend, statically exported: market discovery, portfolio,
transaction workbenches. It reads whatever chain you point it at; with
nothing deployed, expect it to refuse with reasons -- that is the protocol
working.</li>
</ul>
<p class="appnote"><em>Note: the application build uses root-absolute asset
URLs and works when this site is served at a domain root. Under a repository
subpath its assets will not resolve (an upstream static-export limitation,
stated here rather than hidden).</em></p>
`;

// ------------------------------------------------------------------- build

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, "style.css"), STYLE);

const titles = new Map(); // siteRel -> title
for (const [src, dst] of renderSet) {
  const md = fs.readFileSync(path.join(REPO, src), "utf8");
  const m = md.match(/^# (.*)$/m);
  titles.set(dst, m ? m[1].replace(/`/g, "") : dst);
}

for (const [src, dst] of renderSet) {
  const md = fs.readFileSync(path.join(REPO, src), "utf8");
  const body = mdToHtml(md, src);
  const depth = dst.split("/").length - 1;
  const html = page({ title: `${titles.get(dst)} - dClutch`, body, depth });
  const p = path.join(outDir, dst);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, html);
}

fs.writeFileSync(
  path.join(outDir, "index.html"),
  page({ title: "dClutch", body: LANDING_BODY, depth: 0, crumbs: "&nbsp;" }),
);

if (appDir) {
  const src = path.resolve(appDir);
  if (!fs.existsSync(path.join(src, "index.html"))) {
    console.error(
      `render-site: --app ${appDir} has no index.html (build the export first)`,
    );
    process.exit(1);
  }
  fs.cpSync(src, path.join(outDir, "app"), { recursive: true });
} else {
  // The landing links to app/index.html; without an app build, say why.
  fs.mkdirSync(path.join(outDir, "app"), { recursive: true });
  fs.writeFileSync(
    path.join(outDir, "app", "index.html"),
    page({
      title: "dClutch application",
      body: `<h1>Application not included</h1>
<p>This artifact was assembled without the frontend's static export
(<code>render-site.mjs</code> ran without <code>--app</code>). Build it with
<code>DCLUTCH_PAGES_EXPORT=1 npm run build</code> in
<code>apps/dclutch-web</code> and pass <code>--app
apps/dclutch-web/dist/client</code>.</p>`,
      depth: 1,
    }),
  );
}

// -------------------------------------------------------------- link check

let broken = 0;
const walkOut = (dir, acc = []) => {
  for (const e of fs.readdirSync(dir).sort()) {
    const p = path.join(dir, e);
    if (fs.statSync(p).isDirectory()) walkOut(p, acc);
    else acc.push(p);
  }
  return acc;
};
for (const file of walkOut(outDir)) {
  if (!file.endsWith(".html")) continue;
  if (path.relative(outDir, file).startsWith("app" + path.sep)) continue;
  const html = fs.readFileSync(file, "utf8");
  for (const m of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
    const target = m[1];
    if (/^[a-z][a-z0-9+.-]*:/.test(target) || target.startsWith("#")) continue;
    const resolved = path.resolve(
      path.dirname(file),
      target.split("#")[0],
    );
    if (!fs.existsSync(resolved)) {
      console.error(
        `render-site: BROKEN LINK ${path.relative(outDir, file)} -> ${target}`,
      );
      broken++;
    }
  }
}
if (broken > 0) {
  console.error(`render-site: ${broken} broken link(s); refusing the artifact.`);
  process.exit(1);
}

console.log(
  `render-site: wrote ${walkOut(outDir).length} files to ${outDir} (${renderSet.size} pages rendered, links checked).`,
);
