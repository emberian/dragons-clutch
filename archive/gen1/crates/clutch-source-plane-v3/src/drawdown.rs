use crate::codec::{Reader, Writer};
use crate::source::MAX_SOURCE_VALUE;
use crate::{Error, FixedCodec, Result};

const DRAWDOWN_MAGIC: [u8; 8] = *b"DCDRWV3\0";

/// Exact scale for maximum drawdown: one whole equals one million ppm.
pub const DRAWDOWN_PPM_SCALE: u64 = 1_000_000;
/// Exact fixed encoding width of one ordered drawdown summary.
pub const DRAWDOWN_SUMMARY_BYTES: usize = 112;

/// Conservative inclusive maximum-drawdown interval in integer ppm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawdownIntervalV3 {
    /// Lower bound under the frozen round-up statistic.
    pub low_ppm: u64,
    /// Upper bound under the frozen round-up statistic.
    pub high_ppm: u64,
}

/// Ordered associative summary for maximum peak-to-subsequent-trough drawdown.
///
/// For an exact positive peak `p` and later trough `t`, the statistic is
/// `ceil(1_000_000 * max(p - t, 0) / p)`. Zero peaks contribute zero. Interval
/// inputs return conservative lower and upper bounds for that same rounded-up
/// statistic. This summary requires complete adjacent buckets; gaps belong to
/// a different explicitly defined statistic rather than being skipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawdownSummaryV3 {
    start_bucket: u64,
    end_bucket_exclusive: u64,
    record_count: u64,
    maximum_low: u128,
    maximum_high: u128,
    minimum_low: u128,
    minimum_high: u128,
    drawdown_low_ppm: u64,
    drawdown_high_ppm: u64,
}

impl DrawdownSummaryV3 {
    /// Construct one conservative interval at one canonical bucket.
    pub fn singleton(bucket: u64, low: u128, high: u128) -> Result<Self> {
        if low > high || high > MAX_SOURCE_VALUE {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            start_bucket: bucket,
            end_bucket_exclusive: bucket.checked_add(1).ok_or(Error::ArithmeticOverflow)?,
            record_count: 1,
            maximum_low: low,
            maximum_high: high,
            minimum_low: low,
            minimum_high: high,
            drawdown_low_ppm: 0,
            drawdown_high_ppm: 0,
        })
    }

    /// Combine exact adjacent ranges in chronological order.
    pub fn combine(self, later: Self) -> Result<Self> {
        self.validate()?;
        later.validate()?;
        if self.end_bucket_exclusive != later.start_bucket {
            return Err(Error::DiscontinuousPage);
        }
        let cross_low = drawdown_ppm(self.maximum_low, later.minimum_high)?;
        let cross_high = drawdown_ppm(self.maximum_high, later.minimum_low)?;
        let value = Self {
            start_bucket: self.start_bucket,
            end_bucket_exclusive: later.end_bucket_exclusive,
            record_count: self
                .record_count
                .checked_add(later.record_count)
                .ok_or(Error::ArithmeticOverflow)?,
            maximum_low: self.maximum_low.max(later.maximum_low),
            maximum_high: self.maximum_high.max(later.maximum_high),
            minimum_low: self.minimum_low.min(later.minimum_low),
            minimum_high: self.minimum_high.min(later.minimum_high),
            drawdown_low_ppm: self
                .drawdown_low_ppm
                .max(later.drawdown_low_ppm)
                .max(cross_low),
            drawdown_high_ppm: self
                .drawdown_high_ppm
                .max(later.drawdown_high_ppm)
                .max(cross_high),
        };
        value.validate()?;
        Ok(value)
    }

    /// Conservative maximum-drawdown interval.
    pub const fn interval(self) -> DrawdownIntervalV3 {
        DrawdownIntervalV3 {
            low_ppm: self.drawdown_low_ppm,
            high_ppm: self.drawdown_high_ppm,
        }
    }

    /// Inclusive first canonical bucket.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Exclusive end canonical bucket.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }

    /// Number of complete ordered observations represented.
    pub const fn record_count(self) -> u64 {
        self.record_count
    }

    /// Validate fixed summary shape. Original source authentication remains external.
    pub fn validate(&self) -> Result<()> {
        if self.start_bucket >= self.end_bucket_exclusive
            || self.record_count == 0
            || self
                .end_bucket_exclusive
                .checked_sub(self.start_bucket)
                .ok_or(Error::ArithmeticOverflow)?
                != self.record_count
            || self.maximum_low > self.maximum_high
            || self.minimum_low > self.minimum_high
            || self.maximum_high > MAX_SOURCE_VALUE
            || self.minimum_high > MAX_SOURCE_VALUE
            || self.minimum_low > self.maximum_low
            || self.minimum_high > self.maximum_high
            || self.drawdown_low_ppm > self.drawdown_high_ppm
            || self.drawdown_high_ppm > DRAWDOWN_PPM_SCALE
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

impl FixedCodec for DrawdownSummaryV3 {
    const ENCODED_LEN: usize = DRAWDOWN_SUMMARY_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&DRAWDOWN_MAGIC);
        writer.u64(self.start_bucket);
        writer.u64(self.end_bucket_exclusive);
        writer.u64(self.record_count);
        writer.u128(self.maximum_low);
        writer.u128(self.maximum_high);
        writer.u128(self.minimum_low);
        writer.u128(self.minimum_high);
        writer.u64(self.drawdown_low_ppm);
        writer.u64(self.drawdown_high_ppm);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&DRAWDOWN_MAGIC)?;
        let value = Self {
            start_bucket: reader.u64(),
            end_bucket_exclusive: reader.u64(),
            record_count: reader.u64(),
            maximum_low: reader.u128(),
            maximum_high: reader.u128(),
            minimum_low: reader.u128(),
            minimum_high: reader.u128(),
            drawdown_low_ppm: reader.u64(),
            drawdown_high_ppm: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn drawdown_ppm(peak: u128, trough: u128) -> Result<u64> {
    if peak == 0 || peak <= trough {
        return Ok(0);
    }
    let numerator = peak
        .checked_sub(trough)
        .and_then(|difference| difference.checked_mul(u128::from(DRAWDOWN_PPM_SCALE)))
        .ok_or(Error::ArithmeticOverflow)?;
    let quotient = numerator / peak;
    let rounded = quotient
        .checked_add(u128::from(numerator % peak != 0))
        .ok_or(Error::ArithmeticOverflow)?;
    u64::try_from(rounded).map_err(|_| Error::ArithmeticOverflow)
}
