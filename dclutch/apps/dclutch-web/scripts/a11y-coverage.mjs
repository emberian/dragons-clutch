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
 * THE THREE CLASSES, and why only these three.
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
 * All three are syntactic properties of the source, decidable without running
 * anything, which is what makes them gateable here. Landmark nesting needs a
 * document tree and lives in `lib/landmarks.test.tsx`; focus ORDER needs a
 * rendered one and is named in the report rather than half-measured, because a
 * check that quietly covers a third of its class is how a surface comes to look
 * guarded. The 223 rules whose background this survey will not guess are the
 * same kind of honesty, counted rather than waved through.
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

/**
 * The page's own ground, and the contrast floor small text must clear.
 *
 * `body` paints a gradient that ends at `--ground`, and `--ground` is the
 * darkest thing under any of this app's text, so it is the background a rule
 * inherits unless it paints its own. WCAG 1.4.3 asks 4.5:1 of text below
 * 18.66px bold / 24px regular, which is every size this palette uses except
 * the display headings.
 */
const CONTRAST_FLOOR = 4.5;
const LARGE_TEXT_PX = 24;

function channel(value) {
  const scaled = value / 255;
  return scaled <= 0.03928 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4;
}

function luminance([red, green, blue]) {
  return 0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue);
}

function contrast(foreground, background) {
  const [lighter, darker] = [luminance(foreground), luminance(background)].sort((left, right) => right - left);
  return (lighter + 0.05) / (darker + 0.05);
}

/** `#rgb`, `#rrggbb`, or `rgba(r,g,b,a)` -> `[r,g,b,a]`, else null. */
function colour(text) {
  const hex = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.exec(text.trim());
  if (hex !== null) {
    const digits = hex[1].length === 3 ? [...hex[1]].map((one) => one + one).join('') : hex[1];
    return [0, 2, 4].map((at) => Number.parseInt(digits.slice(at, at + 2), 16)).concat(1);
  }
  const rgba = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+))?\s*\)$/.exec(text.trim());
  if (rgba === null) return null;
  return [Number(rgba[1]), Number(rgba[2]), Number(rgba[3]), rgba[4] === undefined ? 1 : Number(rgba[4])];
}

/** Lay a possibly translucent colour over an opaque one. */
/**
 * One rule's contrast AFTER its element opacity, exported so it can be proven.
 *
 * CSS renders an element and then composites it: the text is laid on its own
 * background first, and the pair is dimmed over what is behind them together.
 * So a dimmed rule that paints its own background moves BOTH sides of the
 * ratio, not just the ink -- and a dimmed rule that paints nothing has its
 * background unchanged and only its ink pulled toward the ground.
 *
 * THIS IS EXPORTED BECAUSE THE SURVEY CANNOT EXERCISE IT. Both live opacity
 * defects were fixed by receding in colour instead, and the one dimmer left in
 * the sheet reaches no colour rule by selector prefix -- so `alpha < 1` never
 * fires in a real run today, and a composition path that is never taken is
 * indistinguishable from one that is broken. `lib/a11yCoverage.test.ts` calls
 * this directly with the two colours that were actually on the page, and holds
 * it to the two ratios that were actually measured.
 */
export function effectiveContrastV1(foreground, background, ground, alpha) {
  const painted = over(foreground, background);
  if (alpha >= 1) return contrast(painted, background);
  return contrast(over([...painted, alpha], ground), over([...background, alpha], ground));
}

function over([red, green, blue, alpha], base) {
  if (alpha >= 1) return [red, green, blue];
  return [0, 1, 2].map((index) => Math.round([red, green, blue][index] * alpha + base[index] * (1 - alpha)));
}

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

/** `:root`'s custom properties, resolved through each other. */
function tokens() {
  const found = new Map();
  for (const sheet of STYLESHEETS) {
    for (const match of readFileSync(sheet, 'utf8').matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
      if (!found.has(match[1])) found.set(match[1], match[2].trim());
    }
  }
  const resolve = (value, depth = 0) => {
    if (depth > 8) return value;
    return value.replace(/var\((--[\w-]+)[^)]*\)/g, (whole, name) => (found.has(name) ? resolve(found.get(name), depth + 1) : whole));
  };
  return new Map([...found].map(([name, value]) => [name, resolve(value)]));
}

/**
 * Every rule that paints small text, with the background it paints it on.
 *
 * The background is the rule's OWN if it declares an opaque one, composited
 * over the ground if it declares a translucent one, and the page ground
 * otherwise. That distinction is the whole accuracy of this check: measuring
 * every foreground against the ground reports `.skip-link`'s `#17200f` on
 * `--acid` as 1.15:1, when it is dark ink on a bright button and among the
 * most legible things on the site. Eight of the first fifty-six findings were
 * that mistake, and a check that cries wolf on the accessible cases is one
 * nobody keeps.
 */
