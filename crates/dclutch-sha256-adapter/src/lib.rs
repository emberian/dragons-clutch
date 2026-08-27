#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The one named adapter between first-party SHA-256 call sites and whatever
//! actually computes the digest.
//!
//! # Why this crate exists
//!
//! SHA-256 is a pure function of its preimage, so *which* implementation
//! produces a digest is invisible to the protocol: the value is the value. It
//! is emphatically not invisible to a program's compute budget. A software
//! SHA-256 compiled into an SBF ELF costs roughly **104.75 CU per byte** —
//! one straight-line compression loop per 64-byte block — while the runtime's
//! `sol_sha256` syscall costs **85 CU plus about half a CU per byte**, a
//! ~210x cheaper margin. Measured on this tree, a single digest of one
//! 4,288-byte verified candidate was 456,008 CU in software against 2,234 CU
//! through the syscall: 86% of the whole `Consider` action.
//!
//! The software implementation was never a decision. It arrived because the
//! contract and kernel crates are deliberately SDK-free — a kernel that cannot
//! name the Solana SDK cannot name the syscall either — so every crate that
//! needed a digest reached for the one hasher available to a `no_std` library.
//! This crate is the named adapter that policy always intended: it is the only
//! place in the tree that knows a runtime exists, and it keeps the SDK out of
//! the crates that depend on it.
//!
//! # The seam
//!
//! Neither backend is an unconditional dependency:
//!
//! - On `target_os = "solana"` the digest is the `sol_sha256` syscall, reached
//!   through `solana-sha256-hasher` with its `sha2` feature off, so no
//!   software SHA-256 is linked into any shipped ELF.
//! - Off the Solana target — host tests, emitters, the release tool, the
//!   program-test harnesses — there is no runtime to ask, so the digest is
//!   `sha2`, and no Solana SDK crate enters the build.
//!
//! Both branches compute SHA-256 of the same preimage, so both return the same
//! bytes. A digest committed by a host emitter and re-derived on chain agrees,
//! which is exactly the property every stored `ContentId` in this tree relies
//! on.
//!
//! # Streaming is not offered on purpose
//!
//! The runtime primitive is one-shot over a slice list: it hashes the
//! concatenation of the slices it is handed. There is no syscall that resumes a
//! partially absorbed state, and `solana_sha256_hasher::Hasher` — the
//! incremental type — is *software even on chain*, which is precisely the trap
//! this crate exists to close. So the API here is [`digest`] and [`digestv`],
//! and a caller that used to stream `update` calls restates its preimage as the
//! slice list it always was. That restatement is mechanical and preserves the
//! digest exactly, because SHA-256 of a concatenation does not care where the
//! caller drew the boundaries.
//!
//! Callers with small scalar fields in the preimage should encode them into a
//! stack buffer that outlives the [`digestv`] call and pass one slice over it;
//! every slice carries a small floor cost in the runtime's accounting, so
//! fewer and larger slices are cheaper than many tiny ones.

/// Bytes in a SHA-256 digest.
pub const DIGEST_BYTES: usize = 32;

/// SHA-256 of the concatenation of `slices`, in the order given.
///
/// This is exactly the digest a streaming hasher would produce from one
/// `update` per slice in the same order: the slice boundaries are not part of
/// the preimage. A caller that needs boundaries to be committed must put them
/// in the preimage itself, as the length-prefixing digest conventions in this
/// tree do.
///
/// An empty slice list yields SHA-256 of the empty string.
#[must_use]
pub fn digestv(slices: &[&[u8]]) -> [u8; DIGEST_BYTES] {
    #[cfg(target_os = "solana")]
    {
        solana_sha256_hasher::hashv(slices).to_bytes()
    }
    #[cfg(not(target_os = "solana"))]
    {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        for slice in slices {
            hasher.update(slice);
        }
        hasher.finalize().into()
    }
}

/// SHA-256 of exactly `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> [u8; DIGEST_BYTES] {
    digestv(&[bytes])
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// The published SHA-256 of the empty string.
    const EMPTY: [u8; DIGEST_BYTES] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];

    /// The published SHA-256 of `abc`.
    const ABC: [u8; DIGEST_BYTES] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    #[test]
    fn the_empty_preimage_is_the_published_vector() {
        assert_eq!(digest(&[]), EMPTY);
        assert_eq!(digestv(&[]), EMPTY);
        assert_eq!(digestv(&[&[], &[], &[]]), EMPTY);
    }

    #[test]
    fn the_published_short_vector_reproduces() {
        assert_eq!(digest(b"abc"), ABC);
    }

    #[test]
    fn slice_boundaries_are_not_part_of_the_preimage() {
        assert_eq!(digestv(&[b"a", b"bc"]), ABC);
        assert_eq!(digestv(&[b"ab", b"c"]), ABC);
        assert_eq!(digestv(&[b"a", b"b", b"c"]), ABC);
        assert_eq!(digestv(&[&[], b"abc", &[]]), ABC);
    }

    #[test]
    fn a_multiblock_preimage_splits_anywhere_without_moving_the_digest() {
        let long = [0x5a_u8; 1000];
        let whole = digest(&long);
        for split in [0_usize, 1, 55, 63, 64, 65, 128, 511, 999, 1000] {
            let (head, tail) = long.split_at(split);
            assert_eq!(digestv(&[head, tail]), whole, "split at {split}");
        }
    }
}
