/// Batch handler — execute multiple commands in a single PowerPoint.run().

type CommandExecutor = (method: string, params: any) => Promise<any>;

export async function executeBatch(
  params: { operations: Array<{ method: string; params: any }> },
  executeCommand: CommandExecutor,
): Promise<any> {
  const results = [];

  // Execute all operations — each goes through PowerPoint.run individually.
  // For true single-context batching, we'd need to inline all handlers,
  // but this gives us the batch command interface now.
  for (const op of params.operations) {
    try {
      const result = await executeCommand(op.method, op.params);
      results.push({ method: op.method, success: true, result });
    } catch (err: any) {
      results.push({
        method: op.method,
        success: false,
        error: {
          code: err.code || 'execution_error',
          message: err.message || String(err),
        },
      });
    }
  }

  return {
    results,
    operations_count: params.operations.length,
    succeeded: results.filter((r) => r.success).length,
    failed: results.filter((r) => !r.success).length,
  };
}
