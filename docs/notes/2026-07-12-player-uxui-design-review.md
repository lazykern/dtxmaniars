# DTXManiaRS UX/UI Design Review — Every Screen

Date: 2026-07-12
Status: Companion to `2026-07-12-player-ux-audit.md` (behavioral audit). That doc covers reachability/journeys; this one covers visual and interaction design per screen, judged against the project's own stated direction.
Sources: source sweep of `dtx-ui`, `game-menu`, `game-shell`, `game-results`, `gameplay-drums` (HUD, pause, practice, editor); 13 live 1080p screenshots; AGENTS.md ADR summaries.

> Correction (2026-07-13): The vendored reference is present at
> `references/DTXmaniaNX/`, and binding reconstructed ADRs now live under
> `docs/decisions/`. The original observation below described the repository
> state visible during that earlier audit.

**Original caveat on references:** The vendored reference and original ADR files were not visible in the audited snapshot. The design direction below was reconstructed from code comments: *"osu-inspired UX redesign. Game mechanics stay NX-ported; visuals are new"* (`dtx-ui/src/lib.rs:1-3`), *"300ms OutQuint (not 1500ms NX snapshot)"* (`lib.rs:22`), *"osu-lazer uses OutQuint for most UI fades"* (`easing.rs:3`). The review judges each screen against that yardstick: **fluid, motion-rich, dark-themed, feedback-forward**.

---

## 0. Verdict in one paragraph

The foundation is genuinely good: a real token module (`theme.rs`), a verified 300 ms OutQuint transition system, and a motion kit (spring, rolling number, beat pulse, enter choreography) that makes the title and song select feel like the osu-inspired game ADR-0014 describes. But the investment stops at song select. Gameplay, results, the stage banner, practice, and the entire Customize surface are static, and several finished animation behaviors are computed and then silently discarded. The single worst UI fact found: **normal play renders no gauge at all while the stage-failure rule defaults on** — the player can be failed by a meter that does not exist on screen. The system-level problems are (a) uneven motion adoption, (b) no typographic scale (~17 ad-hoc px sizes), (c) three competing highlight colors, and (d) fixed screen-px overlays colliding with the ref-px-scaled HUD.

---

## 1. Design system assessment

### 1.1 Tokens — good skeleton, leaky walls
`Theme` (`dtx-ui/src/theme.rs:37-61`) centralizes color: dark navy pair (`#1a1a2e`/`#16213e`), cyan accent `#00d4aa`, judgment colors, stage palette, difficulty colors, 9 lane colors. Real strengths: one place to retheme, semantic difficulty mapping, `judgment_color()` helper.

Leaks:
- **Three accents.** Theme accent cyan `#00d4aa` (HUD, sliders, pause selection) vs editor chrome `ACCENT #5b8cff` blue (`editor/chrome.rs:17-28`) vs selection-box gold `srgb(1,0.75,0.1)` (`selection_box.rs:43`). Same meaning — "this is selected/active" — three hues depending on surface. Plus `select_yellow #ffcc00` as a fourth selection signal in menus.
- **No Poor/Ok judgment color.** `judgment_color()` maps only PERFECT/GREAT/GOOD/MISS (`theme.rs:68-76`); the score panel invents its own purple for Ok (`score_detailed.rs:122`). The judgment popup renders "OK" plain white — the one judgment tier with no color identity, on the surface where tier identity matters most.
- **No spacing/size tokens.** Paddings, gaps, and every font size are inline literals.
- **Keyboard focus ring is red** (`FOCUS_RING srgb(0.89,0.20,0.20)`, `panel.rs:63-65`). Red reads as *error* everywhere else in the app (`chrome::ERR`, `judgment_miss`, destructive buttons). Focus ≠ error.

