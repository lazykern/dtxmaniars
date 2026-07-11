# Dirty Customize Close Dialog

## Problem

Changing keyboard, MIDI, or lane profiles makes Customize drafts dirty. Pressing
Escape raises `PendingCloseState`, but no UI renders that state and no click path
can submit `DiscardAll`. Escape then cancels the invisible guard, so users repeat
the same cycle and cannot intentionally discard changes and exit.

## Behavior

When Customize closes with dirty profile drafts, show a modal above editor UI:

- One dirty draft: `Cancel`, `Discard changes`, `Save changes`.
- Multiple dirty drafts: `Cancel`, `Discard all`, `Save all`.
- Escape chooses Cancel and keeps Customize open.
- Enter chooses Save and exits only after all writes succeed.
- Clicking Discard reverts dirty drafts and exits.
- Failed saves keep the modal open with only failed draft kinds pending.

Clean drafts continue closing immediately without a modal.

## Design

Reuse existing `PendingCloseState`, `dirty_dialog_layout`, `CloseDecision`, and
`reduce_close_decision`. Add only missing UI and input transport:

1. Spawn/despawn modal from `PendingCloseState` changes.
2. Represent each button with a component carrying `CloseDecision`.
3. Button presses write a small decision message.
4. Existing close resolver consumes keyboard or button decision and performs
   current save/discard/cancel logic.
5. Modal root uses editor chrome z-order and blocks pointer interaction with
   controls beneath it.

No new persistence layer, profile state model, or generic dialog framework.

## Testing

- Dirty MIDI close request raises visible modal state.
- Cancel clears pending state and keeps editor open.
- Discard restores saved draft and closes editor.
- Save success marks draft clean and closes editor.
- Existing clean-close behavior remains unchanged.
