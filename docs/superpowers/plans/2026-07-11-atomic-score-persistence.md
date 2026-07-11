# Atomic Score Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A completed score (and config/score.ini) survives interruption during file replacement: write to a sibling temp file, flush to disk, rename over the destination; any failure preserves the last valid file.

**Architecture:** One shared pure helper `atomic_write` in `dtx-core` (the pure base crate both `dtx-scoring` and `dtx-config` already depend on), then swap the three direct-overwrite sinks onto it. Linux durability detail: `sync_all` the temp file BEFORE rename, and fsync the parent directory AFTER rename (otherwise the rename itself can be lost on power cut).

**Tech Stack:** std only (`File::create`/`write_all`/`sync_all`, `fs::rename`). No tempfile crate — the helper is 30 lines and dependency-free.

**Source basis (verified 2026-07-11):**
- Target 1: `crates/dtx-scoring/src/store.rs:191-206` `ScoreStore::save` — `serde_json::to_vec_pretty` + `std::fs::write(path, bytes)` at :204 (truncate-then-write; crash mid-write corrupts scores.json).
- Target 2: `crates/dtx-config/src/lib.rs:333-340` `save()` — `toml::to_string_pretty` + `std::fs::write` (config.toml).
- Target 3: `crates/dtx-scoring/src/score_ini.rs` — `write_result` :286-317 (`std::fs::write` at :316) and `write_bgm_adjust` :220/:247.
- No atomic-write/rename/tempfile/sync_all usage exists anywhere in the workspace (grepped).
- Error types to preserve: `ScoreStoreError::{Io, Json, UnsupportedVersion}` (store.rs:140-151), `ConfigError::{Io, Parse, Serialize}` (dtx-config lib.rs:288-292).
- dtx-core is Pure (no bevy); both dtx-scoring and dtx-config depend on it (`crates/dtx-scoring/Cargo.toml`, `crates/dtx-config/Cargo.toml` — verify the dtx-core edge exists in each before Task 2; if dtx-config lacks it, it gains a pure-to-pure dep, which is layer-legal).
- Test conventions: dtx-scoring integration tests in `crates/dtx-scoring/tests/` (`store_v2.rs`, `edge_cases.rs` are the models).

---

### Task 1: `atomic_write` helper in dtx-core

**Files:**
- Create: `crates/dtx-core/src/fsio.rs`
- Modify: `crates/dtx-core/src/lib.rs` (add `pub mod fsio;`)

- [ ] **Step 1: Write the failing tests**

