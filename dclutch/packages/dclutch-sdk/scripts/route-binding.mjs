/**
 * Route-binding gate: prove a generator scrapes the file the live route binds.
 *
 * The conviction this exists to prevent (ebebbd4d): `abi:direct-v3` scraped
 * `dclutch-vm/src/v3.rs`'s `SCHEMA_RELEASE_ID` and emitted it as the
 * effect schema, while the live authenticator
 * (`dclutch-trading/src/artifacts_v4.rs`) binds `v4.rs`'s
 * `SCHEMA_RELEASE_ID_V4`. The `--check` byte gate stayed green the whole time:
 * it proves the output is fresh against whatever file the generator points at,
 * never that the pointed-at file is the route's author. The naming trap
 * compounded it -- v3.rs's preimage reads `effect-program-v4-...` while the
 * real V4 reads `effect-program-v5-...`, so the wrong file looked right.
 *
 * Every function here is PURE TEXT: no fs, no paths resolved against the real
 * tree. That is deliberate -- it lets a vitest feed doctored route text and
 * watch the gate red, so the tested logic is the shipped logic rather than a
 * reconstruction of it.
 */

/**
 * Drop whole-line comments so a doc comment mentioning `use foo::bar;` cannot
 * be mistaken for a binding. Trailing comments after code are left alone: Rust
 * `use` statements do not carry them in this tree, and stripping them properly
 * would need a string-literal-aware scanner for no gain here.
 */
function withoutLineComments(text) {
  return text.split('\n').filter((line) => !line.trimStart().startsWith('//')).join('\n');
}

function matchingBrace(text, open) {
  let depth = 0;
  for (let index = open; index < text.length; index += 1) {
    if (text[index] === '{') depth += 1;
    else if (text[index] === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error('unbalanced use-tree braces');
}

function splitTopLevel(text) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === '{') depth += 1;
    else if (text[index] === '}') depth -= 1;
    else if (text[index] === ',' && depth === 0) {
      parts.push(text.slice(start, index));
      start = index + 1;
    }
  }
  parts.push(text.slice(start));
  return parts.map((part) => part.trim()).filter((part) => part.length > 0);
}

function flattenInto(prefix, tree, out) {
  const trimmed = tree.trim();
  if (trimmed.length === 0) return;
  const brace = trimmed.indexOf('{');
  if (brace === -1) {
    // A leaf: `a::b::C`, `a::b::C as D`, `self`, or a `*` glob.
    const [pathPart, aliasPart] = trimmed.split(/\s+as\s+/);
    const path = `${prefix}${pathPart.trim()}`;
    const segments = path.split('::');
    let name = segments[segments.length - 1];
    let resolved = path;
    if (name === 'self') {
      segments.pop();
      resolved = segments.join('::');
      name = segments[segments.length - 1];
    }
    // A glob binds no name this gate can follow; recording it would invent a
    // resolution the source does not state.
    if (name === '*') return;
    out.push(Object.freeze({ alias: (aliasPart ?? name).trim(), path: resolved }));
    return;
  }
  const head = trimmed.slice(0, brace);
  const close = matchingBrace(trimmed, brace);
  for (const part of splitTopLevel(trimmed.slice(brace + 1, close))) {
    flattenInto(`${prefix}${head}`, part, out);
  }
}

/** Every `use`/`pub use` leaf in a Rust file, as `{alias, path}` pairs. */
export function parseUseTrees(text) {
  const source = withoutLineComments(text);
  const bindings = [];
  const statement = /^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?use[ \t]+/gm;
  let match = statement.exec(source);
  while (match !== null) {
    let depth = 0;
    let end = match.index + match[0].length;
    while (end < source.length) {
      if (source[end] === '{') depth += 1;
      else if (source[end] === '}') depth -= 1;
      else if (source[end] === ';' && depth === 0) break;
      end += 1;
    }
    flattenInto('', source.slice(match.index + match[0].length, end), bindings);
    statement.lastIndex = end;
    match = statement.exec(source);
  }
  return bindings;
}

/**
 * The module path a file's use-trees bind `aliasOrName` to, or null. Handles
 * `X as Y` aliases and nested braces, which is how the route actually spells
 * its bindings today.
 */
