use bashli_core::*;
use bashli_steps::{StepContext, StepRegistry};

/// Run all steps regardless of failure.
pub(crate) async fn run_independent(
    steps: &[Step],
    registry: &StepRegistry,
    ctx: &mut StepContext<'_>,
) -> (Vec<StepResult>, Option<(usize, ExecError)>) {
    let mut results = Vec::new();
    let mut first_error: Option<(usize, ExecError)> = None;

    for (i, step) in steps.iter().enumerate() {
        ctx.vars.set("$_STEP_INDEX", serde_json::Value::Number(i.into()));

        let executor = match registry.resolve(step) {
            Ok(e) => e,
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some((i, ExecError::VarError(err.to_string())));
                }
                continue;
            }
        };

        match executor.execute(i, ctx).await {
            Ok(result) => {
                if first_error.is_none() {
                    if let Some(code) = result.exit_code {
                        if code != 0 {
                            first_error = Some((i, ExecError::NonZeroExit(code)));
                        }
                    }
                }
                results.push(result);
            }
            Err(err) => {
                let result = StepResult::from_error(i, executor.kind(), 0, &err);
                results.push(result);
                if first_error.is_none() {
                    first_error = Some((i, err));
                }
            }
        }
    }

    (results, first_error)
}
