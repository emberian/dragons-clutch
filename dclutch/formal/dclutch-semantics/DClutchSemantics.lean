import DClutchSemantics.IR
import DClutchSemantics.Direct
import DClutchSemantics.DirectLifecycle
import DClutchSemantics.DirectLifecycleProgram
import DClutchSemantics.DirectLifecycleAbi
import DClutchSemantics.RegisteredPhysical
import DClutchSemantics.RelayedMainnetStateV1Abi
import DClutchSemantics.RelayedVenueDecodingRulesV1
import DClutchSemantics.RegisteredControllerAbi
import DClutchSemantics.DirectProofs
import DClutchSemantics.Codec
import DClutchSemantics.Examples
import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical
import DClutchSemantics.ClaimSbfProfile
import DClutchSemantics.TransitionVM
import DClutchSemantics.TransitionVMV2
import DClutchSemantics.TransitionVMV3
import DClutchSemantics.DirectProgram
import DClutchSemantics.DirectProgramV2
import DClutchSemantics.DirectOrdinaryV3
import DClutchSemantics.DirectRegisteredFillV4
import DClutchSemantics.CompiledPhysical
import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.EconomicKernel
import DClutchSemantics.EconomicExamples
import DClutchSemantics.EconomicCodec
import DClutchSemantics.ExecutionRelease
import DClutchSemantics.ExecutionStrategyV2Abi
import DClutchSemantics.ProtocolInfrastructure
import DClutchSemantics.SourceResolution
import DClutchSemantics.SourceResolutionAbi
import DClutchSemantics.SourceResolutionControllerAbi
import DClutchSemantics.SourceMaterialV2Abi
import DClutchSemantics.SourceMaterialV3Abi
import DClutchSemantics.SourcePrincipalCapacityV1
import DClutchSemantics.SourceScheduledMedianV1
import DClutchSemantics.SourceRecoveryPolicyV2Abi
import DClutchSemantics.SourceResolutionStateV2Abi
import DClutchSemantics.SourceResolutionTerminalV2Abi
import DClutchSemantics.GeneralClearing
import DClutchSemantics.GeneralClearingExamples
import DClutchSemantics.GeneralV5Assurance
import DClutchSemantics.GeneralControllerAbi
import DClutchSemantics.GeneralControllerRequestV3
import DClutchSemantics.GeneralConfigAbi
import DClutchSemantics.GeneralConfigV3Abi
import DClutchSemantics.GeneralRequestProfilesV1
import DClutchSemantics.GeneralTransitionV3
import DClutchSemantics.ProductPayoff
import DClutchSemantics.ProductPayoffExamples
import DClutchSemantics.ProductPayoffAbi
import DClutchSemantics.ProductGradedBasisAdmissionV3Abi
import DClutchSemantics.ProductRuntimeV2
import DClutchSemantics.ProductRuntimeV2Abi
import DClutchSemantics.LiabilityBasisV2
import DClutchSemantics.LiabilityBasisV2Spline
import DClutchSemantics.LiabilityBasisV2SplineAbi
import DClutchSemantics.LiabilityBasisV2SplineExamples
import DClutchSemantics.LiabilityBasisV2PriceGate
import DClutchSemantics.LiabilityBasisV2PriceGateAbi
import DClutchSemantics.LiabilityBasisV2PriceGateExamples
import DClutchSemantics.CustodyAbi
import DClutchSemantics.DealerLiquidity
import DClutchSemantics.DealerLiquidityExamples
import DClutchSemantics.DealerScenarioCollateral
import DClutchSemantics.DealerScenarioSolvency
import DClutchSemantics.DealerTradingProfile
import DClutchSemantics.Series
import DClutchSemantics.SeriesEscrowV3
import DClutchSemantics.SeriesOccurrenceV3
import DClutchSemantics.SeriesOccurrenceV3Abi
import DClutchSemantics.SeriesReplayV3
import DClutchSemantics.SeriesReplayPlanV3
import DClutchSemantics.SeriesCoreFoundAckV2Abi
import DClutchSemantics.SeriesExamples
import DClutchSemantics.MarketCore
import DClutchSemantics.MarketCoreAbi
import DClutchSemantics.MarketCorePhysicalAbi
import DClutchSemantics.MarketCoreExamples
import DClutchSemantics.MarketRetirementV1Abi
import DClutchSemantics.CapabilityProgramV3Abi
import DClutchSemantics.CapabilityProgramV4Abi
import DClutchSemantics.CapabilityProgramAbi
import DClutchSemantics.CapabilityProgramSetV1
import DClutchSemantics.CapabilityProgramSetV2
import DClutchSemantics.CapabilityExecutionAbi
import DClutchSemantics.RequestProfileAbi
import DClutchSemantics.RequestProfileV4Abi
import DClutchSemantics.StateLifecyclePolicyV5Abi
import DClutchSemantics.CapabilityFundingLedgerV2
import DClutchSemantics.AccountProfileAbi
import DClutchSemantics.AccountProfileV2Abi
import DClutchSemantics.AccountProfileV2Profile13
import DClutchSemantics.AccountProfileV2Profile14
import DClutchSemantics.RepresentationCompositionV3Abi
import DClutchSemantics.ProductRepresentationV3Abi
import DClutchSemantics.ProductRepresentationExposureV3Abi
import DClutchSemantics.EffectProgramV4Abi
import DClutchSemantics.RationalCrossDomainV3
import DClutchSemantics.RationalRepresentationV2
import DClutchSemantics.RationalRepresentationV2Examples
import DClutchSemantics.RationalRepresentationV2PhysicalAbi
import DClutchSemantics.RationalTerminalHotV3Abi
import DClutchSemantics.StructuredV2
import DClutchSemantics.StructuredV2Abi
import DClutchSemantics.StructuredV2Examples
import DClutchSemantics.RealmPositionAbi
import DClutchSemantics.CapabilityManifestV1Abi
import DClutchSemantics.TsEmit

