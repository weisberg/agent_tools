# deckli: CLI-to-Office.js Bridge for Live PowerPoint Manipulation

## What This Is

A sideloaded Office add-in that acts as a **WebSocket bridge** between your terminal (Claude Code, GitHub Copilot CLI) and a live, open PowerPoint document. You type CLI commands or an agent issues them — the add-in translates them into Office.js API calls against the active presentation in real time. You see changes appear instantly in PowerPoint as if you made them by hand.

This is not file manipulation. This is live document control.

---

## The Bridge Architecture

```
Terminal (Claude Code / Copilot CLI)
│
│  $ deckli add slide --layout "Title Slide"
│  $ deckli set /slides/1/title "Q3 Business Review"
│
▼
┌──────────────────────────────┐
│  deckli CLI (Rust binary)    │
│                              │
│  Parses command ──────────►  │
│  Serializes to JSON ──────►  │
│  Sends over WebSocket ────►  │──── ws://localhost:9716 ────┐
│  Waits for response ◄─────  │                              │
│  Prints structured JSON ◄──  │                              │
└──────────────────────────────┘                              │
                                                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  PowerPoint (macOS)                                             │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  deckli Add-in (Task Pane / Hidden)                       │  │
│  │                                                           │  │
│  │  ┌─────────────────┐    ┌──────────────────────────────┐  │  │
│  │  │  WebSocket       │    │  Command Router              │  │  │
│  │  │  Server          │◄──►│                              │  │  │
│  │  │  (ws on :9716)   │    │  inspect ──► readMasters()   │  │  │
│  │  │                  │    │  get ──────► readShapes()     │  │  │
│  │  │  Receives JSON   │    │  set ──────► updateShape()   │  │  │
│  │  │  commands from   │    │  add ──────► addSlide()      │  │  │
│  │  │  CLI process     │    │             addShape()       │  │  │
│  │  │                  │    │  rm ───────► deleteShape()   │  │  │
│  │  │  Sends JSON      │    │  render ──► exportBase64()   │  │  │
│  │  │  results back    │    │  batch ───► atomicBatch()    │  │  │
│  │  └─────────────────┘    └──────────┬───────────────────┘  │  │
│  │                                     │                      │  │
│  │                                     ▼                      │  │
│  │                          ┌──────────────────────┐          │  │
│  │                          │  Office.js Runtime   │          │  │
│  │                          │                      │          │  │
│  │                          │  PowerPoint.run()    │          │  │
│  │                          │  context.sync()      │          │  │
│  │                          │  Proxy Object Model  │          │  │
│  │                          └──────────┬───────────┘          │  │
│  └──────────────────────────────────────┼─────────────────────┘  │
│                                         │                        │
│                            ┌────────────▼─────────────┐          │
│                            │  Live Document           │          │
│                            │  (slides, shapes, text,  │          │
│                            │   masters, layouts,      │          │
│                            │   theme, animations)     │          │
│                            └──────────────────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

The key insight: the Office.js add-in web view can open a WebSocket server on localhost. The CLI binary connects to it. Every CLI invocation becomes a JSON-RPC message over WebSocket, executed inside `PowerPoint.run()` with proper `context.sync()` batching, and the result flows back as structured JSON. The human sees the change happen in real time in the PowerPoint window.

---

## Component 1: The Office Add-in (TypeScript)

### 1.1 What It Does

A sideloaded PowerPoint add-in that:

- Starts a WebSocket server on `ws://localhost:9716` when loaded
- Accepts JSON-RPC commands from any local process
- Executes commands inside `PowerPoint.run()` against the live document
- Returns structured JSON results over the same WebSocket connection
- Optionally shows a minimal task pane with connection status and command log

### 1.2 Add-in Manifest

Using the add-in only manifest (XML) for maximum macOS compatibility. The unified manifest is still in preview for PowerPoint.

```xml
<!-- manifest.xml — key elements -->
<OfficeApp>
  <Id>deckli-bridge-addin</Id>
  <DefaultSettings>
    <SourceLocation DefaultValue="https://localhost:3000/taskpane.html"/>
  </DefaultSettings>
  <Hosts>
    <Host Name="Presentation"/>
  </Hosts>
  <Requirements>
    <Sets>
      <Set Name="PowerPointApi" MinVersion="1.1"/>
    </Sets>
  </Requirements>
</OfficeApp>
```

