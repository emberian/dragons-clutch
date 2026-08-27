# Source release, deployment, and revenue boundary

## 1. Three separate acts

Dragon's Clutch must not conflate:

1. publishing AGPL source, specifications, proofs, and local fixtures;
2. deploying immutable programs or creating onchain Market infrastructure; and
3. operating, promoting, trading on, solving for, or receiving revenue from a
   real-money venue.

Each act has a different technical, economic, and legal perimeter. Passing the
source-release gates authorizes neither deployment nor operation.

## 2. Current U.S. warning

This is a design warning, not legal advice. As of 2026-08-17, the CFTC says event
contracts are typically swaps and that a prediction market offering swaps or
futures to the general public generally must register as a designated contract
market. It has separately treated crypto-price binary options as commodity
options/swaps and enforced against unregistered online and DeFi facilities.
Fully collateralized design removes credit and liquidation risk but does not, by
itself, remove the instrument or venue from that perimeter.

“Broker” is therefore too narrow a question. Depending on exact activities, the
relevant categories can include DCM, SEF, derivatives clearing organization,
futures commission merchant, introducing broker, commodity trading adviser,
commodity pool, money transmitter, securities exchange/broker, state gaming law,
and sanctions/consumer-protection obligations. Some will not apply, but code
cannot decide that.

FinCEN guidance distinguishes merely creating a dapp from using or deploying it
to engage in money transmission. That supports a source-publication distinction;
it does not create a universal smart-contract exemption.

Primary starting points:

- [CFTC 2026 prediction-markets ANPRM](https://www.cftc.gov/LawRegulation/FederalRegister/proposedrules/2026-05105.html)
- [CFTC prediction-market overview](https://www.cftc.gov/LearnandProtect/PredictionMarkets)
- [CFTC Polymarket order announcement](https://www.cftc.gov/PressRoom/PressReleases/8478-22)
- [CFTC Coinflip Bitcoin-options action](https://www.cftc.gov/PressRoom/PressReleases/7231-15)
- [CFTC bZeroX/Ooki action](https://www.cftc.gov/PressRoom/PressReleases/8590-22)
- [FinCEN convertible-virtual-currency guidance](https://www.fincen.gov/sites/default/files/2019-05/FinCEN%20Guidance%20CVC%20FINAL%20508.pdf)
- [NFA introducing-broker definition](https://www.nfa.futures.org/registration-membership/who-has-to-register/ib.html)

Before an author-affiliated real-money deployment or JOSHI trading, obtain a
written analysis from counsel experienced in CFTC derivatives, crypto protocols,
FinCEN/BSA, securities, sanctions, state law, and developer constitutional issues.
Freeze the exact instruments, users/jurisdictions, frontend, fees, admin powers,
deployment entity, trading role, and revenue paths presented to counsel.

## 3. Revenue is a deployment policy, not Eggcrate law

Eggcrate proves fee conservation and protected-pool separation. It should not
hard-code a fee destination, maintainer identity, or one deployment's treasury
policy.

A Realm selects an immutable audited `RevenuePolicy` from a closed set. Candidate
sinks may include:

- maker rebate;
- bounded auction verifier/executor reward;
- audit/security/public-goods reserve;
- transparent development treasury;
- zero protocol take.

Every sink is outside Hoard principal and prepaid liveness. A revenue policy must
name its recipients, caps, vesting or withdrawal conditions, accounting,
conflicts, and whether any keeper is paid. A Realm cannot silently redirect the
fees of an already active Market.

## 4. Affiliated interests and sustainability

Any deployer, maintainer, fee recipient, collateral holder, market maker, source
participant, or affiliated trader may have an economic interest in a deployment.
That does not make the deployment inherently improper, but it makes neutrality
claims untenable and demands accurate conflict analysis and disclosure.

Two practical constraints follow:

1. protocol revenue may be volatile, delayed, and insufficient to fund continued
   engineering; and
2. compensation, promotion, affiliated trading, collateral holdings, and control
   can alter securities, derivatives, manipulation, disclosure, and conflict
   analysis.

The engineering default keeps revenue policy explicit and outside consensus
solvency. Any author-affiliated deployment must freeze and disclose recipients,
interests, control, and permitted affiliated activity before accepting funds.

## 5. Recommended release tracks

### Track A: public research/source

Specifications, verified kernel, Rocq model, SBF adapter, fixtures, static client,
benchmarks, and deployment tooling. No official real-money deployment. This track
can succeed independently.

### Track B: permissionless third-party deployment kit

Reproducible build and immutable deployment manifest, Realm/source conformance,
warnings, and no Dragon representation that a deployment is lawful. Whether this
meaningfully separates the authors from operation is a legal question, not a
README incantation.

### Track C: author-affiliated devnet or research deployment

No real value, explicitly authorized, exact build identity, measured operation,
and no claim that devnet answers the legal question.

### Track D: author-affiliated real-money deployment

Blocked until written legal analysis, independent audits, incident/disclosure
plan, conflict policy, surveillance design, economic capitalization, exact revenue
policy, and separate explicit authorization all exist.

### Track E: JOSHI principal trading or market making

Separately blocked even if Track D opens. It needs person/jurisdiction authority,
operator/principal conflict controls, source-influence exclusions, address and
self-trade policy, and a staged shadow-to-user-signed promotion.

## 6. Architectural consequence

The core roadmap proceeds without deciding that Ember will deploy, operate, or
trade. Release manifests identify deployer, upgrade authority, fee beneficiaries,
revenue policy, and known affiliated trading/solver addresses where applicable.
The static client shows those facts before signature. No UI describes an
author-affiliated venue as neutral, ownerless, or legally permissionless merely
because the programs are immutable.
