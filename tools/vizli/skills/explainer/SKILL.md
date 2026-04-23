---
name: explainer
description: >
  Create interactive visual explainers — HTML files with embedded SVG diagrams,
  charts, sliders, and steppers that teach concepts through spatial and interactive
  means. Use this skill whenever the user asks to "explain", "visualize", "diagram",
  "illustrate", or "show how something works", and the output should be a standalone
  file they can open in a browser. Also trigger when the user asks for an interactive
  demo, a visual walkthrough, an animated explanation, a calculator or simulator
  for a concept, or any educational/explanatory artifact that benefits from
  interactivity rather than static text. This includes statistical concepts,
  system architectures, algorithms, scientific processes, business models,
  decision frameworks, data pipelines, and any "how does X work" question where
  seeing is understanding. Do NOT use for slide decks (use pptx), formal reports
  (use docx), or data dashboards with real data connections. This skill is for
  teaching and explaining through interactive visuals.
---

# Explainer Skill

Create standalone HTML files that explain concepts through interactive SVG
diagrams, charts, calculators, and steppers. The output is a single `.html`
file the user can open in any browser — no server, no dependencies, no build step.

## When to use this skill

The explainer is the right tool when **seeing and interacting** teaches better
than reading. Ask yourself: does this concept have spatial relationships,
sequential stages, tunable parameters, or a mechanism that becomes obvious
when drawn? If yes, build an explainer.

**Good fits**: How CUPED reduces variance (slider for correlation), how a
load balancer distributes requests (animated flow), how compound interest
compounds (chart that redraws as you drag), git branching (visual tree),
TCP handshake (stepper), A/B test power (overlapping distributions).

**Poor fits**: A 40-page technical report, a slide deck for a meeting, a
spreadsheet of quarterly numbers, a letter to a client.

## Architecture: the hybrid pattern

Every explainer uses the same three-layer structure:

1. **HTML controls** — sliders, toggles, buttons, dropdowns that the user
   manipulates. These are standard HTML form elements.
2. **SVG canvas** — the visual payload. Shapes, paths, text labels, color
   fills that represent the concept spatially. Embedded inline in the HTML
   via `<svg>` tags.
3. **JS glue** — vanilla JavaScript that reads control values, runs
   computations, and mutates SVG attributes (`setAttribute`, `textContent`,
   `style` changes) to update the visual in real time.

The HTML controls fire events → JS computes new state → JS reaches into the
SVG DOM to update shapes and labels. No frameworks, no build tools.

```
┌─────────────────────────────────┐
│  HTML  (controls, metric cards) │
│  ┌───────────────────────────┐  │
│  │  SVG  (the visual)        │  │
│  │  <rect>, <path>, <text>   │  │
│  │  <line>, <circle>, etc.   │  │
│  └───────────────────────────┘  │
│  <script> (JS glue)            │
└─────────────────────────────────┘
```

## Step-by-step process

### 1. Identify the explainer type

Read `references/explainer-types.md` to classify the request. The five types
are: **calculator**, **stepper**, **flowchart**, **illustrative**, and
**comparison**. Each has distinct layout patterns and interaction models.
Pick the one that best fits the concept.

### 2. Plan the visual

Before writing code, decide:
- **What is the central visual?** (distribution curves, a process flow, a
  cross-section, a data chart, a tree/graph)
- **What controls does the user get?** (sliders for parameters, buttons for
  states, dropdowns for variants)
- **What updates when controls change?** (which SVG elements, which numbers)
- **What are the key "aha" moments?** (dragging MDE to 1% and watching
  sample size explode, toggling a feature and seeing the architecture change)

### 3. Write the HTML file

Follow the template in `references/html-template.md`. Key rules:

- **Single file, zero dependencies.** Everything — HTML, CSS, SVG, JS — lives
  in one `.html` file. No CDN links unless absolutely necessary (Chart.js is
  the one allowed exception; load from cdnjs.cloudflare.com).
- **Light/dark mode support.** Use CSS custom properties in `:root` with a
  `@media (prefers-color-scheme: dark)` override. Every color must work in
  both modes.
- **SVG viewBox is sacred.** Use `viewBox="0 0 680 H"` where H fits the
  content. Set `width="100%"` on the SVG element. This gives you a 680px
  coordinate system that scales to any screen.
- **No frameworks.** Vanilla HTML, CSS, JS only. The file must open cold
  in any browser with zero setup.
- **Responsive.** Use `max-width: 720px; margin: 0 auto;` on the body.
  Controls use flexbox. The SVG scales via viewBox.

### 4. Build the SVG carefully

SVG is the hardest part. Common failures and how to avoid them:

- **Text overflow**: At 14px sans-serif, each character is ~8px wide. A label
  "Authentication Service" (22 chars) needs a rect at least 200px wide. Always
  calculate: `rect_width = max(label_chars × 8) + 48` (24px padding each side).
- **Arrow collisions**: Before drawing any line, trace its path against every
  box. If it crosses a box it shouldn't, route around with an L-bend path.
- **ViewBox clipping**: Find your bottommost element (`max(y + height)`), add
  40px buffer. That's your viewBox height. Check rightmost element similarly.
- **Text needs explicit positioning**: SVG `<text>` never auto-wraps. Every
  line break requires `<tspan>`. If a subtitle needs wrapping, it's too long.
- **Dark mode colors**: Never hardcode `fill="#333"` or `color: #333`. Use
  CSS variables. In SVG, use semantic class names or CSS variable references.