Sideloaded on macOS via `~/Library/Containers/com.microsoft.Powerpoint/Data/Documents/wef/` or through the Insert > Add-ins > My Add-ins workflow.

### 1.3 WebSocket Server Inside the Add-in

The task pane's JavaScript runtime can create a WebSocket server bound to localhost. Since Office add-ins on macOS run in a WKWebView (Safari engine), standard WebSocket APIs are available. The add-in hosts the server; the CLI is the client.

```typescript
// Simplified — the add-in's bridge server
import { WebSocketServer } from 'ws'; // bundled with webpack

const wss = new WebSocketServer({ port: 9716, host: '127.0.0.1' });

wss.on('connection', (socket) => {
  socket.on('message', async (raw) => {
    const request = JSON.parse(raw.toString());
    try {
      const result = await executeCommand(request);
      socket.send(JSON.stringify({ id: request.id, success: true, result }));
    } catch (err) {
      socket.send(JSON.stringify({
        id: request.id,
        success: false,
        error: { code: err.code, message: err.message, suggestion: err.suggestion }
      }));
    }
  });
});
```

**Important constraint**: Office add-ins on macOS run inside a WKWebView sandbox. The WebSocket server may need to be implemented differently — potentially as a thin Node.js sidecar process that the add-in communicates with via `fetch` to localhost, rather than hosting the server directly inside WKWebView. Two viable patterns:

**Pattern A — Add-in hosts HTTP endpoint, CLI polls/posts:**
- Add-in runs a minimal Express/Fastify server on localhost:9716
- CLI sends POST requests with commands, receives JSON responses
- Simpler but higher latency per command

**Pattern B — External sidecar bridges WebSocket to add-in:**
- A small Node.js process runs the WebSocket server on localhost:9716
- The add-in connects to the sidecar via WebSocket as a *client*
- Sidecar relays commands from CLI → add-in and results back
- More moving parts but cleaner separation

**Pattern C — Add-in polls a local file or shared state:**
- CLI writes command JSON to a known file path
- Add-in polls via `setInterval`, picks up commands, executes, writes results
- Simplest but slowest — only viable as fallback

**Recommended: Pattern B** — the sidecar is a single Node.js script (`deckli-bridge`) that the CLI auto-starts if not running. The add-in connects to it on load. The CLI connects to it per invocation.

### 1.4 Command Router

The router maps incoming JSON-RPC commands to Office.js API calls. Every handler runs inside `PowerPoint.run()` and follows the load → sync → operate → sync pattern.

```typescript
async function executeCommand(cmd: Command): Promise<any> {
  switch (cmd.method) {
    case 'inspect':       return await inspectPresentation(cmd.params);
    case 'inspect.masters': return await inspectMasters(cmd.params);
    case 'inspect.theme':   return await inspectTheme(cmd.params);
    case 'get.slide':      return await getSlide(cmd.params);
    case 'get.shape':      return await getShape(cmd.params);
    case 'set.text':       return await setText(cmd.params);
    case 'set.fill':       return await setFill(cmd.params);
    case 'set.geometry':   return await setGeometry(cmd.params);
    case 'set.font':       return await setFont(cmd.params);
    case 'add.slide':      return await addSlide(cmd.params);
    case 'add.shape':      return await addShape(cmd.params);
    case 'add.image':      return await addImage(cmd.params);
    case 'add.table':      return await addTable(cmd.params);
    case 'rm.slide':       return await removeSlide(cmd.params);
    case 'rm.shape':       return await removeShape(cmd.params);
    case 'move.slide':     return await moveSlide(cmd.params);
    case 'render.slide':   return await renderSlide(cmd.params);
    case 'batch':          return await executeBatch(cmd.params);
    default: throw { code: 'unknown_command', message: `Unknown: ${cmd.method}` };
  }
}
```

### 1.5 Core Office.js Handlers