### 1.2 Typography — no scale, wrong-font alignment hacks
- All UI text is Bevy built-in `FontSource::SansSerif` via `Theme::font(px)` (`theme.rs:89-95`). The declared `FiraMono-subset.ttf` + `pt_to_px` path (`lib.rs:27,41-50`) is dead — no screen uses it.
- ~17 distinct px sizes in use (9–56). Named helpers exist (48/32/18/16) but call sites bypass them constantly. There is no modular scale; hierarchy per screen is improvised.
- **Results screen aligns columns with space padding** (`"Score     {}"`, `game-results/src/lib.rs:154`) — under a proportional sans-serif this cannot align. Either the intended monospace font was lost, or alignment has always been approximate.
- Critical information sits at illegible sizes: card micro-labels 9 px, editor warnings 10 px, footer legend 12 px (see per-screen notes).

### 1.3 Motion — a good kit, unevenly adopted, partly broken
- **Verified working:** 300 ms OutQuint black fade between all screens, gated through the transition director (`transition.rs:37-65`, `game-shell/transition.rs:45-77`); spring song wheel (stiffness 400/damping 0.82, ~250 ms settle); BPM-synced glow pulse on the selected wheel row; staggered `EnterChoreo` slide-ins on title/song-select/loading; key-cap flash decay in the keyboard viz (`keyboard_viz.rs:149-177`).
- **Computed and discarded:** combo bounce (1.3→1.0 over 200 ms) is calculated but never applied to any transform (`hud.rs:527-541` ignores `ComboDisplay::scale()`); judgment popup scale-as-it-fades is calculated and thrown away (`hud.rs:395` binds it to `_scale`). Two signature osu-style feedback moments exist in code and are invisible in game.
- **Dead paths:** `ParallaxInfo` system runs but no entity ever spawns one (ADR-0015 item d); `bevy_tweening::TweeningPlugin` registered, never used; `gradient_background_bundle` paints a flat color — `bg_top` is a token with no consumer; stage ambient art layer deliberately removed.
- **Static where motion is expected:** stage clear/fail banner (zero animation at the emotional peak), results reveal uses *linear* fades (the one easing the codebase's own comment says osu doesn't use), practice HUD, entire editor, toasts (hard 1.5 s cutoff, no fade).

### 1.4 Components — primitives exist only for forms
`dtx-ui` has 28 widget modules but no shared Button, Dialog, or Toast. Consequences: two independent toast systems (import: top-right, 5 s, uncolored; practice: top-center, 1.5 s, uncolored — `import_ui.rs:44-256`, `practice/toast.rs`), dialogs hand-built per case in the editor (they *are* visually consistent, by discipline not by reuse), and the only hover states in the whole game are the editor's tab/list buttons — `Slider`/`Stepper`/`Toggle` have pressed states but no hover (`controls.rs`).

### 1.5 Layout model — two coordinate systems that collide
Menus and HUD position in **ref-px (1280×720) × scale**; the editor chrome and practice rail use **fixed screen-px** (480 px panel, 240 px inspector, 340 px rail). At 1080p (scale ≈ 1.5) ref-scaled widgets grow while fixed panels don't — the confirmed practice-rail/Now-Playing collision is exactly this seam (`full_hud.rs:279` fixed 340 px + no text wrap vs `now_playing.rs` ref-scaled). Any future widget near a fixed panel inherits the same class of bug.

---

## 2. Per-screen review

Format: what works / what's off, with severity tags ((H)igh, (M)edium, (L)ow) feeding the behavioral audit's finding list.

### 2.1 Startup splash (`startup.rs`)
Works: instant, unambiguous, auto-advances at 0.5 s. Off: nothing worth fixing. **Verdict: no change.**

### 2.2 Title (`title.rs`)
Works: the most "designed" moment in the game — logo drop-in (450 ms OutQuint), chip slide-up, perpetual 6% beat pulse on `PRESS ENTER`, blue glow. Reads as intentional.
Off:
- (M) `PRESS ENTER` is a filled yellow button — the strongest click affordance anywhere — and clicks do nothing (behavioral F17). Either make it clickable or restyle as a text prompt.
- (M) No pad legend despite BD working here (behavioral F12); the footer advertises only keyboard.
- (L) Footer is 12 px; `ESC QUIT` is both tiny and instant-destructive (behavioral F11).
- (L) Background is flat `bg_bottom`; the `bg_top` gradient token is dead — the screen is darker/flatter than its own theme intends.
**Verdict: refinement.**

