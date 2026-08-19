#![no_std]
#![forbid(unsafe_code)]

//! MODEL-ONLY terminal lifecycle for creation-time V2 markets.
//!
//! The state here is deliberately smaller than a Solana adapter, but it keeps
//! the facts a terminal adapter must not conflate: each account's role/rent
//! principal, internal and external claims, authoritative mint supply, a
//! neutral surplus sink, and the permanent replay principal.  It is not SBF,
//! a Token-2022 integration, or a migration mechanism.  V1 is an explicit
//! STOP, as are any unmodeled fractional-credit paths.

pub const MAX_OUTCOMES: usize = 4;
pub const MAX_POSITIONS: usize = 4;
/// One ledger slot for each distinct rent-bearing V2 account role.
pub const MAX_RENT_ROLES: usize = 6 + MAX_POSITIONS + MAX_OUTCOMES;
pub type Id = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalVersion {
    V1Legacy,
    V2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountTag {
    pub market: Id,
    pub version: TerminalVersion,
    pub generation: u64,
}

/// Role is part of an account identity, not a display label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Market,
    Hoard,
    Kernel,
    Supply,
    Resolution,
    Position(u8),
    Mint(u8),
    Replay,
}

impl Role {
    const fn account(self) -> Id {
        match self {
            Self::Market => 1,
            Self::Hoard => 2,
            Self::Kernel => 3,
            Self::Supply => 4,
            Self::Resolution => 5,
            Self::Position(slot) => 16 + slot as Id,
            Self::Mint(outcome) => 32 + outcome as Id,
            Self::Replay => 63,
        }
    }

    const fn ledger_index(self) -> usize {
        match self {
            Self::Market => 0,
            Self::Hoard => 1,
            Self::Kernel => 2,
            Self::Supply => 3,
            Self::Resolution => 4,
            Self::Position(slot) => 5 + slot as usize,
            Self::Mint(outcome) => 5 + MAX_POSITIONS + outcome as usize,
            Self::Replay => MAX_RENT_ROLES - 1,
        }
    }
}

/// Prepaid principal for one distinct account.  `account` plus `tag` and
/// `role` are the model's account identity; it cannot alias another role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentIdentity {
    pub tag: AccountTag,
    pub role: Role,
    pub account: Id,
    pub refund_to: Id,
    pub lamports: u64,
}

