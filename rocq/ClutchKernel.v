(**
   Dragon's Clutch: handwritten Rocq shadow of the pure clutch-kernel.

   This is a Rust-independent state machine.  Lists below represent the active
   prefix of the kernel's fixed arrays; [state_validb] checks the exact active
   length.  Public transitions are total and use [None] for refusal.  In
   particular, redemption never floors an unrepresentable payout.

   This file intentionally contains definitions and named proof obligations,
   not a formal-verification claim.  The obligations at the end are Props and
   are not admitted theorems.
*)

From Coq Require Import Arith.PeanoNat Lists.List.
Import ListNotations.

Set Implicit Arguments.

Definition MAX_OUTCOMES : nat := 16.
Definition MAX_PAYOUTS : nat := 8.
Definition MIN_OUTCOMES : nat := 2.
Definition U64_MAX : nat := 18446744073709551615.

Inductive phase : Type :=
| Active
| Resolved.

Record payout_vector : Type := {
  pv_denominator : nat;
  pv_weights : list nat
}.

Record state : Type := {
  st_outcomes : nat;
  st_phase : phase;
  st_resolved_payout : nat;
  st_collateral : nat;
  st_total_supply : list nat;
  st_payouts : list payout_vector;
  st_internal : list nat;
  st_external : list nat
}.

Fixpoint sum_nat (xs : list nat) : nat :=
  match xs with
  | [] => 0
  | x :: tl => x + sum_nat tl
  end.

Fixpoint all_le_b (bound : nat) (xs : list nat) : bool :=
  match xs with
  | [] => true
  | x :: tl => andb (Nat.leb x bound) (all_le_b bound tl)
  end.

Definition all_u64_b (xs : list nat) : bool :=
  all_le_b U64_MAX xs.