`crates/dtx-core/src/fsio.rs` (tests first, in-file per convention):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("dtx_fsio_tests");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn writes_new_file() {
        let p = tmp("new.json");
        std::fs::remove_file(&p).ok();
        atomic_write(&p, b"hello").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"hello");
    }

    #[test]
    fn replaces_existing_content_completely() {
        let p = tmp("replace.json");
        atomic_write(&p, b"a-much-longer-first-payload").unwrap();
        atomic_write(&p, b"short").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"short"); // no trailing bytes
    }

    #[test]
    fn no_temp_file_left_behind() {
        let p = tmp("clean.json");
        atomic_write(&p, b"x").unwrap();
        let dir = p.parent().unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn failure_preserves_original() {
        let p = tmp("preserved/sub.json");
        // parent "preserved" exists as a FILE -> temp creation must fail
        let parent = tmp("preserved");
        std::fs::remove_dir_all(&parent).ok();
        std::fs::remove_file(&parent).ok();
        std::fs::write(&parent, b"i am a file").unwrap();
        assert!(atomic_write(&p, b"x").is_err());
        assert_eq!(std::fs::read(&parent).unwrap(), b"i am a file");
        std::fs::remove_file(&parent).ok();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dtx-core -j 2 fsio`
Expected: FAIL — module/function missing (after adding `pub mod fsio;` with an empty body the tests fail to compile — that counts).

- [ ] **Step 3: Implement**

```rust
//! Crash-safe file replacement. Write sibling temp -> fsync -> rename over
//! destination -> fsync parent dir. On any failure the destination is
//! untouched (roadmap: "Preserve prior score/config files when replacement
//! fails").

use std::io::Write;
use std::path::Path;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?
        .to_string_lossy()
        .into_owned();
    let tmp_path = dir.join(format!(".{file_name}.tmp{}", std::process::id()));

    let result = (|| {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?; // data hits disk before the rename can be observed
        std::fs::rename(&tmp_path, path)?;
        // fsync the directory so the rename itself survives power loss (Linux).
        #[cfg(unix)]
        {
            if let Ok(d) = std::fs::File::open(&dir) {
                let _ = d.sync_all();
            }
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}
```

(The pid suffix keeps concurrent processes from clobbering each other's temp; same-directory placement guarantees same-filesystem rename.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dtx-core -j 2 fsio`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-core
git commit -m "feat(dtx-core): atomic_write with fsync + dir-fsync durability"
```

---

### Task 2: ScoreStore::save goes atomic

**Files:**
- Modify: `crates/dtx-scoring/src/store.rs:191-206`
- Modify: `crates/dtx-scoring/Cargo.toml` (only if `dtx-core` isn't already a dep — check first)

- [ ] **Step 1: Write the failing test**

In `crates/dtx-scoring/tests/store_v2.rs`:

```rust
#[test]
fn save_leaves_no_temp_and_replaces_atomically() {
    let dir = std::env::temp_dir().join("dtx_store_atomic_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.json");

    let mut s = ScoreStore::with_path(path.clone());
    s.add(/* build an entry the way store_v2.rs's existing tests do */);
    s.save().unwrap();
    let first = std::fs::read(&path).unwrap();

    s.add(/* second entry */);
    s.save().unwrap();
    let second = std::fs::read(&path).unwrap();
    assert_ne!(first, second);
    // valid JSON both times, and no temp litter
    let _: serde_json::Value = serde_json::from_slice(&second).unwrap();
    let leftovers = std::fs::read_dir(&dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    std::fs::remove_dir_all(&dir).ok();
}
```

(Copy the entry-construction from the file's existing tests.)

- [ ] **Step 2: Swap the write**

In `save()` replace `std::fs::write(path, bytes)?;` (:204) with:

```rust
dtx_core::fsio::atomic_write(path, &bytes)?;
```

`ScoreStoreError::Io(#[from] io::Error)` already absorbs the error type — no signature change. Keep the `create_dir_all` above it.

- [ ] **Step 3: Run the scoring suite**

Run: `cargo test -p dtx-scoring -j 2`
Expected: PASS (all existing round-trip/migration tests + the new one).

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-scoring
git commit -m "fix(scoring): atomic scores.json replacement"
```

---

### Task 3: Config save goes atomic

**Files:**
- Modify: `crates/dtx-config/src/lib.rs:333-340`

- [ ] **Step 1: Write the failing test**

In dtx-config's test mod (existing save/load tests at lib.rs:342-454 are the model):

```rust
#[test]
fn config_save_is_atomic_and_clean() {
    let dir = std::env::temp_dir().join("dtx_config_atomic_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let cfg = Config::default();
    save_to(&cfg, &path).unwrap(); // adapt to the actual save API (save() may take a path or use default_path)
    let loaded = /* load from path the way existing tests do */;
    assert_eq!(loaded.gameplay.play_speed, cfg.gameplay.play_speed);
    let leftovers = std::fs::read_dir(&dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    std::fs::remove_dir_all(&dir).ok();
}
```

(First `grep -n 'pub fn save' crates/dtx-config/src/lib.rs` — mirror the real signature; the existing tests show how a custom path is injected.)

- [ ] **Step 2: Swap the write**

Replace the `std::fs::write` in `save()` with `dtx_core::fsio::atomic_write(&path, contents.as_bytes())` (map the io::Error into `ConfigError::Io` as the current code does — `?` should just work if `From<io::Error>` exists). dtx-config already depends on dtx-core.

- [ ] **Step 3: Run tests**

Run: `cargo test -p dtx-config -j 2`
Expected: PASS. Note: `persist_hovered_selection` saves config on every hover change — atomic replacement also fixes a real corruption window there.

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-config
git commit -m "fix(config): atomic config.toml replacement"
```

---

### Task 4: score.ini writes go atomic

**Files:**
- Modify: `crates/dtx-scoring/src/score_ini.rs` (`write_result` :286-317, `write_bgm_adjust` :220/:247)

- [ ] **Step 1: Locate every `std::fs::write` in the file**

Run: `grep -n 'fs::write' crates/dtx-scoring/src/score_ini.rs`
Expected: the :316 site plus the `write_bgm_adjust` site(s).

- [ ] **Step 2: Swap each for `dtx_core::fsio::atomic_write`**

Same mechanical substitution as Task 2 (these are read-modify-write flows — the read half is untouched).

- [ ] **Step 3: Run tests**

Run: `cargo test -p dtx-scoring -j 2`
Expected: PASS (score_ini round-trip tests exercise the new path).

- [ ] **Step 4: Commit**

```bash
git add crates/dtx-scoring
git commit -m "fix(scoring): atomic score.ini replacement"
```

---

### Task 5: Interruption simulation (roadmap success check)

"A completed score survives simulated interruption during replacement."

**Files:**
- Modify: `crates/dtx-scoring/tests/edge_cases.rs`

- [ ] **Step 1: Write the simulation test**

A crash mid-`atomic_write` leaves either (a) the old file intact + a temp file, or (b) the new file. Simulate (a) — the dangerous half — by planting a stale temp alongside a valid store and proving load ignores it and the next save clears nothing it shouldn't:

```rust
#[test]
fn stale_temp_from_interrupted_save_is_harmless() {
    let dir = std::env::temp_dir().join("dtx_interrupt_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.json");

    // last valid save
    let mut s = ScoreStore::with_path(path.clone());
    s.add(/* entry */);
    s.save().unwrap();

    // interrupted next save: temp exists with garbage, destination untouched
    std::fs::write(dir.join(".scores.json.tmp99999"), b"{ truncated garba").unwrap();

    // reload sees the last valid store
    let mut reloaded = ScoreStore::with_path(path.clone());
    reloaded.load().unwrap();
    assert_eq!(reloaded.len(), 1);

    // and a subsequent save still succeeds
    reloaded.add(/* second entry */);
    reloaded.save().unwrap();
    let mut again = ScoreStore::with_path(path.clone());
    again.load().unwrap();
    assert_eq!(again.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p dtx-scoring -j 2 stale_temp`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/dtx-scoring
git commit -m "test(scoring): interrupted-save simulation preserves last valid store"
```

---

## Verification (whole plan)

1. `cargo test -p dtx-core -p dtx-scoring -p dtx-config -j 2` green.
2. `grep -rn 'fs::write' crates/dtx-scoring crates/dtx-config` → zero hits outside tests.
3. Manual: play a song, confirm `scores.json` updates and no `.tmp` files remain in its directory; `kill -9` the game during rapid hover changes (config saves) and confirm config.toml still parses on next launch.
