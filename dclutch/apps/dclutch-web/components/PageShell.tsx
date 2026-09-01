import type { ReactNode } from 'react';

/**
 * THE page frame: site chrome, then the one main landmark.
 *
 * It happened the way the nav's own duplication happened. Every page opened
 * its own `<main>` and put the header inside it, twenty-eight times, and the
 * consequence was invisible to a suite with no DOM: `<header>` maps to the
 * `banner` landmark only while it is NOT a descendant of `main`, `article`,
 * `aside`, `nav` or `section`. Nesting it inside `main` silently demoted the
 * site header to an anonymous group on every page, and put the `navigation`
 * landmark inside the main content it exists to get you out of. A reader
 * navigating by landmark lost the one answer to "where does the chrome stop
 * and the page start" — while 1,012 substring assertions on the rendered HTML
 * ran green, because a substring cannot see what encloses what.
 *
 * One component, one frame, ends that class of drift the same way `Nav` ended
 * the twenty hand-rolled nav bars: a page states what it is and what its
 * chrome is, and the ordering is decided here.
 *
 * `#main-content` lives on the `<main>` itself rather than on a zero-height
 * span the nav used to render just inside it. That span was the skip link's
 * target, which meant "Skip to main content" landed on an empty element inside
 * the main it named — right answer by accident. The main element IS the main
 * content, so it carries the id, and `tabIndex={-1}` makes it focusable by the
 * skip link without putting it in the tab order.
 *
 * The gate is `lib/landmarks.test.tsx`, which renders every page shell the
 * source survey finds and parses it into a real tree.
 */
export default function PageShell({
  className,
  header,
  children,
  ...rest
}: Readonly<{
  /** The `<main>`'s classes. Pages keep their own layout. */
  className: string;
  /** The site chrome — a `Nav`, or a `ConsoleHeader`. Rendered before `main`. */
  header: ReactNode;
  children: ReactNode;
}> & Readonly<Record<string, unknown>>) {
  return <>
    {header}
    <main className={className} id="main-content" tabIndex={-1} {...rest}>{children}</main>
  </>;
}
