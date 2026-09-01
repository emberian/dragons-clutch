/**
 * The keyboard-and-name inventory: every control this browser renders that a
 * person using a keyboard, or a screen reader, cannot actually operate.
 *
 * WHY THIS EXISTS AT ALL. C-12 asks for "mobile and accessible interaction
 * complete", and until this file the application had no way to answer. The
 * suite runs under `environment: 'node'` (`vitest.config.ts`), so there is no
 * DOM: across 171 test files there are zero `getByRole` queries, zero
 * `getByLabelText` queries and zero axe runs. Accessibility was asserted the
 * only way a DOM-less suite can assert it -- by pinning attribute strings that
 * somebody remembered to pin, in 1,012 `expect(html).toContain(...)` assertions
 * across 59 files. That catches a deleted `aria-label` on the nav and nothing
 * else, which is why both defect classes below survived the whole suite.
 *
 * A source survey answers what a missing DOM cannot, in the shape this
 * repository already uses for exactly this problem: `abi-coverage.mjs`
 * enumerates hand-mirrored protocol facts and `explorer-coverage.mjs`
 * enumerates unrendered record magics, both against a written baseline that
 * may only shrink. This is the third of those. It needs no browser, no new
 * dependency and no DOM, and it fails the build when the inventory grows.
 *
 * THE TWO CLASSES, and why only these two.
 *
 *   1. A SCROLL CONTAINER NO KEYBOARD CAN REACH (WCAG 2.1.1). A `div` whose
 *      class sets `overflow-x: auto` scrolls under a mouse or a finger and is
 *      inert to a keyboard unless it is focusable. This is the one defect the
 *      mobile work created: the fix for narrow screens was to push wide tables
 *      and charts into horizontal scrollers, and every column pushed off-screen
 *      became unreachable to somebody navigating by keyboard. The container
 *      needs `tabIndex={0}` and a name, and nothing else.
 *
 *   2. A CONTROL WITH NO ACCESSIBLE NAME. An `<input>` with no wrapping
 *      `<label>`, no `aria-label`, no `aria-labelledby` and no `id` any
 *      `htmlFor` names is announced as "edit, blank". Neighbouring text is not
 *      a name: a screen reader does not read the table cell to the left.
 *
 * Both are syntactic properties of the source, decidable without running
 * anything, which is what makes them gateable here. Contrast ratios, landmark
 * nesting and focus ORDER are equally real defects and are deliberately not
 * surveyed: the first two need a resolved cascade and a document tree, and the
 * third needs a rendered one. They are named in the report rather than
 * half-measured here, because a check that quietly covers a third of its class
 * is how a surface comes to look guarded.
 *
 * Usage:
 *   node scripts/a11y-coverage.mjs           # print the inventory
 *   node scripts/a11y-coverage.mjs --json    # machine-readable survey
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = fileURLToPath(new URL('..', import.meta.url));
const exemptPath = join(webRoot, 'scripts', 'a11y-coverage.exempt.json');

/** Directories surveyed. Everything else is build output or third-party. */
const SURVEYED = ['app', 'components'];

/** Stylesheets that decide whether a class scrolls. */
const STYLESHEETS = [join(webRoot, 'app', 'globals.css'), join(webRoot, 'app', 'charts.css')];

/**
 * Tags that are a form control. The capitalised ones are this app's `ui/`
 * primitives, which forward every prop to the real element -- so an
 * `aria-label` on `<Input>` reaches the `<input>`, and its absence is just as
 * real a defect.
 */
const CONTROL_TAGS = ['input', 'textarea', 'select', 'Input', 'Textarea', 'Select'];

/** Elements that name what they wrap, in both spellings this app uses. */
const LABEL_TAGS = ['label', 'Label'];

function sourceFiles() {
  const found = [];
  const walk = (absolute) => {
    for (const entry of readdirSync(absolute).sort()) {
      if (entry === 'node_modules' || entry === 'dist' || entry.startsWith('.')) continue;
      const child = join(absolute, entry);
      if (statSync(child).isDirectory()) walk(child);
      // Test files are surveyed by neither class: a fixture that renders a
      // bare input is pinning what a decoder must accept, not shipping a
      // control anyone operates.
      else if (/\.tsx$/.test(entry) && !/\.test\.tsx$/.test(entry)) found.push(child);
    }
  };
  for (const directory of SURVEYED) walk(join(webRoot, directory));
  return found;
}

const web = (file) => relative(webRoot, file).split('\\').join('/');
const lineOf = (source, index) => source.slice(0, index).split('\n').length;

/**
 * The file with every comment blanked to spaces of the same length.
 *
 * Offsets are preserved so a reported line is still the real line. Blanking
 * rather than deleting matters here more than usual: this codebase explains
 * itself at length, and `MarketFilterBar`'s header comment argues about
 * whether a search box counts as an `<input>` -- which the first run of this
 * survey dutifully reported as two unlabelled controls. A survey that reads
 * prose as code produces exactly the kind of finding that gets a gate
 * disbelieved and then switched off.
 */
