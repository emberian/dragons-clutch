import DClutchSemantics.Series

/-!
# Executable hostile cases for recurring Series

These examples run the Lean transition functions.  They are semantic evidence,
not evidence for Solana account authentication, Registry SBF, CPI, persistence,
or transaction rollback.
-/

namespace DClutch.Series.Examples

open DClutch

def slotLimit : Nat := 1000
def lamportLimit : Nat := 10000
def scalarLimit : Nat := 10000

def coreBinding : ExecutionRelease.Binding := {
  program := 101
  artifactRelease := 201
  semanticRelease := 301
}

def claimsBinding : ExecutionRelease.Binding := {
  program := 102
  artifactRelease := 202
  semanticRelease := 302
}

def tradingBinding : ExecutionRelease.Binding := {
  program := 103
  artifactRelease := 203
  semanticRelease := 303
}

def resolutionBinding : ExecutionRelease.Binding := {
  program := 104
  artifactRelease := 204
  semanticRelease := 304
}

def custodyBinding : ExecutionRelease.Binding := {
  program := 105
  artifactRelease := 205
  semanticRelease := 305
}

def releaseSet : ExecutionRelease.ReleaseSet := {
  releaseSetId := 77
  core := coreBinding
  claims := claimsBinding
  trading := tradingBinding
  resolution := resolutionBinding
  custody := custodyBinding
}

def marketRegistryProgram : ExecutionRelease.Identity := 106

def releaseAdmission : ExecutionRelease.Admission := {
  marketRegistryProgram
  marketReleaseSetId := 77
  selected := releaseSet
  receipt := {
    registryProgram := marketRegistryProgram
    releaseSetId := 77
    role := .core
    observed := coreBinding
    activationCacheAuthenticated := true
    currentDeploymentReauthenticated := true
  }
}

theorem registry_ownership_is_distinct_and_exactly_joined :
    marketRegistryProgram ≠ coreBinding.program /\
    releaseAdmission.marketRegistryProgram = marketRegistryProgram /\
    releaseAdmission.receipt.registryProgram = marketRegistryProgram /\
    releaseAdmission.marketReleaseSetId = releaseSet.releaseSetId /\
    ExecutionRelease.admits releaseAdmission .core = true := by
  native_decide

def template : Template := {
  templateId := 11
  realmId := 12
  productId := 13
  releaseSetId := 77
  outcomeCount := 3
  firstOccurrenceSlot := 100
  periodSlots := 10
  occurrenceCount := 3
  retryWindowSlots := 5
  seedQuantity := 10
  marketRentLamports := 20
  capabilityRentLamports := 0
  foundingWorkLamports := 5
  seriesCloseRentLamports := 7
  seriesRefundOwner := 900
}

def ticket0 : Ticket := {
  ticketId := 1000
  templateId := template.templateId
  occurrence := 0
  founder := 500
  refundOwner := 501
  committedMarketId := 2000
  revision := 0
  phase := .ready
  funds := requiredFunds template
}

def series0 : State := {
  seriesId := 800
  templateId := template.templateId
  phase := .active
  nextOccurrence := 0
  revision := 0
  closeRentLamports := template.seriesCloseRentLamports
}

def snapshot0 : Snapshot := { template, series := series0, ticket := ticket0 }

def consume0 : Frame := {
  slotLimit
  lamportLimit
  scalarLimit
  nowSlot := 100
  expectedSeriesRevision := 0
  expectedTicketRevision := 0
  pre := snapshot0
  releaseAdmission
  physicalSucceeded := true
  command := .consume 700
}

theorem exact_release_and_funded_ticket_are_admissible :
    ExecutionRelease.admits releaseAdmission .core = true /\
    valid slotLimit lamportLimit scalarLimit snapshot0 = true /\
    semanticAccepts consume0 = true := by
  native_decide

theorem consumption_creates_one_exact_seeded_market_atomically :
    (runState consume0).series.nextOccurrence = 1 /\
    (runState consume0).series.revision = 1 /\
    (runState consume0).ticket.phase = .consumed /\
    fundsZero (runState consume0).ticket.funds = true /\
    (compile consume0).market = some (marketFounding consume0) /\
    (marketFounding consume0).marketId = ticket0.committedMarketId /\
    (marketFounding consume0).scheduledSlot = 100 /\
    (marketFounding consume0).economicState.hoard = template.seedQuantity /\
    (marketFounding consume0).economicState.supply = [10, 10, 10] /\
    (marketFounding consume0).claimEffects.effects.length = template.outcomeCount := by
  native_decide

theorem founding_custody_uses_only_exact_ticket_compartments :
    (compile consume0).custodyTransfers = [
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .marketHoard ticket0.committedMarketId
        amount := template.seedQuantity
      },
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .marketAccount ticket0.committedMarketId
        amount := template.marketRentLamports
      },
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .beneficiary 700
        amount := template.foundingWorkLamports
      }
    ] := by
  native_decide

def physicalFailure : Frame := { consume0 with physicalSucceeded := false }
def retryAt101 : Frame := { consume0 with nowSlot := 101 }

