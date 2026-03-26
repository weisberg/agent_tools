use bashli_core::{BudgetAllocation, OverflowStrategy, TokenBudget};

use crate::allocator::allocate_for_step;
use crate::truncator::{estimate_tokens, smart_truncate};

/// Result of attempting to charge output against the budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetResult {
    /// Output accepted as-is.
    Accepted(String),
    /// Output was truncated to fit.
    Truncated { output: String, lines_dropped: usize },
    /// Output dropped entirely (metadata only).
    Dropped,
    /// Budget exhausted, abort remaining steps.
    Abort,
}

/// Tracks token budget consumption across pipeline steps.
pub struct BudgetTracker {
    total: usize,
    consumed: usize,
    allocation: BudgetAllocation,
    overflow: OverflowStrategy,
    step_count: usize,
}

impl BudgetTracker {
    /// Create a new tracker from a `TokenBudget` configuration and the number
    /// of steps in the pipeline.
    pub fn new(budget: &TokenBudget, step_count: usize) -> Self {
        Self {
            total: budget.max_tokens,
            consumed: 0,
            allocation: budget.allocation.clone(),
            overflow: budget.overflow.clone(),
            step_count,
        }
    }

    /// Create an unlimited tracker (for when no budget is configured).
    /// All outputs are accepted without limit.
    pub fn unlimited() -> Self {
        Self {
            total: usize::MAX,
            consumed: 0,
            allocation: BudgetAllocation::Equal,
            overflow: OverflowStrategy::Truncate,
            step_count: 1,
        }
    }

    /// Attempt to charge `output` against the budget for `step_index`.
    ///
    /// Returns a `BudgetResult` indicating whether the output was accepted,
    /// truncated, dropped, or whether the pipeline should abort.
    pub fn charge(&mut self, step_index: usize, output: &str) -> BudgetResult {
        // Unlimited tracker: always accept
        if self.total == usize::MAX {
            return BudgetResult::Accepted(output.to_string());
        }

        let tokens = estimate_tokens(output);
        let step_budget = self.allocation_for_step(step_index);

        // Also consider overall remaining budget
        let remaining = self.remaining();
        let effective_budget = step_budget.min(remaining);

        if tokens <= effective_budget {
            // Fits within budget
            self.consumed += tokens;
            return BudgetResult::Accepted(output.to_string());
        }

        // Over budget — apply overflow strategy
        match &self.overflow {
            OverflowStrategy::Truncate => {
                if effective_budget == 0 {
                    // No budget left at all
                    return BudgetResult::Dropped;
                }
                // Convert token budget to approximate line count.
                // Average ~10 tokens per line as a rough estimate, but we also
                // do iterative fitting: figure out max_lines that keeps us
                // under budget.
                let max_lines = self.tokens_to_max_lines(output, effective_budget);
                if max_lines == 0 {
                    self.consumed += 0;
                    return BudgetResult::Dropped;
                }
                let (truncated, dropped) = smart_truncate(output, max_lines);
                let actual_tokens = estimate_tokens(&truncated);
                self.consumed += actual_tokens;
                if dropped > 0 {
                    BudgetResult::Truncated {
                        output: truncated,
                        lines_dropped: dropped,
                    }
                } else {
                    BudgetResult::Accepted(truncated)
                }
            }
            OverflowStrategy::MetadataOnly => {
                // Drop the output entirely
                BudgetResult::Dropped
            }
            OverflowStrategy::Abort => BudgetResult::Abort,
        }
    }

    /// Return the number of tokens remaining in the total budget.
    pub fn remaining(&self) -> usize {
        self.total.saturating_sub(self.consumed)
    }

