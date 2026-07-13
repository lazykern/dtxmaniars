# Library Discovery and Measured Scanning — Design

## Goals

Cycle 5 makes large libraries easier to explore without introducing a cache before
there is evidence that one is required. It adds Favorites, Unplayed, Recent,
Near My Level, and Random Within Results as combinable discovery filters. It
preserves a selected chart when a filter/sort/rescan still contains it, otherwise
clamps selection predictably. Every empty result names the active filters and
offers a reset.

## Architecture

A small versioned LibraryPreferences JSON file stores only favorites, keyed by
canonical chart identity/path-normalized chart key. It is separate from
ScoreStore because favorites are user choices, while score history remains a
record of completed compatible play.

SongSelect owns a DiscoveryFilters resource containing boolean favorite/unplayed
flags, optional recent and near-level constraints, and deterministic session RNG
state. It filters SongDb songs using LibraryPreferences plus ScoreStore history:
unplayed means no compatible native score for the chart; recent means one of the
most recently played compatible chart entries; near level compares dlevel with
the player's existing skill-derived target range. Random chooses only from the
already filtered visible set.

ScanReport expands from elapsed/discovered/loaded/problems to explicit directory
count, parsed chart count, skipped count, and an optional on-demand chart-stat
measurement. The scanner measures the whole scan and reports these values after
startup and F5. ChartStatsTask records selected-chart parse duration when it
finishes. Measurements are displayed in the existing nonblocking scan status
surface and logged; no cache or database is added in this cycle.

## UX

The song-select header gains a compact filter chip row and clear keyboard
shortcuts. Filters combine with existing text search and sorting. The empty
state says which active constraints produced zero songs and provides Reset
Filters. A random action selects from the current filtered results while
retaining current filters. The details/status line names startup versus rescan
timing, parsed/skipped counts, and latest selected-chart-stat cost.

## Safety and tests

Missing/corrupt preference data becomes an empty preference set and is reported
without preventing song discovery. Missing score history makes all songs
unplayed and omits Near My Level until a target can be derived. A selected chart
is matched by path before recomputation and restored when present.

Test pure filter intersections, favorite persistence/malformed recovery,
unplayed/recent/near-level eligibility, random candidate bounds, selection
preservation, empty-state copy, scan metric aggregation, and chart-stat timing.
Verify dtx-library and game-menu package suites, then workspace format/check/
Clippy/library tests.
