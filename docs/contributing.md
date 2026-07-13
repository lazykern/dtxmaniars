# Contributing

Purpose: maintained contributor workflow for architecture, reference evidence,
and local quality gates.

Audience: code and documentation contributors.

Status: Maintained. Package-specific boundaries remain in each crate's
`AGENTS.md`; verified commands are being completed in Cycle 8.

Neighboring guides: [roadmap](roadmap.md), [decision records](decisions/README.md),
[compatibility](compatibility.md), and [data and persistence](data-and-persistence.md).

## Core rules

- Treat `references/` as read-only input. Mechanics are reference-first; UX is
  governed by current product decisions.
- Preserve the Pure → Engine → Game → Binary dependency direction in the root
  workspace handbook.
- Run the smallest relevant package/test target while developing, then the
  documented local release gates before merge.
- Keep CI/CD changes out of the current improvement program.

The complete command table and documentation-check workflow will be populated
before Cycle 8 is marked complete.
