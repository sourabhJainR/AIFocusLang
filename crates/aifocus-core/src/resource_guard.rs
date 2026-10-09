//! Explicit resource budgets for code that needs bounded execution.
//!
//! Reservations are cooperative accounting: callers must reserve before allocating
//! or starting work and release by dropping the returned lease. This module does
//! not intercept arbitrary Rust allocations, OS threads, or blocking system calls.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_tasks: usize,
    pub max_reserved_bytes: usize,
    pub max_operations: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_tasks: 64,
            max_reserved_bytes: 64 * 1024 * 1024,
            max_operations: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResourceSnapshot {
    pub active_tasks: usize,
    pub reserved_bytes: usize,
    pub operations_consumed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Tasks,
    Bytes,
    Operations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimitExceeded {
    pub kind: ResourceKind,
    pub requested: usize,
    pub current: usize,
    pub limit: usize,
}

impl std::fmt::Display for ResourceLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "resource budget exceeded ({:?}): requested {}, current {}, limit {}",
            self.kind, self.requested, self.current, self.limit
        )
    }
}

impl std::error::Error for ResourceLimitExceeded {}

#[derive(Debug, Default)]
struct Counters {
    active_tasks: AtomicUsize,
    reserved_bytes: AtomicUsize,
    operations_consumed: AtomicUsize,
}

#[derive(Debug, Clone)]
pub struct ResourceBudget {
    limits: ResourceLimits,
    counters: Arc<Counters>,
}

impl ResourceBudget {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            counters: Arc::new(Counters::default()),
        }
    }

    pub fn limits(&self) -> ResourceLimits {
        self.limits
    }

    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            active_tasks: self.counters.active_tasks.load(Ordering::Acquire),
            reserved_bytes: self.counters.reserved_bytes.load(Ordering::Acquire),
            operations_consumed: self.counters.operations_consumed.load(Ordering::Acquire),
        }
    }

    pub fn reserve_task(&self) -> Result<ResourceLease, ResourceLimitExceeded> {
        self.reserve(ResourceKind::Tasks, 1, self.limits.max_tasks)
    }

    pub fn reserve_bytes(&self, bytes: usize) -> Result<ResourceLease, ResourceLimitExceeded> {
        self.reserve(ResourceKind::Bytes, bytes, self.limits.max_reserved_bytes)
    }

    /// Consume a cumulative work budget. Unlike task/byte reservations, consumed
    /// operations are not returned when a lease is dropped.
    pub fn consume_operations(&self, operations: usize) -> Result<(), ResourceLimitExceeded> {
        reserve_counter(
            &self.counters.operations_consumed,
            ResourceKind::Operations,
            operations,
            self.limits.max_operations,
        )?;
        Ok(())
    }

    fn reserve(
        &self,
        kind: ResourceKind,
        amount: usize,
        limit: usize,
    ) -> Result<ResourceLease, ResourceLimitExceeded> {
        let counter = match kind {
            ResourceKind::Tasks => &self.counters.active_tasks,
            ResourceKind::Bytes => &self.counters.reserved_bytes,
            ResourceKind::Operations => &self.counters.operations_consumed,
        };
        reserve_counter(counter, kind, amount, limit)?;
        Ok(ResourceLease {
            counters: Arc::clone(&self.counters),
            kind,
            amount,
        })
    }
}

fn reserve_counter(
    counter: &AtomicUsize,
    kind: ResourceKind,
    amount: usize,
    limit: usize,
) -> Result<(), ResourceLimitExceeded> {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(amount) else {
            return Err(ResourceLimitExceeded {
                kind,
                requested: amount,
                current,
                limit,
            });
        };
        if next > limit {
            return Err(ResourceLimitExceeded {
                kind,
                requested: amount,
                current,
                limit,
            });
        }
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

/// RAII reservation. Dropping it releases the reserved task or byte capacity.
#[derive(Debug)]
pub struct ResourceLease {
    counters: Arc<Counters>,
    kind: ResourceKind,
    amount: usize,
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        let counter = match self.kind {
            ResourceKind::Tasks => &self.counters.active_tasks,
            ResourceKind::Bytes => &self.counters.reserved_bytes,
            ResourceKind::Operations => return,
        };
        let previous = counter.fetch_sub(self.amount, Ordering::AcqRel);
        debug_assert!(previous >= self.amount, "resource lease accounting underflow");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ResourceLimits {
        ResourceLimits {
            max_tasks: 2,
            max_reserved_bytes: 128,
            max_operations: 10,
        }
    }

    #[test]
    fn task_and_byte_reservations_are_bounded_and_released() {
        let budget = ResourceBudget::new(limits());
        let task_a = budget.reserve_task().unwrap();
        let task_b = budget.reserve_task().unwrap();
        assert_eq!(budget.snapshot().active_tasks, 2);
        assert_eq!(budget.reserve_task().unwrap_err().kind, ResourceKind::Tasks);
        drop(task_a);
        let _task_c = budget.reserve_task().unwrap();

        let bytes = budget.reserve_bytes(96).unwrap();
        assert_eq!(budget.snapshot().reserved_bytes, 96);
        assert_eq!(budget.reserve_bytes(33).unwrap_err().kind, ResourceKind::Bytes);
        drop(bytes);
        assert_eq!(budget.snapshot().reserved_bytes, 0);
        drop(task_b);
    }

    #[test]
    fn operation_budget_is_cumulative_and_never_wraps() {
        let budget = ResourceBudget::new(limits());
        budget.consume_operations(7).unwrap();
        assert_eq!(budget.consume_operations(4).unwrap_err().kind, ResourceKind::Operations);
        assert_eq!(budget.snapshot().operations_consumed, 7);
    }

    #[test]
    fn cloned_budget_shares_atomic_limits() {
        let budget = ResourceBudget::new(limits());
        let other = budget.clone();
        let _lease = budget.reserve_bytes(80).unwrap();
        assert_eq!(other.snapshot().reserved_bytes, 80);
        assert!(other.reserve_bytes(49).is_err());
    }

    #[test]
    fn zero_cost_reservation_is_safe() {
        let budget = ResourceBudget::new(limits());
        let _lease = budget.reserve_bytes(0).unwrap();
        assert_eq!(budget.snapshot().reserved_bytes, 0);
    }
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::new(ResourceLimits::default())
    }
}
