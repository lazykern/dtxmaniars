# Cycle 2A Parser, Channel, and Discovery Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correctly select conditional DTX branches, support NX's complete SE01-SE32 range, and discover/import `.dtx` files regardless of filename case.

**Architecture:** Add a deterministic conditional preprocessor in the pure `dtx-core` layer and expose parse reports without breaking existing `parse` callers. Centralize SE classification on `EChannel::is_se()` so every parser/gameplay path shares the same channel range. Centralize case-insensitive chart-extension matching in `dtx-library` and reuse it for scans and archive validation.

**Tech Stack:** Rust 1.95, pure dtx-core parser, dtx-library filesystem/archive tests, gameplay-drums scheduling tests.

## Global Constraints

- Complete Cycle 0 before this plan; Cycle 1 may execute before or after this plan.
- Port conditional semantics and channel values from `references/DTXmaniaNX/`.
- Keep `parse(reader) -> Result<Chart>` source-compatible.
- Conditional tests use explicit seeds; production callers must not depend on two parses choosing the same branch.
- Unknown channels remain skippable and nonfatal.
- Do not advertise or discover GDA/BMS/BME in this cycle.
- Do not change CI/CD configuration or workflows.

---

## File map

- Create: `crates/dtx-core/src/conditional.rs` — conditional directive parser/state machine
- Create: `crates/dtx-core/tests/fixtures/conditional_branches.dtx`
- Create: `crates/dtx-core/tests/fixtures/conditional_nested.dtx`
- Modify: `crates/dtx-core/src/lib.rs` — module and API re-exports
- Modify: `crates/dtx-core/src/parser.rs` — parse options/reports and preprocessing
- Modify: `crates/dtx-core/tests/parser_tests.rs` — fixture-level conditional behavior
- Modify: `crates/dtx-core/tests/parser_edge_cases.rs` — warning/recovery behavior
- Modify: `crates/dtx-core/src/channel.rs` — SE06-SE32 and `is_se`
- Modify: `crates/dtx-core/src/chip_classify.rs` — centralized SE classification
- Modify: `crates/dtx-core/src/trigger_pipeline.rs` — centralized SE triggers
- Modify: `crates/gameplay-drums/src/se_scheduler.rs` — SE32 scheduling/replacement
- Modify: `crates/gameplay-drums/src/seek.rs` — SE skip-set seeding
- Modify: `crates/gameplay-drums/src/sound_bank.rs` — SE32 preload classification
- Modify: `crates/dtx-library/src/lib.rs` — shared extension rule and scan
- Modify: `crates/dtx-library/src/import.rs` — archive chart counting
- Modify: `crates/dtx-library/tests/import.rs` — uppercase archive fixture

### Task 1: Add deterministic conditional preprocessing and parse reports

**Files:**

- Create: `crates/dtx-core/src/conditional.rs`
- Modify: `crates/dtx-core/src/lib.rs:1-50`
- Modify: `crates/dtx-core/src/parser.rs:1-55`
- Test: `crates/dtx-core/src/conditional.rs`

**Interfaces:**

- Produces: `ParseOptions { random_seed: u64 }`
- Produces: `ParseReport { chart: Chart, warnings: Vec<ParseWarning> }`
- Produces: `ParseWarning { line: usize, kind: ParseWarningKind }`
- Preserves: `parse<R: Read>(reader) -> Result<Chart>`

- [ ] **Step 1: Write failing state-machine tests**

Create `conditional.rs` with a test module first. Tests must call the not-yet-implemented `select_active_lines` and assert:

```rust
#[test]
fn explicit_seeds_select_opposite_branches() {
    let src = "#RANDOM 2\n#IF 1\n#TITLE: One\n#ENDIF\n#IF 2\n#TITLE: Two\n#ENDIF\n";
    let (one, warnings) = select_active_lines(src, 0);
    assert!(warnings.is_empty());
    assert!(one.iter().any(|(_, line)| *line == "#TITLE: One"));
    assert!(!one.iter().any(|(_, line)| *line == "#TITLE: Two"));

    let (two, warnings) = select_active_lines(src, 1);
    assert!(warnings.is_empty());
    assert!(two.iter().any(|(_, line)| *line == "#TITLE: Two"));
    assert!(!two.iter().any(|(_, line)| *line == "#TITLE: One"));
}

#[test]
fn inactive_parent_forces_nested_branch_inactive() {
    let src = "#RANDOM: 2\n#IF 2\n#IF 1\n#TITLE: Hidden\n#ENDIF\n#ENDIF\n#TITLE: Visible\n";
    let (lines, warnings) = select_active_lines(src, 0);
    assert!(warnings.is_empty());
    assert!(!lines.iter().any(|(_, line)| line.contains("Hidden")));
    assert!(lines.iter().any(|(_, line)| line.contains("Visible")));
}

#[test]
fn malformed_structure_warns_without_panicking() {
    let src = "#ENDIF\n#RANDOM nope\n#IF nope\n#TITLE: Recovered\n";
    let (_, warnings) = select_active_lines(src, 0);
    assert!(warnings.iter().any(|w| w.kind == ParseWarningKind::UnmatchedEndIf));
    assert!(warnings.iter().any(|w| w.kind == ParseWarningKind::InvalidRandom));
    assert!(warnings.iter().any(|w| w.kind == ParseWarningKind::InvalidIf));
    assert!(warnings.iter().any(|w| matches!(w.kind, ParseWarningKind::UnclosedIf { .. })));
}
```