export function resolveUseBinding(routeText, aliasOrName) {
  const found = parseUseTrees(routeText).filter((binding) => binding.alias === aliasOrName);
  if (found.length === 0) return null;
  if (found.length > 1 && new Set(found.map((binding) => binding.path)).size > 1) {
    throw new Error(`route binds ${aliasOrName} to more than one path: ${found.map((binding) => binding.path).join(', ')}`);
  }
  return found[0].path;
}

function collapse(text) {
  return text.replace(/\s+/g, ' ').trim();
}

/**
 * Throw unless the authentication conjunct is still in the route. Without this
 * the gate dies silently the day the route refactors: a binding check over a
 * conjunct that no longer exists proves nothing, but still passes.
 * Whitespace-insensitive, because rustfmt rewraps these lines freely.
 */
export function requireRouteConjunct(routeText, anchorSnippet) {
  if (!collapse(withoutLineComments(routeText)).includes(collapse(anchorSnippet))) {
    throw new Error(`route no longer contains the authentication conjunct \`${collapse(anchorSnippet)}\` -- this gate is checking a binding the route may have stopped using`);
  }
}

/** Throw unless the route binds `name` to exactly `expectedModulePath`. */
export function requireRouteBinding({ routeText, name, expectedModulePath }) {
  const actual = resolveUseBinding(routeText, name);
  if (actual === null) {
    throw new Error(`route does not bind ${name} in any use-tree`);
  }
  if (actual !== expectedModulePath) {
    throw new Error(`route binds ${name} to ${actual}, not ${expectedModulePath}`);
  }
}

/**
 * True when a file declares `mod name;` / `mod name { … }`.
 *
 * A re-export can name a sibling MODULE of the same crate rather than another
 * crate (`pub use principal_capacity_v1::{…}` in that crate's own lib.rs).
 * Reading the `mod` declaration is how the walker tells the two apart from
 * source, instead of guessing from the shape of the name.
 */
export function declaresModule(text, name) {
  return new RegExp(`^[ \\t]*(?:pub(?:\\([^)]*\\))?[ \\t]+)?mod[ \\t]+${name}[ \\t]*[;{]`, 'm')
    .test(withoutLineComments(text));
}

/** True when a file defines the named constant itself (rather than re-exporting it). */
export function definesConstant(text, name) {
  return new RegExp(`(?:pub(?:\\([^)]*\\))?\\s+)?const ${name}\\s*:`).test(withoutLineComments(text));
}

/**
 * The files an `include!("…")` pulls into this one, as paths relative to the
 * including file's own directory.
 *
 * Several crates state a generated module as
 * `mod generated { include!("generated_x.rs") } pub use generated::*;`, which
 * is neither a definition nor a `use` this walker could follow. Without this
 * the gate would report "neither defines nor re-exports" and read as a defect
 * in the source rather than a gap in the walker.
 */
export function includedFiles(text) {
  const includes = [];
  const pattern = /include!\s*\(\s*"([^"]+)"\s*\)/g;
  let match = pattern.exec(withoutLineComments(text));
  while (match !== null) {
    includes.push(match[1]);
    match = pattern.exec(withoutLineComments(text));
  }
  return includes;
}

/** Resolve `relative` against the directory holding `file`. */
function siblingOf(file, relative) {
  const directory = file.slice(0, file.lastIndexOf('/'));
  const parts = `${directory}/${relative}`.split('/');
  const stack = [];
  for (const part of parts) {
    if (part === '.' || part === '') continue;
    if (part === '..') stack.pop();
    else stack.push(part);
  }
  return stack.join('/');
}

/**
 * The source file a `crate_name::module::path::CONSTANT` names, under this
 * repo's one-module-per-file layout.
 */
export function modulePathToSource(modulePath, currentCrate) {
  const segments = modulePath.split('::');
  const constant = segments.pop();
  const crateSegment = segments.shift();
  if (crateSegment === undefined) throw new Error(`\`${modulePath}\` names no module`);
  const crate = crateSegment === 'crate' || crateSegment === 'super' ? currentCrate : crateSegment;
  const directory = crate.replace(/_/g, '-');
  const file = segments.length === 0
    ? `crates/${directory}/src/lib.rs`
    : `crates/${directory}/src/${segments.join('/')}.rs`;
  return Object.freeze({ file, constant, crate });
}

