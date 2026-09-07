/-!
# The Series Expire V5 account frame

An ACCOUNT layout, in the sense `CoreFoundFrameV3Abi` established: the exact
ordered list of eighty-two logical coordinates a pre-Market Series Expire
presents to the common Hot outer, each with its privileges and, for the
thirty-seven that are route aliases, the representative coordinate it stands
for.  Five child-route windows partition the coordinates after the Ticket, and
one coordinate -- the Custody callee -- sits past every window.

## Why this frame needs a Lean owner

The width of this frame had two authors on 2026-09-06, and they disagreed.
`programs/dclutch-trading-sbf/src/series/expire_funding_artifacts_v5.rs`
declared `SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 = 82` after `272fb867d`
appended the Custody callee at 81; `programs/dclutch-trading-sbf/src/hot_v3/series_expiry.rs`
still pinned `SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1 = 81`, with a `const _` assert
tying the precommit caller to `81 - 1`.  The artifact emitted eighty-two rules,
the authenticator refused any vector that was not eighty-one, and every commit
in between compiled green.  `git log -S` shows the second author never moved:
the wall `272fb867d` measured at 530,018 CU in the first Custody preflight was
measured on a working tree, not on the committed sources.

A count that two files both spell is a count nobody owns.  Here the count is
the length of the slot list, every named coordinate is a position in it, the
route windows are contiguous ranges over it, and the alias table is read off
the slots -- so the profile emitter, the authenticator, the operator and the
fixture all read one emission.

## What the frame is

- Coordinates 0-4 are the common Hot runtime prefix (root, config, Product,
  Portfolio, linked basis); coordinate 5 is the outer writable Ticket replay.
- Routes 0-2 are three canonical `CustodyFrameSpecV1` windows -- `Transfer`
  (the escrow refund), `CloseVault`, `CloseReplay` -- whose common prefix
  (Market, activation, Registry, Trading, ProgramData, Realm raw and staging,
  replay) is presented once at 7-14 and aliased in the later windows.
- Route 3 is the Trading-owned projected-custody `AbortOpenAndClose` window.
- Route 4 is Core's twenty-five-account unallocated-permit precommit frame
  plus the release-pinned Trading caller PDA that only the CPI makes a signer.
- Coordinate 81 is the activated Custody program.  A CPI's callee is not a
  member of its own frame and `CustodyFrameRoleV1` has no `CustodyProgram`
  role, so the executor resolves the callee by scanning the logical vector for
  the key the activation cache names; it must therefore be a coordinate, and
  appending it past every window renumbers nothing.
- Coordinate 7 is the VACANT future occurrence Market -- every Custody window's
  `CoreMarket` and the projected window's last slot (54).  It is not the fixed
  Hot Market, which is the live Series controller.
-/

namespace DClutch.SeriesExpireFrameV5Abi

/-- What one coordinate is: its own account, or an alias of an earlier one. -/
inductive Binding where
  /-- The coordinate presents its own physical account. -/
  | own
  /-- A route alias: the coordinate stands for the representative at `rep`. -/
  | alias (rep : Nat)
  deriving DecidableEq, Repr

/-- One logical coordinate of the frame. -/
structure Slot where
  /-- The role the coordinate plays, in the words the routes use for it. -/
  role : String
  writable : Bool
  executable : Bool
  binding : Binding
  deriving DecidableEq, Repr

private def ro (role : String) : Slot :=
  { role, writable := false, executable := false, binding := .own }
private def rw (role : String) : Slot :=
  { role, writable := true, executable := false, binding := .own }
private def ex (role : String) : Slot :=
  { role, writable := false, executable := true, binding := .own }
private def al (role : String) (rep : Nat) : Slot :=
  { role, writable := false, executable := false, binding := .alias rep }

/-- The common Hot runtime prefix and the outer Ticket: coordinates 0-5. -/
def prefixSlots : List Slot := [
  rw "series root",                       -- 0
  ro "Template record (config raw)",      -- 1
  ro "Product record",                    -- 2
  ro "Portfolio record",                  -- 3
  ro "linked basis record",               -- 4
  rw "Ticket replay state"                -- 5
]

