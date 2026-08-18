# Dragon's Clutch: pre-deployment CFTC discussion packet

Date: 2026-08-17  
Prepared by: [legal name / proposed entity]  
Contact: [email and telephone]  
Project status: specification only; no deployment, users, funds, or live markets

## 1. Purpose of the requested discussion

The requester is considering whether to develop and personally publish a
fully collateralized conditional-asset protocol on Solana. Before doing so, the
requester seeks an Innovation Task Force meeting and direction to the appropriate
CFTC divisions concerning:

1. the classification of the proposed claims and transactions;
2. whether the proposed program and native multilateral auction would constitute
   a designated contract market (DCM), swap execution facility (SEF), derivatives
   clearing organization (DCO), or other regulated facility or activity;
3. which persons, if any, the Commission would regard as operating, controlling,
   intermediating, or clearing the protocol;
4. whether a legally viable limited research pilot, exemption, no-action path, or
   registration path exists; and
5. which facts must be frozen before a formal request can be evaluated.

This is a prospective inquiry, not a self-report. The requester has received no
agency communication and is not aware of any live Dragon's Clutch activity.

The requester recognizes that the CFTC currently describes event-contingent
contracts as potentially falling within the definitions of swaps or futures, and
states that a prediction market offering swaps or futures to the general public
must register as a DCM. See the Commission's 2026
[Prediction Markets ANPRM](https://www.cftc.gov/LawRegulation/FederalRegister/proposedrules/2026-05105.html).
The requester is seeking the agency's view before assuming that a different
technical architecture changes that analysis.

## 2. Proposed protocol in one page

Dragon's Clutch would compile a finite, objective future state into a set of
fully collateralized claims. The deterministic core is called Eggcrate; a
complete set of claims is a Clutch; a segregated market-local collateral vault is
a Hoard; and final immutable resolution is a Hatch.

For an illustrative three-state terminal-price market, one collateral unit might
mint one claim for each of:

```text
state 0: terminal price < L
state 1: L <= terminal price < H
state 2: terminal price >= H
```

The state cells must be canonical, exhaustive, and disjoint. Before resolution,
one complete set may be merged back into one collateral unit. After an objective
and predetermined observation procedure identifies the realized cell, its claim
redeems for one unit and the other claims redeem for zero. More generally, only
an immutable bounded payout vector admitted at market creation may be used.

The intended solvency condition is:

```text
market Hoard collateral >= maximum payout in every reachable protocol state
```

The protocol would create no borrowing, margin loan, margin call, liquidation,
insurance deficit, or socialized loss. Every accepted order would be fully
reserved. “Fully collateralized” describes economic construction; it is not
offered as a conclusion about product, facility, clearing, intermediary, or
other regulatory classification.

### Proposed V1 subject matter

The contemplated initial templates are restricted to objective crypto-native
facts or deterministic functions of authenticated onchain market data, such as:

- a token's price at a fixed time falling within one of several bands;
- relative performance of specified crypto assets over a fixed interval;
- sampled price crossings, drawdown bands, or volatility bands computed under a
  frozen formula and observation schedule; and
- specified, machine-verifiable Solana account or protocol lifecycle states.

Political, sports, entertainment, social, and subjective adjudication are not in
the proposed scope. Every source identity, version, interval, transformation,
rounding rule, repair deadline, ambiguity rule, and payout rule would be fixed at
market creation. No human reporter would choose the winning outcome.

### Claims and trading

- Each outcome may have a canonical transferable Token-2022 mint.
- A cheaper program-owned internal balance representation may be used inside the
  native venue; a holder could materialize one selected outcome as an ordinary
  token and later dematerialize it.
- The native venue would collect orders into epochs and use a deterministic,
  permissionlessly verified batch process. Prices for the complete outcome basis
  would be checked together as a simplex rather than as unrelated books.
- A solver may propose a result, but an offchain solver has no authority. The
  onchain program accepts only a result whose public witness satisfies the fixed
  rules. If the design cannot cheaply prove an optimum, it will call the result
  the best valid submitted candidate rather than an optimum.
- Materialized claims could potentially trade on independent third-party Solana
  venues. Such venues would not determine resolution and may not support or list
  the assets.

### Observation and settlement

Markets would reference a closed list of authenticated onchain source adapters.
Permissionless callers could submit qualifying source observations, repair
missing intervals, perform bounded verification work, finalize an epoch, resolve
a market, or clean up eligible state. Mandatory future work would be funded at
admission from segregated balances rather than depending on later trading volume.
Anyone satisfying the instruction's public conditions could perform it.

The program—not a human custodian—would hold collateral in a market-local token
account controlled by immutable program rules. No administrator could redirect
Hoard principal, change live contract terms, choose an outcome, reverse a trade,
or seize a user's claim. Program upgrade authority during testing and the point
and conditions of immutability have not yet been selected.

### Client and infrastructure

The contemplated client is a reproducible static application publishable on
GitHub Pages or IPFS. It would read user-selected Solana RPC endpoints, construct
transactions locally, and display program, terms, source, and release-manifest
identities. It would have no server-side order router, proprietary matching
engine, hosted account, custody service, privileged index, or protocol secret.

No uniquely operated backend is intended. Solana validators, RPC providers,
wallet software, token programs, oracle publishers, and third-party trading
venues remain independent ecosystem infrastructure.

The requester understands that decentralization, immutability, self-custody,
open-source publication, and static hosting do not themselves answer who legally
offers or operates a product or facility. That is one of the questions presented.

### Collateral, fees, and compensation

The kernel would be collateral-generic. A market belongs to an immutable Realm
that identifies one vetted collateral mint, token program, decimals, and
extension/authority profile. Potential profiles include SOL-compatible assets,
appropriately vetted stablecoins, and `$DREGG`; the first proposed profile has not
been selected. `$DREGG` is a Solana token and would not be hard-coded into the
kernel or required of other Realms.

No fee schedule or developer compensation arrangement is frozen. Current design
work separates:

- claimant principal in the Hoard;
- prepaid SOL and token balances for observation, finalization, rent, and cleanup;
- order or execution fee balances; and
- any maintainer or public treasury revenue.

Hoard principal may pay only valid claims. Future fee revenue would never be
counted as capital for an existing market's settlement or mandatory work. The
requester specifically seeks guidance on how compensation, retained control,
listing conduct, client publication, promotion, and affiliated trading affect
the analysis.

## 3. Proposed lifecycle and control map

| Phase | Proposed action | Who can initiate | Proposed authority |
|---|---|---|---|
| Realm creation | Register one immutable collateral profile | Permissionless, subject to program validation | Onchain validation |
| Template registration | Register exact source/formula/partition/terms digest | Permissionless from a closed language | Onchain validation |
| Market creation | Bind template, interval, collateral cap, fees, and prepaid work | Permissionless or separately scoped for a pilot | Onchain validation |
| Complete-set split | Deposit collateral; credit every outcome | User | Deterministic program |
| Trading | Submit fully reserved single-outcome or admitted portfolio orders | User | Frozen epoch rules |
| Clearing | Propose and verify a batch candidate | Any caller | Deterministic onchain verifier |
| Observation | Submit source-account evidence for a scheduled boundary | Any caller | Frozen source adapter |
| Repair | Fill a permitted missing boundary before deadline | Any caller | Frozen repair rule |
| Resolution | Evaluate a sealed window and freeze payout vector | Any caller | Deterministic evaluator |
| Redemption | Burn/debit claim and release payout | Claim holder | Frozen payout vector |
| Cleanup | Close eligible accounts and distribute frozen bond/reward | Any caller | Frozen lifecycle rule |

The requester may author, audit, publish, and initially deploy the program and
static client; may select or publish proposed Realm and Template configurations;
and may receive compensation if a legally permitted model is identified. These
facts make it inappropriate to infer from “permissionless” that no person has a
legally relevant role.

## 4. Questions for CFTC staff

### Product classification

1. Would each Token-2022 outcome claim, a complete Clutch, a split/merge, an
   internal balance, or an epoch fill be treated as a swap, future, commodity
   option, option on a future, spot commodity transaction, security-related
   instrument, or another product?
2. Does classification differ between terminal crypto-price bands, deterministic
   path statistics, and exact Solana protocol-state facts?
3. Does the ability to merge an exhaustive complete set before resolution or the
   absence of debt/liquidation affect classification, and if so how?
4. Does transferable tokenization of each outcome change the analysis relative
   to a conventional book-entry event contract?
5. Would a broad partition compiler require product-by-product review? Could a
   narrow class share treatment only when source identity, formula, procedure,
   methodology, collateral currency, and payment calculation are identical?

### Trading-facility and operator classification

6. Would the native multilateral batch auction be a trading facility requiring
   DCM or SEF registration? If the intended users include the general public,
   does that make DCM designation the presumptive path?
7. Who would staff regard as operating the facility: program deployer, upgrade
   authority, template publisher, market creator, static-client publisher,
   repository maintainers, fee recipient, governance body, Solana validators,
   or some combination?
8. What control or ongoing conduct remains legally relevant after the settlement
   program is immutable and anyone may publish a client or create an eligible
   market?
9. Would publishing source without deployment, publishing an unsigned static
   client, deploying only an issuance/redemption kernel, or deploying the native
   auction constitute different regulatory activities?

### Clearing, custody, and intermediaries

10. Does a program Hoard that receives collateral, mints a complete set, nets
    complete sets, and settles outcome liabilities provide clearing services
    requiring DCO registration or exemption? Which aspects are analogous to
    novation, multilateral settlement/netting, or mutualization of credit risk?
11. Does deterministic program custody differ from receiving customer funds as
    an FCM, or is a person associated with deploying/controlling the program
    treated as accepting money or property?
12. Under what facts could the deployer, client publisher, market/template
    publisher, or compensated maintainer become an FCM, introducing broker,
    associated person, commodity trading advisor, commodity pool operator, or
    other intermediary?
13. What conditions would be necessary for a self-custodial static client to be
    treated as technology rather than solicitation, order acceptance, or
    intermediation?

### Compliance architecture

14. If DCM/DCO registration is required, how can requirements involving
    surveillance, market supervision, emergency authority, customer protection,
    system safeguards, recordkeeping, compliance staff, and 24/7 operations be
    satisfied by an immutable program without a singular operating service?
15. Would permissionless onchain records satisfy any recordkeeping or audit-trail
    elements, and what offchain books, communications, identities, surveillance,
    or regulatory reporting would still be required?
16. What participant eligibility, identity, sanctions, geographic, transaction,
    open-interest, market, or collateral controls would be required?
17. How should manipulation analysis address a cash-settled claim whose source is
    an onchain oracle or DEX state that a trader may economically influence?

### Relief or registration path

18. Which divisions should participate in a formal interpretive, no-action, or
    exemptive request under Regulation 140.99?
19. Is Commission exemptive authority under CEA section 4(c), an innovation
    exemption or safe harbor if adopted, a limited no-action pilot, DCM/DCO
    registration, partnership with existing registrants, or some other route the
    most appropriate vehicle?
20. Could any limited retail pilot be considered, or would an exemptive route be
    limited to eligible contract participants or other “appropriate persons”?
21. Which features would need to be removed, capped, or controlled before staff
    could consider relief?
22. What minimum factual and technical package should accompany a formal request,
    and when should DMO, DCR, MPD, Enforcement, or other offices be consulted?

### Affiliates and future research

23. If the requester or an affiliated research system later traded on the venue,
    made markets, submitted clearing candidates, supplied observations, or
    received fees, what conflict, registration, surveillance, disclosure, or
    separation requirements would arise?
24. Would staff prefer that any initial relief categorically forbid deployer and
    affiliate trading, market making, template creation, or observation work?

## 5. A possible bounded research pilot for discussion

The following is not a commitment or assertion that staff can lawfully authorize
it. It is a menu of limitations that could make a formal proposal concrete:

- one legally identified requester and one disclosed program deployment;
- one audited program version and reproducible release manifest;
- immutable economic terms and source adapter for each admitted market;
- objective crypto-native terminal facts only;
- no politics, sports, entertainment, social questions, or subjective resolution;
- one vetted collateral profile;
- one narrow source/formula/methodology class rather than a general market factory;
- no debt, margin, lending, liquidation, or cross-market collateral netting;
- fully prefunded payout liability and mandatory lifecycle work;
- aggregate collateral, participant, market, and duration caps;
- deterministic market-expiration and repair procedures;
- disclosed source manipulation analysis and concentration limits;
- no deployer/affiliate trading, market making, or private information advantage;
- no discretionary resolver or administrative change to live terms;
- complete onchain audit trail plus any required offchain records;
- independent security audit and incident/disclosure plan;
- a defined upgrade-to-immutability or emergency-stop posture acceptable to staff;
- no U.S. public access until the applicable relief or registration is effective;
  and
- periodic staff reporting plus an automatic sunset unless affirmatively extended.

The research objective would be to test solvency, deterministic settlement,
market integrity, source resistance, operational liveness, and whether a
fully prefunded conditional-asset design can provide useful price discovery
without liquidation risk. Profitability or token appreciation would not be a
pilot success criterion.

## 6. Current stop gate

The requester is willing to continue offline specification, formal proof,
simulation, and adversarial testing. The requester does not presently intend to
deploy the program, create a market, accept funds, solicit users, or enable live
trading until the U.S. deployment path is understood.

The immediate request is therefore:

> Please meet with the requester, identify the likely classification and relevant
> divisions, explain which legal path is realistically available to a small
> open-source developer, and identify the minimum facts and conditions needed for
> a formal request concerning a bounded Solana deployment.

## 7. Materials available after initial staff direction

- canonical protocol and account-state specification;
- state-transition and solvency theorem inventory;
- partition language and example market terms;
- source/settlement/manipulation threat analysis;
- protected-funds and liveness-capitalization model;
- fee and conflict alternatives;
- program authority, upgrade, static-client, and release diagrams;
- local simulation and formal-methods results;
- exact proposed collateral/source profiles;
- pilot limits and proposed disclosures; and
- proposed legal person, personnel, compensation, affiliated holdings, and
  trading restrictions.

