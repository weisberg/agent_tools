# deckli — Live PowerPoint Control from the Terminal

## What It Is

deckli bridges your CLI to a live, open PowerPoint document via a WebSocket-connected Office add-in. Commands execute instantly against the active presentation through Office.js.

## Setup

1. Start the bridge: `deckli connect`
2. Open PowerPoint and activate the deckli add-in (Insert > Add-ins > My Add-ins)
3. Verify: `deckli status` → should show bridge + add-in connected

## Workflow: Always Do This

1. **Inspect first**: `deckli inspect --masters` → learn available layouts and placeholders
2. **Use theme colors**: `deckli inspect --theme` → use theme colors, not hardcoded hex
3. **Batch operations**: Use `deckli batch --stdin` for multi-step changes (one round-trip)
4. **Verify visually**: `deckli render --slide N --out slide.png` after major changes

## Command Reference

### Inspect (read-only)
```
deckli inspect                        # Slide count, basic info
deckli inspect --masters              # Master → layout tree
deckli inspect --theme                # Colors and fonts
deckli inspect --slide 3              # Shapes on slide 3
```

### Read
```
deckli get /slides                    # All slides
deckli get /slides/3                  # Slide 3 detail
deckli get /slides/3/shapes/2         # Shape detail
deckli get /selection                 # Current selection
```

### Write
```
deckli set /slides/3/shapes/title/text "New Title"
deckli set /slides/3/shapes/2/fill "#2E75B6"
deckli set /slides/3/shapes/2/font --size 24 --bold
deckli set /slides/3/shapes/2/geometry --left 1in --top 2in --width 8in
```

### Add
```
deckli add slide --layout "Title Slide"
deckli add shape --slide 3 --type rectangle --left 1in --top 2in --width 4in --height 2in
deckli add image --slide 3 --src chart.png --left 6in --top 2in --width 5in --height 3in
deckli add table --slide 3 --data '[["Q1","Q2"],["$1M","$2M"]]' --left 1in --top 4in --width 10in --height 2in
```

### Remove & Reorder
```
deckli rm /slides/5
deckli rm /slides/3/shapes/2
deckli move /slides/5 --to 2
```

### Render (vision verification)
```
deckli render --slide 3                # Base64 PNG to stdout
deckli render --slide 3 --out s3.png   # Save to file
deckli render --all --out ./slides/    # All slides
```

### Batch
```
echo '[{"method":"add.slide","params":{"layoutName":"Blank"}},
       {"method":"add.shape","params":{"slideIndex":0,"type":"rectangle","left":72,"top":72,"width":288,"height":144}}]' \
  | deckli batch --stdin
```

## Units

All position/size arguments accept: `1in`, `72pt`, `2.54cm`, `100px`, `914400emu`, or bare numbers (points).

## Output

All commands return JSON with `{ success, command, result, timing_ms }` envelope. Errors include `{ code, message, suggestion }`.

## Error Recovery

- **shape_not_found**: Run `deckli inspect --slide N` to list shapes
- **layout_not_found**: Run `deckli inspect --masters` to list layouts
- **no_addin**: Open PowerPoint and activate the deckli add-in
