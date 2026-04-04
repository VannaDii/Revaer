//! Allocation safety helpers for indexer handlers.
//!
//! # Design
//! - Use live system memory data on every platform (via `systemstat`), preferring `MemAvailable` on
//!   Linux for better accuracy and falling back to `memory.free` elsewhere.
//! - Return stable RFC9457 errors with constant messages and contextual fields.
//! - Fail closed when memory availability cannot be determined.

use crate::http::errors::ApiError;
#[cfg(any(target_os = "linux", target_os = "android"))]
use systemstat::ByteSize;
use systemstat::{Memory, Platform, System};

/// Maximum fraction of currently available memory that a single allocation may consume.
///
/// # Why 80%?
/// - Live system memory data reflects pressure from all processes, not just this one.
/// - The limit is applied to the *available* memory figure (for example, Linux
///   `MemAvailable`), which already accounts for reclaimable caches and buffers.
/// - Capping a single allocation at 80% of that available value still leaves
///   headroom for allocator overhead, fragmentation, other processes, and
///   concurrent work in this process so we do not hog resources.
/// - 80% is a conservative, operations-friendly default that reduces the risk of
///   the process or host being OOM-killed while still allowing large allocations.
///
/// # When is this check invoked?
/// [`ensure_allocation_safe`] is intended to be called by indexer HTTP handlers
/// before performing request-dependent allocations (for example, building
/// in-memory buffers sized from request parameters). By funnelling those
/// allocations through this helper, we enforce a single, auditable policy and
/// fail closed when live memory data cannot be obtained.
///
/// If you change this value:
/// - lowering it makes the system more conservative (more requests rejected);
/// - raising it reduces safety margins and should only be done with a clear
///   operational rationale and supporting monitoring.
/// - adjust the build-time constant in this module (`MEMORY_USAGE_LIMIT_PERCENT`).
const MEMORY_USAGE_LIMIT_PERCENT: u64 = 80;
const MEMORY_USAGE_LIMIT_DENOM: u64 = 100;
const MIN_AVAILABLE_BYTES: u64 = 1024 * 1024;
const ALLOCATION_TOO_LARGE: &str = "requested allocation exceeds safe memory limit";
#[cfg(any(target_os = "linux", target_os = "android"))]
const MEMINFO_AVAILABLE_KEY: &str = "MemAvailable";
const MEMINFO_AVAILABLE_SOURCE: &str = "systemstat.mem_available";
const MEMORY_AVAILABLE_UNAVAILABLE: &str = "available memory data unavailable";

pub(super) fn ensure_allocation_safe(requested_bytes: usize) -> Result<(), ApiError> {
    let system = System::new();
    ensure_allocation_safe_with_system(requested_bytes, &system)
}

/// Reuse a cached system snapshot when performing repeated allocation checks.
pub(super) fn ensure_allocation_safe_with_system(
    requested_bytes: usize,
    system: &System,
) -> Result<(), ApiError> {
    let available_bytes = available_memory_bytes_with_system(system)?;
    ensure_allocation_safe_with_available(requested_bytes, available_bytes)
}

/// Allocate a vector with a checked capacity based on live memory data.
pub(super) fn checked_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, ApiError> {
    let available_bytes = available_memory_bytes_with_system(&System::new())?;
    checked_vec_capacity_with_available(capacity, available_bytes)
}

/// Allocate a string with a checked capacity based on live memory data.
pub(super) fn checked_string_capacity(capacity: usize) -> Result<String, ApiError> {
    let available_bytes = available_memory_bytes_with_system(&System::new())?;
    checked_string_capacity_with_available(capacity, available_bytes)
}

fn checked_vec_capacity_with_available<T>(
    capacity: usize,
    available_bytes: u64,
) -> Result<Vec<T>, ApiError> {
    ensure_allocation_safe_with_available(allocation_bytes::<T>(capacity)?, available_bytes)?;
    let mut vec = Vec::new();
    vec.try_reserve_exact(capacity)
        .map_err(|_| allocation_too_large_error())?;
    Ok(vec)
}

