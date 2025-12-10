# TUI2 Viewport Porting Execplan

This document captures the working plan for replaying the `joshka/viewport`
changes onto the new `tui2` crate, so the work can be resumed in later Codex
sessions without rediscovering the process.

## Baseline

- `main` already contains the `tui2` feature-flag plumbing:
  - `codex-tui2` originally started as a thin shim crate that delegated to
    `codex-tui`.
  - We now maintain a full copy of the `codex-rs/tui/src` tree under
    `codex-rs/tui2/src` so viewport and history work can evolve in `tui2`
    without destabilizing the legacy TUI.
  - `codex` CLI selects the frontend via the `tui2` feature flag, resolved from
    config (including profiles and CLI `-c` / `--enable` overrides).
- The `joshka/viewport` bookmark holds the original viewport work against the
  old TUI:
  - Use `jj log -r 'main..bookmarks("joshka/viewport")'` to list the relevant
    commits.
  - The refactor base is `rmntvvqt` ("refactor: tui.rs extract several pieces").
  - The first "real" viewport change after that is `kzvkyynm`
    ("feat: render transcript above composer").
- The original bookmark must remain untouched; new work is done on top of
  `main` using `tui2`.

## General JJ Workflow

- Always understand commands via `jj help`:
  - `jj help -k revsets` for revset syntax and idioms.
  - `jj log --help`, `jj diff --help`, `jj status --help`,
    `jj duplicate --help`, `jj desc --help`, `jj new --help` as needed.
  - For each viewport commit, inspect the **full commit message** (header and
    body) using:
    - `jj log -Tbuiltin_log_detailed -r <rev>`
    This ensures new `tui2` commits preserve at least the same level of
    documentation and context.

- Change lifecycle for each viewport port:
  - Before reading any code or tests for the next iteration, create a new
    change and set an initial description:
    - `jj new`
    - `jj desc -m "feat(tui2): <short summary>"`.
  - It is fine to read `jj log` output (including the original viewport
    commit’s detailed message) to decide on that summary before creating the
    change, but avoid opening source files until the new description is set.
  - As work progresses, update the change description to reflect the actual
    behavior and scope; never shrink the amount of context, only refine and
    expand it where helpful.

- Working copy and history:
  - `jj status` to confirm the current working change `@` and its parent `@-`.
  - `jj log -r 'main..bookmarks("joshka/viewport")'` to see the viewport
    commits that need to be replayed.
  - `jj diff -r @-` to see the diff between the previous change and the
    current working copy.

- New work:
  - Start a fresh change when beginning a new chunk of work, before touching
    any non-log files:
    - `jj new`.
  - Immediately set a descriptive change message:
    - `jj desc -m "feat(tui2): <short summary>"`.
  - Keep `jj status` clean and meaningful throughout, and refine the
    description after the implementation and verification steps so it matches
    the final behavior.

- Diff inspection:
  - For each viewport commit being ported, inspect the full original diff:
    - `jj diff -r <change-id>` (no `--stat`).
  - Use `--stat` (`jj diff -r <change-id> --stat`) only for a high-level
    overview; always read the detailed diff before porting.

- Optional duplication:
  - When helpful to preserve the original description or structure, duplicate
    a viewport commit on top of the current change:
    - `jj duplicate -r <change-id> -d @`
  - Then edit the duplicated change's contents to match the `tui2`
    implementation instead of the original `tui` changes.
  - This leaves the `joshka/viewport` bookmark unchanged.

## Porting Strategy Per Commit

For each viewport commit after `rmntvvqt`:

1. **Identify and understand the change**
   - Use `jj diff -r <viewport-change>` to see which files and behaviors are
     involved (typically `codex-rs/tui/src/app.rs`, `codex-rs/tui/src/tui.rs`,
     and related modules).
   - Classify the change:
     - Pure refactor (structure only).
     - New viewport behavior (scrolling, selection, transcript printing, etc.).
     - Bug fix around repainting/standby/suspend.
     - Documentation.

2. **Decide ordering**
   - Prefer to apply structural or helper changes earlier in the `tui2` tree
     if that makes later ports simpler, rather than strictly following the
     historical order.
   - It's acceptable to split a single historical commit into multiple smaller
     atomic changes in `tui2` if that improves clarity.

