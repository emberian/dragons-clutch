use clutch_source_plane_v3::ContentId;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

const RECIPE_DOMAIN: &[u8] = b"dragons-clutch/source-plane-v3/pda-recipe/v1";

/// Maximum proposed seed count, including the ASCII family prefix.
pub const MAX_PDA_SEEDS: usize = 5;

/// One canonical Solana PDA seed, fixed-capacity and allocation-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedComponentV3 {
    length: u8,
    bytes: [u8; 32],
}

impl SeedComponentV3 {
    const ZERO: Self = Self {
        length: 0,
        bytes: [0; 32],
    };

    /// Copy an exact nonempty seed of at most 32 bytes.
    pub fn new(input: &[u8]) -> Result<Self> {
        if input.is_empty() || input.len() > 32 {
            return Err(Error::InvalidParameter);
        }
        let mut value = Self::ZERO;
        value.length = u8::try_from(input.len()).map_err(|_| Error::InvalidParameter)?;
        value.bytes[..input.len()].copy_from_slice(input);
        Ok(value)
    }

    /// Canonical component bytes without fixed-capacity padding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }

    fn validate(&self) -> Result<()> {
        let length = usize::from(self.length);
        if length == 0
            || length > self.bytes.len()
            || self.bytes[length..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(())
    }
}

/// Proposed PDA namespaces. This registry supplies seeds, not addresses: a
/// live adapter must derive under and authenticate the exact deployed program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum PdaFamilyV3 {
    /// Existing live V2 SourceSpec address; its historical prefix is retained.
    V2SourceSpec = 1,
    /// V3 source head for one SourceSpec repair generation.
    SourceHead = 2,
    /// Mutable page at one state-assigned page index.
    OpenRawPage = 3,
    /// Immutable page addressed by its content identity.
    RawPage = 4,
    /// Resumable work at one predictable WindowKey.
    WindowWork = 5,
    /// Final evidence at one predictable WindowKey.
    WindowSeal = 6,
    /// Result slot at one predictable StatisticKey, not at ResultDigest.
    StatisticResult = 7,
    /// Reusable Template addressed by content.
    ProductTemplate = 8,
    /// Finite Series plan addressed by content.
    SeriesPlan = 9,
    /// Mutable funding/cursor account paired with a Series identity.
    SeriesFunding = 10,
    /// Convergent Instance descriptor addressed by semantic identity.
    Instance = 11,
    /// Resumable drawdown fold paired with a StatisticKey.
    DrawdownWork = 12,
    /// Immutable reviewed Source release manifest addressed by its content.
    SourceRelease = 13,
    /// Immutable Source work/terminal receipt addressed by its exact receipt identity.
    SourceWorkReceipt = 14,
}

/// Canonical fixed-capacity PDA seed recipe proposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PdaRecipeV3 {
    /// Disjoint semantic family.
    pub family: PdaFamilyV3,
    count: u8,
    seeds: [SeedComponentV3; MAX_PDA_SEEDS],
}

impl PdaRecipeV3 {
    /// Immutable reviewed Source release manifest content address.
    pub fn source_release(release_id: ContentId) -> Result<Self> {
        live(release_id)?;
        Self::two(
            PdaFamilyV3::SourceRelease,
            b"dc-sp3-release",
            &release_id.bytes(),
        )
    }

    /// Immutable Source work or terminal receipt content address.
    pub fn source_work_receipt(receipt_id: ContentId) -> Result<Self> {
        live(receipt_id)?;
        Self::two(
            PdaFamilyV3::SourceWorkReceipt,
            b"dc-sp3-work-receipt",
            &receipt_id.bytes(),
        )
    }

    /// Existing V2 SourceSpec PDA: `[b"source-spec-v1", feed_id]`.
    pub fn v2_source_spec(feed_id: ContentId) -> Result<Self> {
        live(feed_id)?;
        Self::two(
            PdaFamilyV3::V2SourceSpec,
            b"source-spec-v1",
            &feed_id.bytes(),
        )
    }

    /// V3 source head, segregated by exact repair generation.
    pub fn source_head(
        source_plane_contract_id: ContentId,
        source_spec_id: ContentId,
        repair_generation: u64,
    ) -> Result<Self> {
        live(source_plane_contract_id)?;
        live(source_spec_id)?;
        Self::four(
            PdaFamilyV3::SourceHead,
            b"dc-sp3-head",
            &source_plane_contract_id.bytes(),
            &source_spec_id.bytes(),
            &repair_generation.to_le_bytes(),
        )
    }

    /// Mutable page derived only from source-owned cursor coordinates.
    pub fn open_raw_page(
        source_plane_contract_id: ContentId,
        source_spec_id: ContentId,
        repair_generation: u64,
        page_index: u64,
    ) -> Result<Self> {
        live(source_plane_contract_id)?;
        live(source_spec_id)?;
        Self::five(
            PdaFamilyV3::OpenRawPage,
            b"dc-sp3-open",
            &source_plane_contract_id.bytes(),
            &source_spec_id.bytes(),
            &repair_generation.to_le_bytes(),
            &page_index.to_le_bytes(),
        )
    }

    /// Immutable raw page derived from its full content identity.
    pub fn raw_page(source_plane_contract_id: ContentId, page_id: ContentId) -> Result<Self> {
        live(source_plane_contract_id)?;
        live(page_id)?;
        Self::three(
            PdaFamilyV3::RawPage,
            b"dc-sp3-page",
            &source_plane_contract_id.bytes(),
            &page_id.bytes(),
        )
    }

