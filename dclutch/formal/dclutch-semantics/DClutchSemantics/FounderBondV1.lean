import DClutchSemantics.EconomicKernel
import Std.Tactic

/-!
# FounderBondV1: the founder's stake against their own oracle

A founder chooses the data source, the window and whether there is a recovery
ladder.  Decision 0025 stops an outage from PAYING them for that choice; this
module makes the choice COST them.  At founding the founder posts a bond in
lamports, sized to the market's own terminal cost, and the terminal certificate
decides which of two exits it leaves by:

* **honest** -- the certificate names an ordinary winner; the bond returns to
  the founder in full;
* **exhausted** -- the certificate names the failure selector; the bond is
  walked to the ordinary claims, pro rata, in the same redemptions that pay the
  escrow refund of `EconomicKernel`.

The bond is lamports and the escrow refund is collateral atoms, so the two
walks share a redemption and not a unit.  That is why the pro-rata rule here
is not the escrow's.  The escrow made its remainder impossible by choosing the
scale at founding -- one ordinary claim redeems for exactly one atom.  A
lamport bond cannot: on cohort-15's numbers the bond is 4,031,465 lamports
against 1,500,000,000 ordinary claims, so a per-claim constant is zero and the
whole bond would be remainder.  The rule below makes the remainder impossible
a different way.  Every redemption draws `remaining * quantity / outstanding`
-- the bond still standing, times the share of the claims still outstanding
that this redemption retires -- and the redemption that retires the last
ordinary claim draws everything left.  Conservation is then a telescoping sum
over ANY partition of the claims among holders, in ANY order, with no
divisibility hypothesis and nothing left over
(`an_exhausting_walk_pays_the_bond_exactly`).  The one named rounding boundary
is the floor in `draw`, and `draw_within_one_lamport_of_the_exact_share` bounds
it at one lamport per redemption.

## What is modeled and what is not

The size rule, the two exits, the pro-rata walk and the founding admission are
modeled.  The widths the size rule reads are PARAMETERS: the Rust owns them
(`crates/dclutch-claims-svm`, `openerTerms.ts` pins them by a source gate) and
this module refuses to be a second author.  The cohort witnesses instantiate
them and say so.  Certificate immutability, account authentication, the
refund wallet's authority, CPI and transaction atomicity are adapter
obligations, exactly as in `EconomicKernel`.
-/

namespace DClutch.FounderBond

open DClutch.Economic (Phase failureSelector)

/-! ## The size rule

The bond is the founding's own projection of what the terminal costs, and
every term is rent arithmetic over widths the founding already holds.  Nothing
here is a typed lamport figure. -/

/-- Rent-exempt minimum for `bytes` of account data at `rate` lamports per
byte-year under `exemption_threshold = 1`: the 128-byte account overhead plus
the data, times the rate.  This is `rent_exempt_reference_v1`'s shape. -/
def rentFor (rate bytes : Nat) : Nat := (128 + bytes) * rate

/-- The account widths the size rule reads.  Parameters, never constants. -/
structure Widths where
  /-- The certificate seat the settle allocates (`settle_certificate_bytes`). -/
  certificateSeat : Nat
  /-- `CLAIM_CHECK_ESCROW_BYTES_V1`, the escrow a compaction opener creates. -/
  claimCheckEscrow : Nat
  /-- A Token-2022 account, the vault the opener funds beside the escrow. -/
  tokenAccount : Nat
  /-- The admission record the first crank sweeps. -/
  admission : Nat
  /-- `CLAIM_CHECK_BYTES_V1`, the record the first crank mints. -/
  claimCheck : Nat
  /-- Position header bytes; a Position is header plus `perOutcome` per outcome. -/
  positionHeader : Nat
  positionPerOutcome : Nat
  deriving Repr

/-- The seat prepay: the certificate seat's rent, a founding-time caller
obligation the terminal consumes and nothing reimburses. -/
def seatPrepay (widths : Widths) (rate : Nat) : Nat :=
  rentFor rate widths.certificateSeat

/-- What a compaction opener advances: the escrow record and its vault. -/
def openerAdvance (widths : Widths) (rate : Nat) : Nat :=
  rentFor rate widths.claimCheckEscrow + rentFor rate widths.tokenAccount

