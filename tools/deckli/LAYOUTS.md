# deckli Layout Selection Heuristics

## Standard PowerPoint Layouts

Most corporate templates include these standard layouts. Always run `deckli inspect --masters` to discover the actual names in your template.

| Layout Name | When to Use |
|---|---|
| Title Slide | Opening slide, section dividers |
| Title and Content | Standard content with heading + body |
| Section Header | Section dividers within deck |
| Two Content | Side-by-side comparison |
| Comparison | Labeled side-by-side comparison |
| Title Only | When you need custom shape placement |
| Blank | Full creative control, charts, diagrams |
| Content with Caption | Content with sidebar explanation |

## Placeholder Types

Placeholders are pre-positioned shapes in layouts. Address them by type when possible:

| Type | Typical Names | Usage |
|---|---|---|
| title | Title, Title 1 | Slide heading |
| body | Content Placeholder, Text Placeholder | Main body text |
| subtitle | Subtitle | Subtitle on title slides |
| footer | Footer Placeholder | Slide footer |

## Layout Selection Strategy

1. **Always inspect first** — template layouts vary widely
2. **Prefer layouts with placeholders** — they respect the template's design system
3. **Use "Blank" for custom layouts** — when adding many shapes manually
4. **Match content to layout** — two data points → "Two Content", not manual columns
