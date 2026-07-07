//! Selection overlay (fleshed out in the next task). Stub: exposes the corner
//! scale-handle marker so the gesture state machine can query it.

use bevy::prelude::*;

/// One of the four corner scale handles; index 0..4 = TL, TR, BL, BR.
#[derive(Component, Clone, Copy)]
pub struct ScaleHandle(pub usize);
