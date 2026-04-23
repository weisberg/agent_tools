# HTML Template

Use this as the starting structure for every explainer. Customize the
controls, SVG content, metrics, and JS logic for the specific concept.

## Full template

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>TITLE — Interactive Explainer</title>
<style>
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
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    background: var(--bg); color: var(--text);
    padding: 2rem; max-width: 720px; margin: 0 auto;
  }
  h1 { font-size: 18px; font-weight: 500; margin-bottom: 1.5rem; }

  /* Controls */
  .ctrl-row {
    display: flex; align-items: center; gap: 12px; margin: 0 0 14px;
  }
  .ctrl-label {
    font-size: 13px; color: var(--muted); min-width: 140px;
  }
  .ctrl-value {
    font-size: 14px; font-weight: 500; min-width: 54px; text-align: right;
  }

  /* Range slider styling */
  input[type="range"] {
    flex: 1; height: 4px; -webkit-appearance: none; appearance: none;
    background: var(--border); border-radius: 2px; outline: none;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 18px; height: 18px; border-radius: 50%;
    background: var(--text); cursor: pointer;
    border: 2px solid var(--bg); box-shadow: 0 0 0 1px var(--border);
  }
  input[type="range"]::-moz-range-thumb {
    width: 18px; height: 18px; border-radius: 50%;
    background: var(--text); cursor: pointer;
    border: 2px solid var(--bg); box-shadow: 0 0 0 1px var(--border);
  }

  /* Metric cards */
  .metric-grid {
    display: grid; grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px; margin: 20px 0;
  }
  .metric-card {
    background: var(--bg2); border-radius: 8px; padding: 12px 14px;
  }
  .metric-card .label {
    font-size: 12px; color: var(--muted); margin-bottom: 4px;
  }
  .metric-card .val {
    font-size: 22px; font-weight: 500;
  }
  .metric-card .sub {
    font-size: 11px; color: var(--muted); margin-top: 4px;
  }

  /* Stepper navigation (for stepper type) */
  .step-nav {
    display: flex; justify-content: space-between; align-items: center;
    margin: 16px 0;
  }
  .step-btn {
    background: transparent; border: 0.5px solid var(--border);
    border-radius: 6px; padding: 6px 16px; font-size: 13px;
    color: var(--text); cursor: pointer;
  }
  .step-btn:hover { background: var(--bg2); }
  .step-btn:disabled { opacity: 0.3; cursor: default; }
  .step-dots { display: flex; gap: 6px; }
  .step-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: var(--border);
  }
  .step-dot.active { background: var(--text); }

  /* Detail panel (for flowcharts with clickable nodes) */
  .detail-panel {
    background: var(--bg2); border-radius: 8px; padding: 16px;
    margin: 16px 0; display: none;
  }
  .detail-panel.visible { display: block; }
  .detail-panel h3 {
    font-size: 15px; font-weight: 500; margin-bottom: 8px;
  }
  .detail-panel p {
    font-size: 13px; color: var(--muted); line-height: 1.6;
  }

  /* Footer */
  .footer {
    text-align: center; font-size: 11px; color: var(--muted); margin-top: 16px;
  }

  /* SVG text inherits */
  svg text { font-family: inherit; }
</style>
</head>
<body>

<h1>TITLE</h1>

<!-- === CONTROLS === -->
<!-- Add slider rows, toggles, dropdowns here -->

<!-- === METRIC CARDS === -->
<!-- Add metric grid here if the type calls for it -->

<!-- === SVG CANVAS === -->
<svg id="main-svg" width="100%" viewBox="0 0 680 HEIGHT" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5"
      markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M2 1L8 5L2 9" fill="none" stroke="context-stroke"
        stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </marker>
  </defs>

  <!-- SVG content goes here -->

</svg>

<!-- === DETAIL PANEL (optional) === -->

<!-- === FOOTER === -->
<div class="footer">Methodology note or attribution</div>

<script>
// === STATE ===
// Define any state variables here

// === CORE UPDATE FUNCTION ===
function update() {
  // 1. Read all control values
  // 2. Run computations
  // 3. Update metric cards
  // 4. Update SVG elements
}

// === HELPER FUNCTIONS ===
// Statistical functions, path builders, etc.

// === INITIALIZATION ===
update();
</script>

</body>
</html>
```

## Adapting the template

### For calculators
- Keep all sections. Controls at top, metric cards below, SVG chart at bottom.
- The `update()` function does the heavy math.

### For steppers
- Remove metric cards. Add step navigation below the SVG.
- Replace `update()` with `renderStep(n)` that shows/hides SVG groups.
- Add keyboard listener: `document.addEventListener('keydown', e => { ... })`.
- Each step is a `<g id="step-N">` group in the SVG, all initially hidden
  except step 0.

### For flowcharts
- Remove controls and metric cards. The SVG is the main content.
- Add a detail panel div below the SVG.
- Nodes get `onclick` handlers that populate the detail panel.
- Add `cursor: pointer` and hover effects to clickable nodes.

### For illustrative
- Controls map to physical parameters (temperature, pressure, rate).
- Add CSS `@keyframes` for continuous animations (flow, convection).
- Wrap animations in `@media (prefers-reduced-motion: no-preference)`.
- JS toggles animation play state based on controls.

### For comparison
- Use two SVG elements side by side, or two `<g>` groups in one SVG.
- Toggle buttons swap which side is highlighted.
- Shared controls affect both sides but differently.

## Chart.js integration (when needed)

For complex charts (multiple datasets, annotations, time series), Chart.js
is the one allowed external dependency:

```html
<canvas id="chart" style="margin:2rem 0;height:240px;"></canvas>
<script src="https://cdnjs.cloudflare.com/ajax/libs/Chart.js/4.4.1/chart.umd.js"></script>
<script>
  // Chart.js initialization after the library loads
  const ctx = document.getElementById('chart').getContext('2d');
  const chart = new Chart(ctx, { /* config */ });

  // In update(), call chart.update() after changing data
</script>
```

For simple charts (one or two lines/areas), draw directly in SVG instead —
it's lighter and more controllable.

## Static SVG companion template

```svg
<svg width="680" height="HEIGHT" viewBox="0 0 680 HEIGHT"
  xmlns="http://www.w3.org/2000/svg"
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;">

  <!-- Light background (standalone SVG doesn't inherit page bg) -->
  <rect width="680" height="HEIGHT" fill="#FAFAF8" rx="12"/>

  <!-- Title -->
  <text x="40" y="38" font-size="16" font-weight="500" fill="#2C2C2A">TITLE</text>

  <!-- Parameter summary -->
  <text x="40" y="62" font-size="12" fill="#888780">Param A: value · Param B: value</text>

  <!-- Visual content with baked-in values -->
  <!-- Use inline fills and strokes — no CSS variables -->

  <!-- Footer -->
  <text x="340" y="BOTTOM" font-size="11" fill="#B4B2A9" text-anchor="middle">
    Default parameters: describe what was used
  </text>
</svg>
```

Key differences from interactive version:
- All colors are hardcoded hex (light mode palette)
- Computed paths use the default parameter values
- No `id` attributes needed (nothing to update)
- Include background rect (no page to inherit from)
- Include parameter summary so the snapshot is self-documenting
