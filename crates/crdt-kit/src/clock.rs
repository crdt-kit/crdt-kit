//! Hybrid Logical Clock (HLC) for causal ordering.
//!
//! HLC combines physical time with a logical counter to provide:
//! - **Monotonic timestamps** even when the physical clock goes backward
//! - **Causal ordering** without full vector clocks
//! - **Fixed size** (12 bytes) regardless of the number of nodes
//!
//! This is ideal for edge/IoT where clocks are not perfectly synchronized.
//!
//! # Example
//!
//! ```
//! use crdt_kit::clock::{HybridClock, HybridTimestamp};
//!
//! let mut clock = HybridClock::new(1); // node_id = 1
//!
//! // Generate a timestamp for a local event
//! let ts1 = clock.now();
//! let ts2 = clock.now();
//! assert!(ts2 > ts1);
//!
//! // Receive a timestamp from a remote node
//! let remote_ts = HybridTimestamp { physical: ts2.physical + 1000, logical: 0, node_id: 2 };
//! let ts3 = clock.receive(&remote_ts);
//! assert!(ts3 > remote_ts);
//! ```

use core::cmp;

/// A timestamp from a Hybrid Logical Clock.
///
/// Consists of:
/// - `physical`: milliseconds since Unix epoch (or any monotonic source)
/// - `logical`: counter for events within the same physical millisecond
/// - `node_id`: tiebreaker to ensure total ordering across nodes
///
/// Total size: 12 bytes (u64 + u16 + u16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HybridTimestamp {
    /// Physical time component (milliseconds).
    pub physical: u64,
    /// Logical counter for same-millisecond ordering.
    pub logical: u16,
    /// Node identifier for deterministic tiebreaking.
    pub node_id: u16,
}

impl HybridTimestamp {
    /// Create a zero timestamp.
    pub fn zero() -> Self {
        Self {
            physical: 0,
            logical: 0,
            node_id: 0,
        }
    }

    /// Pack into a u128 for efficient comparison and storage.
    /// Layout: [physical: 64 bits][logical: 16 bits][node_id: 16 bits][reserved: 32 bits]
    pub fn to_u128(&self) -> u128 {
        ((self.physical as u128) << 64)
            | ((self.logical as u128) << 48)
            | ((self.node_id as u128) << 32)
    }
}

impl Ord for HybridTimestamp {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.physical
            .cmp(&other.physical)
            .then(self.logical.cmp(&other.logical))
            .then(self.node_id.cmp(&other.node_id))
    }
}

impl PartialOrd for HybridTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A Hybrid Logical Clock instance for a single node.
///
/// Call [`now`](HybridClock::now) to generate timestamps for local events.
/// Call [`receive`](HybridClock::receive) when processing a remote timestamp.
pub struct HybridClock {
    node_id: u16,
    last: HybridTimestamp,
    /// Function to get the current physical time in milliseconds.
    /// On `std`, this defaults to `SystemTime`. On `no_std`, you provide it.
    physical_time_fn: fn() -> u64,
}

#[cfg(feature = "std")]
fn system_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(not(feature = "std"))]
fn fallback_time_ms() -> u64 {
    0 // In no_std, user must provide a time source
}

impl HybridClock {
    /// Create a new clock for the given node.
    ///
    /// On `std` targets, uses `SystemTime` for physical time.
    /// On `no_std` targets, use [`with_time_source`](Self::with_time_source).
    pub fn new(node_id: u16) -> Self {
        Self {
            node_id,
            last: HybridTimestamp::zero(),
            #[cfg(feature = "std")]
            physical_time_fn: system_time_ms,
            #[cfg(not(feature = "std"))]
            physical_time_fn: fallback_time_ms,
        }
    }

    /// Create a clock with a custom physical time source.
    /// The function should return milliseconds (monotonic if possible).
    pub fn with_time_source(node_id: u16, time_fn: fn() -> u64) -> Self {
        Self {
            node_id,
            last: HybridTimestamp::zero(),
            physical_time_fn: time_fn,
        }
    }

    /// Generate a timestamp for a local event.
    ///
    /// Guarantees monotonically increasing timestamps even if the
    /// physical clock goes backward.
    pub fn now(&mut self) -> HybridTimestamp {
        let pt = (self.physical_time_fn)();

        if pt > self.last.physical {
            self.last = HybridTimestamp {
                physical: pt,
                logical: 0,
                node_id: self.node_id,
            };
        } else {
            self.last = HybridTimestamp {
                physical: self.last.physical,
                logical: self.last.logical + 1,
                node_id: self.node_id,
            };
        }

        self.last
    }