- [ ] **Step 2: Run and observe missing implementation failures**

Run:

```bash
cargo test -p dtx-core conditional::tests -- --nocapture
```

Expected: compile failure for missing types/functions.

- [ ] **Step 3: Implement warning types and deterministic selection**

Implement these public data types in `conditional.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    pub line: usize,
    pub kind: ParseWarningKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseWarningKind {
    InvalidRandom,
    InvalidIf,
    UnmatchedEndIf,
    UnclosedIf { depth: usize },
    ConditionalDepthExceeded,
}
```

Use a process-independent deterministic generator for explicit seeds:

```rust
struct SeededSelector(u64);

impl SeededSelector {
    fn choose(&mut self, max: u32) -> u32 {
        let selected = (self.0 % u64::from(max.max(1))) as u32 + 1;
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        selected
    }
}
```

Implement:

```rust
pub(crate) fn select_active_lines(text: &str, seed: u64) -> (Vec<(usize, &str)>, Vec<ParseWarning>)
```

The line pass must:

- recognize case-insensitive `#RANDOM`, `#IF`, and `#ENDIF` in `name value`, `name:value`, and attached numeric forms;
- execute `#RANDOM` only under an active parent;
- default invalid/nonpositive values to 1 and emit the corresponding warning;
- push inherited inactivity when the parent is inactive;
- accept at most 255 nested `#IF` levels and warn/ignore excess levels safely;
- consume conditional directives rather than returning them to the ordinary parser;
- return every active ordinary line with its original one-based line number;
- warn once at EOF with the remaining unclosed depth.

- [ ] **Step 4: Add parse options/report without breaking `parse`**

In `parser.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    pub random_seed: u64,
}

impl Default for ParseOptions {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Self { random_seed: time ^ COUNTER.fetch_add(1, Ordering::Relaxed) }
    }
}

#[derive(Debug)]
pub struct ParseReport {
    pub chart: Chart,
    pub warnings: Vec<crate::conditional::ParseWarning>,
}
```

Change `parse` and add `parse_with_options`:

```rust
pub fn parse<R: Read>(reader: R) -> Result<Chart> {
    parse_with_options(reader, ParseOptions::default()).map(|report| report.chart)
}

pub fn parse_with_options<R: Read>(mut reader: R, options: ParseOptions) -> Result<ParseReport> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let text = decode_dtx_text(&bytes);
    let (lines, warnings) = crate::conditional::select_active_lines(&text, options.random_seed);
    let mut chart = Chart::default();
    for (line_no, line) in lines {
        process_line(line, line_no, &mut chart)?;
    }
    resolve_bpm_ex_chips(&mut chart);
    Ok(ParseReport { chart, warnings })
}
```

Declare `pub mod conditional` and re-export `ParseWarning`, `ParseWarningKind`, `ParseOptions`, `ParseReport`, and `parse_with_options` from `dtx-core/src/lib.rs`.

- [ ] **Step 5: Run pure parser tests**

Run:

```bash
cargo test -p dtx-core conditional::tests -- --nocapture
cargo test -p dtx-core --lib parser -- --nocapture
```

Expected: all tests pass and existing `parse` callers compile unchanged.

- [ ] **Step 6: Commit conditional infrastructure**

```bash
git add crates/dtx-core/src/conditional.rs crates/dtx-core/src/parser.rs crates/dtx-core/src/lib.rs
git commit -m "feat: parse conditional DTX branches"
```

### Task 2: Add conditional compatibility fixtures

**Files:**

- Create: `crates/dtx-core/tests/fixtures/conditional_branches.dtx`
- Create: `crates/dtx-core/tests/fixtures/conditional_nested.dtx`
- Modify: `crates/dtx-core/tests/parser_tests.rs`
- Modify: `crates/dtx-core/tests/parser_edge_cases.rs`

**Interfaces:**

