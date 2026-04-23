# deckli Workflow Recipes

## Recipe 1: Create a New Deck from Scratch

```bash
# 1. See what layouts are available
deckli inspect --masters

# 2. Add title slide
deckli add slide --layout "Title Slide"

# 3. Set title and subtitle
deckli set /slides/1/shapes/title/text "Q3 Business Review"
deckli set /slides/1/shapes/subtitle/text "Engineering Division — October 2025"

# 4. Add content slides
deckli add slide --layout "Title and Content"
deckli set /slides/2/shapes/title/text "Key Metrics"

# 5. Verify visually
deckli render --slide 1 --out title.png
deckli render --slide 2 --out metrics.png
```

## Recipe 2: Batch Create Multiple Slides

```bash
echo '[
  {"method":"add.slide","params":{"layoutName":"Title Slide"}},
  {"method":"add.slide","params":{"layoutName":"Title and Content"}},
  {"method":"add.slide","params":{"layoutName":"Two Content"}},
  {"method":"add.slide","params":{"layoutName":"Blank"}}
]' | deckli batch --stdin
```

## Recipe 3: Add a Chart-Style Layout

```bash
# Add a blank slide for custom layout
deckli add slide --layout "Blank"

# Add title
deckli add shape --slide 3 --type textbox \
  --left 0.5in --top 0.3in --width 9in --height 0.5in \
  --text "Revenue by Quarter"

# Add chart image
deckli add image --slide 3 --src revenue_chart.png \
  --left 0.5in --top 1in --width 9in --height 5.5in

# Verify
deckli render --slide 3 --out chart_slide.png
```

## Recipe 4: Modify Existing Presentation

```bash
# 1. Survey the deck
deckli inspect
deckli get /slides

# 2. Inspect specific slide
deckli inspect --slide 3

# 3. Update text and styling
deckli set /slides/3/shapes/title/text "Updated: Q3 Results"
deckli set /slides/3/shapes/2/fill "#2E75B6"
deckli set /slides/3/shapes/2/font --size 20 --bold

# 4. Reorder
deckli move /slides/5 --to 2
```

## Recipe 5: Vision Verification Loop

```bash
# Make changes
deckli set /slides/1/shapes/title/text "Final Review"

# Render and inspect
deckli render --slide 1 --out check.png

# If layout looks wrong, adjust
deckli set /slides/1/shapes/title/geometry --left 1in --top 2in --width 8in --height 1in

# Re-render to confirm
deckli render --slide 1 --out check2.png
```

## Recipe 6: Build a Data Table Slide

```bash
deckli add slide --layout "Title Only"
deckli set /slides/4/shapes/title/text "Performance Summary"

deckli add table --slide 4 \
  --data '[["Metric","Q1","Q2","Q3"],["Revenue","$1.2M","$1.5M","$1.8M"],["Users","10K","15K","22K"],["NPS","72","75","81"]]' \
  --left 0.75in --top 1.5in --width 8.5in --height 3in
```