theorem failed_physical_attempt_rolls_back_and_remains_retryable :
    refusal? physicalFailure = some .physicalFailure /\
    runState physicalFailure = snapshot0 /\
    semanticAccepts retryAt101 = true /\
    (runState retryAt101).ticket.phase = .consumed := by
  native_decide

def replayConsumed : Frame := {
  consume0 with
  expectedSeriesRevision := 1
  expectedTicketRevision := 1
  pre := runState consume0
}

theorem consumed_ticket_replay_refuses_without_mutation :
    valid slotLimit lamportLimit scalarLimit replayConsumed.pre = true /\
    semanticAccepts replayConsumed = false /\
    refusal? replayConsumed = some .notAdmissible /\
    runState replayConsumed = replayConsumed.pre := by
  native_decide

def earlyConsume : Frame := { consume0 with nowSlot := 99 }
def lateConsume : Frame := { consume0 with nowSlot := 106 }

def underfunded : Frame := {
  consume0 with
  pre := { snapshot0 with ticket := { ticket0 with
    funds := { requiredFunds template with hoardPrincipal := 9 } } }
}

def overfunded : Frame := {
  consume0 with
  pre := { snapshot0 with ticket := { ticket0 with
    funds := { requiredFunds template with marketRent := 21 } } }
}

def staleRelease : Frame := {
  consume0 with
  releaseAdmission := { releaseAdmission with receipt := {
    releaseAdmission.receipt with
    currentDeploymentReauthenticated := false
  } }
}

def substitutedRelease : Frame := {
  consume0 with
  releaseAdmission := { releaseAdmission with marketReleaseSetId := 78 }
}

theorem hostile_schedule_funding_and_release_cases_refuse_and_roll_back :
    semanticAccepts earlyConsume = false /\
    runState earlyConsume = snapshot0 /\
    semanticAccepts lateConsume = false /\
    runState lateConsume = snapshot0 /\
    semanticAccepts underfunded = false /\
    runState underfunded = underfunded.pre /\
    semanticAccepts overfunded = false /\
    runState overfunded = overfunded.pre /\
    semanticAccepts staleRelease = false /\
    runState staleRelease = snapshot0 /\
    semanticAccepts substitutedRelease = false /\
    runState substitutedRelease = snapshot0 := by
  native_decide

def aliasedButIncoherentRelease : ExecutionRelease.ReleaseSet := {
  releaseSet with
  claims := { claimsBinding with program := coreBinding.program }
}

theorem role_alias_cannot_present_a_second_artifact_truth :
    ExecutionRelease.releaseSetValid aliasedButIncoherentRelease = false := by
  native_decide

def expire0 : Frame := { consume0 with nowSlot := 106, command := .expire }

theorem expired_ticket_refunds_every_nonzero_compartment_exactly :
    semanticAccepts expire0 = true /\
    (runState expire0).series.nextOccurrence = 1 /\
    (runState expire0).ticket.phase = .expired /\
    fundsZero (runState expire0).ticket.funds = true /\
    (compile expire0).custodyTransfers = [
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .beneficiary ticket0.refundOwner
        amount := template.seedQuantity
      },
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .beneficiary ticket0.refundOwner
        amount := template.marketRentLamports
      },
      {
        source := .ticketEscrow ticket0.ticketId
        destination := .beneficiary ticket0.refundOwner
        amount := template.foundingWorkLamports
      }
    ] := by
  native_decide

def ticket2 : Ticket := {
  ticket0 with
  ticketId := 1002
  occurrence := 2
  committedMarketId := 2002
}

def beforeLast : Snapshot := {
  template
  series := { series0 with nextOccurrence := 2, revision := 2 }
  ticket := ticket2
}

def consumeLast : Frame := {
  consume0 with
  nowSlot := 120
  expectedSeriesRevision := 2
  pre := beforeLast
}

def closeSeries : Frame := {
  consume0 with
  nowSlot := 120
  expectedSeriesRevision := 3
  expectedTicketRevision := 1
  pre := runState consumeLast
  command := .close
}

theorem last_occurrence_terminalizes_and_close_drains_only_series_rent :
    semanticAccepts consumeLast = true /\
    (runState consumeLast).series.phase = .terminal /\
    (runState consumeLast).series.nextOccurrence = template.occurrenceCount /\
    semanticAccepts closeSeries = true /\
    (runState closeSeries).series.phase = .closed /\
    (runState closeSeries).series.closeRentLamports = 0 /\
    (runState closeSeries).ticket = (runState consumeLast).ticket /\
    (compile closeSeries).custodyTransfers = [{
      source := .seriesEscrow series0.seriesId
      destination := .beneficiary template.seriesRefundOwner
      amount := template.seriesCloseRentLamports
    }] := by
  native_decide

def overflowingSchedule : Template := {
  template with
  firstOccurrenceSlot := 990
  periodSlots := 10
  retryWindowSlots := 5
}

theorem schedule_must_fit_the_named_physical_slot_profile :
    templateValid slotLimit lamportLimit overflowingSchedule = false := by
  native_decide

end DClutch.Series.Examples
