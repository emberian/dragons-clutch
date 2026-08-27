# Authority and relief matrix

Research date: 2026-08-17.

This is an issue-spotting research ledger, not a legal opinion. It deliberately
separates statutes/rules and Commission actions from staff advisories, speeches,
enforcement fact patterns, and requester-specific relief. “Relevance” is an
inference for counsel and agency discussion; it is not an agency conclusion about
Dragon's Clutch.

## 1. CFTC product and facility perimeter

| Authority | What the primary source says | Relevance and limit |
|---|---|---|
| [2026 Prediction Markets ANPRM](https://www.cftc.gov/LawRegulation/FederalRegister/proposedrules/2026-05105.html) | Event-contingent contracts may be swaps or futures; a prediction market offering swaps or futures to the general public must register as a DCM. A SEF may trade swaps but is limited to eligible contract participants. | This is the strongest current agency framing of the initial classification problem. The document is an advance notice requesting comment, not a final rule adjudicating Dragon's Clutch. |
| [CFTC prediction-market overview](https://www.cftc.gov/LearnandProtect/PredictionMarkets) | CFTC-regulated event contracts are derivatives; users are entitled to transparent terms, settlement rules, and decision sources. | Useful current public-facing vocabulary, not individualized classification advice. |
| [DCM overview](https://www.cftc.gov/IndustryOversight/TradingOrganizations/DCMs/index.htm) | A market providing a trading facility for non-ECPs to trade futures, options on futures, or commodity options must seek DCM designation unless an exemption or exclusion applies. | A public Dragon's Clutch venue cannot assume SEF status or decentralization solves retail access. |
| [SEF overview](https://www.cftc.gov/IndustryOversight/TradingOrganizations/SEF2/index.htm) | A system in which more than one participant can execute or trade swaps with more than one other participant must register as a SEF or be a DCM unless an exemption/exclusion applies; SEF users are ECPs. | The native batch auction appears deliberately multilateral. Product classification and retail access decide whether this is directly applicable. |
| [DCO overview](https://www.cftc.gov/IndustryOversight/ClearingOrganizations/index.htm) | A DCO may substitute its credit, arrange multilateral settlement/netting, mutualize or transfer credit risk, or otherwise provide clearing services; clearing covered products generally requires registration. | Complete-set mint/merge, the Hoard, batch netting, and settlement require a precise DCO analysis. Full collateralization may reduce credit risk but does not itself answer the statutory definition. |
| [ForecastEx DCM and DCO registrations](https://www.cftc.gov/PressRoom/PressReleases/8926-24) | The Commission granted the same event-contract company DCM designation and DCO registration. | Demonstrates that fully collateralized/event-contract innovation can fit a registered path, not that every architecture requires both registrations. |

## 2. Current prediction-market staff guidance

| Authority | What it says | Relevance and limit |
|---|---|---|
| [CFTC Staff Letter 26-08](https://www.cftc.gov/csl/26-08/download) | DMO describes current expectations for DCM event-contract listing, including core-principle compliance, complete settlement methodology, named sources, source reliability/objectivity/manipulation resistance, surveillance, and product terms. | Dragon's Clutch's frozen source/formula design is directionally responsive, but a correct source adapter does not replace facility registration or DCM supervision. This is staff advice, not relief or a binding rule. |
| [CFTC Staff Letter 26-22](https://www.cftc.gov/csl/26-22/download) | DMO disfavors broad template certifications under section 40.2(a). A class under 40.2(d) requires, among other things, identical pricing source, formula, procedure, methodology, payment calculation, and currency, tied to a prior specific product. Individual review may still be required. | A generic permissionless partition compiler is legally unlike a pre-cleared product class. A possible registered design should identify narrow source/formula families and preserve exact product terms. The letter addresses DCM/SEF submissions, not unregistered deployment. |
| [CFTC Staff Letter 26-16](https://www.cftc.gov/csl/26-16/download) | For registered DCM/SEF/DCO/FCM operations extended to 24/7, staff emphasizes continuous surveillance, risk controls, systems safeguards, staffing, incident response, business continuity, and financial-resource planning. | A “no operator service” architecture conflicts with many assumptions of the ordinary registered model. The letter is informational and creates no new duty; it does not decide whether Dragon's Clutch is registered activity. |

## 3. Blockchain and decentralized-protocol enforcement precedents

| Authority | Facts and holding summarized by the agency | Relevance and limit |
|---|---|---|
| [Polymarket order release (2022)](https://www.cftc.gov/PressRoom/PressReleases/8478-22) | The CFTC found pairs of blockchain-hosted event-based binary options were swaps and that the operator created, defined, hosted, resolved, and offered an unregistered public facility. | The closest adverse analogue. Dragon's Clutch differs in objective source mechanics, categorical complete sets, and proposed operatorless publication, but those facts cannot be assumed legally dispositive. |
| [Coinflip/Derivabit order release (2015)](https://www.cftc.gov/PressRoom/PressReleases/7231-15) | A facility connecting buyers and sellers of Bitcoin options violated commodity-option and SEF/DCM requirements; the CFTC treated Bitcoin and other virtual currencies as commodities. | Crypto-price options and a multilateral venue are within the historical enforcement perimeter even without conventional futures margin. |
| [bZeroX/Ooki release (2022)](https://www.cftc.gov/PressRoom/PressReleases/8590-22) | CFTC charged leveraged/margined retail commodity transactions, FCM/BSA violations, and a DAO successor; it rejected an attempt to make the activity enforcement-proof through decentralization. | Warns against treating a protocol or DAO label as a safe harbor. The products there were leveraged/margined retail commodity transactions, materially different from fully prefunded Eggs. |

## 4. Existing relief models—and why none can simply be copied

| Authority | Relief facts | Relevance and limit |
|---|---|---|
| [Victoria University/PredictIt Letter 14-130](https://www.cftc.gov/csl/14-130/download) | DMO gave no-action relief for a small-scale, not-for-profit academic market under extensive conditions: educational/research purpose, limited products and participation, KYC, investment/trader caps, self-directed trading, limited fees, no commissions, and uncompensated operators. | Shows staff can address an unregistered event market through narrow relief. It is not a general retail protocol precedent and cannot be relied on by Dragon's Clutch. |
| [PredictIt amendment Letter 25-20](https://www.cftc.gov/csl/25-20/download) | DMO permitted transfer to a U.S. nonprofit research consortium, certain director compensation, operational/research use of trading fees, for-profit service providers, removal of a trader-number cap, and an inflation-linked investment cap while retaining the earlier conditions. | Shows compensation and service providers are not categorically incompatible with that specific academic relief. The relief is personal, conditional, nontransferable, and built around a nonprofit research market. |
| [Phantom Letter 26-09](https://www.cftc.gov/csl/26-09/download) | MPD granted conditional IB/AP no-action relief for a self-custodial wallet frontend providing access to registered collaborators. The request distinguishes client software from custody and discretionary routing but contains significant representations, contracts, disclosures, and recordkeeping. | Useful for a future static-client boundary only if the underlying venue is registered and the exact conditions fit. It is not permission to create or operate an unregistered market. |
| [Rule 140.99 letter definitions](https://www.cftc.gov/LawRegulation/CFTCStaffLetters/lettersdefined) | Exemptive, no-action, and interpretive letters have different effects. No-action relief binds only issuing staff for the persons and facts addressed; third parties cannot rely on it. | Dragon's Clutch needs its own correctly directed request, not analogy as permission. |
| [Rule 140.99 adopting release](https://www.cftc.gov/sites/default/files/opa/press98/opa4211-98-att.htm) | The Commission did not require a requester to retain counsel, but requires a full statement of material facts, clear issues, and a thorough examination of applicable law with representative authority. | The present packet is preparation, not yet a compliant formal request. Counsel is still strongly recommended because multiple divisions and registration categories may be implicated. |
| [Relief publication FAQ](https://www.cftc.gov/Transparency/relieffaqs) | Granted requests and responses are ordinarily public; confidential treatment may generally delay publication for up to 120 days under the described procedure. | The factual submission should be drafted for eventual public release and should not contain secrets. |
| [Innovation Task Force meeting form](https://forms.cftc.gov/forms/InnovationMeetingRequest) and [meeting log](https://www.cftc.gov/About/Innovation/meetings) | Interested parties may request meetings; the public log shows frequent meetings with crypto and DeFi developers and organizations. | Appropriate first contact. A meeting is dialogue, not interpretive, exemptive, or no-action relief. |
| [Chair Selig, “Next Phase of Project Crypto” (2026)](https://www.cftc.gov/PressRoom/SpeechesTestimony/opaselig1) | The Chair described exploration of innovation exemptions and clear safe harbors for novel products and platforms. | Evidence of policy interest, not presently effective law or a safe harbor. Any route must be confirmed through actual Commission/staff action. |
| [Example Commission section 4(c) order](https://www.cftc.gov/sites/default/files/opa/press99/opa4247-99-attch.htm) | The Commission described authority to grant conditional, prospective or retroactive exemptions to promote responsible financial innovation and fair competition when statutory public-interest and other findings are satisfied. The example restricted participation to sophisticated persons and relied on comprehensive foreign supervision. | Section 4(c) is a real Commission-level tool, but it is not a general retail innovation waiver. The “appropriate persons,” public-interest, regulatory-impact, and any provision-specific limits require counsel and Commission analysis. |

## 5. Other federal perimeters to brief separately

| Authority | What it says | Relevance and limit |
|---|---|---|
| [FinCEN 2019 convertible-virtual-currency guidance](https://www.fincen.gov/sites/default/files/2019-05/FinCEN%20Guidance%20CVC%20FINAL%20508.pdf) | Mere creation of a decentralized application is not money transmission, while using or deploying it to perform covered value transmission can be; analysis is functional and fact-specific. | Publication, deployment, fees, and control require a separate Bank Secrecy Act/MSB analysis. CFTC relief would not decide it. |
| [OFAC virtual-currency compliance guidance](https://ofac.treasury.gov/system/files/126/virtual_currency_guidance_brochure.pdf) | OFAC obligations apply to U.S. persons and virtual-currency activity; enforcement may use strict liability, and OFAC recommends risk-based controls. | Immutable/permissionless access creates a real sanctions-design question that “no server” does not erase. |
| [Joint SEC/CFTC 2026 crypto-asset interpretation](https://www.cftc.gov/LawRegulation/FederalRegister/finalrules/2026-05635.html) | The agencies distinguish non-security crypto assets and address when transactions or promises may create securities-law consequences. | A Realm collateral token may be a non-security crypto asset, but that does not establish that a contingent Egg referencing its price is spot or non-security. Product and marketing analysis remain separate. |
| [DOJ “Ending Regulation by Prosecution” memorandum](https://www.justice.gov/dag/media/1395781/dl) and [2025 developer remarks](https://www.justice.gov/opa/speech/acting-assistant-attorney-general-matthew-r-galeotti-delivers-remarks-american) | DOJ described an internal charging posture against using criminal law as a substitute for digital-asset regulation and discussed neutral open-source, noncustodial software and criminal intent. | Potentially relevant to criminal prosecutorial posture, but it creates no private rights and is not a CFTC, FinCEN, OFAC, SEC, or state civil safe harbor. |

State derivatives/gaming/consumer law, SEC product/token analysis, tax, privacy,
intellectual property, and state money-transmission law remain outside this
matrix. CFTC jurisdiction or relief would not necessarily preempt or resolve all
of those issues.

## 6. Working conclusions to test with counsel and staff

1. **High-confidence:** technical noncustody, full collateralization, open source,
   and decentralization are not categorical exemptions from the CEA.
2. **High-confidence:** a public multilateral venue for products classified as
   futures, swaps, or commodity options has a substantial DCM/SEF problem.
3. **High-confidence:** existing relief letters cannot be copied or relied on by
   a third party.
4. **Likely but unresolved:** the complete-set mint/merge and program settlement
   require a serious DCO analysis even if no counterparty credit is created.
5. **Likely but unresolved:** a general market compiler is harder to fit into
   current product-review practice than one narrow, exact source/formula class.
6. **Likely but unresolved:** ordinary 24/7 registrant obligations assume a legal
   operator capable of surveillance, intervention, reporting, staffing, and
   recovery; an immutable unattended program needs bespoke treatment or a
   different operating structure.
7. **Open:** whether Commission/staff authority can support a capped retail
   research pilot for this architecture, or whether the only realistic paths are
   registration, an existing registrant partnership, ECP-only scope, or no U.S.
   deployment.
