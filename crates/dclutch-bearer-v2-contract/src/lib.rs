#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bearer claims as a strict basis-vector specialization of the immutable
//! Rational Representation V2 descriptor.
//!
//! This crate owns no balance, holder, supply, quantity, or replay state. Token
//! owns transferable holders and supplies; Claims owns native and materialized
//! quantities. The only Bearer-specific fact is that one authenticated rational
//! descriptor is exactly `denominator` at one selected outcome and zero at all
//! other outcomes.

use dclutch_rational_representation_v2_contract::{
    AssetV2, Error as RepresentationError, PreparedRepresentationV2, RepresentationActionV2,
    RepresentationRequestV2, prepare as prepare_representation,
};
use dclutch_rational_representation_v2_kernel::{
    Coalescing, Error as KernelError, RepresentationDescriptorV2, RepresentationGraphV2,
    ShardCoordinateSuccessor, StructuredProjectionV2, coalesce, prepare_denominate,
    prepare_reconstitute,
};

/// Exact immutable identities against which one finalized descriptor is used.
///
/// This is an ephemeral join input, not another persisted descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerBindingV2 {
    /// Finalized descriptor content identity.
    pub descriptor_id: [u8; 32],
    /// Finalized payoff graph content identity.
    pub graph_id: [u8; 32],
    /// Digest of the exact finalized graph bytes.
    pub graph_digest: [u8; 32],
    /// Selected graph root.
    pub root_id: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable executable release set.
    pub release_set: [u8; 32],
    /// Token-owned Structured receipt Mint retained by the shared ABI.
    pub receipt_mint: [u8; 32],
    /// Exact Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Claims-owned representation authority.
    pub representation_authority: [u8; 32],
    /// Product-owned runtime outcome width.
    pub outcome_count: u32,
    /// Shard atoms in one native claim atom.
    pub denominator: u64,
    /// The sole nonzero outcome coordinate.
    pub selected_outcome: u32,
}

/// Canonical physical identities derived by the operator and rederived by the
/// Claims adapter for the selected descriptor coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerAssetIdentityV2 {
    /// Descriptor/outcome-derived shard Mint.
    pub shard_mint: [u8; 32],
    /// Token Account currently holding the actor's transferable shards.
    pub actor_shard_account: [u8; 32],
    /// Canonical ATA of the representation authority and shard Mint.
    pub structured_custody_account: [u8; 32],
    /// Descriptor/outcome-derived Claims custody Position owner.
    pub claims_custody_owner: [u8; 32],
}

/// Terminal observation required for a Bearer terminal redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BearerResolutionV2 {
    /// No final winning Product outcome was supplied.
    Unresolved,
    /// Claims authenticated this final winning Product outcome.
    Resolved {
        /// Final winning outcome.
        winner: u32,
    },
}

/// Stable Bearer specialization refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An immutable descriptor field differed from the exact expected binding.
    BindingMismatch,
    /// Descriptor and finalized graph/root/digest did not authenticate.
    GraphMismatch,
    /// Coefficients were not exactly one basis vector at the descriptor denominator.
    NotBasisVector,
    /// A non-Bearer Rational Representation action was requested.
    UnsupportedAction,
    /// The request selected another outcome or physical asset identity.
    AssetMismatch,
    /// Terminal redemption lacked the exact selected winning outcome.
    TerminalMismatch,
    /// The shared Rational Representation contract refused the physical request.
    Representation(RepresentationError),
    /// The pure rational kernel refused a conservation calculation.
    Kernel(KernelError),
}

/// Result alias for the Bearer V2 specialization.
pub type Result<T> = core::result::Result<T, Error>;

/// Authenticated immutable Bearer specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerDescriptorV2<'a> {
    descriptor: RepresentationDescriptorV2<'a>,
    graph: RepresentationGraphV2<'a>,
    selected_outcome: u32,
}

impl<'a> BearerDescriptorV2<'a> {
    /// Authenticate every immutable descriptor/graph binding and require one
    /// exact basis vector: `denominator` at `selected_outcome`, zero elsewhere.
    pub fn authenticate(
        descriptor: RepresentationDescriptorV2<'a>,
        graph: RepresentationGraphV2<'a>,
        binding: BearerBindingV2,
    ) -> Result<Self> {
        if descriptor.descriptor_id() != binding.descriptor_id
            || descriptor.graph_id() != binding.graph_id
            || descriptor.graph_digest() != binding.graph_digest
            || descriptor.root_id() != binding.root_id
            || descriptor.market_id() != binding.market
            || descriptor.release_set_id() != binding.release_set
            || descriptor.receipt_mint() != binding.receipt_mint
            || descriptor.token_program() != binding.token_program
            || descriptor.representation_authority() != binding.representation_authority
            || descriptor.outcome_count() != binding.outcome_count
            || descriptor.denominator() != binding.denominator
            || binding.selected_outcome >= binding.outcome_count
        {
            return Err(Error::BindingMismatch);
        }
        descriptor
            .authenticate_graph(graph)
            .map_err(|_| Error::GraphMismatch)?;
        let mut outcome = 0_u32;
        while outcome < binding.outcome_count {
            let expected = if outcome == binding.selected_outcome {
                binding.denominator
            } else {
                0
            };
            if descriptor.coefficient(outcome).map_err(Error::Kernel)? != expected {
                return Err(Error::NotBasisVector);
            }
            outcome = outcome.checked_add(1).ok_or(Error::NotBasisVector)?;
        }
        Ok(Self {
            descriptor,
            graph,
            selected_outcome: binding.selected_outcome,
        })
    }

