# Decision 0001: thin Market Core and optional capability children

Status: accepted at repository bootstrap.

## Context

Dragon's Clutch made General, Failure, Fractional, resolution, replay, and
bearer infrastructure part of a fixed eager Market foundation. It also exposed
its internal multi-stage proof and retirement workflow through a cumulative
public action namespace. This made optional sophistication universal in rent,
creation fanout, program size, operator complexity, and audit surface.

## Decision

dClutch keeps liabilities and collateral in a small universal Market Core.
Execution venues, liquidity facilities, wrappers, materialized mints, source
adapters, and recovery depth are immutable optional capability children.

A compact Market root persists through terminal state as replay authority.
Physical lazy creation is allowed only for an already selected, canonically
addressed, fully prepaid child.

Public APIs are organized by present semantic transitions. Pre-release action
numbers and account layouts have no compatibility privilege.

## Consequences

- Simple markets do not pay for unused venue or wrapper state.
- General and Dealer can evolve without redefining claim solvency.
- The core must define a strict execution-receipt boundary and authenticate the
  selected venue release.
- Program splitting versus capability-specific ELFs remains a measured choice.
- Some Dragon's Clutch integration work becomes specification evidence rather
  than transplanted runtime code.
