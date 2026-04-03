use std::panic::{catch_unwind, AssertUnwindSafe};

pub(crate) fn catch_panic_or<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}