/-- What the first crank sweeps: the Position and the admission record. -/
def firstCrankSwept (widths : Widths) (rate outcomes : Nat) : Nat :=
  rentFor rate (widths.positionHeader + widths.positionPerOutcome * outcomes)
    + rentFor rate widths.admission

/-- What the first crank repays the opener, in the crank-first order decision
0024 item 3 keeps: the sweep, less the claim check's own rent, less the
cranker's capped reward.  Truncated subtraction is the kernel's `min`. -/
def firstCrankRepayment (widths : Widths) (rate outcomes crankCap : Nat) : Nat :=
  firstCrankSwept widths rate outcomes - rentFor rate widths.claimCheck - crankCap

/-- The opener's shortfall on a single-crank market: the figure the economics
note measures at 1,244,945 lamports on the cohorts' rate. -/
def firstCrankShortfall (widths : Widths) (rate outcomes crankCap : Nat) : Nat :=
  openerAdvance widths rate - firstCrankRepayment widths rate outcomes crankCap

/-- The ladder's funding: each rung's Bounty quote, read off the capability
manifest the founding finalizes.  A market with no policy has none. -/
def ladderFunding (rungBounties : List Nat) : Nat := rungBounties.sum

/-- **The size rule.**  The bond is the seat prepay plus the first crank's
shortfall plus the ladder's funding: the cost of the terminal the founder's
oracle, if it goes quiet, makes the holders walk. -/
def bondSize (widths : Widths) (rate outcomes crankCap : Nat) (rungBounties : List Nat) : Nat :=
  seatPrepay widths rate + firstCrankShortfall widths rate outcomes crankCap
    + ladderFunding rungBounties

theorem bond_size_is_at_least_the_seat_prepay
    (widths : Widths) (rate outcomes crankCap : Nat) (rungBounties : List Nat) :
    seatPrepay widths rate ≤ bondSize widths rate outcomes crankCap rungBounties := by
  unfold bondSize; omega

theorem a_rung_never_lowers_the_bond
    (widths : Widths) (rate outcomes crankCap bounty : Nat) (rungBounties : List Nat) :
    bondSize widths rate outcomes crankCap rungBounties
      ≤ bondSize widths rate outcomes crankCap (bounty :: rungBounties) := by
  simp only [bondSize, ladderFunding, List.sum_cons]; omega

/-! ### The cohort witnesses

The widths are the ones `openerTerms.ts` pins against the Rust and the seat is
`settle_certificate_bytes = 312`; the cap is `COMPACTION_CRANK_REWARD_LAMPORTS_V1`.
Cohort-15's market has four outcomes (three ordinary and the failure
selector), no recovery policy, and was founded at 6,333 lamports a byte.  The
other two rates are the ones the economics note prices everything at: 5,080,
where devnet moved at epoch 1141, and 6,960, the kernel's reference. -/

def cohortWidths : Widths :=
  { certificateSeat := 312, claimCheckEscrow := 256, tokenAccount := 165,
    admission := 512, claimCheck := 288, positionHeader := 128, positionPerOutcome := 8 }

def compactionCrankRewardCap : Nat := 200000

example : seatPrepay cohortWidths 6333 = 2786520 := by decide
example : openerAdvance cohortWidths 6333 = 4287441 := by decide
example : firstCrankRepayment cohortWidths 6333 4 compactionCrankRewardCap = 3042496 := by decide
example : firstCrankShortfall cohortWidths 6333 4 compactionCrankRewardCap = 1244945 := by decide

/-- The bond on cohort-15's numbers: 0.004031465 SOL. -/
theorem cohort_fifteen_bond :
    bondSize cohortWidths 6333 4 compactionCrankRewardCap [] = 4031465 := by decide

example : bondSize cohortWidths 5080 4 compactionCrankRewardCap [] = 3273400 := by decide
example : bondSize cohortWidths 6960 4 compactionCrankRewardCap [] = 4410800 := by decide

/-! ## The founding admission

The bond is the lamports the failure escrow holds above its own rent.  A
founding that leaves the escrow holding less than rent plus the bond refuses;
what the walk later observes is whatever stands above rent, so a lamport
somebody donates enlarges the holders' draw rather than stranding. -/