function code(source) {
  const blank = (text) => text.replace(/[^\n]/g, ' ');
  return source
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    .replace(/(^|[^:])\/\/[^\n]*/g, (match, lead) => lead + blank(match.slice(lead.length)));
}

/**
 * Every class name whose rule sets a scrolling overflow.
 *
 * Read from the stylesheets rather than listed here, so a class that stops
 * scrolling stops being surveyed and one that starts scrolling starts, without
 * anybody remembering to edit this file.
 */
export function scrollingClasses() {
  const found = new Set();
  for (const sheet of STYLESHEETS) {
    const css = readFileSync(sheet, 'utf8');
    for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const body = match[2];
      if (!/overflow(?:-x)?\s*:\s*(?:auto|scroll)/.test(body)) continue;
      for (const selector of match[1].split(',')) {
        // The class the element itself carries is the LAST one in a descendant
        // selector: `.viz-table-scroll table` styles the table, not the box.
        const parts = selector.trim().split(/\s+/);
        const last = parts[parts.length - 1] ?? '';
        for (const name of last.matchAll(/\.([A-Za-z][\w-]*)/g)) found.add(name[1]);
      }
    }
  }
  return [...found].sort();
}

/** One JSX opening tag, from `<` to its matching `>`, quote-aware. */
function openingTag(source, start) {
  let quote = null;
  let depth = 0;
  for (let index = start; index < source.length; index += 1) {
    const character = source[index];
    if (quote !== null) {
      if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'" || character === '`') quote = character;
    else if (character === '{') depth += 1;
    else if (character === '}') depth -= 1;
    else if (character === '>' && depth === 0) return source.slice(start, index + 1);
  }
  return source.slice(start);
}

/**
 * Whether the control at `index` sits inside a label element.
 *
 * Counted rather than parsed: walking backwards, an unmatched `<label`/`<Label`
 * opening means the control is inside one. This app writes the wrapping label
 * in one JSX expression every time -- `<label><span>…</span><input/></label>`
 * -- so the count is exact for the source it surveys, and a self-closing
 * `<Label … />` (a sibling that names by `htmlFor`, not by wrapping) is not
 * counted as an opening at all.
 */
function insideLabel(source, index) {
  const before = source.slice(0, index);
  let open = 0;
  const pattern = new RegExp(`</?(?:${LABEL_TAGS.join('|')})(?=[\\s/>])`, 'g');
  for (const match of before.matchAll(pattern)) {
    if (match[0].startsWith('</')) { open -= 1; continue; }
    // A self-closing label wraps nothing.
    if (!openingTag(before, match.index).trimEnd().endsWith('/>')) open += 1;
  }
  return open > 0;
}

/**
 * The source of one JSX element, from its opening `<` past its matching close.
 *
 * Depth-counted over element tags, which is exact for well-formed JSX and is
 * all this file needs: it never has to understand an expression, only find
 * where an element stops.
 */
function elementExtent(source, start) {
  const opening = openingTag(source, start);
  if (opening.trimEnd().endsWith('/>')) return opening;
  const name = /^<([A-Za-z][\w.]*)/.exec(opening)?.[1];
  if (name === undefined) return opening;
  let depth = 1;
  let index = start + opening.length;
  const pattern = new RegExp(`</?${name.replace('.', '\\.')}(?=[\\s/>])`, 'g');
  pattern.lastIndex = index;
  for (const match of source.slice(index).matchAll(new RegExp(pattern.source, 'g'))) {
    const at = index + match.index;
    if (match[0].startsWith('</')) {
      depth -= 1;
      if (depth === 0) return source.slice(start, at);
      continue;
    }
    if (!openingTag(source, at).trimEnd().endsWith('/>')) depth += 1;
  }
  return source.slice(start);
}

/**
 * Whether anything inside this element can take keyboard focus.
 *
 * WCAG 2.1.1 is satisfied for a scrolling region either by making the region
 * itself focusable or by its holding something focusable: a browser scrolls
 * the nearest scrollable ancestor of whatever has focus. Checking only the
 * container is therefore a check that over-reports, and it over-reported here
 * on the first run -- `ClusterPicker`'s dialog scrolls, carries a focus trap,
 * an Escape handler and a grid of buttons, and was reported as unreachable.
 * A gate that cries wolf on the one component in the app with real focus
 * management is a gate nobody keeps.
 */
const FOCUSABLE = /\btabIndex[=\s]|<(?:a|button|input|select|textarea|summary|Anchor|Button|Input|Textarea|Select)(?=[\s/>])/;

/** Every `htmlFor` value in one file: the ids that are spoken for. */
function labelledIds(source) {
  const found = new Set();
  for (const match of source.matchAll(/htmlFor=(?:"([^"]*)"|\{`([^`]*)`\}|\{([^}]*)\})/g)) {
    found.add((match[1] ?? match[2] ?? match[3] ?? '').trim());
  }
  return found;
}