    /// Underlying immutable Rational Representation V2 descriptor.
    pub const fn representation(self) -> RepresentationDescriptorV2<'a> {
        self.descriptor
    }

    /// Authenticated finalized payoff graph.
    pub const fn graph(self) -> RepresentationGraphV2<'a> {
        self.graph
    }

    /// Sole Bearer outcome coordinate.
    pub const fn selected_outcome(self) -> u32 {
        self.selected_outcome
    }

    /// Shard atoms in one native claim atom.
    pub const fn denominator(self) -> u64 {
        self.descriptor.denominator()
    }

    /// Coalesce transferable shards without creating a hidden remainder.
    /// `change_shards` remains an explicit Token-owned holder balance.
    pub const fn coalesce(self, input_shards: u64) -> Result<Coalescing> {
        match coalesce(self.denominator(), input_shards) {
            Ok(value) => Ok(value),
            Err(error) => Err(Error::Kernel(error)),
        }
    }

    /// Exact denomination successor for the selected basis coordinate.
    pub fn denominate_successor(
        self,
        projection: StructuredProjectionV2<'_>,
        quantity: u64,
    ) -> Result<ShardCoordinateSuccessor> {
        self.authenticate_projection(projection)?;
        prepare_denominate(projection, self.selected_outcome, quantity).map_err(Error::Kernel)
    }

    /// Exact reconstitution successor for the selected basis coordinate.
    pub fn reconstitute_successor(
        self,
        projection: StructuredProjectionV2<'_>,
        quantity: u64,
    ) -> Result<ShardCoordinateSuccessor> {
        self.authenticate_projection(projection)?;
        prepare_reconstitute(projection, self.selected_outcome, quantity).map_err(Error::Kernel)
    }

    fn authenticate_projection(self, projection: StructuredProjectionV2<'_>) -> Result<()> {
        if projection.descriptor_id() != self.descriptor.descriptor_id()
            || projection.market_id() != self.descriptor.market_id()
            || projection.receipt_mint() != self.descriptor.receipt_mint()
            || projection.outcome_count() != self.descriptor.outcome_count()
            || projection.denominator() != self.descriptor.denominator()
        {
            return Err(Error::BindingMismatch);
        }
        let mut outcome = 0_u32;
        while outcome < projection.outcome_count() {
            let expected = if outcome == self.selected_outcome {
                self.denominator()
            } else {
                0
            };
            if projection
                .coordinate(outcome)
                .map_err(Error::Kernel)?
                .coefficient
                != expected
            {
                return Err(Error::NotBasisVector);
            }
            outcome = outcome.checked_add(1).ok_or(Error::NotBasisVector)?;
        }
        Ok(())
    }
}

/// Join a Bearer request to the exact shared Rational Representation V2
/// physical contract. No private Bearer wire or mutable Bearer ledger exists.
pub fn prepare<'a>(
    bearer: BearerDescriptorV2<'a>,
    request: RepresentationRequestV2<'a>,
    projection: StructuredProjectionV2<'a>,
    asset_identity: BearerAssetIdentityV2,
    resolution: BearerResolutionV2,
) -> Result<PreparedRepresentationV2<'a>> {
    let header = request.header();
    if !matches!(
        header.action,
        RepresentationActionV2::Denominate
            | RepresentationActionV2::Reconstitute
            | RepresentationActionV2::RedeemTerminal
    ) {
        return Err(Error::UnsupportedAction);
    }
    if header.selected_outcome != bearer.selected_outcome || header.asset_count != 1 {
        return Err(Error::AssetMismatch);
    }
    let asset = request.asset(0).map_err(Error::Representation)?;
    authenticate_asset(asset, asset_identity, bearer.denominator())?;
    if header.action == RepresentationActionV2::RedeemTerminal
        && resolution
            != (BearerResolutionV2::Resolved {
                winner: bearer.selected_outcome,
            })
    {
        return Err(Error::TerminalMismatch);
    }
    prepare_representation(request, bearer.descriptor, projection, bearer.graph)
        .map_err(Error::Representation)
}

fn authenticate_asset(
    asset: AssetV2,
    identity: BearerAssetIdentityV2,
    denominator: u64,
) -> Result<()> {
    if asset.shard_mint != identity.shard_mint
        || asset.actor_shard_account != identity.actor_shard_account
        || asset.structured_custody_account != identity.structured_custody_account
        || asset.claims_custody_owner != identity.claims_custody_owner
        || asset.coefficient != denominator
    {
        return Err(Error::AssetMismatch);
    }
    Ok(())
}