/-- The founding conjunct: the escrow account holds its rent and the bond. -/
def founded (lamports rent bond : Nat) : Bool := decide (rent + bond ≤ lamports)

/-- What every later route reads as the bond: the account's lamports above its
rent minimum.  An observation, never a caller's number. -/
def observedBond (lamports rent : Nat) : Nat := lamports - rent

theorem an_admitted_founding_holds_at_least_the_bond
    (lamports rent bond : Nat) (admitted : founded lamports rent bond = true) :
    bond ≤ observedBond lamports rent := by
  unfold founded at admitted
  unfold observedBond
  have := of_decide_eq_true admitted
  omega

/-- **Hostile: a founding without the bond.**  One lamport short refuses. -/
theorem a_founding_one_lamport_short_refuses (rent bond : Nat) (positive : 0 < bond) :
    founded (rent + bond - 1) rent bond = false := by
  unfold founded
  apply decide_eq_false
  omega

example : founded (rentFor 6333 160 + 4031465) (rentFor 6333 160) 4031465 = true := by decide
example : founded (rentFor 6333 160 + 4031464) (rentFor 6333 160) 4031465 = false := by decide

/-! ## The two exits

The certificate is written once, and its kind decides the exit.  A market that
is open has no exit at all: that is property (d), the bond cannot be withdrawn
while the market is live.  A retired market has none either, because by then
the bond has already left by one of the two. -/

inductive Exit where
  | honest
  | exhausted
  deriving DecidableEq, Repr

/-- The exit a phase enables.  Exactly the terminal phases enable one, and
which one is a function of the winner alone. -/
def exit? (ordinaryCount : Nat) : Phase → Option Exit
  | .open => none
  | .terminal winner =>
      some (if winner = failureSelector ordinaryCount then .exhausted else .honest)
  | .retiring winner =>
      some (if winner = failureSelector ordinaryCount then .exhausted else .honest)
  | .retired => none

/-- Where the bond goes under an exit, in the aggregate. -/
structure Settlement where
  toFounder : Nat
  toHolders : Nat
  deriving DecidableEq, Repr

def settle : Exit → Nat → Settlement
  | .honest, bond => { toFounder := bond, toHolders := 0 }
  | .exhausted, bond => { toFounder := 0, toHolders := bond }

/-- **Property (a), conservation.**  Whatever the exit, the whole bond leaves
and nothing else does. -/
theorem the_bond_leaves_by_exactly_one_exit (exit : Exit) (bond : Nat) :
    (settle exit bond).toFounder + (settle exit bond).toHolders = bond := by
  cases exit <;> simp [settle]

/-- Never both: one side of every settlement is zero. -/
theorem never_both_exits (exit : Exit) (bond : Nat) :
    (settle exit bond).toFounder = 0 ∨ (settle exit bond).toHolders = 0 := by
  cases exit <;> simp [settle]

/-- **Hostile: a bond paid on an honest resolution.**  The honest exit pays the
holders nothing. -/
theorem an_honest_exit_pays_the_holders_nothing (bond : Nat) :
    (settle .honest bond).toHolders = 0 := rfl

/-- **Property (c), no rent extraction.**  The exhausted exit pays the founder
nothing -- their share as a holder of ordinary claims is the walk's, below. -/
theorem an_exhausted_exit_pays_the_founder_nothing (bond : Nat) :
    (settle .exhausted bond).toFounder = 0 := rfl

/-- **Property (d).**  An open market enables no exit. -/
theorem no_exit_while_the_market_is_open (ordinaryCount : Nat) :
    exit? ordinaryCount .open = none := rfl

theorem no_exit_once_retired (ordinaryCount : Nat) :
    exit? ordinaryCount .retired = none := rfl

/-- Every terminal enables exactly one exit. -/
theorem a_terminal_enables_exactly_one_exit (ordinaryCount winner : Nat) :
    ∃ exit, exit? ordinaryCount (.terminal winner) = some exit := by
  simp [exit?]

/-- The failure selector is the only winner that exhausts the bond. -/
theorem only_the_failure_selector_exhausts_the_bond (ordinaryCount winner : Nat) :
    exit? ordinaryCount (.terminal winner) = some .exhausted
      ↔ winner = failureSelector ordinaryCount := by
  unfold exit?
  by_cases h : winner = failureSelector ordinaryCount <;> simp [h]