**Inspect masters and layouts** (the metadata Claude's plugin reads via SlideMasterCollection):

```typescript
async function inspectMasters(_params: any) {
  return await PowerPoint.run(async (context) => {
    const masters = context.presentation.slideMasters;
    masters.load("id, name");
    await context.sync();

    const schema = [];
    for (const master of masters.items) {
      master.layouts.load("id, name");
    }
    await context.sync();

    for (const master of masters.items) {
      const layouts = master.layouts.items.map(l => ({
        id: l.id, name: l.name
      }));
      schema.push({ id: master.id, name: master.name, layouts });
    }
    return { masters: schema };
  });
}
```

**Add a shape with precise coordinates:**

```typescript
async function addShape(params: {
  slideIndex: number,
  type: string,
  left: number,    // points
  top: number,
  width: number,
  height: number,
  fill?: string,
  text?: string
}) {
  return await PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shapes = slide.shapes;

    const options = {
      left: params.left,
      top: params.top,
      width: params.width,
      height: params.height
    };

    const shapeType = PowerPoint.GeometricShapeType[params.type];
    const shape = shapes.addGeometricShape(shapeType, options);
    shape.load("id, name");

    if (params.fill) {
      shape.fill.setSolidColor(params.fill);
    }

    if (params.text) {
      shape.textFrame.textRange.text = params.text;
    }

    await context.sync();

    return {
      shape_id: shape.id,
      name: shape.name,
      geometry: { left: params.left, top: params.top,
                  width: params.width, height: params.height }
    };
  });
}
```

**Batch execution** (replicates the context.sync() batching advantage):

```typescript
async function executeBatch(params: { operations: Command[] }) {
  return await PowerPoint.run(async (context) => {
    const results = [];

    for (const op of params.operations) {
      // Each operation queues work on proxy objects
      // but we only sync ONCE at the end
      const result = await routeWithinContext(context, op);
      results.push(result);
    }

    await context.sync(); // Single round-trip for all operations
    return { results, operations_count: params.operations.length };
  });
}
```

### 1.6 Render to Base64 (Vision Verification Loop)

The critical capability that closes the agentic feedback loop:

```typescript
async function renderSlide(params: { slideIndex: number }) {
  return await PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const base64 = slide.exportAsBase64();
    await context.sync();

    return {
      slideIndex: params.slideIndex,
      image_base64: base64.value,
      format: "png"
    };
  });
}
```

The agent calls `deckli render --slide 3`, gets back a base64 PNG, feeds it to the vision model, and self-corrects if the layout looks wrong. This is exactly what Claude's plugin does via `exportAsBase64()`, but now accessible from the CLI.

---

## Component 2: The CLI Binary (Rust)

### 2.1 What It Does

- Parses human-readable commands into JSON-RPC messages
- Connects to the WebSocket bridge on localhost:9716
- Sends the command, waits for the response
- Prints structured JSON (or human-friendly output with `--pretty`)
- Auto-starts the bridge sidecar if not running
- Exits after each command (stateless CLI, stateful bridge)

### 2.2 Command Surface

```bash
# Connection management
deckli status                                    # Check bridge connection
deckli connect                                   # Start sidecar + verify add-in

# Inspection (read-only)
deckli inspect                                   # Full presentation schema
deckli inspect --masters                         # Master + layout tree
deckli inspect --theme                           # Color + font scheme
deckli inspect --slide 3                         # Shape inventory for slide 3

# Read
deckli get /slides                               # Slide list with titles
deckli get /slides/3                             # Full slide content
deckli get /slides/3/shapes/2                    # Shape detail
deckli get /slides/3/notes                       # Speaker notes
deckli get /selection                            # Currently selected shape/slide

# Write
deckli set /slides/3/shapes/title/text "New Title"
deckli set /slides/3/shapes/2/fill "#2E75B6"
deckli set /slides/3/shapes/2/font --size 24 --bold
deckli set /slides/3/shapes/2/geometry --left 1in --top 2in --width 8in --height 1in

# Add
deckli add slide --layout "Title Slide"
deckli add slide --layout "Two Content" --at 3
deckli add shape --slide 3 --type rectangle \
  --left 1in --top 2in --width 4in --height 2in \
  --fill "#2E75B6"
deckli add shape --slide 3 --type textbox \
  --left 1in --top 1in --width 10in --height 0.5in \
  --text "Revenue Summary"
deckli add image --slide 3 --src ./chart.png \
  --left 6in --top 2in --width 5in --height 3in
deckli add table --slide 3 \
  --data '[["Metric","Q1","Q2"],["Revenue","$1.2M","$1.5M"]]' \
  --left 1in --top 4in --width 10in --height 2in

# Remove
deckli rm /slides/5
deckli rm /slides/3/shapes/2

# Reorder
deckli move /slides/5 --to 2

# Render (vision verification)
deckli render --slide 3                          # Returns base64 PNG to stdout
deckli render --slide 3 --out slide3.png         # Saves to file
deckli render --all --out ./slides/              # All slides to directory

# Batch (agent efficiency)
deckli batch --file commands.json
deckli batch --stdin                             # Pipe JSON array from stdin
echo '[{"method":"add.slide","params":{"layout":"Blank"}},
       {"method":"set.text","params":{"slideIndex":-1,"shapeType":"title","text":"Hello"}}]' \
  | deckli batch --stdin

# MCP server mode (for Claude Code native integration)
deckli mcp-serve                                 # stdio JSON-RPC MCP server
```

### 2.3 Unit Conversion

The CLI accepts human units and converts to points (Office.js native unit):

| Input | Points | EMU |
|-------|--------|-----|
| `1in` | 72 | 914,400 |
| `72pt` | 72 | 914,400 |
| `2.54cm` | 72 | 914,400 |
| `100px` | 75 | 952,500 |
| `914400emu` | 72 | 914,400 |
| `72` (bare number) | 72 | 914,400 |

### 2.4 JSON Output Envelope

```json
{
  "success": true,
  "command": "add.shape",
  "params": { "slide": 3, "type": "rectangle" },
  "result": {
    "shape_id": "s7",
    "name": "Rectangle 7",
    "geometry": {
      "left": "1.00in", "left_pt": 72,
      "top": "2.00in", "top_pt": 144,
      "width": "4.00in", "width_pt": 288,
      "height": "2.00in", "height_pt": 144
    }
  },
  "timing_ms": 47
}
```

On error:

```json
{
  "success": false,
  "command": "set.text",
  "error": {
    "code": "shape_not_found",
    "message": "Slide 3 has 4 shapes (indices 0-3), but you requested index 5",
    "suggestion": "Run `deckli inspect --slide 3` to see available shapes"
  }
}
```

---

## Component 3: The Sidecar Bridge (Node.js)

### 3.1 What It Does

A lightweight Node.js process that mediates between the CLI and the add-in:

```
CLI (Rust) ──ws──► Sidecar (Node.js, :9716) ──ws──► Add-in (inside PowerPoint)
            ◄──                                ◄──
```

Why not direct? The WKWebView sandbox on macOS may restrict the add-in from hosting a server directly. The sidecar runs outside the sandbox as a normal process.

### 3.2 Lifecycle

- `deckli connect` (or any command when sidecar isn't running) auto-launches it
- Sidecar starts a WebSocket server on `ws://127.0.0.1:9716`
- Add-in connects as a client on load (`ws://127.0.0.1:9716/addin`)
- CLI connects per-command (`ws://127.0.0.1:9716/cli`)
- Sidecar routes CLI messages → add-in and responses back
- Sidecar exits after 10 minutes of no connections (auto-cleanup)

### 3.3 Implementation

~100 lines of Node.js. Single file, no build step, ships alongside the Rust binary:

```javascript
// deckli-bridge.mjs
import { WebSocketServer, WebSocket } from 'ws';

const PORT = 9716;
const wss = new WebSocketServer({ port: PORT, host: '127.0.0.1' });

let addinSocket = null;
const pendingRequests = new Map();

wss.on('connection', (ws, req) => {
  if (req.url === '/addin') {
    addinSocket = ws;
    ws.on('message', (data) => {
      const response = JSON.parse(data);
      const pending = pendingRequests.get(response.id);
      if (pending) {
        pending.socket.send(data);
        pendingRequests.delete(response.id);
      }
    });
    ws.on('close', () => { addinSocket = null; });
  } else {
    // CLI connection
    ws.on('message', (data) => {
      const request = JSON.parse(data);
      if (!addinSocket || addinSocket.readyState !== WebSocket.OPEN) {
        ws.send(JSON.stringify({
          id: request.id, success: false,
          error: { code: 'no_addin', message: 'PowerPoint add-in not connected',
                   suggestion: 'Open PowerPoint and activate the deckli add-in' }
        }));
        return;
      }
      pendingRequests.set(request.id, { socket: ws, timestamp: Date.now() });
      addinSocket.send(data);
    });
  }
});

console.log(`deckli bridge listening on ws://127.0.0.1:${PORT}`);
```

---

## Component 4: Agent Integration

### 4.1 SKILL.md for Claude Code

```
~/.claude/skills/deckli/
├── SKILL.md          # Command reference, workflow patterns, error recovery
├── LAYOUTS.md        # Layout selection heuristics for common corporate templates
└── RECIPES.md        # Full workflow examples (create deck, modify existing, etc.)
```

The SKILL.md teaches Claude Code:

1. Always `deckli inspect --masters` before adding slides (learn available layouts)
2. Always `deckli inspect --theme` before setting colors (use theme colors, not hardcoded hex)
3. Use `deckli batch` for multi-step operations (one round-trip, not N)
4. Use `deckli render` after major changes to visually verify
5. Address placeholders by type (`title`, `body`, `subtitle`) not index when possible
6. The `--stdin` batch mode is ideal for piping from agent-generated JSON

### 4.2 MCP Server Mode

`deckli mcp-serve` runs as a stdio MCP server for native Claude Code integration:

```json
// ~/.claude/settings.json
{
  "mcpServers": {
    "deckli": {
      "command": "deckli",
      "args": ["mcp-serve"]
    }
  }
}
```

This gives Claude Code direct tool access without shelling out. The MCP server internally connects to the same WebSocket bridge. Tools exposed:

- `inspect_presentation` / `inspect_masters` / `inspect_theme`
- `get_slide` / `get_shape` / `get_selection`
- `set_text` / `set_fill` / `set_font` / `set_geometry`
- `add_slide` / `add_shape` / `add_image` / `add_table`
- `remove_slide` / `remove_shape`
- `move_slide`
- `render_slide` (returns base64 PNG inline)
- `batch_operations`

### 4.3 GitHub Copilot Integration

- VS Code: Copilot accesses deckli via MCP (same config) or terminal commands
- Copilot CLI: Direct `deckli` invocations like any other CLI tool
- `.github/copilot-instructions.md` in project repo with deckli patterns

---

## What We Borrow From Each Source

| Source | What We Take | How We Adapt It |
|--------|-------------|-----------------|
| **Claude's Office.js Plugin** | Agentic loop pattern (tool_use → execute → tool_result), SlideMasterCollection introspection, exportAsBase64() for vision feedback, strict schema enforcement, batched context.sync() | Same Office.js calls, but driven by CLI over WebSocket instead of chat task pane |
| **OfficeCLI** | Path-based addressing (`/slides/3/shapes/2`), self-healing JSON errors with suggestions, human-readable unit inputs, SKILL.md distribution pattern | We keep the syntax but execute against live documents instead of files on disk |
| **office-agents** | BYOK architecture, add-in monorepo structure, sandboxed shell concept, multi-provider support pattern | We strip the chat UI and replace it with a WebSocket command bridge |
| **hewliyang/office-agents exec tool** | The `exec` tool that runs arbitrary JS inside the task pane for debugging | We formalize this into a typed command protocol instead of raw eval |
| **socamalo PPT_MCP_Server** | pywin32 direct COM control concept | We use Office.js instead of COM (cross-platform), but same idea of external process controlling live PowerPoint |
| **tooli (your framework)** | JSON envelope pattern, structured error format, caller detection, SKILL.md auto-generation | Native integration — deckli could itself be a tooli-based CLI |
| **docli (your framework)** | Rust + OOXML expertise, cargo workspace patterns | Reuse OOXML knowledge for parsing theme/master metadata in the Rust CLI layer |

---

## Technology Stack

| Component | Technology | Rationale |
|-----------|-----------|-----------|
| CLI binary | Rust (clap v4, tokio, tungstenite) | Fast, single binary, WebSocket client built in. Consistent with your toolchain |
| Add-in runtime | TypeScript, webpack, Office.js | Required by the Office add-in platform |
| Add-in WebSocket client | Native WebSocket API (WKWebView) | No additional deps needed |
| Sidecar bridge | Node.js + `ws` library | Single-file, ~100 LOC, Node already required for add-in dev tooling |
| Dev server | `office-addin-dev-certs` + webpack-dev-server | Standard Office add-in dev toolchain on macOS |
| Testing | Jest (add-in), cargo test (CLI), Playwright (e2e) | Office add-in testing uses Jest; Rust side uses standard cargo test |

---

## Phased Build Plan

### Phase 0: Proof of Concept (3-5 days)

**Goal**: Prove the WebSocket bridge pattern works on macOS Tahoe with PowerPoint.

- Scaffold Office add-in with `yo office` for PowerPoint task pane
- Implement WebSocket client in add-in connecting to external sidecar
- Build sidecar bridge (Node.js, ~100 LOC)
- Implement 3 commands: `inspect` (read slide count), `get /slides/1` (read shapes), `add shape` (add rectangle)
- Build minimal Rust CLI that connects to sidecar and sends one command
- **Validate**: Type `deckli add shape --slide 1 --type rectangle --left 1in --top 1in --width 3in --height 2in` in terminal → rectangle appears in PowerPoint instantly

### Phase 1: Read Everything (1 week)

- Full `inspect` command: masters, layouts, themes, placeholders
- `get` for all content types: text, shapes, notes, images, tables
- `get /selection` for current user selection
- Placeholder type resolution (title, body, subtitle, footer)
- Color cascade reading (theme → master → layout → slide → shape)
- Font cascade reading
- SKILL.md v1 (read-only commands)

### Phase 2: Write Everything (2 weeks)

- `set` commands: text, fill, line, font, geometry on any shape
- `add` commands: slides (from layout), geometric shapes, text boxes, images, tables
- `rm` commands: slides, shapes
- `move` command: slide reorder
- Placeholder-aware content injection (write to title by type)
- `batch` command with atomic execution (single context.sync)
- `render` command via exportAsBase64()
- SKILL.md v2 (full read/write reference)
- Human-unit conversion in CLI (inches, points, cm → Office.js points)

### Phase 3: Agent Polish (1 week)

- MCP server mode (`deckli mcp-serve`)
- Self-healing error messages with `suggestion` field
- Auto-start sidecar from CLI
- Connection health monitoring and reconnection
- `deckli status` and `deckli connect` commands
- SKILL.md v3 with workflow recipes and vision verification patterns
- Copilot instructions file

### Phase 4: Distribution (3-5 days)

- Homebrew formula for CLI binary
- `curl | bash` installer that installs CLI + sidecar + sideloads add-in
- GitHub release with universal macOS binary
- README with quickstart, demo GIF
- Integration tests against 5+ real corporate templates

---

## Risk Register

| Risk | Severity | Mitigation |
|------|----------|------------|
| WKWebView sandbox blocks WebSocket server hosting | High | Pattern B (sidecar bridge) avoids this entirely. Add-in is a WS *client*, not server |
| Office.js PowerPoint API coverage gaps on macOS | High | Audit PowerPointApi requirement sets for macOS support before committing to each feature. Some APIs are Windows-only |
| Sidecar process management complexity | Medium | Auto-start via CLI, auto-shutdown on idle, PID file for cleanup. Keep it to one file |
| WebSocket latency for batch operations | Low | Single batch command = single WS message = single context.sync. Latency is per-batch, not per-operation |
| Add-in sideloading friction on macOS | Medium | Provide a script that copies manifest to the correct macOS path. Document the Insert > Add-ins workflow as fallback |
| PowerPoint not open when CLI runs | Low | Sidecar returns clear error: "PowerPoint add-in not connected. Open PowerPoint and activate deckli." |

---

## Success Criteria

1. **Instant feedback**: Shape appears in PowerPoint within 200ms of CLI command
2. **Full master/layout introspection**: Agent can discover every layout and placeholder before creating content
3. **Vision loop works**: `deckli render --slide 3` returns base64 PNG that agent can feed to vision model
4. **Batch efficiency**: Creating a 5-shape slide is one CLI invocation, one context.sync(), <500ms total
5. **Zero-config for agents**: `deckli mcp-serve` in Claude Code settings.json, start using immediately
6. **SKILL.md < 4K tokens**: Primary skill file fits in context without crowding reasoning
7. **Self-healing > 80%**: Agent recovers from errors without human intervention 4/5 times