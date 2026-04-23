# Explainer Types

Every explainer falls into one of five categories. Pick the one that matches
the concept's structure, not its topic. The same subject can be explained
with different types depending on what the user needs to understand.

## 1. Calculator

**What it is**: The user tunes parameters and sees computed results update in
real time. The visual is secondary to the numbers — often a chart or
distribution that reinforces the math.

**When to use**: The concept has tunable inputs and quantitative outputs.
Statistical formulas, financial models, physics equations, engineering
sizing problems.

**Layout pattern**:
```
┌─ Controls ───────────────────────┐
│  Slider: Parameter A             │
│  Slider: Parameter B             │
│  Slider: Parameter C             │
└──────────────────────────────────┘
┌─ Metric cards (3-4 across) ─────┐
│  Result 1  │  Result 2  │  ...  │
└──────────────────────────────────┘
┌─ SVG chart / distribution ──────┐
│  (updates as sliders move)      │
└──────────────────────────────────┘
  Footer: methodology note
```

**Interaction model**: Every slider fires `update()`. The JS function reads
all inputs, runs the formula, writes results to metric cards, and redraws
the SVG chart.

**Examples**: A/B test sample size calculator, compound interest simulator,
loan amortization, signal-to-noise ratio, CUPED variance reduction.

**Key "aha" moments to engineer**:
- Dragging a parameter to an extreme and watching results change dramatically
- Two distributions overlapping/separating as effect size changes
- A line on a chart inflecting when a threshold is crossed

---

## 2. Stepper

**What it is**: A multi-stage process shown one stage at a time. The user
clicks Next/Prev (or uses arrow keys) to advance. Each step has its own
SVG panel showing the current state of the system.

**When to use**: The concept is sequential and each stage builds on the
previous. Algorithms, protocols, biological processes, manufacturing steps,
approval workflows.

**Layout pattern**:
```
┌─ Progress indicator ────────────┐
│  ● ○ ○ ○ ○  (dots or pills)    │
│  Step 2 of 5: "Process request" │
└──────────────────────────────────┘
┌─ SVG panel ─────────────────────┐
│  (shows current stage visually) │
│  Active elements highlighted    │
│  Previous elements grayed out   │
└──────────────────────────────────┘
┌─ Description ───────────────────┐
│  2-3 sentences about this step  │
└──────────────────────────────────┘
  [ ← Prev ]            [ Next → ]
```

**Interaction model**: `currentStep` state variable. Next/Prev buttons and
arrow keys advance it. A `renderStep(n)` function shows/hides SVG elements
and updates the description. The last step wraps to the first (the loop
*is* the concept for cyclic processes).

**Examples**: TCP handshake, how a compiler works, the Krebs cycle, event
loop phases, OAuth flow, git rebase step by step.

**Key "aha" moments to engineer**:
- Seeing the same diagram evolve across steps (elements appear, move, change color)
- The wrap-around from last to first step for cyclic processes
- A "before/after" comparison when stepping through a transformation

**Important**: Don't draw cycles as rings. Use the stepper's wraparound
to convey the loop. Each step gets the full SVG canvas — no cramming.

---

## 3. Flowchart

**What it is**: A static or lightly interactive diagram showing how parts
connect and flow relates. Boxes, arrows, containment regions. The user may
click nodes to drill down (via `sendPrompt` in claude.ai, or expanding a
detail panel in standalone HTML).

**When to use**: The concept is about relationships, data flow, or decision
branching. System architectures, CI/CD pipelines, organizational structures,
decision trees, data pipelines.

**Layout pattern**:
```
┌─ SVG diagram (full width) ──────┐
│                                  │
│  [Box A] ──→ [Box B]            │
│               │                  │
│               ▼                  │
│            [Box C] ──→ [Box D]  │
│                                  │
└──────────────────────────────────┘
┌─ Detail panel (hidden by default)┐
│  (shows when a node is clicked)  │
│  Description of the clicked node │
└──────────────────────────────────┘
```

**Interaction model**: Nodes are clickable. Clicking a node expands a detail
panel below the SVG (or highlights related nodes). In Claude Code standalone
files, use a `<div>` below the SVG that swaps content. Keep the SVG itself
static — don't try to animate boxes moving around.