/-- Every other winner returns it. -/
theorem an_ordinary_winner_returns_the_bond
    (ordinaryCount winner : Nat) (honest : winner ≠ failureSelector ordinaryCount) :
    exit? ordinaryCount (.terminal winner) = some .honest := by
  simp [exit?, honest]

/-- The exit does not change between terminal and retiring: the certificate is
read, not re-decided. -/
theorem retiring_keeps_the_terminal_exit (ordinaryCount winner : Nat) :
    exit? ordinaryCount (.retiring winner) = exit? ordinaryCount (.terminal winner) := rfl

/-! ## The pro-rata walk

The exhausted exit is realized one redemption at a time, in the same
redemptions that pay the escrow refund.  `remaining` is the bond still standing
on the escrow account (observed, above rent) and `outstanding` the ordinary
claims still outstanding on the aggregate (observed, the sum of the ordinary
coordinates of the supply vector).  Neither is a caller's number. -/

/-- One redemption's draw: the bond still standing, times the share of the
claims still outstanding that this redemption retires, floored.  The floor is
the one rounding boundary. -/
def draw (remaining outstanding quantity : Nat) : Nat :=
  remaining * quantity / outstanding

theorem draw_of_nothing (remaining outstanding : Nat) :
    draw remaining outstanding 0 = 0 := by
  simp [draw]

/-- **Hostile: a payout exceeding the bond.**  No redemption of claims that
are actually outstanding draws more than what remains. -/
theorem no_draw_exceeds_what_remains
    (remaining outstanding quantity : Nat) (held : quantity ≤ outstanding) :
    draw remaining outstanding quantity ≤ remaining := by
  unfold draw
  apply Nat.div_le_of_le_mul
  calc remaining * quantity ≤ remaining * outstanding := Nat.mul_le_mul_left remaining held
    _ = outstanding * remaining := Nat.mul_comm _ _

/-- Retiring every outstanding claim draws everything that remains. -/
theorem the_last_redemption_draws_everything
    (remaining outstanding : Nat) (positive : 0 < outstanding) :
    draw remaining outstanding outstanding = remaining := by
  unfold draw
  exact Nat.mul_div_cancel remaining positive

/-- The draw never exceeds the exact pro-rata share of what remains. -/
theorem no_draw_exceeds_the_exact_share (remaining outstanding quantity : Nat) :
    draw remaining outstanding quantity * outstanding ≤ remaining * quantity := by
  unfold draw
  exact Nat.div_mul_le_self _ _

/-- And it falls short of that share by less than one lamport: the rounding
boundary, bounded. -/
theorem draw_within_one_lamport_of_the_exact_share
    (remaining outstanding quantity : Nat) (positive : 0 < outstanding) :
    remaining * quantity < (draw remaining outstanding quantity + 1) * outstanding := by
  unfold draw
  have split := Nat.div_add_mod (remaining * quantity) outstanding
  have bound := Nat.mod_lt (remaining * quantity) positive
  rw [Nat.add_mul, Nat.one_mul, Nat.mul_comm (remaining * quantity / outstanding) outstanding]
  omega

/-- A larger holding never draws less. -/
theorem a_larger_holding_never_draws_less
    (remaining outstanding smaller larger : Nat) (le : smaller ≤ larger) :
    draw remaining outstanding smaller ≤ draw remaining outstanding larger := by
  unfold draw
  exact Nat.div_le_div_right (Nat.mul_le_mul_left remaining le)

/-- The bond and the claims still standing. -/
structure Walk where
  remaining : Nat
  outstanding : Nat
  deriving DecidableEq, Repr

/-- One redemption of `quantity` ordinary claims. -/
def Walk.step (walk : Walk) (quantity : Nat) : Walk :=
  { remaining := walk.remaining - draw walk.remaining walk.outstanding quantity
    outstanding := walk.outstanding - quantity }

def Walk.run (walk : Walk) : List Nat → Walk
  | [] => walk
  | quantity :: rest => (walk.step quantity).run rest

/-- Every redemption's draw, in order. -/
def Walk.draws (walk : Walk) : List Nat → List Nat
  | [] => []
  | quantity :: rest =>
      draw walk.remaining walk.outstanding quantity :: (walk.step quantity).draws rest

