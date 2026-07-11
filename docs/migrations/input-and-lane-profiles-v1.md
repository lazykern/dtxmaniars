# Input and Lane Profile Migration (v1)

Three independent, versioned TOML registries replace the mixed-schema
`bindings.toml` / `layout.toml [lanes]` model. All live next to
`config.toml` in `$XDG_CONFIG_HOME/dtxmaniars/`.

## Registries

| File | Owner crate | Authority |
|------|-------------|-----------|
| `keyboard-profiles.toml` | `dtx-config` | Keyboard channel→key mappings |
| `midi-profiles.toml` | `dtx-config` | MIDI port filter, velocity threshold, channel→note mappings |
| `lane-profiles.toml` | `dtx-layout` | Display lane order, widths, channel→lane map |

Schema (shared shape, `version = 1`):

```toml
version = 1
active = "Profile name"

[profiles."Profile name"]
# keyboard: CHANNEL = ["KeyX", ...]
# midi:     port / velocity_threshold / [profiles."...".map] CHANNEL = [note, ...]
# lanes:    order / widths / map
```

## Built-ins

Code-only, immutable, never written to disk:

- Keyboard: `DTXMania default`
- MIDI: `General MIDI drums`
- Lanes: `Classic`, `NX Type-B`, `NX Type-D`

Editing a built-in requires **Save As**. Built-in names are reserved.

## Name rules

Names trim whitespace, reject blank/control characters, cap at 48 chars,
and compare through a lowercase key: `"Desk"` and `"desk"` collide.
Built-in names cannot be reused.

## First-run migration matrix

| Registry state | Legacy file state | Result |
|----------------|-------------------|--------|
| missing | `bindings.toml` valid | keyboard/MIDI registries created from a checked partition of the legacy map; keys stay shared, duplicate MIDI notes abort migration before either write |
| missing | `layout.toml [lanes]` names a built-in preset | lane registry created with that built-in active, no user profile |
| missing | `layout.toml [lanes]` custom | lane registry created with a `Migrated lanes` user profile active |
| missing | legacy missing | default registry written |
| missing | legacy malformed | **no registry written**; session runs read-only on built-ins |
| present | anything | legacy input ignored entirely |

Keyboard and MIDI migrate independently: one failing write does not block
the other. A failed migrated-registry write keeps the migrated values for
the session (`LegacySession`) and retries on the next startup while the
target file is still missing (a missing parent directory counts as
missing, so the retry also covers that case).

**Legacy files are never modified or deleted.** `bindings.toml` and the
`layout.toml [lanes]` section remain on disk untouched.

## Lane snapshot precedence

Widget saves (`Ctrl+S`, close-time layout save) write `layout.toml`
`[scene]` plus a non-authoritative snapshot of the **last committed** lane
profile — never an unsaved draft. Whenever `lane-profiles.toml` exists,
startup ignores that snapshot; it exists only for compatibility with
older builds.

## Corrupt registry, backup, and reset

An unreadable or invalid registry puts that profile type into read-only
built-ins mode; every profile mutation is disabled. Recovery is an
explicit **Back up and reset** confirmation: the corrupt file is renamed
to `<file>.backup-<unix-millis>` via an atomic no-clobber hard-link (a
concurrently created backup can never be overwritten; a collision aborts
with the registry untouched), and only after the backup succeeds is the
default registry written. A failed backup leaves both files in place and
reports the error — a corrupt canonical file is never silently replaced.

## Transactions and re-read

Every profile action builds the complete next registry in memory, writes
it through the shared safe-replacement helper, and only then updates
runtime state. On failure the prior file, active selection, and draft are
kept, and the UI shows profile kind + canonical path + cause. After any
failure the next persistence action re-reads the canonical registry
first and aborts if it is malformed.

## Persistence limits

Writes go through `dtx-persistence`, which wraps `atomicwrites` with
overwrite enabled. There is no claim of Windows `ReplaceFileW`
backup-state recovery beyond what `atomicwrites` provides; the workspace
forbids `unsafe`, and no vetted safe wrapper offers that contract.
