# Common Utilities

Copy these into the `<script>` section as needed. Don't load external
libraries for these — they're small and self-contained.

## Statistical functions

### Normal distribution PDF
```javascript
function normalPDF(x, mu, sigma) {
  return Math.exp(-0.5 * ((x - mu) / sigma) ** 2) / (sigma * Math.sqrt(2 * Math.PI));
}
```

### Z-score from alpha (inverse normal approximation)
Rational approximation accurate to ~4 decimal places.
```javascript
function zFromAlpha(a) {
  const t = Math.sqrt(-2 * Math.log(a));
  return t - (2.515517 + t * (0.802853 + t * 0.010328))
           / (1 + t * (1.432788 + t * (0.189269 + t * 0.001308)));
}
```

### Two-proportion z-test sample size
```javascript
function sampleSizeTwoProp(p1, p2, power, alpha) {
  const za = zFromAlpha(alpha / 2);
  const zb = zFromAlpha(1 - power);
  const pBar = (p1 + p2) / 2;
  const n = ((za * Math.sqrt(2 * pBar * (1 - pBar))
            + zb * Math.sqrt(p1*(1-p1) + p2*(1-p2))) ** 2)
            / ((p2 - p1) ** 2);
  return Math.ceil(n);
}
```

### CUPED adjusted variance
```javascript
function cupedVariance(varY, varX, correlation) {
  // theta_star = cov(Y,X) / var(X) = correlation * sd(Y) / sd(X)
  // var(Y_adj) = var(Y) * (1 - correlation^2)
  return varY * (1 - correlation ** 2);
}
```

### Compound interest
```javascript
function compoundInterest(principal, rate, years, compPerYear) {
  return principal * Math.pow(1 + rate / compPerYear, compPerYear * years);
}
```

## SVG path builders

### Bell curve path
Generates an SVG path string for a normal distribution curve.
```javascript
function bellCurvePath(mu, sigma, xMin, xMax, yBase, yScale, steps) {
  let line = '', fill = '';
  for (let i = 0; i <= steps; i++) {
    const x = xMin + (xMax - xMin) * (i / steps);
    const px = LEFT_MARGIN + (x - xMin) / (xMax - xMin) * CHART_WIDTH;
    const py = yBase - normalPDF(x, mu, sigma) * yScale;
    const cmd = i === 0 ? 'M' : 'L';
    line += cmd + px.toFixed(1) + ' ' + py.toFixed(1);
    fill += cmd + px.toFixed(1) + ' ' + py.toFixed(1);
  }
  fill += `L${LEFT_MARGIN + CHART_WIDTH} ${yBase}L${LEFT_MARGIN} ${yBase}Z`;
  return { line, fill };
}
```
Replace `LEFT_MARGIN` and `CHART_WIDTH` with your layout constants
(typically 60 and 560 for a 680px viewBox).

### Smooth curve through points
For arbitrary data series. Uses cardinal spline interpolation.
```javascript
function smoothPath(points, tension) {
  tension = tension || 0.4;
  if (points.length < 2) return '';
  let d = `M${points[0][0]} ${points[0][1]}`;
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[Math.max(0, i - 1)];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[Math.min(points.length - 1, i + 2)];
    const cp1x = p1[0] + (p2[0] - p0[0]) * tension / 3;
    const cp1y = p1[1] + (p2[1] - p0[1]) * tension / 3;
    const cp2x = p2[0] - (p3[0] - p1[0]) * tension / 3;
    const cp2y = p2[1] - (p3[1] - p1[1]) * tension / 3;
    d += ` C${cp1x.toFixed(1)},${cp1y.toFixed(1)} ${cp2x.toFixed(1)},${cp2y.toFixed(1)} ${p2[0].toFixed(1)},${p2[1].toFixed(1)}`;
  }
  return d;
}
```

### Arc path (for gauge/dial indicators)
```javascript
function arcPath(cx, cy, r, startAngle, endAngle) {
  const s = startAngle * Math.PI / 180;
  const e = endAngle * Math.PI / 180;
  const x1 = cx + r * Math.cos(s);
  const y1 = cy + r * Math.sin(s);
  const x2 = cx + r * Math.cos(e);
  const y2 = cy + r * Math.sin(e);
  const large = (endAngle - startAngle) > 180 ? 1 : 0;
  return `M${x1} ${y1} A${r} ${r} 0 ${large} 1 ${x2} ${y2}`;
}
```

## Formatting helpers

### Number display (avoid floating point artifacts)
```javascript
function fmt(n, decimals) {
  decimals = decimals || 0;
  return Number(n.toFixed(decimals)).toLocaleString();
}

// Usage:
// fmt(3914)       → "3,914"
// fmt(0.1234, 2)  → "0.12"
// fmt(1234567)    → "1,234,567"
```

### Percentage display
```javascript
function pct(n, decimals) {
  decimals = decimals || 1;
  return (n * 100).toFixed(decimals) + '%';
}
```

### Duration display
```javascript
function duration(seconds) {
  if (seconds < 60) return Math.round(seconds) + 's';
  if (seconds < 3600) return Math.round(seconds / 60) + 'm';
  return (seconds / 3600).toFixed(1) + 'h';
}
```

## Color utilities

### Interpolate between two hex colors
Useful for heatmap-style SVG fills.
```javascript
function lerpColor(a, b, t) {
  const ah = parseInt(a.replace('#',''), 16);
  const bh = parseInt(b.replace('#',''), 16);
  const ar = (ah >> 16), ag = (ah >> 8 & 0xff), ab = (ah & 0xff);
  const br = (bh >> 16), bg = (bh >> 8 & 0xff), bb = (bh & 0xff);
  const rr = Math.round(ar + (br - ar) * t);
  const rg = Math.round(ag + (bg - ag) * t);
  const rb = Math.round(ab + (bb - ab) * t);
  return '#' + ((rr << 16) + (rg << 8) + rb).toString(16).padStart(6, '0');
}

// Usage: lerpColor('#534AB7', '#D85A30', 0.5) → midpoint purple-to-coral
```

## Animation patterns

### CSS keyframes for continuous flow (convection, data flow)
```css
@media (prefers-reduced-motion: no-preference) {
  @keyframes flow {
    to { stroke-dashoffset: -20; }
  }
  .flow-line {
    stroke-dasharray: 5 5;
    animation: flow 1.6s linear infinite;
  }
  .flow-line.paused {
    animation-play-state: paused;
    opacity: 0.3;
  }
}
```

### JS toggle for animation state
```javascript
function toggleAnimation(on) {
  document.querySelectorAll('.flow-line').forEach(el => {
    el.classList.toggle('paused', !on);
  });
}
```
