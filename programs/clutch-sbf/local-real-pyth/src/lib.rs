//! Reusable host-only construction boundary for the local real-Pyth lab.
//!
//! The campaign binary remains the evidence runner. This library exposes the
//! same real receiver/source-aware plane to a daemon-owned local session so a
//! later interactive loop does not grow a second serializer or fall back to
//! the mock-source Friday fixture.

mod capture;
pub mod plane;
#[cfg(feature = "campaign")]
pub mod provider;
pub mod session_builder;
