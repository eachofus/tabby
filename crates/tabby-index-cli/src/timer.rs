// === IMPORTS ===
use std::time::Instant;

use serde::Serialize;

// === STRUCTS ===
/// Timer that measures execution time of a named task
/// 
/// The timer automatically stops and records the duration when dropped.
/// Can be used to create nested timing measurements.
pub struct OpenTimer<'a> {
    name: &'static str,
    timer_tree: &'a mut TimerTree,
    start: Instant,
    depth: u32,
}

/// Single timing measurement record
#[derive(Debug, Serialize)]
pub struct Timing {
    name: &'static str,
    duration: i64,
    depth: u32,
}

/// Collection of timing measurements organized in a tree structure
#[derive(Debug, Serialize, Default)]
pub struct TimerTree {
    timings: Vec<Timing>,
}

// === IMPLEMENTATIONS ===
impl OpenTimer<'_> {
    /// Starts timing a new named subtask
    ///
    /// The timer is stopped automatically when the `OpenTimer` is dropped.
    /// Creates a nested timer with increased depth for hierarchical timing.
    #[allow(dead_code)] // Method used for nested timing but not currently called in tests
    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
        OpenTimer {
            name,
            timer_tree: self.timer_tree,
            start: Instant::now(),
            depth: self.depth + 1,
        }
    }
}

impl Drop for OpenTimer<'_> {
    fn drop(&mut self) {
        self.timer_tree.timings.push(Timing {
            name: self.name,
            duration: self.start.elapsed().as_micros() as i64,
            depth: self.depth,
        });
    }
}

impl TimerTree {
    /// Returns the total time elapsed in microseconds
    /// 
    /// Gets the duration from the last (root) timing measurement
    pub fn total_time(&self) -> i64 {
        self.timings.last().unwrap().duration
    }

    /// Open a new named subtask timer
    /// 
    /// Creates a root-level timer (depth 0) that will automatically
    /// record its duration when dropped.
    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
        OpenTimer {
            name,
            timer_tree: self,
            start: Instant::now(),
            depth: 0,
        }
    }
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer() {
        let mut timer_tree = TimerTree::default();
        {
            let mut a = timer_tree.open("a");
            {
                let mut ab = a.open("b");
                {
                    let _abc = ab.open("c");
                }
                {
                    let _abd = ab.open("d");
                }
            }
        }
        assert_eq!(timer_tree.timings.len(), 4);
    }
}