    /// Update the clock upon receiving a remote timestamp.
    ///
    /// Returns a new timestamp that is strictly greater than both
    /// the local clock and the received timestamp.
    pub fn receive(&mut self, remote: &HybridTimestamp) -> HybridTimestamp {
        let pt = (self.physical_time_fn)();
        let max_pt = cmp::max(cmp::max(pt, self.last.physical), remote.physical);

        let logical = if max_pt == self.last.physical && max_pt == remote.physical {
            cmp::max(self.last.logical, remote.logical) + 1
        } else if max_pt == self.last.physical {
            self.last.logical + 1
        } else if max_pt == remote.physical {
            remote.logical + 1
        } else {
            0
        };

        self.last = HybridTimestamp {
            physical: max_pt,
            logical,
            node_id: self.node_id,
        };

        self.last
    }

    /// Get the node ID of this clock.
    pub fn node_id(&self) -> u16 {
        self.node_id
    }

    /// Get the last generated timestamp.
    pub fn last_timestamp(&self) -> HybridTimestamp {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};

    static MOCK_TIME: AtomicU64 = AtomicU64::new(1000);

    fn mock_time() -> u64 {
        MOCK_TIME.load(Ordering::SeqCst)
    }

    fn set_mock_time(ms: u64) {
        MOCK_TIME.store(ms, Ordering::SeqCst);
    }

    #[test]
    fn monotonic_within_same_ms() {
        set_mock_time(5000);
        let mut clock = HybridClock::with_time_source(1, mock_time);

        let ts1 = clock.now();
        let ts2 = clock.now();
        let ts3 = clock.now();

        assert!(ts1 < ts2);
        assert!(ts2 < ts3);
        assert_eq!(ts1.physical, 5000);
        assert_eq!(ts1.logical, 0);
        assert_eq!(ts2.logical, 1);
        assert_eq!(ts3.logical, 2);
    }

    #[test]
    fn physical_time_advance_resets_logical() {
        set_mock_time(1000);
        let mut clock = HybridClock::with_time_source(1, mock_time);

        let ts1 = clock.now();
        assert_eq!(ts1.logical, 0);

        let _ts2 = clock.now();

        set_mock_time(2000);
        let ts3 = clock.now();
        assert_eq!(ts3.physical, 2000);
        assert_eq!(ts3.logical, 0);
    }

    #[test]
    fn receive_advances_clock() {
        set_mock_time(1000);
        let mut clock = HybridClock::with_time_source(1, mock_time);

        // Remote is ahead
        let remote = HybridTimestamp {
            physical: 5000,
            logical: 3,
            node_id: 2,
        };

        let ts = clock.receive(&remote);
        assert!(ts > remote);
        assert_eq!(ts.physical, 5000);
        assert_eq!(ts.logical, 4); // remote.logical + 1
    }

    #[test]
    fn receive_same_physical_time() {
        set_mock_time(5000);
        let mut clock = HybridClock::with_time_source(1, mock_time);

        let _local = clock.now(); // physical=5000, logical=0

        let remote = HybridTimestamp {
            physical: 5000,
            logical: 5,
            node_id: 2,
        };

        let ts = clock.receive(&remote);
        assert!(ts > remote);
        assert_eq!(ts.physical, 5000);
        assert_eq!(ts.logical, 6); // max(0, 5) + 1
    }

    #[test]
    fn ordering_is_total() {
        let a = HybridTimestamp {
            physical: 1000,
            logical: 0,
            node_id: 1,
        };
        let b = HybridTimestamp {
            physical: 1000,
            logical: 0,
            node_id: 2,
        };
        let c = HybridTimestamp {
            physical: 1000,
            logical: 1,
            node_id: 1,
        };

        assert!(a < b); // same physical+logical, node_id tiebreak
        assert!(a < c); // same physical, logical tiebreak
        assert!(b < c); // logical > node_id in precedence
    }

    #[test]
    fn to_u128_preserves_ordering() {
        let a = HybridTimestamp {
            physical: 1000,
            logical: 5,
            node_id: 1,
        };
        let b = HybridTimestamp {
            physical: 1000,
            logical: 6,
            node_id: 1,
        };
        let c = HybridTimestamp {
            physical: 1001,
            logical: 0,
            node_id: 1,
        };

        assert!(a.to_u128() < b.to_u128());
        assert!(b.to_u128() < c.to_u128());
    }
}
