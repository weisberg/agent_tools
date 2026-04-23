#!/usr/bin/env node
// deckli-bridge.mjs — WebSocket sidecar that relays commands between CLI and add-in.
//
// Architecture:
//   CLI (Rust) ──ws://.../cli──► this process ──ws://.../addin──► Office add-in
//               ◄──────────────                ◄──────────────
//
// The add-in connects once on load via /addin.
// Each CLI invocation connects per-command via /cli.
// This process routes CLI messages → add-in and responses back.

import { WebSocketServer, WebSocket } from 'ws';

const PORT = parseInt(process.env.DECKLI_PORT || '9716', 10);
const HOST = '127.0.0.1';
const IDLE_TIMEOUT_MS = 10 * 60 * 1000; // 10 minutes

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let addinSocket = null;
const pendingRequests = new Map(); // id → { socket, timestamp }
let idleTimer = null;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

const wss = new WebSocketServer({ port: PORT, host: HOST });

wss.on('connection', (ws, req) => {
  resetIdleTimer();

  if (req.url === '/addin') {
    handleAddinConnection(ws);
  } else {
    handleCliConnection(ws);
  }
});

wss.on('error', (err) => {
  console.error(`[bridge] server error: ${err.message}`);
  process.exit(1);
});

console.log(`[bridge] deckli bridge listening on ws://${HOST}:${PORT}`);
console.log(`[bridge] waiting for add-in on /addin and CLI on /cli`);

// ---------------------------------------------------------------------------
// Add-in connection (long-lived)
// ---------------------------------------------------------------------------

function handleAddinConnection(ws) {
  console.log('[bridge] add-in connected');
  addinSocket = ws;

  ws.on('message', (data) => {
    resetIdleTimer();
    try {
      const response = JSON.parse(data.toString());
      const pending = pendingRequests.get(response.id);
      if (pending) {
        pending.socket.send(data.toString());
        pendingRequests.delete(response.id);
      } else {
        console.warn(`[bridge] response for unknown request id: ${response.id}`);
      }
    } catch (err) {
      console.error(`[bridge] invalid message from add-in: ${err.message}`);
    }
  });

  ws.on('close', () => {
    console.log('[bridge] add-in disconnected');
    addinSocket = null;
  });

  ws.on('error', (err) => {
    console.error(`[bridge] add-in error: ${err.message}`);
    addinSocket = null;
  });
}

// ---------------------------------------------------------------------------
// CLI connection (per-command, short-lived)
// ---------------------------------------------------------------------------

function handleCliConnection(ws) {
  ws.on('message', (data) => {
    resetIdleTimer();
    try {
      const request = JSON.parse(data.toString());

      // Handle ping locally (no add-in needed for connectivity check)
      if (request.method === 'ping') {
        const addinConnected = addinSocket && addinSocket.readyState === WebSocket.OPEN;
        ws.send(JSON.stringify({
          id: request.id,
          success: true,
          result: {
            bridge: 'connected',
            addin: addinConnected ? 'connected' : 'disconnected',
          },
        }));
        return;
      }

      // Forward to add-in
      if (!addinSocket || addinSocket.readyState !== WebSocket.OPEN) {
        ws.send(JSON.stringify({
          id: request.id,
          success: false,
          error: {
            code: 'no_addin',
            message: 'PowerPoint add-in not connected',
            suggestion: 'Open PowerPoint and activate the deckli add-in',
          },
        }));
        return;
      }

      pendingRequests.set(request.id, { socket: ws, timestamp: Date.now() });
      addinSocket.send(data.toString());

    } catch (err) {
      console.error(`[bridge] invalid message from CLI: ${err.message}`);
    }
  });

  ws.on('close', () => {
    // Clean up any pending requests from this socket
    for (const [id, entry] of pendingRequests) {
      if (entry.socket === ws) {
        pendingRequests.delete(id);
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Idle auto-shutdown
// ---------------------------------------------------------------------------

function resetIdleTimer() {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(() => {
    console.log('[bridge] idle timeout — shutting down');
    wss.close();
    process.exit(0);
  }, IDLE_TIMEOUT_MS);
}

resetIdleTimer();

// ---------------------------------------------------------------------------
// Stale request cleanup (every 30s, drop requests older than 60s)
// ---------------------------------------------------------------------------

setInterval(() => {
  const cutoff = Date.now() - 60_000;
  for (const [id, entry] of pendingRequests) {
    if (entry.timestamp < cutoff) {
      try {
        entry.socket.send(JSON.stringify({
          id,
          success: false,
          error: {
            code: 'timeout',
            message: 'Request timed out waiting for add-in response',
            suggestion: 'Check that PowerPoint is responsive and the add-in is active',
          },
        }));
      } catch { /* socket may be closed */ }
      pendingRequests.delete(id);
    }
  }
}, 30_000);
