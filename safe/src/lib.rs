#![deny(unsafe_op_in_unsafe_fn)]

mod bootstrap;
mod draw;
mod error;
mod ffi;
mod hash;
mod helpers;
mod memory;
mod quantize;

const _: bool = bootstrap::LEGACY_BACKEND_ENABLED;