/-- What the walk paid in total. -/
def Walk.paid (walk : Walk) : List Nat → Nat
  | [] => 0
  | quantity :: rest =>
      draw walk.remaining walk.outstanding quantity + (walk.step quantity).paid rest

theorem Walk.paid_eq_sum_draws (walk : Walk) (redemptions : List Nat) :
    walk.paid redemptions = (walk.draws redemptions).sum := by
  induction redemptions generalizing walk with
  | nil => rfl
  | cons quantity rest ih => simp [Walk.paid, Walk.draws, ih]

/-- A redemption sequence is feasible when no step retires more claims than
are outstanding at that step -- which the aggregate enforces, since a Position
cannot hold more than the supply (`PositionExceedsSupply`). -/
def Walk.feasible (walk : Walk) : List Nat → Prop
  | [] => True
  | quantity :: rest => quantity ≤ walk.outstanding ∧ (walk.step quantity).feasible rest

/-- Any sequence whose total does not exceed the outstanding claims is feasible. -/
theorem Walk.feasible_of_sum_le
    (walk : Walk) (redemptions : List Nat) (le : redemptions.sum ≤ walk.outstanding) :
    walk.feasible redemptions := by
  induction redemptions generalizing walk with
  | nil => trivial
  | cons quantity rest ih =>
      simp only [List.sum_cons] at le
      refine ⟨by omega, ih (walk.step quantity) ?_⟩
      simp only [Walk.step]; omega

theorem Walk.run_outstanding (walk : Walk) (redemptions : List Nat) :
    (walk.run redemptions).outstanding = walk.outstanding - redemptions.sum := by
  induction redemptions generalizing walk with
  | nil => simp [Walk.run]
  | cons quantity rest ih =>
      simp only [Walk.run, List.sum_cons]
      rw [ih]
      simp only [Walk.step]
      omega

/-- The telescoping identity: what was paid plus what remains is what stood. -/
theorem Walk.paid_add_remaining
    (walk : Walk) (redemptions : List Nat) (feasible : walk.feasible redemptions) :
    walk.paid redemptions + (walk.run redemptions).remaining = walk.remaining := by
  induction redemptions generalizing walk with
  | nil => simp [Walk.paid, Walk.run]
  | cons quantity rest ih =>
      obtain ⟨held, rest_feasible⟩ := feasible
      simp only [Walk.paid, Walk.run]
      have inner := ih (walk.step quantity) rest_feasible
      have bounded := no_draw_exceeds_what_remains walk.remaining walk.outstanding quantity held
      have stepped : (walk.step quantity).remaining
          = walk.remaining - draw walk.remaining walk.outstanding quantity := rfl
      rw [stepped] at inner
      omega

/-- A walk is sound when running out of claims means running out of bond. -/
def Walk.sound (walk : Walk) : Prop := walk.outstanding = 0 → walk.remaining = 0

theorem Walk.step_preserves_sound
    (walk : Walk) (quantity : Nat) (held : quantity ≤ walk.outstanding)
    (sound : walk.sound) : (walk.step quantity).sound := by
  intro exhausted
  simp only [Walk.step] at exhausted ⊢
  have all : quantity = walk.outstanding := by omega
  by_cases zero : walk.outstanding = 0
  · have := sound zero
    rw [all, zero, draw_of_nothing]
    omega
  · rw [all, the_last_redemption_draws_everything walk.remaining walk.outstanding
      (Nat.pos_of_ne_zero zero)]
    omega

theorem Walk.run_sound
    (walk : Walk) (redemptions : List Nat) (feasible : walk.feasible redemptions)
    (sound : walk.sound) : (walk.run redemptions).sound := by
  induction redemptions generalizing walk with
  | nil => exact sound
  | cons quantity rest ih =>
      obtain ⟨held, rest_feasible⟩ := feasible
      exact ih (walk.step quantity) rest_feasible (walk.step_preserves_sound quantity held sound)

