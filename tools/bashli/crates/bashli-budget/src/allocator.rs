use bashli_core::BudgetAllocation;

/// Compute the token allocation for a specific step given the total budget,
/// number of steps, and the allocation strategy.
///
/// Returns the number of tokens allocated to `step_index`.
pub fn allocate_for_step(
    total: usize,
    step_count: usize,
    step_index: usize,
    allocation: &BudgetAllocation,
) -> usize {
    if step_count == 0 {
        return 0;
    }
    if step_index >= step_count {
        return 0;
    }

    match allocation {
        BudgetAllocation::Equal => equal_allocation(total, step_count),
        BudgetAllocation::FrontWeighted => front_weighted(total, step_count, step_index),
        BudgetAllocation::BackWeighted => back_weighted(total, step_count, step_index),
        BudgetAllocation::Weighted(weights) => {
            weighted_allocation(total, step_count, step_index, weights)
        }
    }
}

/// Equal: total / step_count per step (remainder distributed to early steps).
fn equal_allocation(total: usize, step_count: usize) -> usize {
    total / step_count
}

/// FrontWeighted: earlier steps get proportionally more budget.
///
/// Uses linear weights: step 0 gets weight `step_count`, step 1 gets
/// `step_count - 1`, etc. The last step gets weight 1.
fn front_weighted(total: usize, step_count: usize, step_index: usize) -> usize {
    let weight = step_count - step_index;
    let total_weight: usize = (1..=step_count).sum();
    (total as f64 * weight as f64 / total_weight as f64).round() as usize
}

/// BackWeighted: later steps get proportionally more budget.
///
/// Uses linear weights: step 0 gets weight 1, step 1 gets weight 2, etc.
fn back_weighted(total: usize, step_count: usize, step_index: usize) -> usize {
    let weight = step_index + 1;
    let total_weight: usize = (1..=step_count).sum();
    (total as f64 * weight as f64 / total_weight as f64).round() as usize
}

/// Weighted: custom per-step weights. If the weights vector is shorter than
/// step_count, missing entries default to 1.0.
fn weighted_allocation(
    total: usize,
    step_count: usize,
    step_index: usize,
    weights: &[f64],
) -> usize {
    let mut effective_weights: Vec<f64> = Vec::with_capacity(step_count);
    for i in 0..step_count {
        let w = weights.get(i).copied().unwrap_or(1.0);
        // Clamp negative weights to 0
        effective_weights.push(if w > 0.0 { w } else { 0.0 });
    }

    let total_weight: f64 = effective_weights.iter().sum();
    if total_weight <= 0.0 {
        return 0;
    }

    (total as f64 * effective_weights[step_index] / total_weight).round() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_allocation() {
        assert_eq!(allocate_for_step(100, 4, 0, &BudgetAllocation::Equal), 25);
        assert_eq!(allocate_for_step(100, 4, 3, &BudgetAllocation::Equal), 25);
    }

    #[test]
    fn test_equal_allocation_remainder() {
        // 100 / 3 = 33 per step
        assert_eq!(allocate_for_step(100, 3, 0, &BudgetAllocation::Equal), 33);
    }

    #[test]
    fn test_front_weighted() {
        // 3 steps: weights 3, 2, 1 -> total_weight 6
        // step 0: 100 * 3/6 = 50
        // step 1: 100 * 2/6 = 33
        // step 2: 100 * 1/6 = 17
        let alloc = BudgetAllocation::FrontWeighted;
        assert_eq!(allocate_for_step(100, 3, 0, &alloc), 50);
        assert_eq!(allocate_for_step(100, 3, 1, &alloc), 33);
        assert_eq!(allocate_for_step(100, 3, 2, &alloc), 17);
    }

    #[test]
    fn test_back_weighted() {
        // 3 steps: weights 1, 2, 3 -> total_weight 6
        // step 0: 100 * 1/6 = 17
        // step 1: 100 * 2/6 = 33
        // step 2: 100 * 3/6 = 50
        let alloc = BudgetAllocation::BackWeighted;
        assert_eq!(allocate_for_step(100, 3, 0, &alloc), 17);
        assert_eq!(allocate_for_step(100, 3, 1, &alloc), 33);
        assert_eq!(allocate_for_step(100, 3, 2, &alloc), 50);
    }

    #[test]
    fn test_custom_weighted() {
        // weights [2.0, 1.0, 1.0] -> total 4.0
        // step 0: 100 * 2/4 = 50
        // step 1: 100 * 1/4 = 25
        // step 2: 100 * 1/4 = 25
        let alloc = BudgetAllocation::Weighted(vec![2.0, 1.0, 1.0]);
        assert_eq!(allocate_for_step(100, 3, 0, &alloc), 50);
        assert_eq!(allocate_for_step(100, 3, 1, &alloc), 25);
        assert_eq!(allocate_for_step(100, 3, 2, &alloc), 25);
    }

    #[test]
    fn test_weighted_missing_entries_default_to_one() {
        // weights [3.0] for 3 steps -> effective [3.0, 1.0, 1.0] -> total 5.0
        let alloc = BudgetAllocation::Weighted(vec![3.0]);
        assert_eq!(allocate_for_step(100, 3, 0, &alloc), 60);
        assert_eq!(allocate_for_step(100, 3, 1, &alloc), 20);
        assert_eq!(allocate_for_step(100, 3, 2, &alloc), 20);
    }

    #[test]
    fn test_zero_steps() {
        assert_eq!(allocate_for_step(100, 0, 0, &BudgetAllocation::Equal), 0);
    }

    #[test]
    fn test_out_of_bounds_index() {
        assert_eq!(allocate_for_step(100, 3, 5, &BudgetAllocation::Equal), 0);
    }

    #[test]
    fn test_negative_weight_clamped() {
        let alloc = BudgetAllocation::Weighted(vec![-1.0, 2.0]);
        // effective [0.0, 2.0] -> total 2.0
        assert_eq!(allocate_for_step(100, 2, 0, &alloc), 0);
        assert_eq!(allocate_for_step(100, 2, 1, &alloc), 100);
    }

    #[test]
    fn test_single_step() {
        assert_eq!(allocate_for_step(100, 1, 0, &BudgetAllocation::Equal), 100);
        assert_eq!(
            allocate_for_step(100, 1, 0, &BudgetAllocation::FrontWeighted),
            100
        );
        assert_eq!(
            allocate_for_step(100, 1, 0, &BudgetAllocation::BackWeighted),
            100
        );
    }
}