### 2.3 Song select (`song_select.rs` + widgets)
Works: the flagship. Spring-driven wheel with row expansion (78→122 px), distance fade, quadratic arc; selected-row glow pulsing at the previewed song's BPM — a genuinely osu-grade touch; staggered cluster entrances; semantic difficulty ladder (blue/yellow/red/purple); density graph; skill/BPM/history panels; empty-library panel with concrete instructions including the actual songs path.
Off:
- (H) **Search affordance is nearly invisible.** No input box, no caret, no focus state — a bare secondary-alpha string "type to search…" top-right that swaps to `search: {q}` (`song_select.rs:786-791,1778-1782`). The single most useful returning-player feature looks like a decorative label. Screenshot confirms it disappears into the corner.
- (M) `SORT: DEFAULT` is styled as a yellow chip/button (screenshot) but only keyboard `Tab` operates it — affordance promises click, delivers nothing.
- (M) Wheel rows are rectangular panels with per-row content — they look clickable and aren't (H1 territory; same as title).
- (M) Album-art swap has two competing drivers — the 150/220 ms crossfade tween (`album_art.rs:59-112`) and a direct alpha overwrite (`song_select.rs:1787-1816`). One of them wins by frame order; the crossfade the code cites osu line numbers for is likely never seen.
- (L) No-art placeholder is a blank 0.18-alpha square — a music-note glyph would separate "no art" from "broken".
- (L) Left column (SKILL/BPM/PLAY HISTORY) uses three stacked bordered panels for three numbers — chrome-heavy relative to information (screenshot shows mostly empty boxes).
- (L) Bottom hint bar: nine verbs at 12 px all-caps, one highlighted. Correct idea (progressive disclosure would be better), marginal execution.
**Verdict: refinement — protect the motion work, fix search/sort affordances.**

### 2.4 Song loading (`song_loading.rs`)
Works: hero card (art + NOW LOADING + title 34 px + progress) with slide-in; progress bar lerps forward only; status strings state phase ("parsing chart…", "loading audio chips… N/M").
Off:
- (M) **Difficulty chip is hardcoded EXTREME red** for every chart (`song_loading.rs:419` uses `difficulty_color(2)`) — wrong information, trivially fixable.
- (M) No stall/failure differentiation (LOAD-1): the bar just stops; failure flashes "failed — returning" at 12 px then auto-fades to song select.
- (L) `Esc` cancel is undiscoverable — no hint on the screen at all (and no pad path; behavioral F4).
**Verdict: refinement.**

### 2.5 Gameplay HUD (normal play)
Works: clean centered playfield (osu-style per `hud.rs:1`), correct BocuD lane palette with hollow secondaries, sub-frame interpolated scrolling, GITADORA dome key-caps with the game's best micro-animation (flash decay), detailed score panel with per-judgment colors, live graph with S/A/B threshold lines, signed early/late ms on the judgment popup.
Off:
- (H) **No gauge is rendered.** `gauge_bar.rs` exists (280 px track, OutQuad fill) and is never spawned; `StageGauge` is mechanics-only. With Damage Level defaulting to Small and stage failure on, the player's survival state is completely invisible until the FAILED banner. This is the single largest feedback hole in the game — it makes failure feel arbitrary and invalidates the damage setting (can't see what it changes). Manual §9's HUD widget list omits the gauge, so the gap is systemic (also absent from the Widgets customize list).
- (M) Combo bounce and judgment-popup grow-on-fade are computed but not applied (see 1.3) — the two highest-frequency feedback events (every hit, every combo tick) are static text swaps. The HUD *feels* less alive than the menus, inverted from where the energy should be.
- (M) Judgment popup is pinned at `left 44.8% / top 200px` of the screen (`judgment_popup.rs:59-75`) rather than anchored to the playfield — off-center relative to the lane strip whenever the playfield is moved/scaled in Customize.
- (L) "OK" judgment renders un-colored on the popup (token gap, see 1.2).
- (L) HitLine height disagreement: 4 px at spawn vs 3 px in layout sync (`hud.rs:204` vs `:295`) — one-frame flicker on entry.
- (L) Speed readout is 13 px in the far-left dead zone; distant-persona relevance near zero (H7).
**Verdict: partial redesign of feedback layer (gauge + hit feedback), rendering core is sound.**

