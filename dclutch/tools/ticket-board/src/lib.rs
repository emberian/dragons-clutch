#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The dClutch ticket board: rung (b) of the transport ladder.
//!
//! A small relay that accepts Direct intent tickets, holds them, and lists them
//! back, so that step ③ of the trade flow can offer a reader something better
//! than "obtain a signed 4096-byte blob out of band and paste it here".
//!
//! IT HOLDS NO KEYS, TAKES NO CUSTODY, AND HAS NO AUTHORITY. A Direct ticket is
//! bearer-signed self-authenticating data: the chain re-derives the signing
//! message and verifies it natively at execution, so a relay can withhold and a
//! relay can never forge. Losing this service loses availability and nothing
//! else — every offer it held is an artifact its maker still has.
//!
//! The library half exists so the routing and every named refusal can be tested
//! without binding a socket. The binary is the socket.

pub mod board;
pub mod http;
pub mod snapshot;
