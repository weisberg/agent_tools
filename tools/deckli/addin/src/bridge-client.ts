/// WebSocket client that connects to the deckli sidecar bridge.
import { setStatus, log } from './ui';

const BRIDGE_URL = 'ws://127.0.0.1:9716/addin';
const RECONNECT_DELAY_MS = 3000;

type CommandHandler = (request: {
  id: string;
  method: string;
  params: any;
}) => Promise<{ id: string; success: boolean; result?: any; error?: any }>;

let ws: WebSocket | null = null;
let handler: CommandHandler | null = null;

export function connectToBridge(onCommand: CommandHandler): void {
  handler = onCommand;
  attemptConnect();
}

function attemptConnect(): void {
  setStatus('connecting');
  log('info', `Connecting to bridge at ${BRIDGE_URL}...`);

  ws = new WebSocket(BRIDGE_URL);

  ws.onopen = () => {
    setStatus('connected');
    log('ok', 'Connected to bridge');
  };

  ws.onmessage = async (event) => {
    if (!handler) return;

    try {
      const request = JSON.parse(event.data as string);
      const response = await handler(request);
      ws?.send(JSON.stringify(response));
    } catch (err: any) {
      log('error', `Failed to process message: ${err.message}`);
    }
  };

  ws.onclose = () => {
    setStatus('disconnected');
    log('info', `Bridge disconnected — reconnecting in ${RECONNECT_DELAY_MS / 1000}s`);
    setTimeout(attemptConnect, RECONNECT_DELAY_MS);
  };

  ws.onerror = (err) => {
    log('error', 'Bridge connection error');
    // onclose will fire after this and handle reconnection
  };
}