/-- **Property (a), realized.**  When the ordinary claims outstanding are
redeemed in ANY partition among holders, in ANY order, the walk pays out the
bond to the last lamport and leaves nothing standing.  No divisibility
hypothesis: the remainder is impossible, not housed. -/
theorem an_exhausting_walk_pays_the_bond_exactly
    (walk : Walk) (redemptions : List Nat)
    (positive : 0 < walk.outstanding)
    (partition : redemptions.sum = walk.outstanding) :
    walk.paid redemptions = walk.remaining ∧ (walk.run redemptions).remaining = 0 := by
  have feasible := walk.feasible_of_sum_le redemptions (by omega)
  have sound : walk.sound := fun zero => by omega
  have final_sound := walk.run_sound redemptions feasible sound
  have final_outstanding := walk.run_outstanding redemptions
  rw [partition, Nat.sub_self] at final_outstanding
  have final_remaining := final_sound final_outstanding
  have telescoped := walk.paid_add_remaining redemptions feasible
  exact ⟨by omega, final_remaining⟩

/-- Property (a) tied to the settlement: the exhausted exit's aggregate is
exactly what the walk pays. -/
theorem the_exhausted_exit_is_the_walk
    (walk : Walk) (redemptions : List Nat)
    (positive : 0 < walk.outstanding)
    (partition : redemptions.sum = walk.outstanding) :
    walk.paid redemptions = (settle .exhausted walk.remaining).toHolders := by
  exact (an_exhausting_walk_pays_the_bond_exactly walk redemptions positive partition).left

/-- Nothing is ever overdrawn along a feasible walk. -/
theorem Walk.paid_le_remaining
    (walk : Walk) (redemptions : List Nat) (feasible : walk.feasible redemptions) :
    walk.paid redemptions ≤ walk.remaining := by
  have := walk.paid_add_remaining redemptions feasible
  omega

/-! ### Which redemptions draw

Only ordinary claims draw, and only under the exhausted exit.  The failure
coordinate's own redemption -- the escrow's, paid nothing in atoms by
`the_escrow_pays_nobody_for_the_failure_coordinate` -- draws nothing here
either. -/

/-- The ordinary quantity a redemption at `index` retires: its quantity below
the failure selector and nothing at it. -/
def ordinaryQuantity (ordinaryCount index quantity : Nat) : Nat :=
  if index < ordinaryCount then quantity else 0

/-- What one redemption draws from the bond, given the phase it runs in. -/
def redemptionDraw
    (ordinaryCount : Nat) (phase : Phase)
    (index quantity remaining outstanding : Nat) : Nat :=
  match exit? ordinaryCount phase with
  | some .exhausted =>
      draw remaining outstanding (ordinaryQuantity ordinaryCount index quantity)
  | _ => 0

/-- **Hostile: a bond paid on an honest resolution**, per redemption. -/
theorem no_redemption_draws_the_bond_on_an_honest_terminal
    (ordinaryCount winner index quantity remaining outstanding : Nat)
    (honest : winner ≠ failureSelector ordinaryCount) :
    redemptionDraw ordinaryCount (.terminal winner) index quantity remaining outstanding = 0 := by
  simp [redemptionDraw, an_ordinary_winner_returns_the_bond ordinaryCount winner honest]

/-- **Hostile: a withdrawal mid-life.**  No redemption draws while open. -/
theorem no_redemption_draws_the_bond_while_open
    (ordinaryCount index quantity remaining outstanding : Nat) :
    redemptionDraw ordinaryCount .open index quantity remaining outstanding = 0 := by
  simp [redemptionDraw, exit?]

/-- The failure coordinate draws nothing, even under the exhausted exit. -/
theorem the_failure_coordinate_draws_nothing
    (ordinaryCount quantity remaining outstanding : Nat) :
    redemptionDraw ordinaryCount (.terminal (failureSelector ordinaryCount))
        (failureSelector ordinaryCount) quantity remaining outstanding = 0 := by
  simp [redemptionDraw, exit?, ordinaryQuantity, failureSelector, draw]

/-- An ordinary redemption under the exhausted exit draws its pro-rata share. -/
theorem an_ordinary_redemption_draws_its_share
    (ordinaryCount index quantity remaining outstanding : Nat)
    (ordinary : index < ordinaryCount) :
    redemptionDraw ordinaryCount (.terminal (failureSelector ordinaryCount))
        index quantity remaining outstanding
      = draw remaining outstanding quantity := by
  simp [redemptionDraw, exit?, ordinaryQuantity, ordinary]

