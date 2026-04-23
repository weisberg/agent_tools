import { connectToBridge } from './bridge-client';
import { executeCommand } from './command-router';
import { log } from './ui';

Office.onReady(async (info) => {
  if (info.host !== Office.HostType.PowerPoint) {
    log('error', 'deckli only works in PowerPoint');
    return;
  }

  log('info', `Office.js ready — host: ${info.host}, platform: ${info.platform}`);
  connectToBridge(handleCommand);
});

async function handleCommand(request: {
  id: string;
  method: string;
  params: any;
}): Promise<{ id: string; success: boolean; result?: any; error?: any }> {
  log('cmd', `← ${request.method} (${request.id.slice(0, 8)})`);

  try {
    const result = await executeCommand(request.method, request.params);
    log('ok', `→ ${request.method} OK`);
    return { id: request.id, success: true, result };
  } catch (err: any) {
    const error = {
      code: err.code || 'execution_error',
      message: err.message || String(err),
      suggestion: err.suggestion,
    };
    log('error', `→ ${request.method} FAIL: ${error.message}`);
    return { id: request.id, success: false, error };
  }
}