### 2.6 Pause overlay (`pause.rs`)
Works: 0.72 dim, 48 px PAUSED, three 32 px options, pad legend when MIDI connected, instant snap (no fade — appropriate for a pause).
Off:
- (M) Selection is **color-only** (cyan vs 50%-white). At throne distance, cyan-vs-dim-white on three stacked lines is a squint test (screenshot confirms subtle difference). No marker, no background, no size change — the menus elsewhere use yellow border+glow; pause uses neither.
- (L) Practice suppresses this overlay entirely and substitutes the full HUD — same key, different world (behavioral F20).
**Verdict: refinement (selection affordance).**

### 2.7 Stage clear/failed banner (`stage_end.rs`)
Works: semantic color (cyan clear/red fail), auto-advance.
Off:
- (M) Zero motion at the highest-emotion moment in the loop — a static 48 px string for 1.6 s, in a codebase with EnterChoreo, springs, and beat pulse sitting unused. The osu-inspired direction is most conspicuously absent exactly here.
- (L) "Press Enter to continue" — no pad verb shown or accepted (behavioral F22).
**Verdict: refinement (celebrate/commiserate with the motion kit that already exists).**

### 2.8 Results (`game-results/src/lib.rs`)
Works: staggered row reveal (120 ms cadence) is the right instinct; content is complete (rank, counts, percentages).
Off:
- (H) **No visual hierarchy at all**: every row — song title, SCORE, rank, judgments, the exit hint — is the same 18 px `label_font`, same white (`lib.rs:250`). The screen the whole session builds toward has less typographic structure than the loading screen. Judgment rows ignore the theme's own judgment colors. Rank (SS…E) — the headline — is a plain text row.
- (M) Space-padded column alignment under a proportional font (see 1.2) — columns wobble.
- (M) Linear fade easing, contradicting the project's own OutQuint convention (`lib.rs:52-53` vs `easing.rs:3`).
- (M) ~1.7 s total reveal with no skip acknowledgment — input works but nothing communicates "save happens when you leave" (behavioral F5 pairs with this: the screen needs a saved/unsaved state line).
- (L) No retry/practice verbs (behavioral: distant-failure round-trip).
**Verdict: partial redesign — highest-value pure-UI screen to rebuild; data layer already delivers everything needed.**

### 2.9 End screen (`end.rs`)
Static "Thanks for playing", 1 s, exit. Fine. (L) A fade-out would cost one `ScreenFade` call. **Verdict: no change / trivial polish.**

### 2.10 Practice — quick tier (`mini_strip.rs`, `chip.rs`)
Works: deliberately minimal (10 px bottom strip + top-right status chip) — correct instinct to keep play unobstructed; chip string densely informative.
Off:
- (H) Minimalism collapses into invisibility: no control legend exists at any tier (behavioral F8), and the 10 px strip with a 2 px playhead is sub-perceptual at distance. The quick tier communicates state only to players who already know everything.
- (L) Toasts confirm actions with a hard cutoff, no fade (1.5 s, `toast.rs`).
**Verdict: refinement (one legend line; slightly taller strip when a loop is armed).**

