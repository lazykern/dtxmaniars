# Chart difficulty labels

## Goal
Show a chart's actual named difficulty instead of its compact position in a folder.

## Behavior
- `set.def` remains authoritative when it assigns a chart to `L1` through `L5`; use that level's label when present, otherwise its standard label.
- Without `set.def`, recognize common filename stems: `bsc`/`bas`/`basic`, `adv`/`advanced`, `ext`/`extreme`, `mas`/`mst`/`mstr`/`master`, and `edit`.
- Unknown names retain current ordinal labels, preserving existing imported folders.
- The selected-chart log and difficulty grid use same resolved label.

## Boundaries
No chart parsing, metadata, selection persistence, or folder sorting changes.

## Tests
Cover common filename aliases, `set.def` slot precedence, and ordinal fallback.
