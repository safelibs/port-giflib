#![deny(unsafe_op_in_unsafe_fn)]

mod bootstrap;
mod draw;
mod encode;
mod error;
mod ffi;
mod gcb;
mod hash;
mod helpers;
mod io;
mod memory;
mod quantize;
mod state;

const _: bool = bootstrap::LEGACY_BACKEND_ENABLED;