    /// Return whether the entire budget has been exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.consumed >= self.total
    }

    /// Return the token allocation for a given step index.
    pub fn allocation_for_step(&self, step_index: usize) -> usize {
        allocate_for_step(self.total, self.step_count, step_index, &self.allocation)
    }

    /// Binary search for the maximum number of lines that fit within a token budget.
    fn tokens_to_max_lines(&self, output: &str, token_budget: usize) -> usize {
        let total_lines = output.lines().count();
        if total_lines == 0 {
            return 0;
        }

        // Binary search: find the largest max_lines where
        // estimate_tokens(smart_truncate(output, max_lines)) <= token_budget
        let mut lo: usize = 0;
        let mut hi: usize = total_lines;

        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let (truncated, _) = smart_truncate(output, mid);
            if estimate_tokens(&truncated) <= token_budget {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }

        lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_budget(max_tokens: usize, overflow: OverflowStrategy) -> TokenBudget {
        TokenBudget {
            max_tokens,
            allocation: BudgetAllocation::Equal,
            overflow,
        }
    }

    #[test]
    fn test_unlimited_always_accepts() {
        let mut tracker = BudgetTracker::unlimited();
        let big_output = "x".repeat(100_000);
        match tracker.charge(0, &big_output) {
            BudgetResult::Accepted(s) => assert_eq!(s, big_output),
            other => panic!("Expected Accepted, got {:?}", other),
        }
        assert!(!tracker.is_exhausted());
    }

    #[test]
    fn test_accepted_within_budget() {
        let budget = make_budget(1000, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 2);

        // "hello" = 5 chars -> ~2 tokens, well within 500 per-step budget
        match tracker.charge(0, "hello") {
            BudgetResult::Accepted(s) => assert_eq!(s, "hello"),
            other => panic!("Expected Accepted, got {:?}", other),
        }
        assert_eq!(tracker.remaining(), 1000 - 2);
    }

    #[test]
    fn test_truncate_over_budget() {
        // Very small budget to force truncation
        let budget = make_budget(10, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 1);

        // 20 lines of text, each ~3 tokens -> way over 10 token budget
        let lines: Vec<String> = (0..20).map(|i| format!("line number {}", i)).collect();
        let output = lines.join("\n");

        match tracker.charge(0, &output) {
            BudgetResult::Truncated {
                lines_dropped,
                output: truncated,
            } => {
                assert!(lines_dropped > 0);
                assert!(estimate_tokens(&truncated) <= 10);
            }
            BudgetResult::Dropped => {
                // Also acceptable if budget is so small nothing fits
            }
            other => panic!("Expected Truncated or Dropped, got {:?}", other),
        }
    }

    #[test]
    fn test_metadata_only_drops() {
        let budget = make_budget(5, OverflowStrategy::MetadataOnly);
        let mut tracker = BudgetTracker::new(&budget, 1);

        let output = "a very long output that exceeds the tiny budget easily";
        match tracker.charge(0, output) {
            BudgetResult::Dropped => {}
            other => panic!("Expected Dropped, got {:?}", other),
        }
    }

    #[test]
    fn test_abort_strategy() {
        let budget = make_budget(5, OverflowStrategy::Abort);
        let mut tracker = BudgetTracker::new(&budget, 1);

        let output = "a very long output that exceeds the tiny budget easily";
        match tracker.charge(0, output) {
            BudgetResult::Abort => {}
            other => panic!("Expected Abort, got {:?}", other),
        }
    }

    #[test]
    fn test_is_exhausted() {
        let budget = make_budget(2, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 1);
        assert!(!tracker.is_exhausted());

        // "abcdefgh" = 8 chars -> 2 tokens, exactly the budget
        tracker.charge(0, "abcdefgh");
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_remaining_tracks_consumption() {
        let budget = make_budget(100, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 2);
        assert_eq!(tracker.remaining(), 100);

        // "abcd" = 4 chars -> 1 token
        tracker.charge(0, "abcd");
        assert_eq!(tracker.remaining(), 99);
    }

    #[test]
    fn test_allocation_for_step_equal() {
        let budget = TokenBudget {
            max_tokens: 100,
            allocation: BudgetAllocation::Equal,
            overflow: OverflowStrategy::Truncate,
        };
        let tracker = BudgetTracker::new(&budget, 4);
        assert_eq!(tracker.allocation_for_step(0), 25);
        assert_eq!(tracker.allocation_for_step(3), 25);
    }

    #[test]
    fn test_allocation_for_step_front_weighted() {
        let budget = TokenBudget {
            max_tokens: 100,
            allocation: BudgetAllocation::FrontWeighted,
            overflow: OverflowStrategy::Truncate,
        };
        let tracker = BudgetTracker::new(&budget, 3);
        let a0 = tracker.allocation_for_step(0);
        let a2 = tracker.allocation_for_step(2);
        assert!(a0 > a2, "Front step should get more than back step");
    }

    #[test]
    fn test_multiple_steps_consume_budget() {
        let budget = make_budget(20, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 3);

        // Each "abcdefgh" is 2 tokens
        tracker.charge(0, "abcdefgh");
        tracker.charge(1, "abcdefgh");
        assert_eq!(tracker.remaining(), 16);
    }

    #[test]
    fn test_dropped_when_zero_effective_budget() {
        let budget = make_budget(2, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 1);

        // Exhaust the budget
        tracker.charge(0, "abcdefgh"); // 2 tokens

        // Now nothing is left
        match tracker.charge(0, "more stuff") {
            BudgetResult::Dropped => {}
            other => panic!("Expected Dropped, got {:?}", other),
        }
    }

    #[test]
    fn test_charge_empty_string() {
        let budget = make_budget(100, OverflowStrategy::Truncate);
        let mut tracker = BudgetTracker::new(&budget, 1);
        match tracker.charge(0, "") {
            BudgetResult::Accepted(s) => assert_eq!(s, ""),
            other => panic!("Expected Accepted, got {:?}", other),
        }
        assert_eq!(tracker.remaining(), 100);
    }
}