    /// Mutable WindowWork derived from the predictable WindowKey.
    pub fn window_work(window_id: ContentId) -> Result<Self> {
        live(window_id)?;
        Self::two(
            PdaFamilyV3::WindowWork,
            b"dc-sp3-win-work",
            &window_id.bytes(),
        )
    }

    /// Immutable WindowSeal slot derived from WindowKey, not final seal digest.
    pub fn window_seal(window_id: ContentId) -> Result<Self> {
        live(window_id)?;
        Self::two(
            PdaFamilyV3::WindowSeal,
            b"dc-sp3-win-seal",
            &window_id.bytes(),
        )
    }

    /// Result slot derived from StatisticKey; the stored body commits ResultDigest.
    pub fn statistic_result(statistic_key_id: ContentId) -> Result<Self> {
        live(statistic_key_id)?;
        Self::two(
            PdaFamilyV3::StatisticResult,
            b"dc-sp3-stat-result",
            &statistic_key_id.bytes(),
        )
    }

    /// Reusable ProductTemplate content address.
    pub fn product_template(template_id: ContentId) -> Result<Self> {
        live(template_id)?;
        Self::two(
            PdaFamilyV3::ProductTemplate,
            b"dc-sp3-template",
            &template_id.bytes(),
        )
    }

    /// Immutable finite Series plan content address.
    pub fn series_plan(series_id: ContentId) -> Result<Self> {
        live(series_id)?;
        Self::two(
            PdaFamilyV3::SeriesPlan,
            b"dc-sp3-series",
            &series_id.bytes(),
        )
    }

    /// Mutable funding/cursor state paired with the immutable Series identity.
    pub fn series_funding(series_id: ContentId) -> Result<Self> {
        live(series_id)?;
        Self::two(
            PdaFamilyV3::SeriesFunding,
            b"dc-sp3-series-fund",
            &series_id.bytes(),
        )
    }

    /// Convergent Instance identity; Series and creator are intentionally absent.
    pub fn instance(instance_id: ContentId) -> Result<Self> {
        live(instance_id)?;
        Self::two(
            PdaFamilyV3::Instance,
            b"dc-sp3-instance",
            &instance_id.bytes(),
        )
    }

    /// Resumable drawdown fold paired with one exact evaluator request.
    pub fn drawdown_work(statistic_key_id: ContentId) -> Result<Self> {
        live(statistic_key_id)?;
        Self::two(
            PdaFamilyV3::DrawdownWork,
            b"dc-sp3-draw-work",
            &statistic_key_id.bytes(),
        )
    }

    /// Number of active seed components.
    pub const fn seed_count(self) -> u8 {
        self.count
    }

    /// Get an active seed by index.
    pub fn seed(&self, index: usize) -> Result<&[u8]> {
        if index >= usize::from(self.count) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.seeds[index].as_bytes())
    }

    /// Stable digest of the exact proposal recipe, excluding the live program id.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(RECIPE_DOMAIN);
        hasher.update((self.family as u16).to_le_bytes());
        hasher.update([self.count]);
        for seed in self.seeds {
            hasher.update([seed.length]);
            hasher.update(seed.bytes);
        }
        Ok(ContentId::from_bytes(hasher.finalize().into()))
    }

    /// Validate active components and exact inactive zero padding.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.count);
        if count == 0 || count > MAX_PDA_SEEDS {
            return Err(Error::InvalidParameter);
        }
        let mut index = 0;
        while index < MAX_PDA_SEEDS {
            if index < count {
                self.seeds[index].validate()?;
            } else if self.seeds[index] != SeedComponentV3::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    fn two(family: PdaFamilyV3, first: &[u8], second: &[u8]) -> Result<Self> {
        Self::from_parts(family, &[first, second])
    }

    fn three(family: PdaFamilyV3, first: &[u8], second: &[u8], third: &[u8]) -> Result<Self> {
        Self::from_parts(family, &[first, second, third])
    }

    fn four(
        family: PdaFamilyV3,
        first: &[u8],
        second: &[u8],
        third: &[u8],
        fourth: &[u8],
    ) -> Result<Self> {
        Self::from_parts(family, &[first, second, third, fourth])
    }

    fn five(
        family: PdaFamilyV3,
        first: &[u8],
        second: &[u8],
        third: &[u8],
        fourth: &[u8],
        fifth: &[u8],
    ) -> Result<Self> {
        Self::from_parts(family, &[first, second, third, fourth, fifth])
    }

    fn from_parts(family: PdaFamilyV3, parts: &[&[u8]]) -> Result<Self> {
        if parts.is_empty() || parts.len() > MAX_PDA_SEEDS {
            return Err(Error::InvalidParameter);
        }
        let mut seeds = [SeedComponentV3::ZERO; MAX_PDA_SEEDS];
        let mut index = 0;
        while index < parts.len() {
            seeds[index] = SeedComponentV3::new(parts[index])?;
            index += 1;
        }
        let value = Self {
            family,
            count: u8::try_from(parts.len()).map_err(|_| Error::InvalidParameter)?,
            seeds,
        };
        value.validate()?;
        Ok(value)
    }
}

fn live(id: ContentId) -> Result<()> {
    if id.is_zero() {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}