/-! ### Witnesses on cohort-15's numbers

Three ordinary outcomes at 500,000,000 claims each stand against the bond of
`cohort_fifteen_bond`.  Cohort-13's own measured table -- a founder holding
almost everything and a stranger holding 200 -- redeemed in both orders: the
total is exact both ways, and the stranger's draw moves by one lamport, which
is the rounding boundary made concrete. -/

def cohortFifteenWalk : Walk := { remaining := 4031465, outstanding := 3 * 500000000 }

example : cohortFifteenWalk.draws [200, 1499999800] = [0, 4031465] := by decide
example : cohortFifteenWalk.draws [1499999800, 200] = [4031464, 1] := by decide
example : cohortFifteenWalk.paid [200, 1499999800] = 4031465 := by decide
example : cohortFifteenWalk.paid [1499999800, 200] = 4031465 := by decide
example : (cohortFifteenWalk.run [1499999800, 200]).remaining = 0 := by decide

/-- A founder who holds no ordinary claims draws nothing on an outage. -/
example : draw 4031465 1500000000 0 = 0 := by decide

/-- Half the claims draw half the bond, to the lamport the floor allows. -/
example : draw 4031465 1500000000 750000000 = 2015732 := by decide

/-! ## The close

The escrow's close is the one place the bond leaves the escrow: the closure
burn hands everything the escrow holds to the aggregate, which carries it to
the founder's refund source (decision 0021).  On the honest exit that is the
return.  On the exhausted exit the bond has already been walked out, one
redemption at a time, so the close is admitted only once no ordinary claim
stands -- `OrdinaryClaimsOutstanding` otherwise -- and whatever then stands
above rent is a donation that arrived after the last redemption, carried as
surplus.  The build's `FounderBondClosureV1::new` is this admission. -/

/-- Whether the close is admitted: always on the honest exit; on the exhausted
exit only once no ordinary claim stands. -/
def closeAdmits : Exit → Nat → Bool
  | .honest, _ => true
  | .exhausted, outstanding => decide (outstanding = 0)

theorem an_honest_close_is_always_admitted (outstanding : Nat) :
    closeAdmits .honest outstanding = true := rfl

/-- **Hostile: a close before the walk finishes.**  While an ordinary claim
stands, the exhausted close refuses. -/
theorem an_exhausted_close_refuses_while_claims_stand
    (outstanding : Nat) (standing : 0 < outstanding) :
    closeAdmits .exhausted outstanding = false := by
  unfold closeAdmits
  apply decide_eq_false
  omega

theorem an_exhausted_close_is_admitted_once_the_walk_is_done :
    closeAdmits .exhausted 0 = true := rfl

/-- Along any exhausting walk the close is admitted at its end and finds
nothing of the bond standing: what the founder's refund source receives on
this arm above rent is a late donation, never the bond. -/
theorem an_admitted_exhausted_close_carries_none_of_the_bond
    (walk : Walk) (redemptions : List Nat)
    (positive : 0 < walk.outstanding)
    (partition : redemptions.sum = walk.outstanding) :
    closeAdmits .exhausted (walk.run redemptions).outstanding = true
      ∧ (walk.run redemptions).remaining = 0 := by
  have paid := an_exhausting_walk_pays_the_bond_exactly walk redemptions positive partition
  have final := walk.run_outstanding redemptions
  rw [partition, Nat.sub_self] at final
  refine ⟨?_, paid.right⟩
  simp [closeAdmits, final]

/-! ## The compaction crank

A sleeping holder's redemption can be submitted by the holder or by the
compaction crank on their behalf.  Two facts make the crank safe for the
sleeper, and they are separate facts.  First, the submitter reaches exactly one
coordinate -- where the drawn lamports land -- and the draw itself is a
function of what stands, what is outstanding and what this redemption retires,
none of which the submitter appears in.  Second, the lamports the crank draws
onto the vacant claim-check address are the holder's and must ride into the
minted record rather than be counted as dust the sweep owes less for; that is
`ClaimCheckCompactionObservationV1::founder_bond_draw` and the subtraction
before `claim_check_top_up`. -/

/-- Who submits a redemption. -/
inductive Submitter where
  | holder
  | crank
  deriving DecidableEq, Repr

