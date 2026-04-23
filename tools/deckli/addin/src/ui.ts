/// Minimal task pane UI helpers.

type LogLevel = 'info' | 'cmd' | 'ok' | 'error';

const levelClass: Record<LogLevel, string> = {
  info: '',
  cmd: 'log-cmd',
  ok: 'log-ok',
  error: 'log-err',
};

export function log(level: LogLevel, message: string): void {
  const el = document.getElementById('log');
  if (!el) {
    console.log(`[deckli:${level}] ${message}`);
    return;
  }

  const entry = document.createElement('div');
  entry.className = `log-entry ${levelClass[level]}`;
  const ts = new Date().toLocaleTimeString('en-US', { hour12: false });
  entry.textContent = `[${ts}] ${message}`;
  el.appendChild(entry);
  el.scrollTop = el.scrollHeight;
}

export function setStatus(state: 'connected' | 'disconnected' | 'connecting'): void {
  const dot = document.getElementById('statusDot');
  const text = document.getElementById('statusText');
  if (dot) {
    dot.className = `dot ${state}`;
  }
  if (text) {
    text.textContent = state.charAt(0).toUpperCase() + state.slice(1);
  }
}
