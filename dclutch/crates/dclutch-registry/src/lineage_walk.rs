//! Following a release-set lineage forward: links into a chain.
//!
//! [`ReleaseLineageV1`] names exactly one hop. A market founded three cuts ago
//! is three hops behind the world, and every reader that needs its history --
//! the SDK, the site, the lifecycle rent credit whose close must reach the
//! *live* activation cache -- needs the whole chain rather than one link. This
//! module is the single authority for walking one into the other.
//!
//! It is deliberately **not** a fetcher. Each caller supplies its own lookup,
//! because the three callers cannot share one: an on-chain route reads accounts
//! already present in its frame, a host tool reads RPC, a test reads a fixture.
//! What they must share is the *rule* -- when a chain has been followed
//! correctly, and by what name it refuses when it has not -- and the rule is
//! what lives here. A second copy of this walk anywhere else is a second author
//! for one fact.
//!
//! # What the walk reads, and what it cannot
//!
//! Two facts per hop: the set a record was keyed by, and the set it names. No
//! clock -- because [`ReleaseLineageV1`] carries no clock. The design omitted a
//! `declared_at_slot` stamp on the grounds that no conjunct would read it, and
//! that omission is what makes lineage **retroactively authorable**: a hop
//! declared today, for two release sets that superseded each other months ago,
//! encodes to exactly the 248 bytes it would have encoded to at the time. There
//! is no timestamp to backdate and no contemporaneity to counterfeit. A history
//! written down late is not a weaker history; it is the same history, later.
//!
//! # What it refuses, and what it does not defend against
//!
//! A chain can be wrong in three ways this module names
//! ([`LineageWalkRefusal`]): it can end before it arrives, it can hand back a
//! record keyed by something other than what was asked for, or it can run
//! longer than [`LINEAGE_WALK_MAX_HOPS_V1`].
//!
//! A *cycle* is not separately named, and that is a decision rather than an
//! oversight. On chain a cycle cannot be built at all: `DeclareSuccessor`
//! refuses a hop whose deployment slots do not strictly advance, so the chain
//! is forward-only by the shape of the data. Off chain, against a hostile
//! source, a cycle terminates as [`LineageWalkRefusal::TooLong`] -- bounded and
//! named, never a hang. Spending a 1 KiB stack buffer of visited sets to
//! rename a refusal the caller already gets would buy nothing and would cost it
//! on a chain whose frame budget is already watchlisted.

use dclutch_core_contract::ContentId;

use crate::{Error, ReleaseLineageV1};

/// Hops one walk will follow before refusing to follow further.
///
/// A market migrates once per cut it is behind, so this is a bound on cuts, not
/// on markets. Thirty-two is far past any plausible backlog and small enough
/// that an adversarial source cannot spend a reader's budget.
pub const LINEAGE_WALK_MAX_HOPS_V1: u8 = 32;

/// What a caller found at the lineage address derived for one release set.
///
/// The caller decodes, because decoding is [`ReleaseLineageV1`]'s job and a
/// second decoder here would be a second author. The walk then judges the
/// chain, which is a fact no single record holds.
// `large_enum_variant`: the lint's only remedy is boxing the large variant, and
// its own note says that costs `Copy`. This crate is `no_std` with no allocator,
// so `Box` is not available at all, and `Copy` is what lets a caller pass a found
// lineage by value instead of borrowing across a decode. The size is the record's.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineageAt {
    /// The derived address is vacant: no successor was ever declared.
    ///
    /// This is the ordinary end of every healthy chain -- the head of the
    /// lineage is precisely the set nobody has superseded yet.
    Undeclared,
    /// A record was found at the derived address and decoded.
    Declared(ReleaseLineageV1),
    /// Bytes were present at the derived address but did not decode.
    Undecodable(Error),
}

/// Why a lineage walk did not arrive.
///
/// Each variant carries the release set the walk was standing on when it
/// refused, because that set is the actionable fact: it is the one a successor
/// must be declared for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineageWalkRefusal {
    /// The chain ended at a set that is not the one the walk was sent to.
    ///
    /// The gap, named. Nothing is corrupt: a hop simply has not been declared
    /// yet, and `at` is the predecessor it must be declared for.
    SuccessorUndeclared {
        /// The last set the chain reached.
        at: ContentId,
    },
    /// A record fetched under one predecessor names a different one.
    ///
    /// The off-chain shape of the on-chain `ReleaseLineageMismatch`: a record
    /// is only evidence about the set whose address derives it, so one that
    /// disagrees with its own address is evidence about nothing.
    Misaddressed {
        /// The set whose lineage address the lookup was made under.
        sought: ContentId,
        /// The predecessor the returned record actually names.
        found: ContentId,
    },
    /// A record was present but could not be decoded.
    Undecodable {
        /// The set whose lineage record failed to decode.
        at: ContentId,
        /// The codec's own refusal.
        cause: Error,
    },
    /// The chain ran past [`LINEAGE_WALK_MAX_HOPS_V1`] without arriving.
    TooLong {
        /// The set the walk had reached when it gave up.
        at: ContentId,
    },
}

