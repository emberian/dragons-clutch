# ADR-0002: hybrid claim representation

Status: proposed

## Context

Mandatory Token-2022 materialization of every outcome makes complete-set issuance
and native batching expensive. Internal-only claims are cheaper but lose ordinary
token composability.

## Decision

Use fixed Eggcrate-owned internal Position balances for native transitions and a
canonical Token-2022 mint per outcome. Materialize or dematerialize one selected
outcome on demand. Maintain conservative internal and accounted-external supply
per outcome.

## Consequences

- Native split and trading avoid one token account/CPI per outcome.
- Users retain an ordinary transferable token boundary.
- Eggcrate must own every internal balance/reservation transition.
- Direct external burns are donations and require reduction-only reconciliation.
- The Token-2022 adapter remains a separately named trust boundary.

## Evidence required

Supply conservation and maximum-liability proofs, two synthetic collateral Realm
profiles, hostile CPI/account tests, direct-burn fixtures, and cost comparison
against mandatory full materialization and internal-only controls.

## Authority impact

None. A materialization plan is not a deployed mint or financial authority.
