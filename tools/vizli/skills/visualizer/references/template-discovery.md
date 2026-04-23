# Template Discovery Playbook

vizli template selection happens through sidecars. The template body is not
the discovery surface.

## Fast search patterns

Search by tag first:

```bash
rg -n "bar-chart|line-chart|scatter-plot|heatmap|diagram|explainer" templates/ --glob "*.md"
```

Search by request language next:

```bash
rg -n "comparison|trend|distribution|flow|hierarchy|interactive" templates/ --glob "*.md"
```

List candidate sidecars near a directory:

```bash
find templates -name "*.vizli.*.md" | sort
```

If the repo does not use a `templates/` root, search the workspace for
sidecars first and adapt:

```bash
find . -name "*.vizli.*.md" | sort
```

## How to choose among candidates

Use this order:

1. `tags`
2. `description`
3. `formats`
4. `interactive`
5. Required `params`
6. The markdown body's "when to use" and "when NOT to use"

Reject a candidate if any of these fail:

- The requested format is not in `formats`
- The user needs interactivity and `interactive: false`
- The sidecar expects params or dataset structure the user does not have
- The sidecar explicitly says the request is a poor fit

## Data-shape heuristics

- One categorical field plus one numeric field: simple bar or similar comparison chart
- Time plus numeric: line or area chart
- Two numeric fields: scatter or regression plot
- Nodes and edges: network or architecture diagram
- Process stages and arrows: flow or pipeline diagram
- User wants sliders, toggles, or teaching interaction: HTML explainer

## Read sidecars like an operator

When reading a sidecar, capture only:

- Template path
- Engine type
- Supported formats
- Required params
- Expected dataset shape
- The "when to use" and "when NOT to use" rules

Anything else is secondary until the template is selected.