impl From<LineageWalkRefusal> for Error {
    fn from(value: LineageWalkRefusal) -> Self {
        match value {
            LineageWalkRefusal::SuccessorUndeclared { .. } => Self::LineageSuccessorUndeclared,
            LineageWalkRefusal::Misaddressed { .. } => Self::LineageMisaddressed,
            LineageWalkRefusal::Undecodable { cause, .. } => cause,
            LineageWalkRefusal::TooLong { .. } => Self::LineageWalkTooLong,
        }
    }
}

/// Where a completed walk arrived, and how far it travelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineageWalkV1 {
    endpoint: ContentId,
    hops: u8,
}

impl LineageWalkV1 {
    /// The release set the walk arrived at.
    pub const fn endpoint(self) -> ContentId {
        self.endpoint
    }

    /// Hops followed to get there.
    ///
    /// This is exactly the number of `MigrateMarket` transactions a market
    /// standing at the walk's origin still owes, since a hop is walked and
    /// never asserted.
    pub const fn hops(self) -> u8 {
        self.hops
    }

    /// Whether the origin was already the destination.
    ///
    /// The lineage form of the deployment set's `AlreadyCurrent` disposition,
    /// and the same kind of fact: nothing to do, admitted on an equality rather
    /// than on a receipt. A zero-hop walk is *satisfied*, not incomplete.
    pub const fn is_already_current(self) -> bool {
        self.hops == 0
    }
}

/// Follow a lineage from `origin` to the head of its chain.
///
/// The head is the set with no declared successor -- the one the world is
/// currently on, as far as the supplied lookup can see. Answers "where did my
/// market's world go?" for a reader holding nothing but a founding pin.
///
/// # Errors
///
/// [`LineageWalkRefusal::Misaddressed`] or [`LineageWalkRefusal::Undecodable`]
/// if the source hands back a record that is not evidence about the set it was
/// asked about, and [`LineageWalkRefusal::TooLong`] past
/// [`LINEAGE_WALK_MAX_HOPS_V1`]. It never refuses
/// [`LineageWalkRefusal::SuccessorUndeclared`]: an undeclared successor is what
/// *ends* this walk rather than what fails it.
pub fn walk_lineage_to_head<F>(
    origin: ContentId,
    lookup: F,
) -> Result<LineageWalkV1, LineageWalkRefusal>
where
    F: FnMut(ContentId) -> LineageAt,
{
    walk(origin, None, lookup)
}

/// Follow a lineage from `origin` until it reaches `destination`.
///
/// Answers the question the cut actually poses: *is this market's history
/// followable from the new world?* A chain that ends short refuses
/// [`LineageWalkRefusal::SuccessorUndeclared`] naming the set that still owes a
/// declaration, which is the whole repair instruction.
///
/// An `origin` equal to `destination` walks zero hops and succeeds; see
/// [`LineageWalkV1::is_already_current`].
///
/// # Errors
///
/// [`LineageWalkRefusal::SuccessorUndeclared`] when the chain ends before
/// arriving, plus every refusal of [`walk_lineage_to_head`].
pub fn walk_lineage_to<F>(
    origin: ContentId,
    destination: ContentId,
    lookup: F,
) -> Result<LineageWalkV1, LineageWalkRefusal>
where
    F: FnMut(ContentId) -> LineageAt,
{
    walk(origin, Some(destination), lookup)
}

fn walk<F>(
    origin: ContentId,
    destination: Option<ContentId>,
    mut lookup: F,
) -> Result<LineageWalkV1, LineageWalkRefusal>
where
    F: FnMut(ContentId) -> LineageAt,
{
    let mut standing = origin;
    let mut hops: u8 = 0;
    loop {
        if destination == Some(standing) {
            return Ok(LineageWalkV1 {
                endpoint: standing,
                hops,
            });
        }
        match lookup(standing) {
            LineageAt::Undeclared => {
                return match destination {
                    // The chain ended short of where it was sent.
                    Some(_) => Err(LineageWalkRefusal::SuccessorUndeclared { at: standing }),
                    // The chain ended, which is what "head" means.
                    None => Ok(LineageWalkV1 {
                        endpoint: standing,
                        hops,
                    }),
                };
            }
            LineageAt::Undecodable(cause) => {
                return Err(LineageWalkRefusal::Undecodable {
                    at: standing,
                    cause,
                });
            }
            LineageAt::Declared(record) => {
                // A record is evidence only about the set whose address derives
                // it. One that names a different predecessor was fetched from
                // the wrong address or forged for another; either way it says
                // nothing about where this market goes next.
                if record.predecessor() != standing {
                    return Err(LineageWalkRefusal::Misaddressed {
                        sought: standing,
                        found: record.predecessor(),
                    });
                }
                // The bound refuses only once a further hop is actually
                // offered. A chain of exactly the bound arrives; it is the
                // hop past it that is refused, and the refusal names where
                // the walk was standing when it declined to go on.
                if hops == LINEAGE_WALK_MAX_HOPS_V1 {
                    return Err(LineageWalkRefusal::TooLong { at: standing });
                }
                standing = record.successor();
                hops = hops.saturating_add(1);
            }
        }
    }
}