- Consumes: `parse_with_options` from Task 1
- Produces: reproducible branch, nesting, and warning regression coverage

- [ ] **Step 1: Add exact fixture contents**

Create `conditional_branches.dtx`:

```text
#TITLE: Conditional Branches
#BPM: 120
#RANDOM 2
#IF 1
#WAV01: branch-one.wav
#00111: 01
#ENDIF
#IF 2
#WAV02: branch-two.wav
#00113: 02
#ENDIF
```

Create `conditional_nested.dtx`:

```text
#TITLE: Conditional Nested
#BPM: 120
#RANDOM: 2
#IF 2
#RANDOM2
#IF1
#00112: 01
#ENDIF
#IF2
#00114: 02
#ENDIF
#ENDIF
```

- [ ] **Step 2: Add fixture-level tests**

In `parser_tests.rs`, parse `conditional_branches.dtx` with seeds 0 and 1. Assert seed 0 contains only `HiHatClose` and WAV slot 1, while seed 1 contains only `BassDrum` and WAV slot 2. Parse `conditional_nested.dtx` with outer seed 1 and assert exactly one drum chip survives.

In `parser_edge_cases.rs`, add explicit tests for unmatched `#ENDIF`, unclosed `#IF`, invalid arguments, and lowercase directives; assert warning kinds and that active metadata still parses.

- [ ] **Step 3: Run fixture and edge-case tests**

Run:

```bash
cargo test -p dtx-core --test parser_tests conditional -- --nocapture
cargo test -p dtx-core --test parser_edge_cases conditional -- --nocapture
```

Expected: all conditional fixture and warning tests pass.

- [ ] **Step 4: Commit conditional fixtures**

```bash
git add crates/dtx-core/tests/fixtures/conditional_branches.dtx crates/dtx-core/tests/fixtures/conditional_nested.dtx crates/dtx-core/tests/parser_tests.rs crates/dtx-core/tests/parser_edge_cases.rs
git commit -m "test: cover conditional DTX compatibility"
```

### Task 3: Complete SE01-SE32 across pure and gameplay layers

**Files:**

- Modify: `crates/dtx-core/src/channel.rs`
- Modify: `crates/dtx-core/src/chip_classify.rs`
- Modify: `crates/dtx-core/src/trigger_pipeline.rs`
- Modify: `crates/gameplay-drums/src/se_scheduler.rs`
- Modify: `crates/gameplay-drums/src/seek.rs`
- Modify: `crates/gameplay-drums/src/sound_bank.rs`

**Interfaces:**

- Produces: `EChannel::SE06` through `EChannel::SE32`
- Produces: `EChannel::is_se() -> bool`
- Consumes: one centralized SE predicate in classification, trigger, preload, seek, and scheduler paths

- [ ] **Step 1: Write failing channel-range tests**

Add to `channel.rs` tests:

```rust
#[test]
fn all_nx_se_channel_values_round_trip() {
    let values = [
        0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
        0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x90, 0x91, 0x92,
    ];
    for value in values {
        let channel = EChannel::from_byte(value).expect("known NX SE channel");
        assert!(channel.is_se(), "0x{value:02X}");
        assert_eq!(channel as u8, value);
    }
    assert!(!EChannel::BGM.is_se());
    assert!(!EChannel::BassDrum.is_se());
}
```

Add a parser test for `#00092: 01` asserting the chip channel is `SE32`.

- [ ] **Step 2: Run and observe missing variants**

Run:

```bash
cargo test -p dtx-core all_nx_se_channel_values -- --nocapture
```

Expected: failure because values above SE05 are unknown.

- [ ] **Step 3: Add the exact NX values and shared predicate**

Extend `EChannel` with:

```rust
SE06 = 0x66, SE07 = 0x67, SE08 = 0x68, SE09 = 0x69,
SE10 = 0x70, SE11 = 0x71, SE12 = 0x72, SE13 = 0x73, SE14 = 0x74,
SE15 = 0x75, SE16 = 0x76, SE17 = 0x77, SE18 = 0x78, SE19 = 0x79,
SE20 = 0x80, SE21 = 0x81, SE22 = 0x82, SE23 = 0x83, SE24 = 0x84,
SE25 = 0x85, SE26 = 0x86, SE27 = 0x87, SE28 = 0x88, SE29 = 0x89,
SE30 = 0x90, SE31 = 0x91, SE32 = 0x92,
```

Add matching `from_byte` arms and:

```rust
pub const fn is_se(self) -> bool {
    matches!(self as u8, 0x61..=0x69 | 0x70..=0x79 | 0x80..=0x89 | 0x90..=0x92)
}
```

- [ ] **Step 4: Replace every five-variant match**

