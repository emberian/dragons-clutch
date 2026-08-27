//! The EXECUTED / NEVER-EXECUTED report.
//!
//! There is no coverage percentage to game and no threshold to pass. The report
//! prints the routes, and it prints them all.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::model::{Blocked, BlockedSet, Inventory, Ledger, Outcome, RouteKind};

struct Coverage<'a> {
    executed: BTreeSet<&'a str>,
    refused: BTreeSet<&'a str>,
    refusal_seen: BTreeSet<&'a str>,
    by_route: BTreeMap<&'a str, usize>,
}

fn coverage(ledger: &Ledger) -> Coverage<'_> {
    let mut executed = BTreeSet::new();
    let mut refused = BTreeSet::new();
    let mut refusal_seen = BTreeSet::new();
    let mut by_route: BTreeMap<&str, usize> = BTreeMap::new();
    for observation in &ledger.observations {
        *by_route.entry(observation.route.as_str()).or_default() += 1;
        match observation.outcome {
            Outcome::Executed => {
                executed.insert(observation.route.as_str());
            }
            Outcome::Refused => {
                refused.insert(observation.route.as_str());
            }
        }
        if let Some(refusal) = &observation.refusal {
            refusal_seen.insert(refusal.as_str());
        }
    }
    Coverage {
        executed,
        refused,
        refusal_seen,
        by_route,
    }
}

fn blocked_for<'a>(blocked: &'a BlockedSet, id: &str) -> Option<&'a Blocked> {
    blocked
        .blocked
        .iter()
        .filter(|entry| match entry.route.strip_suffix('*') {
            Some(prefix) => id.starts_with(prefix),
            None => entry.route == id,
        })
        // Prefer the most specific rule.
        .max_by_key(|entry| entry.route.trim_end_matches('*').len())
}

pub struct Totals {
    pub stale_blocked: usize,
    pub routes: usize,
    pub routes_executed: usize,
    pub routes_refused_only: usize,
    pub routes_never: usize,
    pub routes_never_blocked: usize,
    pub refusals: usize,
    pub refusals_observed: usize,
    pub unclassified: usize,
}

