#![deny(unsafe_op_in_unsafe_fn)]

mod bootstrap;

const _: bool = bootstrap::LEGACY_BACKEND_ENABLED;