/**
 * Walk a module path to the file that actually DEFINES the constant, following
 * `pub use` re-exports. `readSource(file)` returns the file's text or null.
 *
 * This is what makes the gate follow a real alias chain instead of trusting a
 * hand-written claim about where a name comes from: the route says
 * `SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5` comes from the capability
 * contract's `v4`, and `v4.rs` says it is `lifecycle_v3`'s
 * `CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`. Both hops are read from source.
 */
export function followToDefinition(modulePath, currentCrate, readSource, hops = 6) {
  let path = modulePath;
  let crate = currentCrate;
  for (let hop = 0; hop < hops; hop += 1) {
    let located = modulePathToSource(path, crate);
    let text = readSource(located.file);
    if (text === null && !located.file.endsWith('/lib.rs')) {
      // a module that is a directory: `src/a/b/mod.rs` rather than `src/a/b.rs`
      located = Object.freeze({ ...located, file: located.file.replace(/\.rs$/, '/mod.rs') });
      text = readSource(located.file);
    }
    if (text === null || text === undefined) {
      throw new Error(`cannot read ${located.file} while following ${modulePath}`);
    }
    if (definesConstant(text, located.constant)) return located;
    // A file may state the constant through an `include!`d generated module.
    // That included file IS the authority, so name it rather than the shim.
    const included = includedFiles(text)
      .map((relative) => siblingOf(located.file, relative))
      .find((candidate) => {
        // A probe, not a resolution: an `include!` naming a path this reader
        // cannot open is not itself the failure, so it must not throw here.
        let body = null;
        try { body = readSource(candidate); } catch { return false; }
        return body !== null && body !== undefined && definesConstant(body, located.constant);
      });
    if (included !== undefined) {
      return Object.freeze({ file: included, constant: located.constant, crate: located.crate });
    }
    const next = resolveUseBinding(text, located.constant);
    if (next === null) {
      throw new Error(`${located.file} neither defines nor re-exports ${located.constant}`);
    }
    // `pub use sibling_module::X` inside a crate names a module of THIS crate,
    // not another crate. The file's own `mod` declaration says which.
    const head = next.split('::')[0];
    // A module this file declares is a child of THIS file's module: `a/b/mod.rs`
    // and `a/b.rs` both own `a::b::child`, and only `lib.rs` owns `child`.
    const inside = located.file.slice(located.file.indexOf('/src/') + '/src/'.length);
    const prefix = inside === 'lib.rs' ? '' : inside.replace(/\/mod\.rs$/, '').replace(/\.rs$/, '').replace(/\//g, '::');
    path = declaresModule(text, head) ? `${located.crate}::${prefix ? `${prefix}::` : ''}${next}` : next;
    crate = located.crate;
  }
  throw new Error(`re-export chain from ${modulePath} is deeper than ${hops} hops`);
}

/**
 * The gate proper. For one authority-selecting constant, prove the route's
 * binding walks to exactly the file and constant the generator scrapes.
 *
 * `binding.qualified` covers conjuncts that name a full path inline
 * (`dclutch_vm::v3::SCHEMA_RELEASE_ID`) rather than through a use.
 */
export function requireGeneratorFollowsRoute({
  routeText, routeCrate, readSource, binding,
}) {
  requireRouteConjunct(routeText, binding.conjunct);
  const start = binding.qualified
    ?? resolveUseBinding(routeText, binding.routeName);
  if (start === null || start === undefined) {
    throw new Error(`route does not bind ${binding.routeName} in any use-tree, so the conjunct \`${collapse(binding.conjunct)}\` reads a name this gate cannot follow`);
  }
  const located = followToDefinition(start, routeCrate, readSource);
  if (located.file !== binding.sourceFile || located.constant !== binding.sourceConstant) {
    throw new Error(`the route's ${binding.routeName ?? binding.qualified} resolves to ${located.constant} in ${located.file}, but this generator scrapes ${binding.sourceConstant} from ${binding.sourceFile} -- the emitted value would not be the one the live route binds`);
  }
}