#[allow(clippy::too_many_lines)]
pub fn render(inventory: &Inventory, ledger: &Ledger, blocked: &BlockedSet) -> (String, Totals) {
    let coverage = coverage(ledger);
    let mut used_blocked: BTreeSet<&str> = BTreeSet::new();
    let mut stale_because_executed: Vec<(String, String)> = Vec::new();
    let mut out = String::new();
    let mut totals = Totals {
        stale_blocked: 0,
        routes: 0,
        routes_executed: 0,
        routes_refused_only: 0,
        routes_never: 0,
        routes_never_blocked: 0,
        refusals: 0,
        refusals_observed: 0,
        unclassified: 0,
    };

    let _ = writeln!(out, "# dClutch execution census");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Static enumeration of every program's public dispatch surface, joined to the\n\
         routes the gauntlet has actually driven on a validator. A route is EXECUTED only\n\
         when a finalized transaction named it AND the chain's own log messages show that\n\
         program invoked. NEVER-EXECUTED is not an absence of evidence; it is the evidence."
    );
    let _ = writeln!(out);
    if let Some(revision) = &inventory.source_revision {
        let _ = writeln!(out, "source revision: `{revision}`");
    }
    let _ = writeln!(out, "observations in ledger: {}", ledger.observations.len());
    let _ = writeln!(out);

    for program in &inventory.programs {
        let _ = writeln!(out, "## {} (`{}`)", program.label, program.package);
        let _ = writeln!(out);
        if program.entrypoints.is_empty() {
            let _ = writeln!(
                out,
                "No `entrypoint!` was found in `{}`. This program exposes no dispatch\n\
                 surface in this build configuration; its refusal taxonomy is still listed.",
                program.crate_root
            );
            let _ = writeln!(out);
        } else {
            for entrypoint in &program.entrypoints {
                let _ = writeln!(
                    out,
                    "- entrypoint `{}!({})` at `{}`{}",
                    entrypoint.macro_name,
                    entrypoint.function,
                    entrypoint.provenance,
                    if entrypoint.resolved {
                        ""
                    } else {
                        "  **UNRESOLVED — dispatch NOT enumerated**"
                    }
                );
            }
            let _ = writeln!(out);
        }

        if program.routes.is_empty() {
            let _ = writeln!(out, "_No routes enumerated._");
            let _ = writeln!(out);
        } else {
            let _ = writeln!(out, "| status | route | selects on | source |");
            let _ = writeln!(out, "|---|---|---|---|");
            // Blocking reasons are collected once per program and printed
            // under the table: repeating a paragraph on every row makes the
            // table unreadable, and an unreadable report is one nobody checks.
            let mut reasons: Vec<(String, String)> = Vec::new();
            for route in &program.routes {
                totals.routes += 1;
                let id = route.id.as_str();
                let executed = coverage.executed.contains(id);
                let refused = coverage.refused.contains(id);
                let count = coverage.by_route.get(id).copied().unwrap_or(0);
                if executed || refused {
                    // A route that has run does not need a blocker. Remember the
                    // entry so it can be reported as stale and deleted.
                    if let Some(entry) = blocked_for(blocked, id) {
                        // It matched something, so it is not orphaned — it is
                        // stale for the sharper reason reported below.
                        used_blocked.insert(entry.route.as_str());
                        stale_because_executed.push((entry.route.clone(), route.id.clone()));
                    }
                }
                let status = if executed {
                    totals.routes_executed += 1;
                    format!("EXECUTED ({count}x)")
                } else if refused {
                    totals.routes_refused_only += 1;
                    format!("REFUSED-ONLY ({count}x)")
                } else {
                    totals.routes_never += 1;
                    match blocked_for(blocked, id) {
                        Some(entry) => {
                            totals.routes_never_blocked += 1;
                            used_blocked.insert(entry.route.as_str());
                            let held = reasons.iter().position(|(_, r)| r == &entry.reason);
                            let marker = if let Some(index) = held {
                                reasons[index].0.clone()
                            } else {
                                let next = format!("b{}", reasons.len() + 1);
                                reasons.push((next.clone(), entry.reason.clone()));
                                next
                            };
                            format!("NEVER-EXECUTED [{}] ({marker})", entry.owner)
                        }
                        None => "**NEVER-EXECUTED — no stated reason**".to_string(),
                    }
                };
                let selectors = if route.selectors.is_empty() {
                    "(unguarded)".to_string()
                } else {
                    route
                        .selectors
                        .iter()
                        .map(crate::model::Selector::render)
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                let kind = match route.kind {
                    RouteKind::Entry => "",
                    RouteKind::Action => " _(action)_",
                };
                let cfg = if route.cfg.is_empty() {
                    String::new()
                } else {
                    format!(" `{}`", route.cfg.join(" "))
                };
                let _ = writeln!(
                    out,
                    "| {status} | `{}`{kind}{cfg} | {selectors} | `{}` |",
                    route.id, route.provenance
                );
            }
            let _ = writeln!(out);
            if !reasons.is_empty() {
                let _ = writeln!(out, "Blocking reasons:");
                let _ = writeln!(out);
                for (marker, reason) in &reasons {
                    let _ = writeln!(out, "- **({marker})** {reason}");
                }
                let _ = writeln!(out);
            }
        }

        if !program.refusals.is_empty() {
            let observed: Vec<&crate::model::Refusal> = program
                .refusals
                .iter()
                .filter(|refusal| coverage.refusal_seen.contains(refusal.id.as_str()))
                .collect();
            totals.refusals += program.refusals.len();
            totals.refusals_observed += observed.len();
            let _ = writeln!(
                out,
                "**Refusal taxonomy**: {} codes, {} observed on-chain.",
                program.refusals.len(),
                observed.len()
            );
            let _ = writeln!(out);
            let _ = writeln!(out, "| status | code | refusal | meaning | source |");
            let _ = writeln!(out, "|---|---|---|---|---|");
            for refusal in &program.refusals {
                let seen = coverage.refusal_seen.contains(refusal.id.as_str());
                let status = if seen { "OBSERVED" } else { "never raised" };
                let code = refusal
                    .code
                    .map_or_else(|| "?".to_string(), |code| code.to_string());
                let _ = writeln!(
                    out,
                    "| {status} | {code} | `{}::{}` | {} | `{}` |",
                    refusal.enum_name,
                    refusal.variant,
                    refusal.doc.as_deref().unwrap_or(""),
                    refusal.provenance
                );
            }
            let _ = writeln!(out);
        }

        if !program.unclassified.is_empty() {
            totals.unclassified += program.unclassified.len();
            let _ = writeln!(
                out,
                "**UNCLASSIFIED dispatch positions**: {}. The enumerator did not recognise\n\
                 these; they are printed rather than dropped, because an enumerator that\n\
                 silently under-counts is the same mirror failure one level up.",
                program.unclassified.len()
            );
            let _ = writeln!(out);
            for entry in &program.unclassified {
                let _ = writeln!(
                    out,
                    "- `{}` in `{}` — {} — `{}`",
                    entry.expression, entry.context, entry.reason, entry.provenance
                );
            }
            let _ = writeln!(out);
        }
    }

    let orphaned: Vec<&Blocked> = blocked
        .blocked
        .iter()
        .filter(|entry| !used_blocked.contains(entry.route.as_str()))
        .collect();
    if !orphaned.is_empty() || !stale_because_executed.is_empty() {
        totals.stale_blocked = orphaned.len() + stale_because_executed.len();
        let _ = writeln!(out, "## Stale blocking entries");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "A blocking entry outlives its reason as easily as a test outlives its\n\
             invariant. These entries in `blocked.json` no longer describe anything\n\
             true and should be deleted."
        );
        let _ = writeln!(out);
        for entry in orphaned {
            let _ = writeln!(
                out,
                "- `{}` matches no enumerated route [{}]",
                entry.route, entry.owner
            );
        }
        for (pattern, route) in &stale_because_executed {
            let _ = writeln!(
                out,
                "- `{pattern}` still blocks `{route}`, **which has now executed** — delete it"
            );
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "## Totals");
    let _ = writeln!(out);
    let _ = writeln!(out, "| measure | count |");
    let _ = writeln!(out, "|---|---:|");
    let _ = writeln!(out, "| routes enumerated | {} |", totals.routes);
    let _ = writeln!(
        out,
        "| routes EXECUTED (at least one succeeding transaction) | {} |",
        totals.routes_executed
    );
    let _ = writeln!(
        out,
        "| routes REFUSED-ONLY (reached, always refused) | {} |",
        totals.routes_refused_only
    );
    let _ = writeln!(out, "| routes NEVER-EXECUTED | {} |", totals.routes_never);
    let _ = writeln!(
        out,
        "| ...of those, with a named blocker and owning lane | {} |",
        totals.routes_never_blocked
    );
    let _ = writeln!(
        out,
        "| ...of those, with NO stated reason at all | {} |",
        totals.routes_never - totals.routes_never_blocked
    );
    let _ = writeln!(out, "| refusal codes enumerated | {} |", totals.refusals);
    let _ = writeln!(
        out,
        "| refusal codes OBSERVED on-chain | {} |",
        totals.refusals_observed
    );
    let _ = writeln!(
        out,
        "| unclassified dispatch positions | {} |",
        totals.unclassified
    );
    let _ = writeln!(
        out,
        "| stale blocking entries to delete | {} |",
        totals.stale_blocked
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "These numbers are not a score. `NEVER-EXECUTED` with no stated reason is the\n\
         column that matters: it is the set of routes nobody has claimed, blocked, or run."
    );

    (out, totals)
}