In `chip_classify::classify`, return `ChipClass::SE` before the match when
`ch.is_se()`, and retain a final debug-asserted fallback so Rust's
exhaustiveness checker accepts the now-grouped SE variants while every current
non-SE variant remains explicit. In `trigger_pipeline::trigger_for`, handle
`chip.channel.is_se()` before its existing wildcard-bearing match. Replace the
local predicates/matches in `se_scheduler.rs`, `seek.rs`, and `sound_bank.rs`
with `channel.is_se()`.

Use this final classification arm after the explicit non-SE arms:

```rust
other => {
    debug_assert!(other.is_se());
    ChipClass::SE
}
```

Update module comments from `SE01-SE05` to `SE01-SE32`. Extend scheduler tests to assert SE32 is scheduled/replacing and `seed_skip_sets` tests to assert an SE32 chip before the target enters `played_se`.

- [ ] **Step 5: Run pure and gameplay SE tests**

Run:

```bash
cargo test -p dtx-core channel -- --nocapture
cargo test -p dtx-core trigger_pipeline -- --nocapture
cargo test -p gameplay-drums --lib se_ -- --nocapture
cargo test -p gameplay-drums --lib sound_bank -- --nocapture
cargo test -p gameplay-drums --lib seek -- --nocapture
```

Expected: SE01-SE32 round-trip, classify, trigger, preload, seek, and scheduler tests pass.

- [ ] **Step 6: Prove no five-channel match remains and commit**

Run:

```bash
rg -n "SE01.*SE02.*SE03.*SE04.*SE05" crates/dtx-core crates/gameplay-drums
```

Expected: no production match remains; test data matches are acceptable only when explicitly testing endpoints.

```bash
git add crates/dtx-core/src/channel.rs crates/dtx-core/src/chip_classify.rs crates/dtx-core/src/trigger_pipeline.rs crates/dtx-core/tests/parser_tests.rs crates/gameplay-drums/src/se_scheduler.rs crates/gameplay-drums/src/seek.rs crates/gameplay-drums/src/sound_bank.rs
git commit -m "feat: support DTX SE01 through SE32"
```

### Task 4: Make `.dtx` discovery and archive counting case-insensitive

**Files:**

- Modify: `crates/dtx-library/src/lib.rs:140-180`
- Modify: `crates/dtx-library/src/import.rs:205-230`
- Modify: `crates/dtx-library/tests/import.rs`
- Test: `crates/dtx-library/src/lib.rs`

**Interfaces:**

- Produces: `pub fn is_dtx_path(path: &Path) -> bool`
- Consumes: the same predicate in recursive scan and recursive import count

- [ ] **Step 1: Add failing filesystem tests**

Add a temp-directory test in `dtx-library/src/lib.rs` that writes `UPPER.DTX` with a valid minimal chart, calls `scan_directory`, and asserts the song is returned. Add an import test whose ZIP contains `Song/MASTER.DTX` and assert `chart_count == 1`.

- [ ] **Step 2: Run and observe current case-sensitive failure**

Run:

```bash
cargo test -p dtx-library uppercase -- --nocapture
```

Expected: the scan/import assertions fail because only lowercase `.dtx` is counted.

- [ ] **Step 3: Implement and reuse one extension predicate**

Add to `dtx-library/src/lib.rs`:

```rust
pub fn is_dtx_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dtx"))
}
```

Use `is_dtx_path(&path)` in `walk_dtx`. Use `crate::is_dtx_path(&path)` in `import::count_dtx`. Update comments that currently claim lowercase/case-sensitive behavior.

- [ ] **Step 4: Run scan and import tests**

Run:

```bash
cargo test -p dtx-library --lib -- --nocapture
cargo test -p dtx-library --test import -- --nocapture
```

Expected: uppercase, lowercase, and existing import tests all pass; GDA/BMS/BME remain ignored.

- [ ] **Step 5: Commit discovery compatibility**

```bash
git add crates/dtx-library/src/lib.rs crates/dtx-library/src/import.rs crates/dtx-library/tests/import.rs
git commit -m "fix: discover DTX charts case-insensitively"
```

### Task 5: Run the Cycle 2A gate

**Files:**

- Test: dtx-core, dtx-library, gameplay-drums, workspace

**Interfaces:**

- Consumes: Tasks 1-4
- Produces: verified parser/channel/discovery deliverable

- [ ] **Step 1: Run focused and workspace verification**

Run:

```bash
cargo test -p dtx-core
cargo test -p dtx-library
cargo test -p gameplay-drums --lib
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: every command exits 0.

- [ ] **Step 2: Inspect scope**

Run:

```bash
git diff --check
git status --short
```

Expected: clean output; `references/` and CI/CD files are untouched.
