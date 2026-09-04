// tools/genref/substrate-control.mjs -- the positive control behind
// `tools/gauntlet/substrates.json`'s `substrate` column.
//
// A campaign's row DECLARES which substrate its transactions ran on, and a
// declaration nobody checks is the mirror the census exists to refuse. This
// module is the check: it answers, from the runner's own code, whether that
// runner reaches a `solana-test-validator` at all.
//
// ## Why it is not `runner.includes("solana-test-validator")`
//
// Measured 2026-09-04. That is what the check used to be, and
// `tools/gauntlet/lineage/run-lineage.sh` PASSED it on this line:
//
//     # `solana-test-validator`, and drives `DCLRLND1` through preflight,
//
// A comment. The runner never starts a validator and never could -- it hands
// the work to `tools/lineage-loopback/run-lineage-loopback.sh`, which does.
// The declaration was true and the control was measuring the header that
// asserted it, which is a control that cannot fail for the reason it exists.
//
// So: comments are removed before the token is looked for, and the search
// follows ONE level into the scripts the runner actually invokes. Both halves
// are needed. Stripping alone turns `lineage-loopback` red for telling the
// truth; following alone leaves the comment as evidence.
//
// ## What this control does NOT claim
//
// It does not prove a validator process started. Every one of this tree's
// local-validator campaigns ultimately spawns its validator INDIRECTLY --
// `tools/local-validator/dclutch-successor-validator` resolves the binary with
// `command -v` and then `exec "$validator"`, and
// `tools/gauntlet/relayed-vertical` spawns it from Rust with
// `Command::new("solana-test-validator")` -- so a rule that demanded the
// literal token at command position would turn three of the four correctly
// declared campaigns red and would be deleted within a week. What is checked
// is the weaker, checkable thing: the runner's own code, or that of a script it
// invokes, NAMES the validator binary in executable text. A campaign that runs
// entirely inside a `solana-program-test` bank has no reason to, and a runner
// that only mentions it in prose no longer counts as having done so.

import fs from "node:fs";
import path from "node:path";

/** The binary whose presence in executable text is the control. */
export const VALIDATOR_TOKEN = "solana-test-validator";

/**
 * Blank out shell comments, preserving line and column numbering.
 *
 * A `#` opens a comment only when it is unquoted AND begins a word -- at the
 * start of a line, after whitespace, or after one of `;&|(`. `foo#bar` and
 * `"#"` are not comments, which matters here because a URL fragment or a
 * printf format would otherwise swallow the rest of a live line.
 */
export function stripShellComments(text) {
  let out = "";
  let quote = null; // "'" or '"' while inside one
  let previous = "\n";
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote === "'") {
      out += character;
      if (character === "'") quote = null;
      previous = character;
      continue;
    }
    if (quote === '"') {
      out += character;
      if (character === "\\" && index + 1 < text.length) {
        out += text[index + 1];
        index += 1;
        previous = "\\";
        continue;
      }
      if (character === '"') quote = null;
      previous = character;
      continue;
    }
    if (character === "\\" && index + 1 < text.length) {
      out += character + text[index + 1];
      index += 1;
      previous = "\\";
      continue;
    }
    if (character === "'" || character === '"') {
      quote = character;
      out += character;
      previous = character;
      continue;
    }
    if (character === "#" && (previous === "\n" || /[\s;&|(]/.test(previous))) {
      // Blank to end of line, keeping the newline so line numbers survive.
      while (index < text.length && text[index] !== "\n") {
        out += " ";
        index += 1;
      }
      out += "\n";
      previous = "\n";
      continue;
    }
    out += character;
    previous = character;
  }
  return out;
}

/** Words that precede the real command and must be stepped over. */
const COMMAND_PREFIXES = new Set([
  "exec",
  "nohup",
  "time",
  "env",
  "bash",
  "sh",
  "source",
  ".",
  "sudo",
  "caffeinate",
  "swarm-build",
]);

/**
 * Resolve a shell word to a repository-relative script path, or null.
 *
 * Runners spell their siblings through variables -- `"$repo_root/tools/x.sh"`,
 * `"$GAUNTLET/tier1/launcher.sh"` -- so the literal word is not a path. Take
 * the longest path-shaped tail of the word and walk its leading segments off
 * until one names a file in the repository. `$GAUNTLET/tier1/launcher.sh`
 * resolves to nothing, which is honest: nothing in the text says what
 * `$GAUNTLET` is.
 */
export function resolveRepoScript(word, repoRoot) {
  const match = word.replace(/["']/g, "").match(/[A-Za-z0-9._/-]+\.sh\b/);
  if (!match) return null;
  const parts = match[0].split("/").filter((part) => part !== "" && part !== ".");
  for (let index = 0; index < parts.length; index += 1) {
    const relative = parts.slice(index).join("/");
    const absolute = path.join(repoRoot, relative);
    if (fs.existsSync(absolute) && fs.statSync(absolute).isFile()) return relative;
  }
  return null;
}

/**
 * Repository scripts INVOKED by this text, at command position.
 *
 * Command position and not "mentioned anywhere": a positive control that
 * followed every `.sh` a file names would follow the ones it names in order to
 * say it does not run them.
 */
export function invokedScripts(text, repoRoot) {
  const found = new Set();
  for (const line of stripShellComments(text).split("\n")) {
    for (const fragment of line.split(/;|&&|\|\||\||`|\$\(|\(|\)|\{|\}/)) {
      const words = fragment.trim().split(/\s+/).filter(Boolean);
      let position = 0;
      // Step over `VAR=value` assignment prefixes and command wrappers.
      while (
        position < words.length &&
        (COMMAND_PREFIXES.has(words[position]) || /^[A-Za-z_][A-Za-z0-9_]*=/.test(words[position]))
      ) {
        position += 1;
      }
      if (position >= words.length) continue;
      const resolved = resolveRepoScript(words[position], repoRoot);
      if (resolved) found.add(resolved);
    }
  }
  return [...found].sort();
}

/**
 * Does this runner, or a script it invokes one level down, name the validator
 * binary in executable text?
 *
 * Returns `{ launches, sites }`; `sites` names every file the token was found
 * in, so a red row can say what was searched rather than only that it failed.
 */
export function launchesLocalValidator(repoRoot, runnerRelative) {
  const read = (relative) => {
    try {
      return fs.readFileSync(path.join(repoRoot, relative), "utf8");
    } catch {
      return null;
    }
  };
  const runner = read(runnerRelative);
  if (runner === null) return { launches: false, sites: [], searched: [runnerRelative] };
  const searched = [runnerRelative];
  const sites = [];
  if (stripShellComments(runner).includes(VALIDATOR_TOKEN)) sites.push(runnerRelative);
  for (const invoked of invokedScripts(runner, repoRoot)) {
    if (invoked === runnerRelative) continue;
    searched.push(invoked);
    const body = read(invoked);
    if (body !== null && stripShellComments(body).includes(VALIDATOR_TOKEN)) sites.push(invoked);
  }
  return { launches: sites.length > 0, sites, searched };
}