3. **Reuse vs copy**
   - **Prefer reuse** when the change can be expressed as:
     - "tui2 calls a public API in `codex-rs/tui`," or
     - "tui2 composes existing `tui` widgets/helpers."
   - Only consider widening visibility in `tui` (e.g. `pub(crate)` to `pub`)
     when:
     - The surface area is small, and
     - It avoids duplicating substantial logic.
   - **Default to copying** when reuse would require:
     - Making many internal `tui` details public, or
     - Broad changes to the `tui` crate's structure.
   - When copying:
     - Copy the minimal code required into `codex-rs/tui2` (e.g. a dedicated
       transcript/viewport module).
     - Adjust imports to use the same core/common types as the original.
     - Keep duplicated code localized to ease future comparison/cleanup.

4. **Implement in `tui2`**
   - Treat the tree as fresh: implement the behavior directly in `tui2`
     instead of trying to patch `tui` and then re-expose it.
   - Add new modules or types under `codex-rs/tui2/src/` as needed.
   - Where appropriate, delegate to `codex_tui` via the shim if that cleanly
     expresses the behavior.

5. **Testing and validation**
   - After each logically complete step:
     - Run `just fmt` in `codex-rs`.
     - Run `cargo check` (at minimum; `cargo test -p codex-cli` and
       `cargo test -p codex-tui2` when changes touch behavior).
   - If the change is too large, use `jj split` to break it into smaller
     atomic commits with clear messages.

## Visibility / Duplication Guidelines

- Avoid heavy changes to `tui` visibility:
  - Do **not** turn large swaths of `pub(crate)` APIs into `pub` just to
    support `tui2`.
  - If a refactor or visibility change would impact many call sites, prefer
    copying the relevant code into `tui2` instead.
- Before making any non-trivial visibility changes in `tui`:
  - Pause and validate the approach manually (or with the human reviewer).
  - Compare:
    - Lines changed in `tui`, vs.
    - Lines copied into `tui2`.
  - Prefer the option with the smaller, more localized impact.

## Progress Tracking

Use this section to keep track of how far the `joshka/viewport` work has been
ported into `tui2`. Update it at the end of each iteration.