/-- Route 0, `CustodyFrameSpecV1::Transfer`: the escrow refund, 6-19. -/
def refundWindow : List Slot := [
  ro "refund caller authority",           -- 6
  ro "vacant future Market",              -- 7
  ro "activation cache",                  -- 8
  ex "Registry program",                  -- 9
  ex "Trading program",                   -- 10
  ro "Trading ProgramData",               -- 11
  ro "Realm record",                      -- 12
  ro "Realm staging",                     -- 13
  rw "Custody replay",                    -- 14
  ro "collateral Mint",                   -- 15
  rw "escrow vault (transfer source)",    -- 16
  rw "refund destination",                -- 17
  ro "Custody authority",                 -- 18
  ex "token program"                      -- 19
]

/-- Route 1, `CustodyFrameSpecV1::CloseVault`: 20-33. -/
def closeVaultWindow : List Slot := [
  ro "close-vault caller authority",      -- 20
  al "vacant future Market" 7,            -- 21
  al "activation cache" 8,                -- 22
  al "Registry program" 9,                -- 23
  al "Trading program" 10,                -- 24
  al "Trading ProgramData" 11,            -- 25
  al "Realm record" 12,                   -- 26
  al "Realm staging" 13,                  -- 27
  al "Custody replay" 14,                 -- 28
  al "collateral Mint" 15,                -- 29
  al "escrow vault" 16,                   -- 30
  al "Custody authority" 18,              -- 31
  al "token program" 19,                  -- 32
  rw "lifecycle RentCredit (rent refund)" -- 33
]

/-- Route 2, `CustodyFrameSpecV1::CloseReplay`: 34-43. -/
def closeReplayWindow : List Slot := [
  ro "close-replay caller authority",     -- 34
  al "vacant future Market" 7,            -- 35
  al "activation cache" 8,                -- 36
  al "Registry program" 9,                -- 37
  al "Trading program" 10,                -- 38
  al "Trading ProgramData" 11,            -- 39
  al "Realm record" 12,                   -- 40
  al "Realm staging" 13,                  -- 41
  al "Custody replay" 14,                 -- 42
  al "lifecycle RentCredit" 33            -- 43
]

/-- Route 3, projected-custody `AbortOpenAndClose`: 44-54. -/
def projectedAbortWindow : List Slot := [
  ro "projected caller authority",        -- 44
  rw "projected custody state",           -- 45
  al "activation cache" 8,                -- 46
  al "Registry program" 9,                -- 47
  al "Trading program" 10,                -- 48
  al "Trading ProgramData" 11,            -- 49
  al "lifecycle RentCredit" 33,           -- 50
  rw "empty Hoard vault",                 -- 51
  al "Custody authority" 18,              -- 52
  al "token program" 19,                  -- 53
  al "vacant future Market" 7             -- 54
]

/-- Route 4, Core `series_permit_expiry_precommit_v1`: the twenty-five-account
permissionless expiry frame plus the caller, 55-80. -/
def corePrecommitWindow : List Slot := [
  rw "unallocated permit",                -- 55
  al "lifecycle RentCredit" 33,           -- 56
  ex "Rent program",                      -- 57
  ro "infrastructure profile",            -- 58
  ro "Registry artifact raw",             -- 59
  ro "Registry artifact staging",         -- 60
  al "Registry program" 9,                -- 61
  ro "Registry ProgramData",              -- 62
  ro "Rent artifact raw",                 -- 63
  ro "Rent artifact staging",             -- 64
  ro "Rent ProgramData",                  -- 65
  al "activation cache" 8,                -- 66
  al "Trading program" 10,                -- 67
  al "Trading ProgramData" 11,            -- 68
  al "series root" 0,                     -- 69
  al "Ticket replay state" 5,             -- 70
  al "Template record" 1,                 -- 71
  ro "Template staging",                  -- 72
  ro "occurrence record",                 -- 73
  ro "occurrence staging",                -- 74
  ro "Ticket record",                     -- 75
  ro "Ticket staging",                    -- 76
  ro "Clock sysvar",                      -- 77
  ro "Rent sysvar",                       -- 78
  ex "System program",                    -- 79
  ro "precommit caller PDA"               -- 80
]

/-- The activated Custody program, past every window. -/
def custodyCallee : List Slot := [ ex "Custody program" ]  -- 81

def frame : List Slot :=
  prefixSlots ++ refundWindow ++ closeVaultWindow ++ closeReplayWindow ++
    projectedAbortWindow ++ corePrecommitWindow ++ custodyCallee

def fixedAccountCount : Nat := frame.length

