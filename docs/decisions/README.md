# Decision Records

Original ADR files predate this repo snapshot and were lost. The decisions
below are reconstructed from code comments and remain binding. Link new code
comments to this index.

- **ADR-0002 — Never judge on `Time::delta()`.** The gameplay clock free-runs
  and uses the BGM position only for drift correction, never as a gate.
  Implementation: `GameplayClock::tick` in `crates/gameplay-drums/src/resources.rs`,
  doc header of `crates/dtx-timing/src/lib.rs`.
- **ADR-0008 — Reference-first workflow.** Port behavior from
  `references/DTXmaniaNX-BocuD/` first; deviate only with a written reason.
- Other ADR numbers cited in code comments (0003, 0004, 0009, 0010, 0014,
  0015) refer to lost documents; when you touch code citing one, record the
  decision here from the surrounding comment.
