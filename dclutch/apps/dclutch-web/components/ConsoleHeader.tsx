import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

/**
 * The chrome every operator console shares: the site {@link Nav} plus one thin
 * strip that says what this console is, in one plain sentence, and marks it as
 * an operator tool. The consoles' working internals stay their own; this is
 * wayfinding, so a reader who lands mid-site knows immediately whether they
 * are on a product page or inside the toolbox — and can get back to the
 * toolbox index either way.
 */
export default function ConsoleHeader({
  path,
  title,
  purpose,
}: Readonly<{
  /** The console's route, e.g. `/release`. Lights the nav's Console entry. */
  path: string;
  /** The console's name, e.g. "Release activation". */
  title: string;
  /** One sentence: what this console does, for whom. Second person, concrete. */
  purpose: string;
}>) {
  return <>
    <Nav current={path} status="operator tool" />
    <div className="console-strip">
      <span className="console-marker">Operator tool</span>
      <strong>{title}</strong>
      <span className="console-purpose">{purpose}</span>
      <Anchor href="/console">All consoles →</Anchor>
    </div>
  </>;
}