/-- The five child-route windows, as `(start, count)`. -/
def routeWindows : List (Nat × Nat) := [
  (prefixSlots.length, refundWindow.length),
  (prefixSlots.length + refundWindow.length, closeVaultWindow.length),
  (prefixSlots.length + refundWindow.length + closeVaultWindow.length,
    closeReplayWindow.length),
  (prefixSlots.length + refundWindow.length + closeVaultWindow.length +
    closeReplayWindow.length, projectedAbortWindow.length),
  (prefixSlots.length + refundWindow.length + closeVaultWindow.length +
    closeReplayWindow.length + projectedAbortWindow.length, corePrecommitWindow.length)
]

def routeStarts : List Nat := routeWindows.map (·.1)
def routeCounts : List Nat := routeWindows.map (·.2)

/-- Position of the first slot whose role is `role`. -/
def indexOf? (role : String) : Option Nat :=
  frame.findIdx? (fun slot => slot.role == role)

def ticketCoordinate : Nat := (indexOf? "Ticket replay state").getD 0
def futureMarketCoordinate : Nat := (indexOf? "vacant future Market").getD 0
def rentCreditCoordinate : Nat :=
  (indexOf? "lifecycle RentCredit (rent refund)").getD 0
def projectedFutureMarketCoordinate : Nat :=
  (routeWindows.getD 3 (0, 0)).1 + (routeWindows.getD 3 (0, 0)).2 - 1
def coreRouteStart : Nat := (routeWindows.getD 4 (0, 0)).1
def coreRouteCount : Nat := (routeWindows.getD 4 (0, 0)).2
def permitCoordinate : Nat := (indexOf? "unallocated permit").getD 0
def coreRentCreditCoordinate : Nat := coreRouteStart + 1
def rentProgramCoordinate : Nat := (indexOf? "Rent program").getD 0
def rootReplayCoordinate : Nat := coreRouteStart + 14
def ticketReplayCoordinate : Nat := coreRouteStart + 15
def templateRawCoordinate : Nat := coreRouteStart + 16
def templateStagingCoordinate : Nat := (indexOf? "Template staging").getD 0
def occurrenceRawCoordinate : Nat := (indexOf? "occurrence record").getD 0
def occurrenceStagingCoordinate : Nat := (indexOf? "occurrence staging").getD 0
def ticketRawCoordinate : Nat := (indexOf? "Ticket record").getD 0
def ticketStagingCoordinate : Nat := (indexOf? "Ticket staging").getD 0
def clockCoordinate : Nat := (indexOf? "Clock sysvar").getD 0
def rentSysvarCoordinate : Nat := (indexOf? "Rent sysvar").getD 0
def systemProgramCoordinate : Nat := (indexOf? "System program").getD 0
def precommitCallerCoordinate : Nat := (indexOf? "precommit caller PDA").getD 0
def custodyProgramCoordinate : Nat := (indexOf? "Custody program").getD 0

/-- The slots paired with their coordinates, from `start`. -/
def indexedFrom : Nat → List Slot → List (Nat × Slot)
  | _, [] => []
  | start, slot :: rest => (start, slot) :: indexedFrom (start + 1) rest

def indexed : List (Nat × Slot) := indexedFrom 0 frame

/-- Every `(alias, representative)` pair, in coordinate order. -/
def routeAliases : List (Nat × Nat) :=
  indexed.filterMap fun (index, slot) =>
    match slot.binding with
    | .alias rep => some (index, rep)
    | .own => none

/-- Coordinates that present their own account with the writable bit. -/
def writableRepresentatives : List Nat :=
  indexed.filterMap fun (index, slot) =>
    if slot.binding == .own && slot.writable then some index else none

/-- Coordinates that present their own account with the executable bit. -/
def executableRepresentatives : List Nat :=
  indexed.filterMap fun (index, slot) =>
    if slot.binding == .own && slot.executable then some index else none

/-! ## What the frame says -/

/-- The count is the list's length, and it is eighty-two: the eighty-one
coordinates the routes name plus the Custody callee. -/
theorem the_count_is_the_lists_length : fixedAccountCount = 82 := by native_decide

/-- The five windows are contiguous, start right after the Ticket, and end
exactly where the callee begins -- so the callee is past every route range and
is the last coordinate.  Either half alone admits a renumbering; together they
are the property `272fb867d` pinned with two `const _` asserts. -/
theorem the_windows_tile_the_routes_and_the_callee_is_last :
    routeStarts = [6, 20, 34, 44, 55] ∧ routeCounts = [14, 14, 10, 11, 26] ∧
    (routeStarts.getD 0 0) = ticketCoordinate + 1 ∧
    (routeStarts.getD 4 0) + (routeCounts.getD 4 0) = custodyProgramCoordinate ∧
    custodyProgramCoordinate + 1 = fixedAccountCount := by
  native_decide

