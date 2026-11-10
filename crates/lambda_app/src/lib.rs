//! Lambda Modbus GPUI application library.
//!
//! - Always available: German strings, shell state mapping, mock/live session.
//! - Feature `gui`: GPUI window bootstrap ([`ui`]).

#![forbid(unsafe_code)]

pub mod persist;
pub mod session;
pub mod state;
pub mod strings;

#[cfg(feature = "gui")]
pub mod ui;
