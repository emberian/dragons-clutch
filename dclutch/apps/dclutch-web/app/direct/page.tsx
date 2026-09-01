import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import ConsoleHeader from '@/components/ConsoleHeader';

export default function DirectPage() {
  return <PageShell className="product-shell trade-v3-shell" header={<ConsoleHeader path="/trade" title="Direct trade" purpose="Inspect a route and preview exact fill arithmetic." />}>
    <section className="trade-v3-card">
      <header><span>→</span><div><h2>Direct trading lives at /trade</h2><p>Open it to inspect a route and preview exact fill arithmetic; nothing is signed or sent.</p></div></header>
      <Anchor className="secondary-action" href="/trade">Open Direct trade →</Anchor>
    </section>
  </PageShell>;
}