impl RentIdentity {
    const fn new(tag: AccountTag, role: Role, refund_to: Id) -> Self {
        Self {
            tag,
            role,
            account: role.account(),
            refund_to,
            lamports: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintCloseAuthority {
    pub tag: AccountTag,
    pub terminal_authority: Id,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Shape,
    Arithmetic,
    Phase,
    Identity,
    RefundIdentity,
    Authority,
    SurplusSink,
    Insufficient,
    OutstandingLiability,
    OutstandingSupply,
    FractionalRemainder,
    /// External bearer fragmentation has no per-account credit representation.
    ExternalBearerStop,
    SurplusRequired,
    CloseOrder,
    AlreadyClosed,
    MissingMintCloseAuthority,
    LegacyStop,
    Replay,
    AggregateClosure,
    MintSupplyMismatch,
}
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Active,
    Resolved,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Market {
    pub tag: AccountTag,
    pub terminal_authority: Id,
    /// Immutable disposal destination, distinct from every rent recipient.
    pub surplus_sink: Id,
    pub outcomes: u8,
    pub phase: Phase,
    pub rent: RentIdentity,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hoard {
    pub tag: AccountTag,
    pub rent: RentIdentity,
    pub collateral: u64,
    pub donations: u64,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Kernel {
    pub tag: AccountTag,
    pub rent: RentIdentity,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Supply {
    pub tag: AccountTag,
    pub rent: RentIdentity,
    pub total: [u64; MAX_OUTCOMES],
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    pub tag: AccountTag,
    pub rent: RentIdentity,
    pub denominator: u64,
    pub weights: [u64; MAX_OUTCOMES],
    pub receipt: u64,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub occupied: bool,
    pub tag: AccountTag,
    pub holder: Id,
    pub rent: RentIdentity,
    pub claims: [u64; MAX_OUTCOMES],
    pub closed: bool,
}

impl Position {
    const EMPTY: Self = Self {
        occupied: false,
        tag: AccountTag {
            market: 0,
            version: TerminalVersion::V2,
            generation: 0,
        },
        holder: 0,
        rent: RentIdentity {
            tag: AccountTag {
                market: 0,
                version: TerminalVersion::V2,
                generation: 0,
            },
            role: Role::Position(0),
            account: 16,
            refund_to: 0,
            lamports: 0,
        },
        claims: [0; MAX_OUTCOMES],
        closed: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeMint {
    pub tag: AccountTag,
    pub outcome: u8,
    pub rent: RentIdentity,
    pub close_authority: Option<MintCloseAuthority>,
    /// Authoritative current mint supply, not a program cache.
    pub authoritative_supply: u64,
    pub closed: bool,
}

impl OutcomeMint {
    const EMPTY: Self = Self {
        tag: AccountTag {
            market: 0,
            version: TerminalVersion::V2,
            generation: 0,
        },
        outcome: 0,
        rent: RentIdentity {
            tag: AccountTag {
                market: 0,
                version: TerminalVersion::V2,
                generation: 0,
            },
            role: Role::Mint(0),
            account: 32,
            refund_to: 0,
            lamports: 0,
        },
        close_authority: None,
        authoritative_supply: 0,
        closed: false,
    };
}

/// Permanent, independently prepaid anti-recreation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayTombstone {
    pub tag: AccountTag,
    pub rent: RentIdentity,
    pub terminal_receipt: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registry {
    pub tombstone: Option<ReplayTombstone>,
}
impl Registry {
    pub const fn empty() -> Self {
        Self { tombstone: None }
    }

    /// Authenticate the only permanent account against its still-resident
    /// terminal record.  Its independently prepaid principal is never entered
    /// into the refundable-role ledger.
    pub fn check_for(&self, model: &Model) -> Result<()> {
        let tombstone = self.tombstone.ok_or(Error::Replay)?;
        if !model.market.closed
            || tombstone.tag != model.market.tag
            || tombstone.rent != model.replay_rent
            || tombstone.rent.role != Role::Replay
            || tombstone.rent.lamports == 0
            || tombstone.terminal_receipt != model.terminal_receipt
            || model.refunded_by_role[Role::Replay.ledger_index()] != 0
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Model {
    pub market: Market,
    pub hoard: Hoard,
    pub kernel: Kernel,
    pub supply: Supply,
    pub resolution: Option<Resolution>,
    pub positions: [Position; MAX_POSITIONS],
    pub mints: [OutcomeMint; MAX_OUTCOMES],
    /// Aggregate bearer holdings separate from program-owned Positions.
    pub external_bearer: [u64; MAX_OUTCOMES],
    /// This is prepaid at creation, survives Market close, and is never refunded.
    pub replay_rent: RentIdentity,
    /// Every Hoard collateral atom enters through one of these model ingress
    /// classes; check() closes this ledger against value disposition.
    pub collateral_issued: u64,
    pub collateral_donated: u64,
    pub redeemed: u64,
    pub surplus_disposed: u64,
    /// Recorded transfer amount for the immutable `Market.surplus_sink`.
    pub surplus_sink_paid: u64,
    pub refunded_rent: u64,
    /// Per-role payment ledger.  A role can return its distinct principal once.
    pub refunded_by_role: [u64; MAX_RENT_ROLES],
    pub terminal_receipt: u64,
}

impl Model {
    pub fn new_v2(
        registry: &Registry,
        market: Id,
        terminal_authority: Id,
        refund_to: Id,
        surplus_sink: Id,
        outcomes: u8,
    ) -> Result<Self> {
        if registry.tombstone.is_some() {
            return Err(Error::Replay);
        }
        if !(2..=MAX_OUTCOMES as u8).contains(&outcomes)
            || market == 0
            || terminal_authority == 0
            || refund_to == 0
            || surplus_sink == 0
            || surplus_sink == refund_to
            || terminal_authority == refund_to
            || terminal_authority == surplus_sink
        {
            return Err(Error::Shape);
        }
        let tag = AccountTag {
            market,
            version: TerminalVersion::V2,
            generation: 0,
        };
        let market_rent = RentIdentity::new(tag, Role::Market, refund_to);
        let mut mints = [OutcomeMint {
            tag,
            outcome: 0,
            rent: RentIdentity::new(tag, Role::Mint(0), refund_to),
            close_authority: Some(MintCloseAuthority {
                tag,
                terminal_authority,
            }),
            authoritative_supply: 0,
            closed: false,
        }; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            mints[i].outcome = i as u8;
            mints[i].rent = RentIdentity::new(tag, Role::Mint(i as u8), refund_to);
            if i >= usize::from(outcomes) {
                mints[i] = OutcomeMint::EMPTY;
            }
            i += 1;
        }
        let model = Self {
            market: Market {
                tag,
                terminal_authority,
                surplus_sink,
                outcomes,
                phase: Phase::Active,
                rent: market_rent,
                closed: false,
            },
            hoard: Hoard {
                tag,
                rent: RentIdentity::new(tag, Role::Hoard, refund_to),
                collateral: 0,
                donations: 0,
                closed: false,
            },
            kernel: Kernel {
                tag,
                rent: RentIdentity::new(tag, Role::Kernel, refund_to),
                closed: false,
            },
            supply: Supply {
                tag,
                rent: RentIdentity::new(tag, Role::Supply, refund_to),
                total: [0; MAX_OUTCOMES],
                closed: false,
            },
            resolution: None,
            positions: [Position::EMPTY; MAX_POSITIONS],
            mints,
            external_bearer: [0; MAX_OUTCOMES],
            replay_rent: RentIdentity::new(tag, Role::Replay, refund_to),
            collateral_issued: 0,
            collateral_donated: 0,
            redeemed: 0,
            surplus_disposed: 0,
            surplus_sink_paid: 0,
            refunded_rent: 0,
            refunded_by_role: [0; MAX_RENT_ROLES],
            terminal_receipt: 0,
        };
        model.check()?;
        Ok(model)
    }

    /// Legacy construction exists only for the uncloseable-mint STOP test.
    pub fn legacy_for_stop(market: Id, refund_to: Id) -> Self {
        let tag = AccountTag {
            market,
            version: TerminalVersion::V1Legacy,
            generation: 0,
        };
        let rent = RentIdentity::new(tag, Role::Market, refund_to);
        Self {
            market: Market {
                tag,
                terminal_authority: 999,
                surplus_sink: 998,
                outcomes: 2,
                phase: Phase::Resolved,
                rent,
                closed: false,
            },
            hoard: Hoard {
                tag,
                rent: RentIdentity::new(tag, Role::Hoard, refund_to),
                collateral: 0,
                donations: 0,
                closed: false,
            },
            kernel: Kernel {
                tag,
                rent: RentIdentity::new(tag, Role::Kernel, refund_to),
                closed: false,
            },
            supply: Supply {
                tag,
                rent: RentIdentity::new(tag, Role::Supply, refund_to),
                total: [0; MAX_OUTCOMES],
                closed: false,
            },
            resolution: Some(Resolution {
                tag,
                rent: RentIdentity::new(tag, Role::Resolution, refund_to),
                denominator: 1,
                weights: [1, 0, 0, 0],
                receipt: 7,
                closed: false,
            }),
            positions: [Position::EMPTY; MAX_POSITIONS],
            mints: [OutcomeMint {
                tag,
                outcome: 0,
                rent: RentIdentity::new(tag, Role::Mint(0), refund_to),
                close_authority: None,
                authoritative_supply: 0,
                closed: false,
            }; MAX_OUTCOMES],
            external_bearer: [0; MAX_OUTCOMES],
            replay_rent: RentIdentity::new(tag, Role::Replay, refund_to),
            collateral_issued: 0,
            collateral_donated: 0,
            redeemed: 0,
            surplus_disposed: 0,
            surplus_sink_paid: 0,
            refunded_rent: 0,
            refunded_by_role: [0; MAX_RENT_ROLES],
            terminal_receipt: 0,
        }
    }

    pub fn issue_to_position(
        &mut self,
        slot: usize,
        holder: Id,
        quantity: u64,
        refund_to: Id,
    ) -> Result<()> {
        self.transact(|next| {
            next.active()?;
            if slot >= MAX_POSITIONS
                || holder == 0
                || refund_to == 0
                || quantity == 0
                || next.positions[slot].occupied
                || refund_to == next.market.surplus_sink
            {
                return Err(Error::Shape);
            }
            let tag = next.market.tag;
            let mut claims = [0; MAX_OUTCOMES];
            let mut i = 0;
            while i < usize::from(next.market.outcomes) {
                claims[i] = quantity;
                next.supply.total[i] = next.supply.total[i]
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                next.mints[i].authoritative_supply = next.mints[i]
                    .authoritative_supply
                    .checked_add(quantity)
                    .ok_or(Error::Arithmetic)?;
                i += 1;
            }
            next.positions[slot] = Position {
                occupied: true,
                tag,
                holder,
                rent: RentIdentity::new(tag, Role::Position(slot as u8), refund_to),
                claims,
                closed: false,
            };
            next.hoard.collateral = next
                .hoard
                .collateral
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.collateral_issued = next
                .collateral_issued
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check()
        })
    }

    /// Move an internal claim into aggregate external bearer supply without
    /// changing Supply or the authoritative Mint total.
    pub fn materialize_bearer(&mut self, slot: usize, outcome: u8, quantity: u64) -> Result<()> {
        let _ = (slot, outcome, quantity);
        // This model cannot bind bearer fragments to account identities before
        // resolution.  Permitting materialization would create a known terminal
        // dead end, so the V2 terminal profile refuses it outright.
        Err(Error::ExternalBearerStop)
    }

    pub fn donate(&mut self, quantity: u64) -> Result<()> {
        self.transact(|next| {
            if quantity == 0 || next.hoard.closed {
                return Err(Error::Shape);
            }
            next.hoard.collateral = next
                .hoard
                .collateral
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.hoard.donations = next
                .hoard
                .donations
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.collateral_donated = next
                .collateral_donated
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
            next.check()
        })
    }

    pub fn resolve(
        &mut self,
        denominator: u64,
        weights: [u64; MAX_OUTCOMES],
        receipt: u64,
    ) -> Result<()> {
        self.transact(|next| {
            next.active()?;
            next.validate_weights(denominator, weights)?;
            next.validate_ownership_lots(denominator, weights)?;
            if next.hoard.collateral < next.liability_for(denominator, weights)? {
                return Err(Error::Insufficient);
            }
            next.resolution = Some(Resolution {
                tag: next.market.tag,
                rent: RentIdentity::new(
                    next.market.tag,
                    Role::Resolution,
                    next.market.rent.refund_to,
                ),
                denominator,
                weights,
                receipt,
                closed: false,
            });
            next.market.phase = Phase::Resolved;
            next.check()
        })
    }

    pub fn redeem_position(&mut self, slot: usize, outcome: u8, quantity: u64) -> Result<u64> {
        let mut paid = 0;
        self.transact(|next| {
            next.resolved()?;
            let i = next.outcome_index(outcome)?;
            if slot >= MAX_POSITIONS
                || !next.positions[slot].occupied
                || quantity == 0
                || next.positions[slot].claims[i] < quantity
            {
                return Err(Error::Insufficient);
            }
            paid = next.exact_payout(i, quantity)?;
            next.positions[slot].claims[i] -= quantity;
            next.burn_supply(i, quantity)?;
            next.hoard.collateral = next
                .hoard
                .collateral
                .checked_sub(paid)
                .ok_or(Error::Insufficient)?;
            next.redeemed = next.redeemed.checked_add(paid).ok_or(Error::Arithmetic)?;
            next.check()
        })?;
        Ok(paid)
    }

    pub fn burn_position(&mut self, slot: usize, outcome: u8, quantity: u64) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            let i = next.outcome_index(outcome)?;
            if slot >= MAX_POSITIONS
                || !next.positions[slot].occupied
                || quantity == 0
                || next.positions[slot].claims[i] < quantity
            {
                return Err(Error::Insufficient);
            }
            next.positions[slot].claims[i] -= quantity;
            next.burn_supply(i, quantity)?;
            next.check()
        })
    }

    /// This internal-only terminal profile has no authenticated bearer account
    /// plane.  The method exists only to make that unsupported boundary total.
    pub fn burn_bearer(&mut self, outcome: u8, quantity: u64) -> Result<()> {
        let _ = (outcome, quantity);
        Err(Error::ExternalBearerStop)
    }

    pub fn close_position(&mut self, slot: usize, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            if slot >= MAX_POSITIONS {
                return Err(Error::Shape);
            }
            if next.positions[slot].closed {
                return Err(Error::AlreadyClosed);
            }
            if !next.positions[slot].occupied || next.position_has_claims(slot) {
                return Err(Error::OutstandingSupply);
            }
            let rent = next.positions[slot].rent;
            next.refund(rent, Role::Position(slot as u8), recipient)?;
            next.positions[slot].closed = true;
            next.positions[slot].occupied = false;
            next.check()
        })
    }

    /// This is a transfer to the immutable neutral sink, never a rent refund.
    pub fn dispose_surplus(&mut self, supplied_sink: Id) -> Result<u64> {
        let mut disposed = 0;
        self.transact(|next| {
            next.resolved()?;
            if supplied_sink != next.market.surplus_sink {
                return Err(Error::SurplusSink);
            }
            if next.liability()? != 0 {
                return Err(Error::OutstandingLiability);
            }
            disposed = next.hoard.collateral;
            if disposed == 0 {
                return Err(Error::SurplusRequired);
            }
            next.hoard.collateral = 0;
            next.surplus_disposed = next
                .surplus_disposed
                .checked_add(disposed)
                .ok_or(Error::Arithmetic)?;
            next.surplus_sink_paid = next
                .surplus_sink_paid
                .checked_add(disposed)
                .ok_or(Error::Arithmetic)?;
            next.check()
        })?;
        Ok(disposed)
    }

    /// The authority is a supplied, authenticated input, as it must be in an
    /// adapter instruction.  Mint close uses authoritative, not cached, supply.
    pub fn close_mint(&mut self, outcome: u8, supplied_authority: Id, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            if next.market.tag.version != TerminalVersion::V2 {
                return Err(Error::LegacyStop);
            }
            let i = next.outcome_index(outcome)?;
            if next.mints[i].closed {
                return Err(Error::AlreadyClosed);
            }
            if next.any_position_occupied() {
                return Err(Error::CloseOrder);
            }
            let mint = next.mints[i];
            let authority = mint
                .close_authority
                .ok_or(Error::MissingMintCloseAuthority)?;
            next.check_tag(mint.tag)?;
            next.check_tag(authority.tag)?;
            if supplied_authority != authority.terminal_authority
                || supplied_authority != next.market.terminal_authority
            {
                return Err(Error::Authority);
            }
            if next.supply.total[i] != 0
                || mint.authoritative_supply != 0
                || next.external_bearer[i] != 0
            {
                return Err(Error::OutstandingSupply);
            }
            next.refund(mint.rent, Role::Mint(outcome), recipient)?;
            next.mints[i].closed = true;
            next.check()
        })
    }

    pub fn close_supply(&mut self, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            if next.supply.closed {
                return Err(Error::AlreadyClosed);
            }
            if !next.all_active_mints_closed() || next.any_position_occupied() {
                return Err(Error::CloseOrder);
            }
            next.refund(next.supply.rent, Role::Supply, recipient)?;
            next.supply.closed = true;
            next.check()
        })
    }
    pub fn close_resolution(&mut self, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            let resolution = next.resolution.ok_or(Error::Phase)?;
            if resolution.closed {
                return Err(Error::AlreadyClosed);
            }
            if !next.supply.closed {
                return Err(Error::CloseOrder);
            }
            next.refund(resolution.rent, Role::Resolution, recipient)?;
            let mut closed = resolution;
            closed.closed = true;
            next.resolution = Some(closed);
            next.check()
        })
    }
    pub fn close_kernel(&mut self, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            if next.kernel.closed {
                return Err(Error::AlreadyClosed);
            }
            if !next.resolution.ok_or(Error::Phase)?.closed {
                return Err(Error::CloseOrder);
            }
            next.refund(next.kernel.rent, Role::Kernel, recipient)?;
            next.kernel.closed = true;
            next.check()
        })
    }
    pub fn close_hoard(&mut self, recipient: Id) -> Result<()> {
        self.transact(|next| {
            next.resolved()?;
            if next.hoard.closed {
                return Err(Error::AlreadyClosed);
            }
            if !next.kernel.closed || next.hoard.collateral != 0 {
                return Err(Error::CloseOrder);
            }
            next.refund(next.hoard.rent, Role::Hoard, recipient)?;
            next.hoard.closed = true;
            next.check()
        })
    }
    pub fn close_market(
        &mut self,
        registry: &mut Registry,
        supplied_authority: Id,
        recipient: Id,
    ) -> Result<()> {
        if self.market.closed {
            return Err(Error::AlreadyClosed);
        }
        self.resolved()?;
        if registry.tombstone.is_some() {
            return Err(Error::Replay);
        }
        if supplied_authority != self.market.terminal_authority {
            return Err(Error::Authority);
        }
        if !self.hoard.closed {
            return Err(Error::CloseOrder);
        }
        let mut next = *self;
        next.refund(next.market.rent, Role::Market, recipient)?;
        next.market.closed = true;
        next.market.phase = Phase::Terminal;
        next.terminal_receipt = next.resolution.ok_or(Error::Phase)?.receipt;
        let tombstone = ReplayTombstone {
            tag: next.market.tag,
            rent: next.replay_rent,
            terminal_receipt: next.terminal_receipt,
        };
        next.check()?;
        let mut next_registry = *registry;
        next_registry.tombstone = Some(tombstone);
        next_registry.check_for(&next)?;
        *self = next;
        *registry = next_registry;
        Ok(())
    }

    /// Checks reachable state only.  Deliberately public so hostile tests can
    /// demonstrate that corrupted adapter prestate is rejected without panic.
    pub fn check(&self) -> Result<()> {
        let count = usize::from(self.market.outcomes);
        if !(2..=MAX_OUTCOMES).contains(&count)
            || self.market.tag.market == 0
            || self.market.terminal_authority == 0
            || self.market.surplus_sink == 0
            || self.market.surplus_sink == self.market.rent.refund_to
            || self.market.terminal_authority == self.market.rent.refund_to
            || self.market.terminal_authority == self.market.surplus_sink
        {
            return Err(Error::Shape);
        }
        self.check_rent(self.market.rent, Role::Market)?;
        self.check_rent(self.hoard.rent, Role::Hoard)?;
        self.check_rent(self.kernel.rent, Role::Kernel)?;
        self.check_rent(self.supply.rent, Role::Supply)?;
        self.check_rent(self.replay_rent, Role::Replay)?;
        if self.replay_rent.lamports == 0 {
            return Err(Error::Identity);
        }
        let mut ledger_total = 0_u64;
        let mut ledger_index = 0_usize;
        while ledger_index < MAX_RENT_ROLES {
            ledger_total = ledger_total
                .checked_add(self.refunded_by_role[ledger_index])
                .ok_or(Error::Arithmetic)?;
            ledger_index += 1;
        }
        if ledger_total != self.refunded_rent {
            return Err(Error::Identity);
        }
        if self.surplus_sink_paid != self.surplus_disposed {
            return Err(Error::Identity);
        }
        if self.external_bearer.iter().any(|quantity| *quantity != 0) {
            return Err(Error::ExternalBearerStop);
        }
        if self.hoard.donations != self.collateral_donated {
            return Err(Error::Identity);
        }
        let ingress = self
            .collateral_issued
            .checked_add(self.collateral_donated)
            .ok_or(Error::Arithmetic)?;
        let disposition = self
            .hoard
            .collateral
            .checked_add(self.redeemed)
            .and_then(|value| value.checked_add(self.surplus_disposed))
            .ok_or(Error::Arithmetic)?;
        if ingress != disposition {
            return Err(Error::Identity);
        }
        self.check_rent_payment(self.market.rent, self.market.closed)?;
        self.check_rent_payment(self.hoard.rent, self.hoard.closed)?;
        self.check_rent_payment(self.kernel.rent, self.kernel.closed)?;
        self.check_rent_payment(self.supply.rent, self.supply.closed)?;
        self.check_rent_payment(self.replay_rent, false)?;
        self.check_tag(self.hoard.tag)?;
        self.check_tag(self.kernel.tag)?;
        self.check_tag(self.supply.tag)?;
        let mut internal = [0_u64; MAX_OUTCOMES];
        let mut slot = 0;
        while slot < MAX_POSITIONS {
            let position = self.positions[slot];
            if position.occupied {
                if position.closed || position.holder == 0 {
                    return Err(Error::CloseOrder);
                }
                self.check_tag(position.tag)?;
                self.check_rent(position.rent, Role::Position(slot as u8))?;
                self.check_rent_payment(position.rent, false)?;
                let mut i = 0;
                while i < count {
                    internal[i] = internal[i]
                        .checked_add(position.claims[i])
                        .ok_or(Error::Arithmetic)?;
                    i += 1;
                }
                while i < MAX_OUTCOMES {
                    if position.claims[i] != 0 {
                        return Err(Error::Shape);
                    }
                    i += 1;
                }
            } else {
                if position.closed && position_has_nonzero(position) {
                    return Err(Error::CloseOrder);
                }
                if position.closed {
                    self.check_rent(position.rent, Role::Position(slot as u8))?;
                    self.check_rent_payment(position.rent, true)?;
                } else if position != Position::EMPTY {
                    return Err(Error::Shape);
                }
            }
            slot += 1;
        }
        let mut i = 0;
        while i < MAX_OUTCOMES {
            if i < count {
                let mint = self.mints[i];
                self.check_tag(mint.tag)?;
                self.check_rent(mint.rent, Role::Mint(i as u8))?;
                self.check_rent_payment(mint.rent, mint.closed)?;
                if mint.outcome != i as u8 {
                    return Err(Error::Shape);
                }
                if self.market.tag.version == TerminalVersion::V2 {
                    let authority = mint
                        .close_authority
                        .ok_or(Error::MissingMintCloseAuthority)?;
                    self.check_tag(authority.tag)?;
                    if authority.terminal_authority == 0
                        || authority.terminal_authority != self.market.terminal_authority
                    {
                        return Err(Error::Authority);
                    }
                }
                let aggregate = internal[i]
                    .checked_add(self.external_bearer[i])
                    .ok_or(Error::Arithmetic)?;
                if self.supply.total[i] != aggregate {
                    return Err(Error::AggregateClosure);
                }
                if mint.authoritative_supply != self.supply.total[i] {
                    return Err(Error::MintSupplyMismatch);
                }
                if mint.closed && mint.authoritative_supply != 0 {
                    return Err(Error::CloseOrder);
                }
                if mint.closed && self.any_position_occupied() {
                    return Err(Error::CloseOrder);
                }
            } else if self.supply.total[i] != 0
                || self.external_bearer[i] != 0
                || self.mints[i] != OutcomeMint::EMPTY
            {
                return Err(Error::Shape);
            }
            i += 1;
        }
        self.check_exact_rent_ledger()?;
        match self.resolution {
            None => {
                if self.market.phase != Phase::Active
                    || self.supply.closed
                    || self.kernel.closed
                    || self.hoard.closed
                    || self.market.closed
                    || self.any_active_mints_closed()
                    || self.any_position_closed()
                    || self.redeemed != 0
                    || self.surplus_disposed != 0
                    || self.terminal_receipt != 0
                {
                    return Err(Error::CloseOrder);
                }
                let mut active_outcome = 0_usize;
                while active_outcome < count {
                    if self.supply.total[active_outcome] != self.collateral_issued {
                        return Err(Error::AggregateClosure);
                    }
                    active_outcome += 1;
                }
            }
            Some(resolution) => {
                self.check_tag(resolution.tag)?;
                self.check_rent(resolution.rent, Role::Resolution)?;
                self.check_rent_payment(resolution.rent, resolution.closed)?;
                self.validate_weights(resolution.denominator, resolution.weights)?;
                self.validate_ownership_lots(resolution.denominator, resolution.weights)?;
                if self.market.phase == Phase::Active {
                    return Err(Error::Phase);
                }
                if self.hoard.collateral < self.liability()? {
                    return Err(Error::Insufficient);
                }
                if self.surplus_disposed != 0 && self.liability()? != 0 {
                    return Err(Error::CloseOrder);
                }
                if self.supply.closed
                    && (!self.all_active_mints_closed() || self.any_position_occupied())
                {
                    return Err(Error::CloseOrder);
                }
                if resolution.closed && !self.supply.closed {
                    return Err(Error::CloseOrder);
                }
                if self.kernel.closed && !resolution.closed {
                    return Err(Error::CloseOrder);
                }
                if self.hoard.closed && (!self.kernel.closed || self.hoard.collateral != 0) {
                    return Err(Error::CloseOrder);
                }
                if self.market.closed != (self.market.phase == Phase::Terminal) {
                    return Err(Error::CloseOrder);
                }
                if self.market.closed && !self.hoard.closed {
                    return Err(Error::CloseOrder);
                }
                if self.market.closed {
                    if self.terminal_receipt != resolution.receipt {
                        return Err(Error::Identity);
                    }
                } else if self.terminal_receipt != 0 {
                    return Err(Error::CloseOrder);
                }
            }
        }
        Ok(())
    }

    fn active(&self) -> Result<()> {
        if self.market.phase == Phase::Active {
            Ok(())
        } else {
            Err(Error::Phase)
        }
    }
    fn resolved(&self) -> Result<()> {
        if self.market.phase == Phase::Resolved {
            Ok(())
        } else {
            Err(Error::Phase)
        }
    }
    fn outcome_index(&self, outcome: u8) -> Result<usize> {
        if outcome < self.market.outcomes {
            Ok(usize::from(outcome))
        } else {
            Err(Error::Shape)
        }
    }
    fn check_tag(&self, tag: AccountTag) -> Result<()> {
        if tag == self.market.tag {
            Ok(())
        } else {
            Err(Error::Identity)
        }
    }
    fn check_rent(&self, rent: RentIdentity, role: Role) -> Result<()> {
        self.check_tag(rent.tag)?;
        if rent.role != role
            || rent.account != role.account()
            || rent.refund_to == 0
            || rent.lamports == 0
            || rent.refund_to == self.market.surplus_sink
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
    fn refund(&mut self, rent: RentIdentity, role: Role, recipient: Id) -> Result<()> {
        self.check_rent(rent, role)?;
        if recipient != rent.refund_to {
            return Err(Error::RefundIdentity);
        }
        let paid = self.refunded_by_role[role.ledger_index()];
        if paid != 0 {
            return Err(Error::AlreadyClosed);
        }
        self.refunded_by_role[role.ledger_index()] = rent.lamports;
        self.refunded_rent = self
            .refunded_rent
            .checked_add(rent.lamports)
            .ok_or(Error::Arithmetic)?;
        Ok(())
    }
    fn check_rent_payment(&self, rent: RentIdentity, closed: bool) -> Result<()> {
        let paid = self.refunded_by_role[rent.role.ledger_index()];
        if paid != 0 && paid != rent.lamports {
            return Err(Error::Identity);
        }
        if closed != (paid == rent.lamports) {
            return Err(Error::CloseOrder);
        }
        Ok(())
    }
    /// No absent/padded role may carry a phantom paid principal.  This also
    /// makes the aggregate rent counter a derived check, never the authority.
    fn check_exact_rent_ledger(&self) -> Result<()> {
        let mut expected = [0_u64; MAX_RENT_ROLES];
        if self.market.closed {
            expected[Role::Market.ledger_index()] = self.market.rent.lamports;
        }
        if self.hoard.closed {
            expected[Role::Hoard.ledger_index()] = self.hoard.rent.lamports;
        }
        if self.kernel.closed {
            expected[Role::Kernel.ledger_index()] = self.kernel.rent.lamports;
        }
        if self.supply.closed {
            expected[Role::Supply.ledger_index()] = self.supply.rent.lamports;
        }
        if let Some(resolution) = self.resolution {
            if resolution.closed {
                expected[Role::Resolution.ledger_index()] = resolution.rent.lamports;
            }
        }
        let mut slot = 0_usize;
        while slot < MAX_POSITIONS {
            if self.positions[slot].closed {
                expected[Role::Position(slot as u8).ledger_index()] =
                    self.positions[slot].rent.lamports;
            }
            slot += 1;
        }
        let mut outcome = 0_usize;
        while outcome < usize::from(self.market.outcomes) {
            if self.mints[outcome].closed {
                expected[Role::Mint(outcome as u8).ledger_index()] =
                    self.mints[outcome].rent.lamports;
            }
            outcome += 1;
        }
        if expected != self.refunded_by_role {
            return Err(Error::Identity);
        }
        Ok(())
    }
    fn burn_supply(&mut self, i: usize, quantity: u64) -> Result<()> {
        self.supply.total[i] = self.supply.total[i]
            .checked_sub(quantity)
            .ok_or(Error::Insufficient)?;
        self.mints[i].authoritative_supply = self.mints[i]
            .authoritative_supply
            .checked_sub(quantity)
            .ok_or(Error::Insufficient)?;
        Ok(())
    }
    fn position_has_claims(&self, slot: usize) -> bool {
        position_has_nonzero(self.positions[slot])
    }
    fn any_position_occupied(&self) -> bool {
        self.positions.iter().any(|position| position.occupied)
    }
    fn any_position_closed(&self) -> bool {
        self.positions.iter().any(|position| position.closed)
    }
    fn all_active_mints_closed(&self) -> bool {
        let mut i = 0;
        while i < usize::from(self.market.outcomes) {
            if !self.mints[i].closed {
                return false;
            }
            i += 1;
        }
        true
    }
    fn any_active_mints_closed(&self) -> bool {
        let mut i = 0_usize;
        while i < usize::from(self.market.outcomes) {
            if self.mints[i].closed {
                return true;
            }
            i += 1;
        }
        false
    }
    fn validate_weights(&self, denominator: u64, weights: [u64; MAX_OUTCOMES]) -> Result<()> {
        if denominator == 0 {
            return Err(Error::Shape);
        }
        let mut sum = 0_u64;
        let mut i = 0;
        while i < MAX_OUTCOMES {
            if i < usize::from(self.market.outcomes) {
                if weights[i] > denominator {
                    return Err(Error::Shape);
                }
                sum = sum.checked_add(weights[i]).ok_or(Error::Arithmetic)?;
            } else if weights[i] != 0 {
                return Err(Error::Shape);
            }
            i += 1;
        }
        if sum == denominator {
            Ok(())
        } else {
            Err(Error::Shape)
        }
    }
    /// Every Position is independently owned.  Aggregate external bearer
    /// supply has no per-holder identity in this internal-only model, so any
    /// bearer amount is a fail-closed STOP until an account/credit model exists.
    fn validate_ownership_lots(
        &self,
        denominator: u64,
        weights: [u64; MAX_OUTCOMES],
    ) -> Result<()> {
        let mut slot = 0_usize;
        while slot < MAX_POSITIONS {
            let position = self.positions[slot];
            if position.occupied {
                let mut outcome = 0_usize;
                while outcome < usize::from(self.market.outcomes) {
                    let numerator = u128::from(position.claims[outcome])
                        .checked_mul(u128::from(weights[outcome]))
                        .ok_or(Error::Arithmetic)?;
                    if numerator % u128::from(denominator) != 0 {
                        return Err(Error::FractionalRemainder);
                    }
                    outcome += 1;
                }
            }
            slot += 1;
        }
        let mut outcome = 0_usize;
        while outcome < usize::from(self.market.outcomes) {
            if self.external_bearer[outcome] != 0 {
                return Err(Error::ExternalBearerStop);
            }
            outcome += 1;
        }
        Ok(())
    }
    fn liability_for(&self, denominator: u64, weights: [u64; MAX_OUTCOMES]) -> Result<u64> {
        let mut total = 0_u64;
        let mut i = 0;
        while i < usize::from(self.market.outcomes) {
            let numerator = u128::from(self.supply.total[i])
                .checked_mul(u128::from(weights[i]))
                .ok_or(Error::Arithmetic)?;
            if numerator % u128::from(denominator) != 0 {
                return Err(Error::FractionalRemainder);
            }
            total = total
                .checked_add(
                    u64::try_from(numerator / u128::from(denominator))
                        .map_err(|_| Error::Arithmetic)?,
                )
                .ok_or(Error::Arithmetic)?;
            i += 1;
        }
        Ok(total)
    }
    fn liability(&self) -> Result<u64> {
        let r = self.resolution.ok_or(Error::Phase)?;
        self.liability_for(r.denominator, r.weights)
    }
    fn exact_payout(&self, outcome: usize, quantity: u64) -> Result<u64> {
        let r = self.resolution.ok_or(Error::Phase)?;
        let n = u128::from(quantity)
            .checked_mul(u128::from(r.weights[outcome]))
            .ok_or(Error::Arithmetic)?;
        if n % u128::from(r.denominator) != 0 {
            return Err(Error::FractionalRemainder);
        }
        u64::try_from(n / u128::from(r.denominator)).map_err(|_| Error::Arithmetic)
    }
    fn transact<F>(&mut self, action: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        let mut next = *self;
        action(&mut next)?;
        *self = next;
        Ok(())
    }
}