/** The `id` an opening tag declares, normalised the way `htmlFor` is. */
function declaredId(tag) {
  const match = /\bid=(?:"([^"]*)"|\{`([^`]*)`\}|\{([^}]*)\})/.exec(tag);
  return match === null ? null : (match[1] ?? match[2] ?? match[3] ?? '').trim();
}

/** Controls with no accessible name, and scroll boxes no keyboard reaches. */
export function survey() {
  const scrolling = scrollingClasses();
  const unnamedControls = [];
  const unreachableScrollers = [];
  for (const file of sourceFiles()) {
    const source = code(readFileSync(file, 'utf8'));
    const named = labelledIds(source);

    for (const tag of CONTROL_TAGS) {
      const pattern = new RegExp(`<${tag}(?=[\\s/>])`, 'g');
      for (const match of source.matchAll(pattern)) {
        const opening = openingTag(source, match.index);
        // `type="hidden"` is not a control anyone operates.
        if (/type=(?:"hidden"|'hidden')/.test(opening)) continue;
        if (/\baria-label(?:ledby)?[=\s]/.test(opening)) continue;
        const id = declaredId(opening);
        if (id !== null && named.has(id)) continue;
        if (insideLabel(source, match.index)) continue;
        unnamedControls.push({ site: `${web(file)}:${lineOf(source, match.index)}`, tag });
      }
    }

    for (const match of source.matchAll(/className=(?:"([^"]*)"|\{`([^`]*)`\})/g)) {
      const classes = (match[1] ?? match[2] ?? '').split(/[\s${}]+/).filter((entry) => entry !== '');
      const scrolls = classes.filter((entry) => scrolling.includes(entry));
      if (scrolls.length === 0) continue;
      // Walk back to this attribute's own opening `<`, then read the whole tag.
      const start = source.lastIndexOf('<', match.index);
      if (start < 0) continue;
      if (FOCUSABLE.test(elementExtent(source, start))) continue;
      unreachableScrollers.push({ site: `${web(file)}:${lineOf(source, match.index)}`, classes: scrolls.sort() });
    }
  }
  return {
    scrollingClasses: scrolling,
    unnamedControls: unnamedControls.sort((left, right) => left.site.localeCompare(right.site)),
    unreachableScrollers: unreachableScrollers.sort((left, right) => left.site.localeCompare(right.site)),
  };
}

/**
 * Files excused, each with a reason and an exact count.
 *
 * Keyed by FILE and not by `file:line`, because a line-keyed exemption expires
 * the moment anybody adds a line above it, and a gate that goes red for a
 * reason unrelated to what it checks is a gate that gets its baseline
 * rewritten instead of read. The count is what keeps a file-wide excuse from
 * becoming a blanket one: a NEW control in an exempt file changes the number
 * and fails, so the reason has to be re-argued for it specifically.
 */
export function readExemptions() {
  return JSON.parse(readFileSync(exemptPath, 'utf8'));
}

function classify(entries, exemptions) {
  const counted = new Map();
  for (const entry of entries) {
    const file = entry.site.slice(0, entry.site.lastIndexOf(':'));
    counted.set(file, (counted.get(file) ?? 0) + 1);
  }
  return entries.map((entry) => {
    const file = entry.site.slice(0, entry.site.lastIndexOf(':'));
    const excused = exemptions[file];
    if (excused === undefined) return { site: entry.site, state: 'open', reason: null };
    // The excuse covers exactly the sites it was written against. One more and
    // every site in the file goes back to open, so the reason is re-read.
    if (excused.sites !== counted.get(file)) {
      return { site: entry.site, state: 'open', reason: `${file} is exempt for ${excused.sites} sites and now has ${counted.get(file)}` };
    }
    return { site: entry.site, state: 'exempt', reason: excused.reason };
  });
}

/** The survey minus the written exemptions: what still has to be fixed. */
export function coverage() {
  const found = survey();
  const exempt = readExemptions();
  return {
    scrollingClasses: found.scrollingClasses,
    unnamedControls: classify(found.unnamedControls, exempt.unnamedControls),
    unreachableScrollers: classify(found.unreachableScrollers, exempt.unreachableScrollers),
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const report = coverage();
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    const open = (rows) => rows.filter((entry) => entry.state === 'open');
    process.stdout.write(`a11y: ${report.scrollingClasses.length} scrolling classes read from the stylesheets\n`);
    process.stdout.write(`a11y: ${open(report.unnamedControls).length} controls with no accessible name (${report.unnamedControls.length - open(report.unnamedControls).length} exempt)\n`);
    for (const entry of open(report.unnamedControls)) process.stdout.write(`  unnamed  ${entry.site}\n`);
    process.stdout.write(`a11y: ${open(report.unreachableScrollers).length} scroll boxes no keyboard reaches (${report.unreachableScrollers.length - open(report.unreachableScrollers).length} exempt)\n`);
    for (const entry of open(report.unreachableScrollers)) process.stdout.write(`  no tabIndex  ${entry.site}\n`);
  }
}