### 5. Wire up interactivity

The JS section at the bottom of the file should:

1. Define an `update()` function that reads all control values.
2. Run any necessary math (statistical formulas, layout computation, etc.).
3. Update SVG elements via `getElementById` + `setAttribute`.
4. Update metric card text via `textContent`.
5. Call `update()` once on load so the initial state renders.

Every slider gets `oninput="update()"`. Every toggle gets `onchange="update()"`.
The `update()` function is the single source of truth for the visual state.

### 6. Also produce a static SVG

After the interactive HTML is working, create a companion `.svg` file that
captures the default state as a static snapshot. This is useful for embedding
in docs, slides, and contexts where interactivity isn't available.

The static SVG should:
- Be a self-contained `.svg` file (no external dependencies).
- Use inline styles (no CSS variables — SVG files don't inherit from a page).
- Include a light background (`fill="#FAFAF8"` rect) since it won't inherit one.
- Bake in the computed values from the default parameter state.
- Include a footer noting the parameter values used.

### 7. Write to the designated output file

Write both files to the working directory. The user has specified the output
filename; use it for the HTML file. Name the SVG companion with the same
base name plus `_static.svg`.

```bash
# Example: if designated file is "cuped_explainer.html"
# Produce:
#   cuped_explainer.html      (interactive)
#   cuped_explainer_static.svg (static snapshot)
```

## Design system

### Colors (CSS custom properties)

```css
:root {
  --bg: #FAFAF8; --bg2: #F1EFE8; --text: #2C2C2A; --muted: #888780;
  --border: rgba(0,0,0,0.12);
  --purple: #534AB7; --purple-light: rgba(83,74,183,0.12);
  --teal: #0F6E56;   --teal-light: rgba(15,110,86,0.12);
  --coral: #D85A30;  --coral-light: rgba(216,90,48,0.12);
  --amber: #BA7517;  --amber-light: rgba(186,117,23,0.12);
  --blue: #185FA5;   --blue-light: rgba(24,95,165,0.12);
  --green: #639922;  --green-light: rgba(99,153,34,0.12);
  --red: #A32D2D;    --red-light: rgba(163,45,45,0.12);
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1a1a18; --bg2: #2C2C2A; --text: #e8e6dc; --muted: #9c9a92;
    --border: rgba(255,255,255,0.12);
    --purple: #AFA9EC; --purple-light: rgba(175,169,236,0.12);
    --teal: #5DCAA5;   --teal-light: rgba(93,202,165,0.12);
    --coral: #F0997B;  --coral-light: rgba(240,153,123,0.12);
    --amber: #FAC775;  --amber-light: rgba(250,199,117,0.12);
    --blue: #85B7EB;   --blue-light: rgba(133,183,235,0.12);
    --green: #C0DD97;  --green-light: rgba(192,221,151,0.12);
    --red: #F09595;    --red-light: rgba(240,149,149,0.12);
  }
}
```

### Typography

```css
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  font-size: 16px; line-height: 1.5; color: var(--text);
  background: var(--bg);
}
```

- Headings: 18px, weight 500
- Control labels: 13px, `var(--muted)`
- Control values: 14px, weight 500
- Metric card labels: 12px, `var(--muted)`
- Metric card values: 22px, weight 500
- SVG text: 14px for labels, 12px for subtitles
- Footer/attribution: 11px, `var(--muted)`

### Component patterns

**Slider row**:
```html
<div style="display:flex;align-items:center;gap:12px;margin:0 0 14px;">
  <span style="font-size:13px;color:var(--muted);min-width:140px;">Label</span>
  <input type="range" min="0" max="100" value="50" style="flex:1" oninput="update()">
  <span style="font-size:14px;font-weight:500;min-width:54px;text-align:right;" id="val-out">50</span>
</div>
```

**Metric card grid**:
```html
<div style="display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin:20px 0;">
  <div style="background:var(--bg2);border-radius:8px;padding:12px 14px;">
    <div style="font-size:12px;color:var(--muted);">Label</div>
    <div style="font-size:22px;font-weight:500;" id="metric-1">—</div>
  </div>
</div>
```

**Toggle**:
```html
<label style="display:flex;align-items:center;gap:8px;cursor:pointer;">
  <input type="checkbox" id="toggle-1" onchange="update()">
  <span style="font-size:13px;color:var(--muted);">Feature name</span>
</label>
```

**SVG arrow marker** (include in every SVG `<defs>`):
```svg
<defs>
  <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5"
    markerWidth="6" markerHeight="6" orient="auto-start-reverse">
    <path d="M2 1L8 5L2 9" fill="none" stroke="context-stroke"
      stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
  </marker>
</defs>
```

## Quality checklist

Before writing the file, verify:

- [ ] Every SVG `<text>` fits inside its container (chars × 8 + padding < rect width)
- [ ] No arrows cross unrelated boxes
- [ ] ViewBox height covers all content + 40px buffer
- [ ] All colors use CSS variables (no hardcoded hex except in SVG gradients for physical properties)
- [ ] `update()` is called once on page load
- [ ] Every slider has `oninput="update()"`, every toggle has `onchange="update()"`
- [ ] File opens correctly with no server (just double-click the .html)
- [ ] Dark mode looks correct (mentally swap --bg to near-black, check all text)
- [ ] Numbers displayed to user are rounded (no floating point artifacts)
- [ ] Static SVG companion uses inline styles, not CSS variables
- [ ] Footer states the parameter defaults used in static SVG