- **Viewport port checklist (from `main..joshka/viewport`)**
  - [x] `rmntvvqt 83256977` – `refactor: tui.rs extract several pieces`
    - Baseline refactor that already landed on `main` before `tui2` was created; the copied `tui2` crate includes this structure (FrameRequester, SuspendContext, etc.) by construction.

  - [x] `kzvkyynm 1590c445` – `feat: render transcript above composer`
    - Ported into `codex-rs/tui2` by:
      - Teaching `App::handle_tui_event` to reserve a bottom-aligned chat area and render the transcript above it via a new `render_transcript_cells` helper (mirroring the original viewport behavior while keeping logic inside `app.rs` for now).
      - Updating `AppEvent::InsertHistoryCell` to append to `transcript_cells` and stop injecting vt100 history directly via `tui.insert_history_lines`, so the transcript is owned by Codex rather than the terminal scrollback.
      - Adjusting `Tui::draw` in `tui2/src/tui.rs` to stop using `scroll_region_up` or inserting `pending_history_lines` into the inline viewport, keeping the viewport stable while the transcript is rendered inside the TUI.
    - This preserves the existing “god-module” structure for now; future viewport commits may move transcript/viewport concerns into a dedicated module once the design stabilizes.

  - [x] `ssupompv 01a18197` – `feat: wrap transcript and enable mouse scroll`
    - Builds on the previous `kzvkyynm` port in `codex-rs/tui2/src/app.rs` and `codex-rs/tui2/src/tui.rs`:
      - `App::render_transcript_cells` now uses `word_wrap_lines_borrowed` from `tui2/src/wrapping.rs` to apply viewport-aware soft wrapping to transcript lines, while preserving the existing logic for spacing non-streaming history cells with a single blank line and avoiding gaps between streaming chunks.
      - The rendered transcript area is cleared before drawing and filled line-by-line, mirroring the original TUI’s wrapped transcript behavior above the composer.
      - `set_modes` / `restore` in `tui2/src/tui.rs` now disable alternate scroll and enable application mouse mode (`EnableMouseCapture` / `DisableMouseCapture`), ensuring scroll wheel events arrive as mouse events for the transcript viewport instead of being translated into Up/Down keys.
    - As with the previous commit, these changes remain in the existing app/tui modules rather than introducing new transcript-specific modules; we can factor this later if the viewport design stabilizes further.

  - [x] `xyqklwts 13ed4470` – `feat: add transcript scroll plumbing`
    - Introduces a dedicated transcript scroll state in `codex-rs/tui2/src/app.rs`:
      - Adds a `TranscriptScroll` enum with `ToBottom` and `Scrolled { cell_index, line_in_cell }` variants plus a `transcript_scroll` field on `App`, defaulting to `ToBottom`, so later viewport changes can distinguish between “pinned” and “scrolled” transcript positions.
      - Updates the inline transcript renderer to remain purely height/width driven for now; scroll state is stored but not yet applied, keeping behavior identical while making the state available.
    - Extends the TUI event plumbing in `codex-rs/tui2/src/tui.rs` and related screens:
      - Adds a `Mouse(crossterm::event::MouseEvent)` variant to `TuiEvent` and wires `Event::Mouse` in `event_stream` through to it.
      - Plumbs `TuiEvent::Mouse(_)` through `App::handle_tui_event` (currently as a no-op) and updates `model_migration.rs`, `onboarding/onboarding_screen.rs`, and `skill_error_prompt.rs` to handle the new variant without changing their behavior.
    - Aligns `tui2` with the legacy TUI’s alt-screen usage:
      - Wraps the main `App::run` call in `tui2/src/lib.rs`’s `run_ratatui_app` in `tui.enter_alt_screen()` / `tui.leave_alt_screen()`, so the combined chat + transcript viewport uses the full terminal while preserving normal scrollback on exit.
    - Keeps existing tests green:
      - Adjusts `App` test constructors in `tui2/src/app.rs` to initialize `transcript_scroll`, and re-runs `cargo test -p codex-tui2` to confirm behavior and snapshots remain unchanged.

  - [x] `wzwouyux 99c761fa` – `feat: implement transcript scrolling statefully`
    - Builds on the scroll plumbing by teaching TUI2 to maintain a stable transcript viewport when scrolled:
      - Refactors inline transcript rendering in `codex-rs/tui2/src/app.rs` to use a shared `build_transcript_lines` helper that returns both the flattened `Line` buffer and a parallel metadata vector mapping each line back to `(cell_index, line_in_cell)` or `None` for spacer lines.
      - Updates `render_transcript_cells` to:
        - Clear the transcript area and reset `transcript_scroll` to `ToBottom` when there are no lines.
        - Compute the top offset from the current `TranscriptScroll` anchor (or fall back to the bottom if the anchor is no longer present), and render only the visible slice of lines.
    - Implements mouse-driven scrolling anchored to history cells:
      - Adds `handle_mouse_event` and `scroll_transcript` on `App`, interpreting scroll wheel events over the transcript area as ±3-line deltas and updating `transcript_scroll` to either `ToBottom` or `Scrolled { cell_index, line_in_cell }` based on the nearest visible line.
      - Wires `TuiEvent::Mouse` through `handle_tui_event` to these helpers and schedules a redraw via `tui.frame_requester().schedule_frame()` whenever scroll state changes.
    - Keeps tests and snapshots stable:
      - Ensures the default behavior remains pinned to bottom (`TranscriptScroll::ToBottom`) so non-scrolled sessions render as before, and re-runs `cargo test -p codex-tui2` to verify all 512 tests pass with no snapshot updates required.

  - [x] `eac367c410170684a2d0689daf6270477f639529` – `tui: lift bottom pane with short transcript`
    - Adjusts the main inline layout so the chat composer sits directly beneath the rendered transcript when history is short:
      - Changes the `TuiEvent::Draw` path in `codex-rs/tui2/src/app.rs` to let `render_transcript_cells` return a `chat_top` row given the desired chat height, and positions the chat area starting at that row instead of always pegging it to the bottom of the terminal.
      - Clears only the region above the chat before drawing the transcript and fills any remaining rows *below* the chat to avoid stale content after layout changes.
    - Refines transcript rendering to respect a bounded transcript region above the chat:
      - Computes a `max_transcript_height = frame.height - chat_height` and renders at most that many lines of wrapped transcript, preserving the existing scroll anchoring and bottom-pinned behavior from the previous iteration.
      - When the transcript is shorter than the available space, places the chat immediately below the transcript with at most a single spacer line; when it is longer, retains the original “chat pinned to bottom” layout.
    - Keeps snapshot behavior unchanged:
      - The default bottom-pinned behavior remains identical once the transcript fills the viewport; the change only affects vertical placement when history is short, and `cargo test -p codex-tui2` continues to pass with existing snapshots.

  - [x] `7a814b470e2f60e16441834994a76ff2e4799d41` – `tui: restore mouse wheel scrolling in overlays`
    - Restores mouse wheel scrolling inside full-screen overlays so they behave consistently with the inline transcript view:
      - Extends `PagerView` in `codex-rs/tui2/src/pager_overlay.rs` with a `handle_mouse_scroll` helper that adjusts `scroll_offset` by a fixed 3-line step on `ScrollUp`/`ScrollDown` events and schedules a new frame via the shared `FrameRequester`.
      - Wires `TuiEvent::Mouse` through both `TranscriptOverlay::handle_event` and `StaticOverlay::handle_event`, delegating to `PagerView::handle_mouse_scroll` so transcript, diff, and approval overlays all respond to wheel input.
    - Simplifies terminal mode handling around alt-screen:
      - Drops the custom `EnableAlternateScroll`/`DisableAlternateScroll` commands in `tui2/src/tui.rs` and uses only application mouse mode (`EnableMouseCapture` / `DisableMouseCapture`), matching the upstream TUI.
      - Updates the suspend/resume path in `tui2/src/tui/job_control.rs` to enter/leave the alternate screen using only `EnterAlternateScreen` / `LeaveAlternateScreen`, keeping terminal behavior aligned while avoiding reliance on terminal-specific alternate scroll quirks.
    - Keeps overlay snapshots and behavior stable:
      - The scroll step matches the inline transcript (3 lines per wheel tick), and `cargo test -p codex-tui2` still passes with existing overlay snapshot tests.

  - [ ] `ppmpnvty 099b42e3` – `docs: document TUI viewport and history model`
    - Documentation-only change in the original TUI. TUI2 has its own execplan (`docs/tui2_viewport_execplan.md`); we may want to mirror or reference the upstream docs once the viewport work stabilizes.

  - [x] `tosqkrlr b0021eae` – `fix: clear screen after suspend/overlay`
    - Tracks the upstream fix that ensures the terminal is left in a predictable state around suspend/overlay flows:
      - TUI2 already clears the screen when re-entering alt-screen in `PreparedResumeAction::RestoreAltScreen`, matching the upstream behavior that avoids leaving stale overlay content behind.
      - The suspend-history plumbing that prints buffered history lines on suspend (and feeds into exit transcript behavior) is intentionally deferred to the later exit-transcript commits (`stsxnzvx`, `wlpmusny`, etc.) so those changes can be ported as a coherent unit.
    - No additional TUI2 code changes are required at this step beyond what is covered by the standby redraw fix below; the execplan records the mapping so we know this viewport-adjacent behavior has been audited.
  - [x] `xlroryvs 87fd5fd5` – `fix: redraw TUI after standby`
    - Makes the inline viewport resume path explicitly clear the screen after standby:
      - Updates `PreparedResumeAction::RealignViewport` in `codex-rs/tui2/src/tui/job_control.rs` so that, when resuming from suspend with a `RealignInline` action, the terminal is cleared after the viewport area is restored.
      - This mirrors the original TUI change that avoids leaving behind pre-standby artifacts when the inline viewport is shifted, ensuring the next draw starts from a clean buffer.
    - Keeps TUI2’s alt-screen behavior aligned with previous commits:
      - The `RestoreAltScreen` path continues to re-enter alt-screen and clear the terminal, but does not reintroduce alternate scroll; TUI2 remains on the simplified mouse/alt-screen model established in earlier viewport ports.
  - [x] `kpxulmqr 2cef77ea` – `fix: pad user prompts in exit transcript`
    - Exit transcript padding behavior is now ported to TUI2:
      - Extends `codex-rs/tui2/src/app.rs` with an ANSI renderer that flattens transcript `Line` buffers via `App::build_transcript_lines` and `insert_history::write_spans`, mirroring the upstream TUI pipeline.
      - Computes an `is_user_cell` bitmap from `transcript_cells` and pads user-authored rows out to the full terminal width using `crate::style::user_message_style`, so prompts appear as solid blocks in scrollback.
    - Wiring into the exit path:
      - `AppExitInfo` in both `tui` and `tui2` now includes a `session_lines: Vec<String>` field; the legacy TUI currently populates this as empty, while TUI2 fills it on shutdown using the ANSI renderer and current terminal width.
      - The top-level CLI’s `handle_app_exit` prints `session_lines` before token usage and resume hints, so TUI2 sessions emit a styled transcript after exit without changing TUI1 behavior.
  - [x] `wlpmusny b5138e63` – `feat: style exit transcript with ANSI`
    - Prepares TUI2 to reuse the same ANSI styling pipeline as the original TUI when printing exit transcripts:
      - Updates `codex-rs/tui2/src/insert_history.rs` to expose `write_spans` as `pub(crate)`, mirroring the upstream change in `tui/src/insert_history.rs` so other modules (like a future exit transcript renderer) can stream styled spans into a vt100 buffer.
      - Keeps the actual exit transcript printing and per-row styling deferred to `stsxnzvx`/`kpxulmqr`, where we will introduce `render_lines_to_ansi` and wire `session_lines` into TUI2’s `AppExitInfo`.
    - No user-visible behavior change yet:
      - The interactive TUI2 viewport and CLI output remain unchanged; this step is about matching the original TUI’s internal extension points so later exit transcript work can layer cleanly on top.
  - [x] `stsxnzvx 892a8c86` – `feat: print session transcript after TUI exit`
    - Implements the “print transcript on exit” behavior for TUI2:
      - `App::run` in `codex-rs/tui2/src/app.rs` now computes `session_lines` from `transcript_cells` at shutdown using the shared `build_transcript_lines` metadata and the new ANSI renderer.
      - The CLI adapter `from_tui2_exit_info` maps TUI2’s `session_lines` into the shared `codex_tui::AppExitInfo` type so `cli/src/main.rs` can print the transcript uniformly before the existing token usage / resume lines.
    - Legacy TUI stays stable:
      - TUI1’s `AppExitInfo` grows a `session_lines` field but all existing exit paths populate it as `Vec::new()`, so `handle_app_exit` prints the transcript only for TUI2-backed sessions.
  - [x] `ovqzxktt 7bc3a11c` – `feat: add clipboard copy for transcript selection`
    - Adds a transcript-selection copy path in `codex-rs/tui2/src/app.rs`:
      - Binds `Ctrl-Y` in the main view to a new `copy_transcript_selection` helper that re-renders the visible transcript region into an off-screen buffer and extracts the selected text based on the screen-space selection coordinates.
      - Uses the new `tui2/src/clipboard_copy.rs` helper (ported from the original TUI) to write the joined lines to the system clipboard via `arboard`, logging any failures without changing the on-screen UI yet.
    - Current TUI2 behavior:
      - Copying a transcript selection now works end-to-end for TUI2 sessions; footer hints and explicit “selection copied / copy failed” status messages will be introduced alongside the later scroll/copy hint work (`qnqzrtwo`) to keep that UX change grouped.
  - [x] `szttkmuz 08436aef` – `docs: describe streaming markdown wrapping`
    - Mirrors the original streaming wrapping design note for TUI2:
      - Adds `codex-rs/tui2/src/streaming_wrapping_design.md`, summarizing where streaming markdown is implemented in TUI2 (`markdown_stream`, `chatwidget`, `history_cell`, `wrapping`) and how it shares the same “pre‑wrap vs. reflow” tradeoffs as the legacy TUI.
      - Documents that TUI2 currently stays close to the legacy behavior while viewport work is being ported, and points to `tui2/src/wrapping.rs` as the primary abstraction for viewport‑aware wrapping.
  - [ ] `ylmxkvop 27265cae` – `feat: show transcript scroll position in footer`
  - [ ] `nlrrtzzr f9d71f35` – `feat: add keyboard shortcuts for transcript scroll`
  - [ ] `qnqzrtwo 2f20caac` – `feat: surface transcript scroll and copy hints`
  - [x] `xvypqmyw 4abba3b1` – `feat: add mouse selection for transcript`
    - Adds inline transcript selection tracking and rendering in `codex-rs/tui2/src/app.rs`:
      - Introduces a `TranscriptSelection` struct on `App` with `anchor` and `head` positions, and uses it to track a mouse-driven selection region within the transcript area.
      - Extends `render_transcript_cells` to call `apply_transcript_selection`, which walks the visible transcript rows, finds non-empty text spans, and applies a reversed style (`Modifier::REVERSED`) to the selected region while skipping the left gutter.
    - Extends `handle_mouse_event` to support click-and-drag selection:
      - Clamps mouse coordinates into the transcript area above the composer, using the same gutter offsets as rendering.
      - On wheel scroll, clears the current selection and delegates to `scroll_transcript` for ±3-line movement; on left-button down/drag/up, updates `TranscriptSelection` so click-drag selects and click-release on the same point clears.
    - Keeps tests and snapshots passing:
      - Updates test `App` constructors to initialize `transcript_selection` and re-runs `cargo test -p codex-tui2`, which passes without snapshot changes (selection is only visible when driven by input).
  - [x] `sxtvkutr ebd8c2aa` – `tui: make transcript selection-friendly while streaming`
    - Makes mouse-driven transcript selection behave sensibly while responses are streaming:
      - Detects when the chat widget is actively running a task and the transcript scroll mode is `ToBottom` (auto-follow).
      - When the user begins a mouse selection (or drags an existing one) in that state, converts the scroll mode into an anchored `TranscriptScroll::Scrolled` position so streaming output no longer moves the viewport under the selection.
    - Documents the interaction between streaming, scrolling, and selection in `tui2/src/app.rs`:
      - Adds a doc comment to `handle_mouse_event` outlining wheel, click/drag, and streaming-aware selection behavior.
      - Adds a doc comment to `scroll_transcript` and introduces `lock_transcript_scroll_to_current_view`, which flattens the transcript via `build_transcript_lines` and anchors scroll state to the current view top.
    - Keeps the rest of the TUI2 surface unchanged:
      - Wheel scrolling still clears selections and delegates to `scroll_transcript`, and existing snapshots remain valid because selection and streaming behavior only affect interactive navigation.

