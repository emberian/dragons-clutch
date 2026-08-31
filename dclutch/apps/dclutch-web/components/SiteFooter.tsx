import Anchor from '@/components/Anchor';

/**
 * THE site footer, rendered once from the root layout so it is on every route
 * — the product pages and the operator consoles alike.
 *
 * It exists to carry the AGPL's §13 source offer. The licence's condition is
 * about *network interaction*: anyone who uses this app over a network must be
 * offered the corresponding source of the version they are using. A footer on
 * some pages would not discharge that, because a reader can land on any route
 * directly, so this is mounted in the layout rather than in the workspaces —
 * the same reason Nav is one component and not twenty.
 *
 * Deliberately the quietest thing on the page: an obligation the licence puts
 * on us, met plainly, not a call to action competing with the content.
 */
export default function SiteFooter() {
  return <footer className="site-footer">
    <Anchor href="https://github.com/emberian/dragons-clutch" rel="noreferrer">Source</Anchor>
  </footer>;
}