fn checked_string_capacity_with_available(
    capacity: usize,
    available_bytes: u64,
) -> Result<String, ApiError> {
    ensure_allocation_safe_with_available(allocation_bytes::<u8>(capacity)?, available_bytes)?;
    let mut value = String::new();
    value
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_too_large_error())?;
    Ok(value)
}

fn allocation_bytes<T>(capacity: usize) -> Result<usize, ApiError> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(allocation_too_large_error)
}

fn ensure_allocation_safe_with_available(
    requested_bytes: usize,
    available_bytes: u64,
) -> Result<(), ApiError> {
    let requested = u64::try_from(requested_bytes).map_err(|_| allocation_too_large_error())?;
    let allowed =
        available_bytes.saturating_mul(MEMORY_USAGE_LIMIT_PERCENT) / MEMORY_USAGE_LIMIT_DENOM;

    if requested > allowed {
        let mut error = allocation_too_large_error();
        error = error.with_context_field("requested_bytes", requested_bytes.to_string());
        error = error.with_context_field("available_bytes", available_bytes.to_string());
        error = error.with_context_field("allowed_bytes", allowed.to_string());
        error = error.with_context_field("limit_percent", MEMORY_USAGE_LIMIT_PERCENT.to_string());
        return Err(error);
    }

    Ok(())
}

fn available_memory_bytes_with_system(system: &System) -> Result<u64, ApiError> {
    let memory = system
        .memory()
        .map_err(|_| memory_probe_error("systemstat.memory"))?;
    if let Some(available) = mem_available_bytes_from_platform(&memory) {
        return ensure_min_available(available, MEMINFO_AVAILABLE_SOURCE);
    }
    let free = memory.free.as_u64();
    ensure_min_available(free, "systemstat.free")
}

fn memory_probe_error(source: &'static str) -> ApiError {
    let mut error = ApiError::service_unavailable(MEMORY_AVAILABLE_UNAVAILABLE);
    error = error.with_context_field("source", source);
    error
}

fn ensure_min_available(available_bytes: u64, source: &'static str) -> Result<u64, ApiError> {
    if available_bytes < MIN_AVAILABLE_BYTES {
        let mut error = memory_probe_error(source);
        error = error.with_context_field("available_bytes", available_bytes.to_string());
        error = error.with_context_field("min_bytes", MIN_AVAILABLE_BYTES.to_string());
        return Err(error);
    }
    Ok(available_bytes)
}

fn allocation_too_large_error() -> ApiError {
    ApiError::service_unavailable(ALLOCATION_TOO_LARGE)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mem_available_bytes_from_platform(memory: &Memory) -> Option<u64> {
    memory
        .platform_memory
        .meminfo
        .get(MEMINFO_AVAILABLE_KEY)
        .map(ByteSize::as_u64)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn mem_available_bytes_from_platform(_memory: &Memory) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_within_limit_is_ok() {
        let result = ensure_allocation_safe_with_available(80, 200);
        assert!(result.is_ok());
    }

    #[test]
    fn allocation_over_limit_returns_error() {
        let result = ensure_allocation_safe_with_available(81, 100);
        assert!(result.is_err());
    }

    #[test]
    fn allocation_at_limit_is_ok() {
        let result = ensure_allocation_safe_with_available(80, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn checked_vec_capacity_allocates() {
        let values = checked_vec_capacity_with_available::<u8>(1, MIN_AVAILABLE_BYTES)
            .expect("reserve vec capacity");
        assert!(values.capacity() >= 1);
    }

    #[test]
    fn checked_string_capacity_allocates() {
        let value = checked_string_capacity_with_available(8, MIN_AVAILABLE_BYTES)
            .expect("reserve string capacity");
        assert!(value.capacity() >= 8);
    }
}