export function surveyContrast() {
  const table = tokens();
  const groundValue = colour(table.get('--ground') ?? '#000000');
  const ground = groundValue === null ? [0, 0, 0] : over(groundValue, [0, 0, 0]);
  const expand = (value) => value.replace(/var\((--[\w-]+)[^)]*\)/g, (whole, name) => table.get(name) ?? whole);

  // Every selector that paints a background, gathered first so a colour rule
  // can ask what its own ancestors put behind it.
  const painted = new Map();
  // AND EVERY SELECTOR THAT DIMS ONE.
  //
  // `opacity` composites the whole element -- its text AND its own background --
  // over what is behind it, and it applies to every descendant unconditionally.
  // This survey modelled it NOWHERE, so a rule could be measured at 7.72:1 and
  // rendered at 3.63:1 and no check in this repository could tell. Two live
  // instances were found by hand on 2026-09-02, both on /market's trade page:
  // `.flow-rail-upcoming a { opacity: .58 }` took the seven-step rail's labels
  // to 2.16:1, and `.flow-step-upcoming > header { opacity: .62 }` took a step's
  // description sentence to 3.63:1. Both are fixed by receding in COLOUR, which
  // is the form a static check can read.
  const dimmed = new Map();
  for (const sheet of STYLESHEETS) {
    for (const rule of readFileSync(sheet, 'utf8').matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const paint = /(?:^|[;\s])background(?:-color)?\s*:\s*([^;]+)(?:;|$)/.exec(rule[2]);
      const fade = /(?:^|[;\s])opacity\s*:\s*([\d.]+)\s*(?:;|$)/.exec(rule[2]);
      for (const one of rule[1].split(',')) {
        const key = one.trim().split('\n').pop().trim();
        if (key === '') continue;
        if (paint !== null && !painted.has(key)) painted.set(key, paint[1]);
        if (fade !== null && Number(fade[1]) < 1) {
          dimmed.set(key, {
            alpha: Number(fade[1]),
            site: `${relative(webRoot, sheet).split('\\').join('/')}:${lineOf(readFileSync(sheet, 'utf8'), rule.index)}`,
            composed: false,
          });
        }
      }
    }
  }

  /**
   * The opacity that provably applies to one selector's subject.
   *
   * SOUND, NOT GUESSED. Unlike a background -- where the nearest painting
   * ancestor is a cascade question this file already refuses to answer -- an
   * `opacity` on an element applies to that element's whole subtree, always.
   * So a dimmer counts when it is the rule ITSELF, or when its selector is a
   * literal compound prefix of this one, because a prefix provably names an
   * ancestor-or-self. A dimmer that reaches a rule through a sibling state
   * class (`.flow-rail-upcoming a` over `.flow-rail a > small`) is NOT derivable
   * from the text of the sheet, and is reported as an open dimmer rather than
   * folded in with a guess.
   */
  function dimmerFor(selector) {
    let alpha = 1;
    for (const [key, entry] of dimmed) {
      if (selector === key || selector.startsWith(`${key} `) || selector.startsWith(`${key}>`)) {
        alpha *= entry.alpha;
        entry.composed = true;
      }
    }
    return alpha;
  }

  const rows = [];
  const unresolved = [];
  for (const sheet of STYLESHEETS) {
    const css = readFileSync(sheet, 'utf8');
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const selector = rule[1].trim().split('\n').pop().trim();
      if (selector.startsWith('@') || selector.startsWith(':root')) continue;
      const body = rule[2];
      const declared = /(?:^|[;\s])color\s*:\s*([^;]+)(?:;|$)/.exec(body);
      if (declared === null) continue;
      const foreground = colour(expand(declared[1]));
      if (foreground === null) continue;

      // THE BACKGROUND, or an honest refusal to name one.
      //
      // A rule that paints its own opaque background is exact. A rule that
      // paints nothing AND has no ancestor selector in these sheets that
      // paints one is exact too: it sits on the page ground. Anything else --
      // a gradient, a translucent stack, a colour whose background comes from
      // an ancestor this file can see but cannot resolve a cascade for -- is
      // UNRESOLVED, and is reported rather than judged.
      //
      // An earlier draft guessed the ancestor's paint. It moved the inventory
      // from 52 to 66 and invented a 3.22:1 finding for a rule that had not
      // changed, because picking "the nearest selector with a background" is
      // not what a cascade does. A contrast number produced by a guess is
      // worse than no number: it gets colours rewritten to satisfy it.
      //
      // AND THE THIRD DRAFT IS REFUSED TOO, measured rather than supposed.
      // The obvious next move is to render the shells with jsdom -- the
      // instrument the landmark gate already uses -- and ask
      // `getComputedStyle`. jsdom does not resolve custom properties:
      // `color: var(--muted)` comes back as that literal string and a
      // `background: var(--ground)` shorthand comes back transparent. This
      // stylesheet is var()-based by construction, so that cascade would call
      // every background transparent. `lib/a11yCoverage.test.ts` holds that
      // control live, with a positive control beside it, and goes red if jsdom
      // ever gains the support. What WOULD work is not a browser either: match
      // each rule against the RENDERED tree with `element.matches`, which jsdom
      // does implement, and composite with this file's own `tokens()`.
      const ownPaint = painted.get(selector);
      const ancestors = selector.split(/\s+|(?=>)/).filter((part) => part !== '' && part !== '>');
      const inherited = ancestors.slice(0, -1).some((_, depth) => painted.has(ancestors.slice(0, depth + 1).join(' ')));
      let background = null;
      if (ownPaint !== undefined) {
        const own = colour(expand(ownPaint));
        if (own !== null && own[3] >= 1) background = over(own, ground);
      } else if (!inherited) {
        background = ground;
      }
      if (background === null) {
        unresolved.push({ site: `${relative(webRoot, sheet).split('\\').join('/')}:${lineOf(css, rule.index)}`, selector });
        continue;
      }

      const sized = /font-size\s*:\s*([\d.]+)px/.exec(body) ?? /font\s*:\s*(?:[^;]*?\s)?([\d.]+)px/.exec(expand(body));
      const size = sized === null ? null : Number(sized[1]);
      if (size !== null && size >= LARGE_TEXT_PX) continue;

      // CSS renders the element, then composites it. So the text is laid on its
      // own background first and the pair is dimmed over the ground together --
      // which is why a dimmed rule that paints its own background moves BOTH
      // sides of the ratio, not just the ink.
      const ratio = effectiveContrastV1(foreground, background, ground, dimmerFor(selector));
      if (ratio >= CONTRAST_FLOOR) continue;
      rows.push({
        site: `${relative(webRoot, sheet).split('\\').join('/')}:${lineOf(css, rule.index)}`,
        selector,
        ratio: Number(ratio.toFixed(2)),
        size,
      });
    }
  }
  return Object.freeze({
    failing: rows.sort((left, right) => left.site.localeCompare(right.site)),
    unresolved: unresolved.sort((left, right) => left.site.localeCompare(right.site)),
    dimmers: [...dimmed]
      .map(([selector, entry]) => ({ selector, alpha: entry.alpha, site: entry.site, composed: entry.composed }))
      .sort((left, right) => left.site.localeCompare(right.site)),
  });
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
    lowContrastText: classify(surveyContrast().failing, exempt.lowContrastText),
    unresolvedContrast: surveyContrast().unresolved,
    /**
     * Every rule that dims text with `opacity`, and whether this survey could
     * see through it.
     *
     * A dimmer this file can compose is one whose effect is already inside the
     * contrast numbers above. A dimmer it cannot -- one that reaches its text
     * through a sibling state class rather than a selector prefix -- is the
     * shape that hid a 2.16:1 rail for as long as the rail existed, so it is
     * listed rather than left silent, and clearing it needs a written reason
     * the same way an unnamed control does.
     */
    dimmedText: classify(
      surveyContrast().dimmers
        .filter((entry) => !entry.composed)
        .map((entry) => ({ site: entry.site, selector: entry.selector, alpha: entry.alpha })),
      exempt.dimmedText,
    ),
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
    const contrast = surveyContrast();
    process.stdout.write(`a11y: ${open(report.lowContrastText).length} small-text rules below ${CONTRAST_FLOOR}:1 on a resolvable background (${report.lowContrastText.length - open(report.lowContrastText).length} exempt)\n`);
    for (const entry of contrast.failing) process.stdout.write(`  ${String(entry.ratio).padStart(5)}:1  ${entry.site}  ${entry.selector.slice(0, 60)}\n`);
    process.stdout.write(`a11y: ${contrast.unresolved.length} rules paint small text on a background this survey will not guess\n`);
    const composed = contrast.dimmers.filter((entry) => entry.composed).length;
    process.stdout.write(`a11y: ${open(report.dimmedText).length} opacity rules dim text this survey cannot follow (${composed} composed, ${report.dimmedText.length - open(report.dimmedText).length} exempt)\n`);
    for (const entry of open(report.dimmedText)) process.stdout.write(`  opacity ${entry.alpha}  ${entry.site}\n`);
  }
}
