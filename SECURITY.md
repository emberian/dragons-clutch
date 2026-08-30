# Security policy

## Why this file exists

The first generation's `SECURITY.md` said this, and meant it:

> A private reporting address and coordinated-disclosure process will be added
> before any public test deployment.

That trigger has fired. dClutch has seven programs on Solana devnet and a
browser client that submits signed transactions to them, and until this file
there was no way for someone who found a problem to tell us privately. This
is that promise being kept, late.

## Scope: what is deployed, and what it is worth

dClutch runs a **public devnet demonstration**. Seven Solana programs —
Registry, Rent, Core, Claims, Trading, Resolution, Custody — are deployed to
devnet, and the web client builds and submits signed transactions against
them.

Nothing here is value-bearing:

- **There is no mainnet deployment.** No program, address, or frontend
  associated with this project is an official mainnet deployment, and none
  should be treated as one. Anything telling you otherwise, that is not a
  checked release manifest in this repository, is a phishing attempt.
- **Collateral is play collateral.** Devnet SOL and devnet tokens have no
  monetary value and come free from public faucets.
- **Markets are demonstrations.** They resolve against real Pyth devnet price
  data, but nothing is owed to anyone and no position is a claim on anything
  of value.

That is the honest scope and it sets the stakes plainly: a vulnerability here
costs nobody money today. It is worth reporting anyway — finding these things
while they are still free is the entire point of running a devnet
demonstration.

## In scope

- The seven deployed devnet programs and the on-chain state they own.
- The web client: transaction construction, the signing flow, and what it
  asks a wallet to approve.
- The published site, and the release/manifest machinery that describes what
  is deployed.
- The SDK and CLI in this repository.
- Supply-chain exposure in the build and publish path.

## Out of scope

- Anything requiring an already-compromised device, browser extension, or
  wallet.
- Denial of service against public devnet RPC endpoints or against devnet
  itself. Devnet's liveness is not ours to defend, and load-testing shared
  public infrastructure harms other developers.
- Findings in `archive/` in the parent repository — the superseded first
  generation, no longer developed. Anything wrong there gets fixed in dClutch
  instead; a note is welcome, but it is not a live vulnerability.
- Automated scanner output with no demonstrated impact.

## Reporting a vulnerability

<!-- EMBER-CONFIRMS: the contact line below is the one thing in this file that
     needs a human decision, and it must not be published until it is made.
     The candidates, with the reasoning already done:

       (a) ember@lunar.town — the identity that authors every commit in this
           repository. It is ALREADY public to anyone who clones a public
           repo and runs `git log`, so naming it here discloses nothing new.
           Costs nothing to stand up, because it already exists.
       (b) GitHub Private Vulnerability Reporting on emberian/dragons-clutch
           — a real private channel needing no new infrastructure, but it is
           OFF by default and must be enabled in the repository's Security
           settings first. Do not publish this route until that switch is on;
           an advertised channel that 404s is worse than none.
       (c) A dedicated security@ mailbox — the nicest answer and the only one
           requiring actual work (MX records on a domain that currently
           serves only a Pages CNAME).

     `security@dregg.pro` was the suggested default and is NOT used here: no
     first-party email address exists anywhere in either repository, so there
     is no mailbox convention to extend, and only `clutch.dregg.pro` is
     evidenced — as a Pages CNAME, which proves DNS control and says nothing
     about mail. Publishing it would have repeated the `portal.dregg.studio`
     mistake this project has already documented against itself, on the one
     page where a bounced report is worst.  -->

**Email `EMBER-CONFIRMS-CONTACT-ADDRESS`.**

Please include:

- What you found, and the security impact you believe it has.
- Concrete steps to reproduce. A transaction signature, a program address, a
  market address, or a script is worth far more than prose.
- The commit or deployed program you tested against, if you know it.

Please do **not** open a public issue for a vulnerability, and please do not
post it publicly before we have had a chance to reply. That is a request, not
a legal threat — the reasoning is below.

## Coordinated disclosure

Be aware of what this project actually is: it is small, and it is not a
company. There is no security team, no ticketing rota, and no one on call.
Promising a corporate response time here would be exactly the kind of
commitment that rotted in the file this one replaces. So, honestly:

**What we commit to:**

- We will acknowledge your report, and we will aim to do it within a week.
- We will tell you plainly whether we think it is real, and why.
- We will agree a disclosure date with you rather than announcing one at you.
  Our default is 90 days from the report, or the day a fix ships, whichever
  comes first.
- We will credit you however you like, including not at all.
- We will not ask you to sign anything, and we will never treat a good-faith
  report as an attack.

**What we do not offer:**

- **There is no bug bounty.** No payment, no token, no promise of future
  consideration. If one ever exists it will be announced and funded first;
  anything claiming otherwise is not us.
- No guarantee of a fix on any timeline. Some findings will be fixed, some
  will be documented as known limits — and we will tell you which one yours
  is rather than leaving you guessing.

**If we go quiet, the finding is yours to publish.** Nothing here is
value-bearing, so there is no one for a disclosure to hurt. If you have had no
reply in two weeks, assume the message went astray rather than that it was
ignored, and escalate however you see fit — including publicly. We would
rather be embarrassed than have a real finding sit on someone's disk for a
year because they were being polite.

## Testing against the devnet deployment

Testing the deployed programs is welcome, within the ordinary courtesies of a
shared public network:

- Use your own keypairs and your own faucet SOL.
- Do not attempt to reach anyone else's keys, funds, or accounts — on devnet
  or anywhere else.
- Do not run sustained load against public RPC endpoints; other developers
  are using them.
- Prefer a local validator where you can. This repository's harnesses run the
  full protocol locally: faster, unlimited, and it disturbs nobody.

## What a serious finding looks like

The first generation's threat model — its security objectives and its list of
primary adversaries — was written for a system that has since been replaced.
It is preserved in this file's history rather than restated here, because
carrying a superseded system's threat model forward as though it were the
current one is precisely the staleness that made this file need rewriting.
dClutch's invariants, refusals, and adversarial tests live beside the code
that enforces them.

Two properties are worth stating, because they shape what matters most:

- **Every claim is fully collateralized before it exists.** There is no
  leverage and no liquidation. A finding that lets a claim exist without its
  collateral, or lets collateral leave without its claim, is the most serious
  class of bug in this system.
- **Static clients are untrusted projections.** The website and any index are
  not authoritative; on-chain state is. A finding that the site displays
  something false is a real bug and worth reporting — but it is a different
  and lesser class than one that lets the chain hold something false.
