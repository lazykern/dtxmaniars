# Wait Chord Feedback

## Goal

Make a practice wait halt unambiguous: display every required pad and show
which members of the chord have already been struck.

## Design

While `WaitState` is halted, a persistent top-centre prompt renders the
halted set in chart lane order. Unjudged members are bright; judged members
are suffixed with a check mark and muted. The prompt is removed immediately
when play resumes, wait mode is disabled, or a seek resets the wait state.

Example: `WAIT: SD ✓ + FT`.

The existing key-cap feedback remains the lane-level pulse: every physical
hit flashes its primary cap and a successful judgment colors the resolved
cap. The prompt supplies the missing chord-level explanation without changing
the 50 ms simultaneity rule.

## Tests

Pure formatting tests cover lane ordering, pending members, and checked
members. The UI system only maps that pure output onto Bevy text visibility.