Fixpoint map_add_amount (q : nat) (xs : list nat) : option (list nat) :=
  match xs with
  | [] => Some []
  | x :: tl =>
      match map_add_amount q tl with
      | None => None
      | Some tl' =>
          if Nat.leb (x + q) U64_MAX
          then Some ((x + q) :: tl')
          else None
      end
  end.

Fixpoint map_sub_amount (q : nat) (xs : list nat) : option (list nat) :=
  match xs with
  | [] => Some []
  | x :: tl =>
      match map_sub_amount q tl with
      | None => None
      | Some tl' =>
          if Nat.leb q x
          then Some ((x - q) :: tl')
          else None
      end
  end.

Definition add_amount (x q : nat) : option nat :=
  if Nat.leb (x + q) U64_MAX then Some (x + q) else None.

Definition sub_amount (x q : nat) : option nat :=
  if Nat.leb q x then Some (x - q) else None.

Fixpoint replace_nth (i x : nat) (xs : list nat) : option (list nat) :=
  match i, xs with
  | 0, _ :: tl => Some (x :: tl)
  | S j, h :: tl =>
      match replace_nth j x tl with
      | Some tl' => Some (h :: tl')
      | None => None
      end
  | _, _ => None
  end.

Fixpoint dot (xs ys : list nat) : nat :=
  match xs, ys with
  | x :: xt, y :: yt => x * y + dot xt yt
  | _, _ => 0
  end.

Definition ceil_div (numerator denominator : nat) : nat :=
  if Nat.eqb denominator 0 then 0
  else if Nat.eqb (Nat.modulo numerator denominator) 0
       then Nat.div numerator denominator
       else S (Nat.div numerator denominator).

Definition required_for_vector (supply : list nat) (p : payout_vector) : nat :=
  ceil_div (dot supply (pv_weights p)) (pv_denominator p).

Fixpoint required_active (supply : list nat)
         (ps : list payout_vector) : nat :=
  match ps with
  | [] => 0
  | p :: tl =>
      Nat.max (required_for_vector supply p)
              (required_active supply tl)
  end.

Definition payout_vector_validb (outcomes : nat) (p : payout_vector) : bool :=
  andb (Nat.leb 1 (pv_denominator p))
    (andb (Nat.leb (pv_denominator p) U64_MAX)
      (andb (Nat.eqb (length (pv_weights p)) outcomes)
        (andb (all_le_b (pv_denominator p) (pv_weights p))
          (Nat.eqb (sum_nat (pv_weights p)) (pv_denominator p))))).

Fixpoint payout_tail_validb (outcomes denominator : nat)
         (ps : list payout_vector) : bool :=
  match ps with
  | [] => true
  | p :: tl =>
      andb (Nat.eqb (pv_denominator p) denominator)
        (andb (payout_vector_validb outcomes p)
              (payout_tail_validb outcomes denominator tl))
  end.

Definition payout_set_validb (outcomes : nat)
           (ps : list payout_vector) : bool :=
  match ps with
  | [] => false
  | p :: tl =>
      andb (Nat.leb 1 (length ps))
        (andb (Nat.leb (length ps) MAX_PAYOUTS)
          (andb (payout_vector_validb outcomes p)
                (payout_tail_validb outcomes (pv_denominator p) tl)))
  end.

Definition phase_validb (s : state) : bool :=
  match st_phase s with
  | Active => true
  | Resolved => Nat.ltb (st_resolved_payout s) (length (st_payouts s))
  end.

Definition shape_validb (s : state) : bool :=
  andb (Nat.leb MIN_OUTCOMES (st_outcomes s))
    (andb (Nat.leb (st_outcomes s) MAX_OUTCOMES)
      (andb (Nat.eqb (length (st_total_supply s)) (st_outcomes s))
        (andb (Nat.eqb (length (st_internal s)) (st_outcomes s))
          (andb (Nat.eqb (length (st_external s)) (st_outcomes s))
                (payout_set_validb (st_outcomes s) (st_payouts s)))))).

Fixpoint position_consistent_b
         (supply internal external : list nat) : bool :=
  match supply, internal, external with
  | t :: ts, i :: is, e :: es =>
      andb (Nat.leb (i + e) t)
        (position_consistent_b ts is es)
  | [], [], [] => true
  | _, _, _ => false
  end.

Definition required (s : state) : nat :=
  match st_phase s with
  | Active => required_active (st_total_supply s) (st_payouts s)
  | Resolved =>
      match nth_error (st_payouts s) (st_resolved_payout s) with
      | Some p => required_for_vector (st_total_supply s) p
      | None => 0
      end
  end.

Definition state_validb (s : state) : bool :=
  andb (shape_validb s)
    (andb (phase_validb s)
      (andb (all_u64_b (st_total_supply s))
        (andb (all_u64_b (st_internal s))
          (andb (all_u64_b (st_external s))
            (andb (Nat.leb (st_collateral s) U64_MAX)
              (andb (position_consistent_b
                       (st_total_supply s) (st_internal s) (st_external s))
                    (Nat.leb (required s) (st_collateral s)))))))).

Definition active_b (s : state) : bool :=
  match st_phase s with
  | Active => true
  | Resolved => false
  end.

Definition finish (s : state) : option state :=
  if state_validb s then Some s else None.

Definition initial_state (outcomes : nat) (ps : list payout_vector)
           (collateral : nat) : option state :=
  let s := {| st_outcomes := outcomes;
              st_phase := Active;
              st_resolved_payout := 0;
              st_collateral := collateral;
              st_total_supply := repeat 0 outcomes;
              st_payouts := ps;
              st_internal := repeat 0 outcomes;
              st_external := repeat 0 outcomes |} in
  finish s.

Definition split (s : state) (q : nat) : option state :=
  if andb (state_validb s)
          (andb (active_b s) (Nat.ltb 0 q)) then
    match add_amount (st_collateral s) q,
          map_add_amount q (st_total_supply s),
          map_add_amount q (st_internal s) with
    | Some collateral', Some supply', Some internal' =>
        finish {| st_outcomes := st_outcomes s;
                  st_phase := st_phase s;
                  st_resolved_payout := st_resolved_payout s;
                  st_collateral := collateral';
                  st_total_supply := supply';
                  st_payouts := st_payouts s;
                  st_internal := internal';
                  st_external := st_external s |}
    | _, _, _ => None
    end
  else None.

Definition merge (s : state) (q : nat) : option state :=
  if andb (state_validb s)
          (andb (active_b s) (Nat.ltb 0 q)) then
    match sub_amount (st_collateral s) q,
          map_sub_amount q (st_total_supply s),
          map_sub_amount q (st_internal s) with
    | Some collateral', Some supply', Some internal' =>
        finish {| st_outcomes := st_outcomes s;
                  st_phase := st_phase s;
                  st_resolved_payout := st_resolved_payout s;
                  st_collateral := collateral';
                  st_total_supply := supply';
                  st_payouts := st_payouts s;
                  st_internal := internal';
                  st_external := st_external s |}
    | _, _, _ => None
    end
  else None.

Definition materialize (s : state) (outcome q : nat) : option state :=
  if andb (state_validb s)
          (andb (active_b s)
            (andb (Nat.ltb 0 q) (Nat.ltb outcome (st_outcomes s)))) then
    match nth_error (st_internal s) outcome with
    | Some available =>
        match sub_amount available q,
              nth_error (st_external s) outcome with
        | Some internal_at, Some external_at =>
            match add_amount external_at q,
                  replace_nth outcome internal_at (st_internal s),
                  replace_nth outcome (external_at + q) (st_external s) with
            | Some _, Some internal', Some external' =>
                finish {| st_outcomes := st_outcomes s;
                          st_phase := st_phase s;
                          st_resolved_payout := st_resolved_payout s;
                          st_collateral := st_collateral s;
                          st_total_supply := st_total_supply s;
                          st_payouts := st_payouts s;
                          st_internal := internal';
                          st_external := external' |}
            | _, _, _ => None
            end
        | _, _ => None
        end
    | None => None
    end
  else None.

Definition dematerialize (s : state) (outcome q : nat) : option state :=
  if andb (state_validb s)
          (andb (active_b s)
            (andb (Nat.ltb 0 q) (Nat.ltb outcome (st_outcomes s)))) then
    match nth_error (st_external s) outcome with
    | Some available =>
        match sub_amount available q,
              nth_error (st_internal s) outcome with
        | Some external_at, Some internal_at =>
            match add_amount internal_at q,
                  replace_nth outcome (internal_at + q) (st_internal s),
                  replace_nth outcome external_at (st_external s) with
            | Some _, Some internal', Some external' =>
                finish {| st_outcomes := st_outcomes s;
                          st_phase := st_phase s;
                          st_resolved_payout := st_resolved_payout s;
                          st_collateral := st_collateral s;
                          st_total_supply := st_total_supply s;
                          st_payouts := st_payouts s;
                          st_internal := internal';
                          st_external := external' |}
            | _, _, _ => None
            end
        | _, _ => None
        end
    | None => None
    end
  else None.

Definition resolve (s : state) (payout_index : nat) : option state :=
  if andb (state_validb s)
          (andb (active_b s)
                (Nat.ltb payout_index (length (st_payouts s)))) then
    finish {| st_outcomes := st_outcomes s;
              st_phase := Resolved;
              st_resolved_payout := payout_index;
              st_collateral := st_collateral s;
              st_total_supply := st_total_supply s;
              st_payouts := st_payouts s;
              st_internal := st_internal s;
              st_external := st_external s |}
  else None.

Definition exact_payout (quantity weight denominator : nat) : option nat :=
  if Nat.eqb denominator 0 then None
  else
    let numerator := quantity * weight in
    if Nat.eqb (Nat.modulo numerator denominator) 0 then
      let payout := Nat.div numerator denominator in
      if Nat.leb payout U64_MAX then Some payout else None
    else None.

Definition redeem (internal : bool) (s : state)
           (outcome quantity : nat) : option (state * nat) :=
  if andb (state_validb s)
          (andb (negb (active_b s))
            (andb (Nat.ltb 0 quantity)
              (Nat.ltb outcome (st_outcomes s)))) then
    let source := if internal then st_internal s else st_external s in
    match nth_error (st_payouts s) (st_resolved_payout s),
          nth_error source outcome,
          nth_error (st_total_supply s) outcome with
    | Some p, Some available, Some accounted =>
        match nth_error (pv_weights p) outcome with
        | Some weight =>
            if andb (Nat.leb quantity available)
                    (Nat.leb quantity accounted) then
              match exact_payout quantity weight (pv_denominator p) with
              | Some payout =>
                  match replace_nth outcome (available - quantity) source,
                        replace_nth outcome (accounted - quantity)
                                   (st_total_supply s),
                        sub_amount (st_collateral s) payout with
                  | Some source', Some supply', Some collateral' =>
                      let internal' :=
                        if internal then source' else st_internal s in
                      let external' :=
                        if internal then st_external s else source' in
                      let s' := {| st_outcomes := st_outcomes s;
                                   st_phase := st_phase s;
                                   st_resolved_payout := st_resolved_payout s;
                                   st_collateral := collateral';
                                   st_total_supply := supply';
                                   st_payouts := st_payouts s;
                                   st_internal := internal';
                                   st_external := external' |} in
                      if state_validb s'
                      then Some (s', payout)
                      else None
                  | _, _, _ => None
                  end
              | None => None
              end
            else None
        | None => None
        end
    | _, _, _ => None
    end
  else None.

Definition redeem_internal (s : state) (outcome quantity : nat) :=
  redeem true s outcome quantity.

Definition redeem_external (s : state) (outcome quantity : nat) :=
  redeem false s outcome quantity.

(** Named obligations.  These are statements for later proof work, not
    [Admitted] theorems and not release claims. *)

Definition split_preserves_solvency : Prop :=
  forall s q s', split s q = Some s' -> state_validb s' = true.

Definition merge_preserves_solvency : Prop :=
  forall s q s', merge s q = Some s' -> state_validb s' = true.

Definition materialize_is_supply_neutral : Prop :=
  forall s outcome q s', materialize s outcome q = Some s' ->
    st_total_supply s' = st_total_supply s /\
    st_collateral s' = st_collateral s.

Definition dematerialize_is_supply_neutral : Prop :=
  forall s outcome q s', dematerialize s outcome q = Some s' ->
    st_total_supply s' = st_total_supply s /\
    st_collateral s' = st_collateral s.

Definition redemption_is_exact : Prop :=
  forall internal s outcome quantity s' payout,
    redeem internal s outcome quantity = Some (s', payout) ->
    exists p,
      nth_error (st_payouts s) (st_resolved_payout s) = Some p /\
      exists weight,
        nth_error (pv_weights p) outcome = Some weight /\
        quantity * weight = payout * pv_denominator p.

Definition successful_transition_is_well_formed : Prop :=
  (forall s q s', split s q = Some s' -> state_validb s' = true) /\
  (forall s q s', merge s q = Some s' -> state_validb s' = true) /\
  (forall s o q s', materialize s o q = Some s' -> state_validb s' = true) /\
  (forall s o q s', dematerialize s o q = Some s' -> state_validb s' = true) /\
  (forall s o, resolve s o = Some s -> state_validb s = true) /\
  (forall b s o q s' p, redeem b s o q = Some (s', p) ->
      state_validb s' = true).
