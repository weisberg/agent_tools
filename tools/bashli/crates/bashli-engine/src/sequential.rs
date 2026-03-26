use bashli_core::*;
use bashli_steps::{StepContext, StepRegistry};

/// Run steps sequentially, stopping on first failure.
pub(crate) async fn run_sequential(
    steps: &[Step],
    registry: &StepRegistry,
    ctx: &mut StepContext<'_>,
) -> (Vec<StepResult>, Option<(usize, ExecError)>) {
    let mut results = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        ctx.vars.set("$_STEP_INDEX", serde_json::Value::Number(i.into()));

        let executor = match registry.resolve(step) {
            Ok(e) => e,
            Err(err) => {
                return (results, Some((i, err)));
            }
        };

        match executor.execute(i, ctx).await {
            Ok(result) => {
                let failed = result.exit_code.map(|c| c != 0).unwrap_or(false);
                let is_skip_rest = result.note.as_ref()
                    .map(|n| n.starts_with("skip_rest:"))
                    .unwrap_or(false);

                results.push(result);

                if failed {
                    // Sequential mode: stop on first failure
                    let exit_code = results.last().unwrap().exit_code.unwrap_or(1);
                    return (results, Some((i, ExecError::NonZeroExit(exit_code))));
                }

                if is_skip_rest {
                    return (results, None);
                }
            }
            Err(err) => {
                let result = StepResult::from_error(i, executor.kind(), 0, &err);
                results.push(result);
                return (results, Some((i, err)));
            }
        }
    }

    (results, None)
}
