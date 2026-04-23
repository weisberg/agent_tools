# Vizli Failure Handling

When vizli fails, classify the failure before changing inputs.

## Failure classes

### 1. Selection failure

Symptoms:

- No sidecars match
- Several templates match but all are bad fits

Action:

- Say that the current template library does not cover the request
- Do not force a weak template

### 2. Payload mapping failure

Symptoms:

- Missing required params
- Unknown field names
- Dataset shape does not match the sidecar or schema

Action:

- Run `vizli inspect <template>` if you need the schema summary
- Read the sidecar again and remap the user's files
- Use `vizli validate <template> --payload ...` before re-rendering

### 3. Engine or format failure

Symptoms:

- User wants HTML from an SVG template
- Browser-only template cannot produce the expected artifact in the environment

Action:

- Check the template `type` and supported `formats`
- Offer the nearest supported format only if it still satisfies the user

### 4. Environment failure

Symptoms:

- Missing Playwright, CairoSVG, WeasyPrint, or related runtime dependencies
- Browser launch or font/render backend issues

Action:

- Run `vizli doctor`
- Report the missing prerequisite directly

### 5. Output quality failure

Symptoms:

- Render succeeds but the visual is clipped, mislabeled, or clearly wrong

Action:

- Check whether the payload mapping is wrong before blaming the template
- Confirm width, height, theme, and scale
- If the template is wrong, report that the template itself needs a fix

## Good operator behavior

- Change one variable at a time
- Prefer validation before repeated render attempts
- Quote the exact blocking param, field, or unsupported format
- If blocked, end with the smallest actionable next step
