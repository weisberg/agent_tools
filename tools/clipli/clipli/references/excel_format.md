# Excel Clipboard HTML Quick Reference

## Critical Rules

- Use `background:` not `background-color:` — Excel ignores `background-color`
- Use `mso-pattern:black none` for cells WITH background; `auto` for cells WITHOUT
- Use `font-weight:700` not `bold`
- Alignment needs BOTH `align=` HTML attribute AND `text-align:` in inline style
- Column widths via `<col>` are ignored on clipboard paste — Excel auto-fits
- Empty cells must contain `&nbsp;`
- Font charset: Aptos family = `mso-font-charset:1`, Calibri = `0`

## Number Format Values for --col Flag

| Value | Display |
|-------|---------|
| `currency` | $4,230,000 (red negatives) |
| `accounting` | Accounting with dash for zero |
| `percent` | Percent (fractional input) |
| `percent_int` | 98% |
| `percent_1dp` | 15.6% |
| `integer` | 12,819 |
| `standard` | Like General with more decimals |
| `text` | Force text (prevent number detection) |
| `datetime_iso` | 2026-03-25 14:30 |

## Conditional Color Tiers (common pattern)

| Range | bg_color | fg_color |
|-------|----------|----------|
| 90-100% | `#A0D771` | `#628048` |
| 80-89% | `#FCCF84` | `#8B7449` |
| 60-79% | `#FBAD56` | `#986F3E` |
| 40-59% | `#E45621` | `white` |
| 0-39% | `#C92E25` | `white` |

## Table Styles

- `--style table` — Excel Table format: banded rows (#D9E1F2), blue-gray borders (#8EA9DB), full inline styles
- `--style plain` — Plain range: thick outer border (1.0pt windowtext), thin inner (.5pt), class-based