- **Last ported viewport change**:
  - `szttkmuz 08436aef` – `docs: describe streaming markdown wrapping`

- **Next planned viewport change to port**:
  - `ylmxkvop 27265cae` – `feat: show transcript scroll position in footer`

- **Estimated iterations**
  - There are ~19 viewport commits after `rmntvvqt`. Many are tightly related
    and can be grouped into fewer, more atomic `tui2` commits.
  - Expect roughly **10–15 local iterations**, each:
    - Focused on a coherent behavior slice (e.g. “render transcript above
      composer”, “add scroll state”, “add selection and copy”, “print exit
      transcript”).
    - Ending with `cargo check` and, where relevant, targeted tests for
      `codex-cli` / `codex-tui2`.

## Concrete Command Examples

- Show viewport commits relative to main:

  ```sh
  jj log -r 'main..bookmarks("joshka/viewport")'
  ```

- Inspect the first non-refactor viewport change:

  ```sh
  jj diff -r kzvkyynm          # full diff
  jj diff -r kzvkyynm --stat   # summary
  ```

- Start a new change on top of trunk/main:

  ```sh
  jj new
  jj desc -m "feat(tui2): render transcript above composer"
  ```

- Work on the change in `codex-rs/tui2`, then check progress:

  ```sh
  jj status
  jj diff                      # current working diff
  cargo check
  ```

- Optionally duplicate an original viewport commit for reference:

  ```sh
  jj duplicate -r kzvkyynm -d @
  # then edit files so the duplicated change uses tui2 instead of tui
  ```

- View the previous change’s diff when stacking work:

  ```sh
  jj diff -r @-
  ```

This plan should be sufficient to resume the TUI viewport porting work in a
future Codex session, without rediscovering the JJ tooling or the reuse-vs-copy
strategy for `tui2`. 