/-!
Fresh Lean-owned semantics for dClutch's compact protocol specializer.

This package intentionally has no dependency on the neighboring research
repositories or on the Rust implementation.  The Rust implementation is a
differential oracle, not a source of formal definitions.

## What this list is, and what it is NOT

It is the package's ENTRY POINT: `import DClutchSemantics` gets everything
named here, so a module missing from it is a module a downstream importer does
not receive.  That is the only thing a missing line costs.

It is NOT what decides whether a module is BUILT.  `lakefile.toml` gives the
library `globs = ["DClutchSemantics.+"]`, and the glob -- not this list --
selects what `lake build` compiles.  a1cb5217 recorded the opposite ("a schema
module nobody imports therefore compiles only when something names it") and
added four modules here to close a build gap.  The facts it reported were
right, the inference was not, and the difference is worth a paragraph because
it changes what a lane does next.

Measured 2026-09-02 with a deliberate `(1 : Nat) = 2` in `RefusalBandsV1`,
which this list did not name at the time: BOTH bare `lake build` and
`lake build DClutchSemantics` exit 1, and the build log names the module at
job 131 of 132.  So the coverage was never missing.  Add a module here because
you want importers to get it, not to make it compile -- and if you want to know
whether a module compiles, put an error in it and look, which is the check that
settled this one.
-/
import DClutchSemantics.AbiCoverage
import DClutchSemantics.ClaimsLiabilityBasisStateV2Abi
import DClutchSemantics.DealerScenarioTradeV4Abi
import DClutchSemantics.CoreFoundFrameV3Abi
import DClutchSemantics.LifecycleRentV2Abi
import DClutchSemantics.CapabilityFundingHeaderV2Abi
import DClutchSemantics.GeneralRuntimeWireV2
import DClutchSemantics.ProductBasisV3
import DClutchSemantics.ProductBasisV3Agreement
import DClutchSemantics.RefusalBandsV1