**Examples**: GitHub Issues + PRs workflow, microservices architecture,
CI/CD pipeline, request lifecycle, how DNS resolution works.

**SVG rules** (critical for flowcharts):
- Max 4-5 nodes per row at 680px width
- Single-line nodes: 44px tall. Two-line nodes: 56px tall
- 60px minimum gap between boxes
- Arrow lines: 0.5px stroke, open chevron marker
- All same-type nodes should be the same height
- Prefer top-to-bottom or left-to-right flow — never both in one diagram

---

## 4. Illustrative

**What it is**: A drawn representation of a mechanism — physical or abstract.
The goal is intuition, not documentation. The visual *is* the explanation.

**When to use**: The user needs to *feel* how something works, not just
know the components. Physical systems (engines, circuits, biological organs)
get cross-sections. Abstract systems (attention, gradient descent, hash maps)
get spatial metaphors.

**Layout pattern**:
```
┌─ Controls (optional) ──────────┐
│  Slider: temperature / rate     │
│  Toggle: heating on/off         │
└─────────────────────────────────┘
┌─ SVG illustration ──────────────┐
│                                  │
│  [Drawn mechanism]    Labels ──┤
│  with color-encoded   with     ─┤
│  state/intensity      leaders  ─┤
│                                  │
└──────────────────────────────────┘
```

**Interaction model**: Controls (if any) should map to real-world parameters.
A thermostat becomes a slider. A switch becomes a toggle. JS updates SVG
fills, stroke opacities, and animations to show the mechanism responding.

**When interactive**: Use CSS `@keyframes` for continuous processes (flow,
convection, rotation). Wrap in `@media (prefers-reduced-motion: no-preference)`.
JS toggles animation state based on controls.

**Examples**: How a water heater works, how attention in transformers works,
how a hash map distributes keys, how a circuit breaker trips, how gradient
descent finds a minimum.

**Color encoding**: Warm ramps (amber, coral, red) = heat, energy, high weight.
Cool ramps (blue, teal) = cold, calm, low weight. Gray = structural/inert.

---

## 5. Comparison

**What it is**: Two or more options displayed side by side with visual
differentiation. The user may toggle between views or adjust weights.

**When to use**: The concept involves choosing between alternatives, or
understanding tradeoffs. Product comparisons, algorithm complexity,
before/after, methodology differences.

**Layout pattern**:
```
┌─ Controls (optional) ──────────┐
│  Toggle: Show Option A / B      │
│  Slider: Weight factor          │
└─────────────────────────────────┘
┌─ Side-by-side panels ──────────┐
│  ┌──────────┐  ┌──────────┐    │
│  │ Option A  │  │ Option B  │   │
│  │ (SVG)     │  │ (SVG)     │   │
│  └──────────┘  └──────────┘    │
└─────────────────────────────────┘
┌─ Summary / verdict ────────────┐
│  Key differences highlighted    │
└─────────────────────────────────┘
```

**Interaction model**: Toggle buttons swap which option is highlighted or
focused. Sliders may adjust a parameter that affects both sides differently,
making the tradeoff visible.

**Examples**: Microservices vs monolith, CUPED vs naive A/B test, React vs
Vue rendering, SQL vs NoSQL for a given workload, before/after optimization.

---

## Choosing the right type

| User says | Type | Why |
|---|---|---|
| "How much sample do I need?" | Calculator | Tunable parameters → numeric result |
| "Walk me through how OAuth works" | Stepper | Sequential stages to step through |
| "Show me our CI/CD pipeline" | Flowchart | Components and their connections |
| "How does a heat pump actually work?" | Illustrative | Physical mechanism to draw |
| "Compare CUPED vs standard A/B" | Comparison | Two approaches, show tradeoffs |
| "Explain gradient descent" | Illustrative | Abstract concept needs spatial metaphor |
| "How do Issues and PRs work together?" | Flowchart | Workflow with linked stages |
| "Show me how the event loop works" | Stepper | Cyclic process, one phase at a time |

When in doubt, ask: **does the user need to tune something (calculator),
step through something (stepper), see connections (flowchart), build
intuition (illustrative), or choose between things (comparison)?**
