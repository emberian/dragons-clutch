#![no_std]

//! Proof-facing bounded algebra for the interval-summary prototype.
//!
//! This file is intentionally a small Verus shadow rather than a second
//! executable implementation. The executable source is the semantic owner;
//! these definitions expose the relations that the Verus gate must close.

use vstd::prelude::*;

verus! {
    pub const MAX_BUCKETS: u64 = 1_000_000;
    pub const MAX_VALUE: u128 = 1_000_000_000_000_000_000_000_000;

    pub struct Grid {
        pub family: u32,
        pub version: u16,
        pub bucket_seconds: u64,
    }

    pub struct Summary {
        pub start: u64,
        pub end: u64,
        pub accepted: u64,
        pub missing: u64,
        pub covered_duration: u64,
        pub integral_low: u128,
        pub integral_high: u128,
        pub has_first: bool,
        pub has_last: bool,
        pub first_low: u128,
        pub first_high: u128,
        pub last_low: u128,
        pub last_high: u128,
        pub has_extrema: bool,
        pub min_low: u128,
        pub min_high: u128,
        pub max_low: u128,
        pub max_high: u128,
    }

    pub open spec fn grid_ok(g: Grid) -> bool {
        g.bucket_seconds > 0 && g.bucket_seconds <= 86_400
    }

    pub open spec fn interval_ok(low: u128, high: u128) -> bool {
        low <= high && high <= MAX_VALUE
    }

    pub open spec fn range_ok(s: Summary, g: Grid) -> bool {
        grid_ok(g)
            && s.end > s.start
            && s.end - s.start <= MAX_BUCKETS
            && s.accepted + s.missing == s.end - s.start
            && s.covered_duration == s.accepted * g.bucket_seconds
            && s.integral_low <= s.integral_high
            && (!s.has_first || interval_ok(s.first_low, s.first_high))
            && (!s.has_last || interval_ok(s.last_low, s.last_high))
            && (!s.has_extrema
                || (interval_ok(s.min_low, s.min_high)
                    && interval_ok(s.max_low, s.max_high)
                    && s.min_low <= s.max_low
                    && s.min_high <= s.max_high))
    }

    pub open spec fn adjacent(a: Summary, b: Summary) -> bool {
        a.end == b.start
    }

    pub open spec fn combined_accepted(a: Summary, b: Summary) -> u64 {
        a.accepted + b.accepted
    }

    pub open spec fn combined_missing(a: Summary, b: Summary) -> u64 {
        a.missing + b.missing
    }

    pub open spec fn combined_integral_low(a: Summary, b: Summary) -> u128 {
        a.integral_low + b.integral_low
    }

    pub open spec fn combined_integral_high(a: Summary, b: Summary) -> u128 {
        a.integral_high + b.integral_high
    }

    proof fn coverage_addition_is_associative(a: u64, b: u64, c: u64)
        requires
            a + b + c <= MAX_BUCKETS,
        ensures
            (a + b) + c == a + (b + c),
    {
        assert((a + b) + c == a + (b + c));
    }

    proof fn integral_addition_is_associative(a: u128, b: u128, c: u128)
        requires
            a + b + c <= u128::MAX,
        ensures
            (a + b) + c == a + (b + c),
    {
        assert((a + b) + c == a + (b + c));
    }

    proof fn adjacent_ranges_conserve_buckets(a: Summary, b: Summary, g: Grid)
        requires
            range_ok(a, g),
            range_ok(b, g),
            adjacent(a, b),
            a.end - a.start + (b.end - b.start) <= MAX_BUCKETS,
        ensures
            (a.end - a.start) + (b.end - b.start) == b.end - a.start,
            combined_accepted(a, b) + combined_missing(a, b) == b.end - a.start,
    {
        assert((a.end - a.start) + (b.end - b.start) == b.end - a.start);
        assert(combined_accepted(a, b) + combined_missing(a, b) == b.end - a.start);
    }

    proof fn exact_integrals_associate(a: Summary, b: Summary, c: Summary)
        requires
            a.integral_low + b.integral_low + c.integral_low <= u128::MAX,
            a.integral_high + b.integral_high + c.integral_high <= u128::MAX,
        ensures
            combined_integral_low(a, b) + c.integral_low
                == a.integral_low + combined_integral_low(b, c),
            combined_integral_high(a, b) + c.integral_high
                == a.integral_high + combined_integral_high(b, c),
    {
        assert((a.integral_low + b.integral_low) + c.integral_low
            == a.integral_low + (b.integral_low + c.integral_low));
        assert((a.integral_high + b.integral_high) + c.integral_high
            == a.integral_high + (b.integral_high + c.integral_high));
    }
}