### 2.11 Practice — full HUD (`full_hud.rs`)
Works: information architecture is right (transport / loop / trainer sections, attempt history, lane diagnosis, density timeline with bar ticks, drag-to-loop with snap); timeline interaction (click-seek ≤4 px slop, drag = bar-snapped loop) is well tuned.
Off:
- (H) **Layout is broken at 1080p** (screenshot + source root cause): fixed 340 px rail pinned `right:0` vs ref-scaled Now-Playing card; 32 px rail rows with no width constraint, no wrap policy, no clip — long labels ("Ramp target ◀ x1.00 ▶") wrap mid-token and overflow into the card. Also 18 rows × 32 px + headers ≈ 800 px: at 720p (648 px available after the 72 px timeline) the rail cannot fit at all.
- (M) 32 px for every row makes the rail shouty and long; values, labels, and hints share one size (same no-hierarchy disease as results).
- (M) Selected row is again color-only cyan (see pause).
- (M) Mouse can click transport and timeline but not rail rows — rows look identical to the clickable transport buttons one flex-row away (consistency violation inside a single screen).
- (L) "Exit practice" arming state is communicated only by a text swap to "Enter again to confirm" at the same size/color.
**Verdict: partial redesign — keep the IA, rebuild the rail as a scaled, wrapped, hierarchy-typed panel with mouse rows.**

### 2.12 Customize — chrome (tab strip, footer, scrim, stage outline)
Works: two-row SETTINGS/KIT tab grammar; live playfield preview behind a 0.72 scrim with Tab-peek; rounded stage outline on Widgets; hover-help footer that swaps to capture instructions contextually.
Off:
- (M) Fixed 480/240 px panels vs scaled preview (see 1.5) — on large windows the editor shrinks proportionally into a corner tool; on small ones it eats the preview.
- (M) Footer legend crams six chords into one 12 px line, bottom-right (screenshot: barely visible). It's the only place Ctrl+S/Tab-peek/Esc semantics are ever stated.
- (M) The amber "keyboard/mouse required — pads: SD to go back" — the single most important message for a pad user — renders at **10 px** (`panel.rs:302-311`). Right message, illegible delivery.
- (L) Tab buttons 12 px with a subtle active tint; active tab uses editor-blue while everything selected elsewhere is yellow/cyan/gold (see 1.1).
**Verdict: refinement (scale-aware panels, promote the warnings).**

### 2.13 Customize — settings tabs (Gameplay/Audio/Drums/System)
Works: consistent row grammar (dirty dot, label, stepper/slider), group headers, RESET TAB confirmation styling, coarse-adjust modifier, pad navigation with a clear enter/adjust state machine (green adjust ring, glyphs swap to −/+), hover-help descriptions per row.
Off:
- (M) Focus ring red / adjust ring green: red-for-focus collides with red-for-error/destructive everywhere else in the same panel (Reset tab is also red).
- (M) Dirty state = 6 px yellow dot, color-alone and nearly invisible (screenshot; behavioral F27). Settings save silently on close (behavioral F6) — so the only persistence cue is a 6 px dot.
- (L) Play Speed row carries no hazard styling despite being the one desync-dangerous control (behavioral F10) — visually identical to Damage Level.
- (L) Section headers at 10 px uppercase muted — fine at desk, gone at distance.
**Verdict: refinement.**

### 2.14 Customize — Controls tab
Works: the best-instrumented panel in the game. Segment toggle with clear active state; connection status dot (green/red); velocity meter with threshold tick and amber below-threshold fill + 150 ms decay; per-channel color dots matching lane colors; warning-tinted unbound rows; shared-source ⧉ marker; chips with inline ×; live spatial highlight into the preview playfield; capture modal with live note/velocity.
Off:
- (M) Chips/micro-labels at 9–11 px: a dense mapping table in eye-strain sizes.
- (M) `Reset tab` styling (dark red, 9 px) undersells that it wipes *both* segments + device fields (behavioral F7) — visual scope matches the visible segment, actual scope doesn't.
- (L) Segment active color is editor-blue; selected channel row border is also editor-blue; meanwhile the preview highlight for the same channel is a different system (BIND_OVERLAY) — two selection languages for one selection.
**Verdict: refinement — this panel's model is right; typography and reset scope need care.**