fn position_has_nonzero(position: Position) -> bool {
    position.claims.iter().any(|claim| *claim != 0)
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests {
    use super::*;
    fn v2() -> (Registry, Model) {
        let r = Registry::empty();
        let m = Model::new_v2(&r, 11, 22, 33, 66, 2).unwrap();
        (r, m)
    }
    fn empty_closeable(model: &mut Model) {
        model.resolve(1, [1, 0, 0, 0], 77).unwrap();
        model.close_mint(0, 22, 33).unwrap();
        model.close_mint(1, 22, 33).unwrap();
    }

    #[test]
    fn arbitrary_prefund_is_donation_not_refund_or_liability() {
        let (_r, mut m) = v2();
        m.donate(9).unwrap();
        m.issue_to_position(0, 44, 2, 55).unwrap();
        m.resolve(1, [1, 0, 0, 0], 1).unwrap();
        m.redeem_position(0, 0, 2).unwrap();
        m.redeem_position(0, 1, 2).unwrap();
        m.close_position(0, 55).unwrap();
        assert_eq!(m.hoard.donations, 9);
        assert_eq!(m.dispose_surplus(66).unwrap(), 9);
    }
    #[test]
    fn guessed_refund_owner_cannot_close_position() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 1, 55).unwrap();
        m.resolve(1, [1, 0, 0, 0], 1).unwrap();
        m.redeem_position(0, 0, 1).unwrap();
        m.redeem_position(0, 1, 1).unwrap();
        assert_eq!(m.close_position(0, 999), Err(Error::RefundIdentity));
        assert!(m.positions[0].occupied);
    }
    #[test]
    fn holder_burn_creates_surplus_but_needs_sink_and_zero_liability() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 1, 55).unwrap();
        m.donate(7).unwrap();
        m.resolve(1, [1, 0, 0, 0], 1).unwrap();
        assert_eq!(m.dispose_surplus(66), Err(Error::OutstandingLiability));
        m.burn_position(0, 0, 1).unwrap();
        m.burn_position(0, 1, 1).unwrap();
        assert_eq!(m.dispose_surplus(33), Err(Error::SurplusSink));
        assert_eq!(m.dispose_surplus(66).unwrap(), 8);
    }
    #[test]
    fn fractional_one_one_over_two_fragment_refuses() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 1, 55).unwrap();
        assert_eq!(
            m.resolve(2, [1, 1, 0, 0], 1),
            Err(Error::FractionalRemainder)
        );
    }
    #[test]
    fn split_owned_fractional_fragments_refuse_even_when_aggregate_is_integral() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 1, 55).unwrap();
        m.issue_to_position(1, 45, 1, 56).unwrap();
        assert_eq!(
            m.resolve(2, [1, 1, 0, 0], 7),
            Err(Error::FractionalRemainder)
        );
    }
    #[test]
    fn external_bearer_materialization_is_profile_stop_before_it_can_strand() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 1, 55).unwrap();
        assert_eq!(
            m.materialize_bearer(0, 0, 1),
            Err(Error::ExternalBearerStop)
        );
        assert_eq!(m.external_bearer, [0; MAX_OUTCOMES]);
    }
    #[test]
    fn wrong_terminal_authority_refuses_mint_and_market_close() {
        let (mut r, mut m) = v2();
        empty_closeable(&mut m);
        m.close_supply(33).unwrap();
        m.close_resolution(33).unwrap();
        m.close_kernel(33).unwrap();
        m.close_hoard(33).unwrap();
        assert_eq!(m.close_market(&mut r, 999, 33), Err(Error::Authority));
        let (_r, mut mint_model) = v2();
        mint_model.resolve(1, [1, 0, 0, 0], 1).unwrap();
        assert_eq!(mint_model.close_mint(0, 999, 33), Err(Error::Authority));
    }
    #[test]
    fn close_roles_refund_once_only_and_roll_back_on_replay() {
        let (_r, mut m) = v2();
        empty_closeable(&mut m);
        m.close_supply(33).unwrap();
        let after_supply = m.refunded_rent;
        assert_eq!(m.close_supply(33), Err(Error::AlreadyClosed));
        assert_eq!(m.refunded_rent, after_supply);
        m.close_resolution(33).unwrap();
        let after_resolution = m.refunded_rent;
        assert_eq!(m.close_resolution(33), Err(Error::AlreadyClosed));
        assert_eq!(m.refunded_rent, after_resolution);
        m.close_kernel(33).unwrap();
        let after_kernel = m.refunded_rent;
        assert_eq!(m.close_kernel(33), Err(Error::AlreadyClosed));
        assert_eq!(m.refunded_rent, after_kernel);
        m.close_hoard(33).unwrap();
        let after_hoard = m.refunded_rent;
        assert_eq!(m.close_hoard(33), Err(Error::AlreadyClosed));
        assert_eq!(m.refunded_rent, after_hoard);
    }
    #[test]
    fn every_close_role_refuses_a_second_attempt() {
        let (_r, mut position_model) = v2();
        position_model.issue_to_position(0, 44, 1, 55).unwrap();
        position_model.resolve(1, [1, 0, 0, 0], 1).unwrap();
        position_model.redeem_position(0, 0, 1).unwrap();
        position_model.redeem_position(0, 1, 1).unwrap();
        position_model.close_position(0, 55).unwrap();
        assert_eq!(
            position_model.close_position(0, 55),
            Err(Error::AlreadyClosed)
        );

        let (_r, mut mint_model) = v2();
        mint_model.resolve(1, [1, 0, 0, 0], 1).unwrap();
        mint_model.close_mint(0, 22, 33).unwrap();
        assert_eq!(mint_model.close_mint(0, 22, 33), Err(Error::AlreadyClosed));

        let (mut registry, mut market_model) = v2();
        empty_closeable(&mut market_model);
        market_model.close_supply(33).unwrap();
        market_model.close_resolution(33).unwrap();
        market_model.close_kernel(33).unwrap();
        market_model.close_hoard(33).unwrap();
        market_model.close_market(&mut registry, 22, 33).unwrap();
        assert_eq!(
            market_model.close_market(&mut registry, 22, 33),
            Err(Error::AlreadyClosed)
        );
    }
    #[test]
    fn close_effect_permutations_refuse_out_of_order_roles() {
        let (mut registry, mut model) = v2();
        model.resolve(1, [1, 0, 0, 0], 1).unwrap();
        assert_eq!(model.close_supply(33), Err(Error::CloseOrder));
        assert_eq!(model.close_resolution(33), Err(Error::CloseOrder));
        assert_eq!(model.close_kernel(33), Err(Error::CloseOrder));
        assert_eq!(model.close_hoard(33), Err(Error::CloseOrder));
        assert_eq!(
            model.close_market(&mut registry, 22, 33),
            Err(Error::CloseOrder)
        );
        model.close_mint(0, 22, 33).unwrap();
        model.close_mint(1, 22, 33).unwrap();
        assert_eq!(model.close_kernel(33), Err(Error::CloseOrder));
        assert_eq!(model.close_hoard(33), Err(Error::CloseOrder));
    }
    #[test]
    fn compact_prepaid_tombstone_blocks_recreate_after_market_rent_refund() {
        let (mut r, mut m) = v2();
        empty_closeable(&mut m);
        m.close_supply(33).unwrap();
        m.close_resolution(33).unwrap();
        m.close_kernel(33).unwrap();
        m.close_hoard(33).unwrap();
        m.close_market(&mut r, 22, 33).unwrap();
        let t = r.tombstone.unwrap();
        assert_eq!(t.rent.role, Role::Replay);
        assert_eq!(t.rent.account, 63);
        assert_eq!(t.rent.lamports, 1);
        assert_ne!(t.rent, m.market.rent);
        assert_eq!(r.check_for(&m), Ok(()));
        assert_eq!(Model::new_v2(&r, 11, 22, 33, 66, 2), Err(Error::Replay));
        assert_eq!(m.close_market(&mut r, 22, 33), Err(Error::AlreadyClosed));
    }
    #[test]
    fn legacy_uncloseable_mint_is_explicit_stop() {
        let mut m = Model::legacy_for_stop(11, 33);
        assert_eq!(m.close_mint(0, 999, 33), Err(Error::LegacyStop));
    }
    #[test]
    fn corrupted_prestate_is_rejected_without_panic() {
        let (_r, mut two) = v2();
        two.issue_to_position(0, 44, 1, 55).unwrap();
        two.issue_to_position(1, 45, 1, 56).unwrap();
        two.supply.total[0] -= 1;
        assert_eq!(two.check(), Err(Error::AggregateClosure));
        let (_r, mut excess) = v2();
        excess.issue_to_position(0, 44, 1, 55).unwrap();
        excess.supply.total[0] += 1;
        assert_eq!(excess.check(), Err(Error::AggregateClosure));
        let (_r, mut mint) = v2();
        mint.issue_to_position(0, 44, 1, 55).unwrap();
        mint.mints[0].authoritative_supply += 1;
        assert_eq!(mint.check(), Err(Error::MintSupplyMismatch));

        let (_r, mut zero_authority) = v2();
        zero_authority.market.terminal_authority = 0;
        zero_authority.mints[0].close_authority = Some(MintCloseAuthority {
            tag: zero_authority.market.tag,
            terminal_authority: 0,
        });
        zero_authority.mints[1].close_authority = Some(MintCloseAuthority {
            tag: zero_authority.market.tag,
            terminal_authority: 0,
        });
        assert_eq!(zero_authority.check(), Err(Error::Shape));

        let (_r, mut phantom_position) = v2();
        phantom_position.refunded_by_role[Role::Position(3).ledger_index()] = 1;
        phantom_position.refunded_rent = 1;
        assert_eq!(phantom_position.check(), Err(Error::Identity));

        let (_r, mut phantom_mint) = v2();
        phantom_mint.refunded_by_role[Role::Mint(2).ledger_index()] = 1;
        phantom_mint.refunded_rent = 1;
        assert_eq!(phantom_mint.check(), Err(Error::Identity));

        let (_r, mut trailing_claim) = v2();
        trailing_claim.issue_to_position(0, 44, 1, 55).unwrap();
        trailing_claim.positions[0].claims[2] = 1;
        assert_eq!(trailing_claim.check(), Err(Error::Shape));

        let (_r, mut rent_role) = v2();
        rent_role.hoard.rent.role = Role::Kernel;
        assert_eq!(rent_role.check(), Err(Error::Identity));

        let (_r, mut tombstone_funding) = v2();
        tombstone_funding.replay_rent.lamports = 0;
        assert_eq!(tombstone_funding.check(), Err(Error::Identity));

        let (_r, mut closed_order) = v2();
        closed_order.resolve(1, [1, 0, 0, 0], 1).unwrap();
        closed_order.kernel.closed = true;
        assert_eq!(closed_order.check(), Err(Error::CloseOrder));

        let (_r, mut active_closed_mint) = v2();
        active_closed_mint.mints[0].closed = true;
        active_closed_mint.refunded_by_role[Role::Mint(0).ledger_index()] = 1;
        active_closed_mint.refunded_rent = 1;
        assert_eq!(active_closed_mint.check(), Err(Error::CloseOrder));

        let (_r, mut active_surplus) = v2();
        active_surplus.donate(9).unwrap();
        active_surplus.hoard.collateral = 0;
        active_surplus.surplus_disposed = 9;
        active_surplus.surplus_sink_paid = 9;
        assert_eq!(active_surplus.check(), Err(Error::CloseOrder));

        let (_r, mut zero_holder) = v2();
        zero_holder.issue_to_position(0, 44, 1, 55).unwrap();
        zero_holder.positions[0].holder = 0;
        assert_eq!(zero_holder.check(), Err(Error::CloseOrder));

        let (_r, mut active_external) = v2();
        active_external.external_bearer[0] = 1;
        active_external.supply.total[0] = 1;
        active_external.mints[0].authoritative_supply = 1;
        assert_eq!(active_external.check(), Err(Error::ExternalBearerStop));

        let (_r, mut active_issued) = v2();
        active_issued.issue_to_position(0, 44, 1, 55).unwrap();
        active_issued.positions[0].claims[0] = 2;
        active_issued.supply.total[0] = 2;
        active_issued.mints[0].authoritative_supply = 2;
        assert_eq!(active_issued.check(), Err(Error::AggregateClosure));

        let (_r, mut premature_receipt) = v2();
        premature_receipt.terminal_receipt = 9;
        assert_eq!(premature_receipt.check(), Err(Error::CloseOrder));

        let (_r, mut forged_mint_close) = v2();
        forged_mint_close.issue_to_position(0, 44, 1, 55).unwrap();
        forged_mint_close.resolve(1, [1, 0, 0, 0], 1).unwrap();
        forged_mint_close.redeem_position(0, 0, 1).unwrap();
        forged_mint_close.mints[0].closed = true;
        forged_mint_close.refunded_by_role[Role::Mint(0).ledger_index()] = 1;
        forged_mint_close.refunded_rent = 1;
        assert_eq!(forged_mint_close.check(), Err(Error::CloseOrder));

        let (_r, mut early_surplus) = v2();
        early_surplus.issue_to_position(0, 44, 2, 55).unwrap();
        early_surplus.resolve(1, [1, 0, 0, 0], 1).unwrap();
        early_surplus.burn_position(0, 0, 1).unwrap();
        early_surplus.hoard.collateral = 1;
        early_surplus.surplus_disposed = 1;
        early_surplus.surplus_sink_paid = 1;
        assert_eq!(early_surplus.check(), Err(Error::CloseOrder));
    }
    #[test]
    fn post_resolution_redistribution_cannot_hide_fractional_position_lots() {
        let (_r, mut m) = v2();
        m.issue_to_position(0, 44, 2, 55).unwrap();
        m.issue_to_position(1, 45, 2, 56).unwrap();
        m.resolve(2, [1, 1, 0, 0], 1).unwrap();
        m.positions[0].claims[0] = 1;
        m.positions[1].claims[0] = 3;
        assert_eq!(m.check(), Err(Error::FractionalRemainder));
    }
    #[test]
    fn neutral_sink_is_disjoint_from_every_created_refund_recipient() {
        let (_r, mut m) = v2();
        assert_eq!(m.issue_to_position(0, 44, 1, 66), Err(Error::Shape));
        m.issue_to_position(0, 44, 1, 55).unwrap();
        m.positions[0].rent.refund_to = 66;
        assert_eq!(m.check(), Err(Error::Identity));
    }
    #[test]
    fn malformed_outcome_never_indexes_past_bounds() {
        let (_r, mut m) = v2();
        assert_eq!(m.close_mint(99, 22, 33), Err(Error::Phase));
        m.resolve(1, [1, 0, 0, 0], 1).unwrap();
        assert_eq!(m.close_mint(99, 22, 33), Err(Error::Shape));
    }
}