/-- Every named coordinate the Rust used to spell as a literal. -/
theorem named_coordinates_are_positions :
    ticketCoordinate = 5 ∧ futureMarketCoordinate = 7 ∧ rentCreditCoordinate = 33 ∧
    projectedFutureMarketCoordinate = 54 ∧ coreRouteStart = 55 ∧ coreRouteCount = 26 ∧
    permitCoordinate = 55 ∧ coreRentCreditCoordinate = 56 ∧ rentProgramCoordinate = 57 ∧
    rootReplayCoordinate = 69 ∧ ticketReplayCoordinate = 70 ∧ templateRawCoordinate = 71 ∧
    templateStagingCoordinate = 72 ∧ occurrenceRawCoordinate = 73 ∧
    occurrenceStagingCoordinate = 74 ∧ ticketRawCoordinate = 75 ∧
    ticketStagingCoordinate = 76 ∧ clockCoordinate = 77 ∧ rentSysvarCoordinate = 78 ∧
    systemProgramCoordinate = 79 ∧ precommitCallerCoordinate = 80 ∧
    custodyProgramCoordinate = 81 := by
  native_decide

/-- The Core window is the permissionless twenty-five plus one caller, and the
three coordinates inside it that Core reads as the root, the Ticket replay and
the Template are aliases of the outer root, the outer Ticket and the config
record -- which is what makes Core's readonly view of them the same account
Trading writes commit-last. -/
theorem the_core_window_is_twenty_five_plus_the_caller :
    coreRouteCount = 25 + 1 ∧ precommitCallerCoordinate = coreRouteStart + 25 ∧
    (frame.getD rootReplayCoordinate (ro "")).binding = .alias 0 ∧
    (frame.getD ticketReplayCoordinate (ro "")).binding = .alias 5 ∧
    (frame.getD templateRawCoordinate (ro "")).binding = .alias 1 := by
  native_decide

/-- Every Custody window presents the VACANT future Market at its `CoreMarket`
slot (local 1), and it is one coordinate, 7: the refund window owns it, the
two closes and the projected abort alias it.  The fixed Hot Market -- the
live Series controller -- is not in this frame at all. -/
theorem every_custody_window_sees_the_one_future_market :
    (frame.getD 7 (ro "")).role = "vacant future Market" ∧
    (frame.getD 7 (ro "")).binding = .own ∧
    (frame.getD 21 (ro "")).binding = .alias 7 ∧
    (frame.getD 35 (ro "")).binding = .alias 7 ∧
    (frame.getD 54 (ro "")).binding = .alias 7 := by
  native_decide

/-- The alias table, exactly as `expire_funding_artifacts_v5.rs` carried it
by hand: thirty-seven pairs, every one strictly backward, and no alias points
at another alias. -/
theorem the_aliases_are_backward_and_point_at_representatives :
    routeAliases.length = 37 ∧
    routeAliases.all (fun (index, rep) => rep < index) = true ∧
    routeAliases.all (fun (_, rep) =>
      (frame.getD rep (ro "")).binding == .own) = true := by
  native_decide

/-- Exactly the coordinates the profile emitter marks writable and executable.
The callee is a readonly executable, and the only executable past the routes. -/
theorem the_privileged_representatives_are_exactly_these :
    writableRepresentatives = [0, 5, 14, 16, 17, 33, 45, 51, 55] ∧
    executableRepresentatives = [9, 10, 19, 57, 79, 81] := by
  native_decide

/-- Exactly one coordinate carries the Custody callee: the executor refuses as
hard on two distinct physical carriers as on none. -/
theorem the_callee_has_one_coordinate :
    (frame.filter (fun slot => slot.role == "Custody program")).length = 1 ∧
    (frame.getD custodyProgramCoordinate (ro "")).executable = true ∧
    (frame.getD custodyProgramCoordinate (ro "")).writable = false := by
  native_decide

/-- An alias carries no privilege of its own: its representative's privileges
are the ones the runtime presents. -/
theorem aliases_carry_no_privileges :
    frame.all (fun slot =>
      match slot.binding with
      | .alias _ => !slot.writable && !slot.executable
      | .own => true) = true := by
  native_decide

end DClutch.SeriesExpireFrameV5Abi