### 2.15 Customize — Lanes tab
Works: select-follows-click both directions (row ↔ preview pad); live drag with real-time width/order mutation is direct-manipulation done right; detail card cleanly separates width/channels/hide; Hidden strip makes restoration one click; undo spans everything.
Off:
- (M) **Zero drag affordance.** Rows are deliberately select-only with no handle glyph; the actual drag surface (preview pads, 6 px edge-grab zones) carries no visual hint — no cursor change, no handle, no edge highlight on hover. Players must discover the game's most physical interaction by accident (KM-F7; behavioral F18's research question). 6 px edge targets are also small for the payoff.
- (L) No ghost/insertion indicator during reorder — the live mutation is honest but ambiguous mid-drag about where a drop will land (drop = nearest center, invisible rule).
- (L) "Hide lane" button styled like Reset (dark red) though it's non-destructive and fully reversible — overstated danger, while Reset understates (2.14).
**Verdict: refinement (affordances), model is sound.**

### 2.16 Customize — Widgets tab
Works: strongest osu parity in the app — gold selection box with 4 corner handles, name tag, anchor/origin visualization (red anchor dot, gold origin dot, connecting line), hover outline, snap guides, Alt-click cycling, 3×3 anchor grid + auto toggle, per-mode visibility toggles, Reset Widget.
Off:
- (M) Inspector rows at 11 px with unbounded offset steppers (screenshot: `offset x -6666` accepted) — no clamp feedback, no "off-screen" warning; a widget can be nudged into the void with Reset as the only rescue.
- (M) Gold selection system vs blue chrome vs cyan theme (1.1) — three highlight colors visible simultaneously on this one screen.
- (L) Playfield appears in the widget list but selecting it yields no inspector and no explanation (`panel.rs:404-407`) — a silent dead end in the list UI.
- (L) Resize = corner handles only, no edge handles; scale slider in inspector duplicates the interaction without stating they're the same value.
**Verdict: refinement.**

### 2.17 Dialogs (close guard, name entry, delete, dirty-switch)
Works: consistent modal grammar (scrim 0.72, dark card, destructive=red, default=accent), block-cursor text entry, inline validation errors in red, width-tiered cards.
Off:
- (M) No keyboard focus traversal — visually the buttons look equally reachable; only mouse reaches Discard (behavioral F14). The default-focus accent on Save is the only focus indication that exists.
- (L) Corrupt-reset dialog is fully styled and unreachable (behavioral F9) — finished UI behind an unwired trigger.
**Verdict: refinement.**

### 2.18 Notifications (import + practice toasts)
Works: both are quiet, capped, and positionally sane.
Off:
- (M) **Import toasts are semantically uncolored** — success, duplicate, error, no-charts all render identical secondary-white (`import_ui.rs`). The import flow has six distinct outcomes the player must distinguish (stories §4) and the UI voices them in one tone. `chrome::OK`/`ERR`/`DIRTY` tokens already exist.
- (L) Two systems, no shared primitive; practice toasts hard-vanish. The `toast.rs:1` comment says generalize on the second consumer — the second consumer already exists.
**Verdict: small correction (color-code import outcomes), then consolidate.**

---

## 3. Consolidated UI defect list (new findings, beyond behavioral audit)

