# Roadmap

Purpose: canonical status index for the approved end-to-end improvement program.

Audience: players, contributors, and maintainers deciding what is current.

Status: Maintained. Implementation and automated gates for cycles 0–9 are
complete. Manual desktop visual, audio, and physical-MIDI verification for
cycle 9 remains pending because the bounded launch attempts did not finish the
cold dynamic-linking build. Earlier verification evidence is recorded in the
[documentation inventory](notes/2026-07-13-documentation-inventory.md).

Neighboring guides: [player guide](player-guide.md),
[compatibility](compatibility.md), [data and persistence](data-and-persistence.md),
[contributing](contributing.md), and [decision records](decisions/README.md).

## Program status

| Cycle | Outcome | Status | Approved design and plan |
|---|---|---|---|
| 0 | Truthful local quality baseline | Complete | [Plan](superpowers/plans/2026-07-13-cycle-0-quality-baseline.md) |
| 1 | Playback-rate and score integrity | Complete | [Plan](superpowers/plans/2026-07-13-cycle-1-playback-score-integrity.md) |
| 2 | Core DTX and audio compatibility | Complete | [Design](superpowers/specs/2026-07-13-player-trust-compatibility-design.md), [parser plan](superpowers/plans/2026-07-13-cycle-2a-parser-channel-discovery.md), [audio plan](superpowers/plans/2026-07-13-cycle-2b-audio-diagnostics-fixtures.md) |
| 3 | Reliable guided calibration | Complete | [Design](superpowers/specs/2026-07-13-guided-calibration-design.md), [plan](superpowers/plans/2026-07-13-guided-calibration.md) |
| 4 | Results analysis and weakest-section practice handoff | Complete | [Design](superpowers/specs/2026-07-13-results-analysis-practice-handoff-design.md), [plan](superpowers/plans/2026-07-13-results-analysis-practice-handoff.md) |
| 5 | Large-library discovery and measured scan performance | Complete | [Design](superpowers/specs/2026-07-13-library-discovery-design.md), [plan](superpowers/plans/2026-07-13-library-discovery.md) |
| 6 | Accessibility and design-system consolidation | Complete | [Design](superpowers/specs/2026-07-13-accessibility-design-system-design.md), [plan](superpowers/plans/2026-07-13-cycle-6-accessibility-design-system.md) |
| 7 | Extended format/media compatibility | Complete | [Design](superpowers/specs/2026-07-13-extended-compatibility-design.md), [plan](superpowers/plans/2026-07-13-cycle-7-extended-compatibility.md) |
| 8 | Documentation and repository truth repair | Complete | [Design](superpowers/specs/2026-07-13-documentation-truth-repair-design.md), [plan](superpowers/plans/2026-07-13-cycle-8-documentation-truth-repair.md) |
| 9 | Mandatory Practice Setup, non-judged preview, saved loops, and completed-attempt Progress | Implementation and automated gates complete; manual desktop QA pending | [Design](superpowers/specs/2026-07-14-practice-mode-overhaul-design.md), [plan](superpowers/plans/2026-07-14-practice-mode-overhaul.md) |

Two approved supporting initiatives have complete implementations: [distant-kit system
binds design](superpowers/specs/2026-07-13-distant-kit-system-binds-design.md)
and [plan](superpowers/plans/2026-07-13-distant-kit-system-binds.md).

## Completion criteria

The program can be marked complete after all of the following hold:

- chart time, audio, visuals, seeking, and stage completion share one effective
  playback rate;
- modified, practice, and No Fail runs cannot overwrite ordinary records;
- supported charts are discovered independent of extension case and
  conditionals select one deterministic branch;
- supported media plays or produces an explicit diagnosis and recovery path;
- calibration reports confidence and cannot auto-apply weak evidence;
- Results explains a weakness and can open Practice at the recommended loop;
- every Practice request opens stopped, non-judged Setup before a run; Pause
  resumes exactly, while Settings continues from pre-roll;
- trainer mode is Off, Wait, or Ramp; only completed loop attempts enter
  Progress;
- saved practice loops require explicit lifecycle actions and remain isolated
  by canonical chart hash and selected difficulty;
- large libraries have explicit, composable discovery controls;
- focus and state never rely on color alone, motion can be reduced, and
  critical text can be scaled;
- player, contributor, compatibility, persistence, and decision documentation
  matches executable behavior;
- local documentation, formatting, workspace check, warnings-as-errors clippy,
  package tests, and workspace library tests pass;
- desktop checks cover the supported window sizes, visual layout, audible
  synchronization, saved-loop interaction, and physical MIDI navigation;
- `references/` remains unchanged and CI/CD files remain outside this program.

Cycle 8 passed this contract on 2026-07-13. Cycle 9 passed its focused package
tests, formatting check, workspace check, and warnings-as-errors clippy on
2026-07-15. Three bounded desktop launch attempts timed out while compiling, so
the required 1280x720 and 1920x1080 visual checks, audible synchronization,
mouse interaction, saved-loop interaction, and physical MIDI-kit checks remain
open. The earlier command and manual-check record is preserved in the
[documentation inventory](notes/2026-07-13-documentation-inventory.md).