/-- Where a redemption's drawn lamports land: the holder's own recipient, or
the vacant claim-check address the crank mints the sleeper's record at. -/
def bondRecipient (holderRecipient claimCheck : Nat) : Submitter → Nat
  | .holder => holderRecipient
  | .crank => claimCheck

/-- One submitted redemption: what it draws, and where the draw lands. -/
def submittedRedemption
    (ordinaryCount : Nat) (phase : Phase)
    (index quantity remaining outstanding holderRecipient claimCheck : Nat)
    (submitter : Submitter) : Nat × Nat :=
  (redemptionDraw ordinaryCount phase index quantity remaining outstanding,
    bondRecipient holderRecipient claimCheck submitter)

/-- **The compaction crank draws for a sleeping holder exactly what the holder
would have drawn.**  The submitter is an argument of the submitted redemption
and not of its draw. -/
theorem a_compacted_redemption_draws_what_the_holder_would_have
    (ordinaryCount : Nat) (phase : Phase)
    (index quantity remaining outstanding holderRecipient claimCheck : Nat) :
    (submittedRedemption ordinaryCount phase index quantity remaining outstanding
        holderRecipient claimCheck .crank).fst
      = (submittedRedemption ordinaryCount phase index quantity remaining outstanding
        holderRecipient claimCheck .holder).fst := rfl

/-- ...and the coordinate the submitter DOES reach is the destination, so the
theorem above is a claim about two different redemptions rather than two names
for one term. -/
theorem the_crank_pays_the_claim_check_address_instead
    (ordinaryCount : Nat) (phase : Phase)
    (index quantity remaining outstanding holderRecipient claimCheck : Nat)
    (distinct : holderRecipient ≠ claimCheck) :
    (submittedRedemption ordinaryCount phase index quantity remaining outstanding
        holderRecipient claimCheck .crank).snd
      ≠ (submittedRedemption ordinaryCount phase index quantity remaining outstanding
        holderRecipient claimCheck .holder).snd := by
  simpa [submittedRedemption, bondRecipient] using Ne.symm distinct

/-- What stands on the vacant claim-check address before the mint: dust that
arrived from anywhere, plus the draw this settlement just made for the sleeper. -/
def claimCheckBefore (dust drawn : Nat) : Nat := dust + drawn

/-- What the sweep tops that address up by.  The draw is subtracted back out of
the observed lamports first, so only the dust counts against the rent floor. -/
def compactionTopUp (recordRent dust : Nat) : Nat := recordRent - dust

/-- What the minted record then holds. -/
def compactedRecord (recordRent dust drawn : Nat) : Nat :=
  claimCheckBefore dust drawn + compactionTopUp recordRent dust

/-- **The sleeper's draw rides into the record.**  A record minted over a draw
holds its own rent AND the draw, whatever dust was already there -- so the
redemption that later sweeps the record whole pays the sleeper exactly what
their own redemption would have drawn. -/
theorem a_compacted_record_carries_the_whole_draw
    (recordRent dust drawn : Nat) (covered : dust ≤ recordRent) :
    compactedRecord recordRent dust drawn = recordRent + drawn := by
  unfold compactedRecord claimCheckBefore compactionTopUp
  omega

/-- The policy this one replaced: the draw counted as dust, so the top-up is
measured against `dust + drawn`. -/
def compactedRecordIfDrawWereDust (recordRent dust drawn : Nat) : Nat :=
  claimCheckBefore dust drawn + compactionTopUp recordRent (dust + drawn)

/-- **Hostile: the draw counted as dust.**  The record holds only its rent, the
sleeper's share has been absorbed into the rent floor the cranker, the opener
and the RentCredit are paid out of, and the sweep pays the holder nothing for
it.  This is the one lamport-losing policy `founder_bond_draw` exists to
refuse. -/
theorem counting_the_draw_as_dust_absorbs_it
    (recordRent dust drawn : Nat) (covered : dust + drawn ≤ recordRent) :
    compactedRecordIfDrawWereDust recordRent dust drawn = recordRent := by
  unfold compactedRecordIfDrawWereDust claimCheckBefore compactionTopUp
  omega

example : closeAdmits .exhausted (cohortFifteenWalk.run [1499999800, 200]).outstanding = true := by
  decide

end DClutch.FounderBond