| # | Severity | Finding | Evidence |
|---|---|---|---|
| U1 | High | No gauge rendered in normal play; failure state invisible | `gauge_bar.rs` never spawned; `StageGauge` mechanics-only |
| U2 | High | Practice rail: fixed 340 px + unwrapped 32 px rows → collision/overflow at 1080p, cannot fit at 720p | `full_hud.rs:279,316-321`; screenshot |
| U3 | High | Results screen: single font size, no judgment colors, plain rank, proportional-font space alignment | `lib.rs:150-278` |
| U4 | High | Search has no input-field affordance | `song_select.rs:786-791` |
| U5 | Medium | Combo bounce + judgment-popup scale computed, never applied | `hud.rs:527-541`, `hud.rs:395` |
| U6 | Medium | Three selection/highlight accents (+ yellow) across surfaces; red focus ring reads as error | `theme.rs`, `chrome.rs:17-28`, `selection_box.rs:43`, `panel.rs:63-65` |
| U7 | Medium | Loading difficulty chip hardcoded EXTREME red | `song_loading.rs:419` |
| U8 | Medium | Import toasts semantically uncolored across six outcomes | `import_ui.rs:44-256` |
| U9 | Medium | Pause/practice selected row is color-only (cyan vs dim white) | `pause.rs:251-257`, `full_hud.rs:632-639` |
| U10 | Medium | Stage banner static; results fades linear — motion absent/off-convention at emotional peaks | `stage_end.rs`, `lib.rs:52-53` |
| U11 | Medium | Album-art crossfade fights direct alpha overwrite | `album_art.rs:59-112` vs `song_select.rs:1787-1816` |
| U12 | Medium | Lane drag surfaces carry zero affordance (no handles, hints, hover cue; 6 px edge grab) | `lane_drag.rs:130-186`, `lanes_panel.rs:267-288` |
| U13 | Medium | Critical text at 9–12 px: pad warning (10), footer legend (12), chips (9–11), amber warning | `panel.rs:302-311`, `footer.rs:24-25` |
| U14 | Medium | Judgment popup screen-pinned, not playfield-anchored | `judgment_popup.rs:59-75` |
| U15 | Medium | Widget offsets unclamped; off-screen widget with no warning (screenshot −6666) | `panel.rs` offset rows |
| U16 | Low | "OK" judgment has no color token | `theme.rs:68-76` |
| U17 | Low | HitLine 4 px spawn vs 3 px sync | `hud.rs:204,295` |
| U18 | Low | Dead design paths: bg gradient, ParallaxInfo, bevy_tweening, FiraMono/pt scale | `theme.rs:78-87`, `parallax.rs`, `lib.rs:27-50,104` |
| U19 | Low | Playfield row in widget list is a silent dead end | `panel.rs:404-407` |
| U20 | Low | "Hide lane" styled destructive-red though reversible; Reset understated though broad | `lanes_panel.rs`, `bindings_panel.rs:313-370` |

Strengths to protect: song-wheel spring + BPM glow, EnterChoreo entrances, 300 ms OutQuint transition director, keyboard-viz decay, velocity meter, anchor/origin visualization, modal grammar, empty-library panel, difficulty color semantics.

---

## 4. Where the design effort should go (per section, whole game)

| Section | Classification | Core move |
|---|---|---|
| Startup / End | No change | — |
| Title | Refinement | Resolve the fake button; pad legend; keep motion |
| Song select | Refinement | Real search field; clickable-or-not consistency; keep everything else |
| Loading | Refinement | Correct chip color; stall/failure states; cancel hint |
| Gameplay HUD | **Partial redesign (feedback layer)** | Ship the gauge; apply the already-built hit/combo animation; anchor popup to playfield |
| Pause | Refinement | Non-color selection marker |
| Stage banner | Refinement | Use the motion kit once |
| Results | **Partial redesign** | Typographic hierarchy + judgment colors + saved-state line + retry verb |
| Practice quick | Refinement | One legend line |
| Practice full HUD | **Partial redesign** | Scale-aware, wrapped, hierarchical rail; mouse rows |
| Customize chrome | Refinement | Scale-aware panels; promote warnings out of 10 px |
| Settings tabs | Refinement | Focus-ring semantics; dirty visibility; hazard-style Play Speed |
| Controls tab | Refinement | Type sizes; reset scope styling |
| Lanes tab | Refinement | Drag affordances + drop indicator |
| Widgets tab | Refinement | Clamp offsets; unify selection color |
| Dialogs | Refinement | Keyboard focus traversal |
| Notifications | Small correction → consolidate | Color-code import outcomes; one toast primitive |
| Design system | **Fundamental (but cheap) consolidation** | One accent + one focus semantics; a 5-step type scale; kill dead paths; decide ref-px vs screen-px once |

Sequenced: U1 (gauge) → U2 (practice rail) → U3 (results) → U4 (search) → U5 (wire the dead animations) → system consolidation (U6, type scale) → the refinement tail.
