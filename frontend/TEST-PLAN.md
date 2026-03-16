# Frontend Manual Test Plan

Use this checklist to validate the Proteus frontend MVP against the current backend.

## Setup

- [ ] Start the frontend:
  ```bash
  cd /Users/joey/dev/proteus-frontend/frontend
  npm run dev
  ```
- [ ] For backend-connected tests, start the backend in a second terminal:
  ```bash
  cd /Users/joey/dev/proteus-frontend/rust
  cargo run --bin proteus-server --features web
  ```
- [ ] Open the frontend in a browser, usually `http://localhost:5173`.

## Pass 1: No Backend

- [ ] Keep the backend stopped and load the frontend.
- [ ] Confirm the app renders without crashing.
- [ ] Confirm the status bar shows disconnected or reconnecting WebSocket state.
- [ ] Wait at least 10 seconds and confirm the app keeps retrying rather than freezing.
- [ ] Confirm the empty grid state is shown.
- [ ] Confirm the controls and config editor are visible and prefilled.
- [ ] Open and close the metrics drawer and confirm it does not crash with no data.

## Pass 2: Backend Startup

- [ ] Start the backend and refresh the page.
- [ ] Confirm the WebSocket status becomes connected.
- [ ] If no simulation exists, confirm the config editor remains editable.
- [ ] Confirm any stale connection error clears after successful startup.

## Config Editor

- [ ] Confirm the current local testing defaults are loaded:
  - `width = 64`
  - `height = 64`
  - `seed = 1`
  - `target TPS = 10`
  - `r_energy = 0.25`
  - `r_mass = 1.0` for now, with the original requested `1.25` blocked by backend probability validation
  - one lithotroph seed program is present near the center
- [ ] Click `Randomize Seed` and confirm the seed changes.
- [ ] Enter invalid values and confirm inline validation:
  - `width = 0`
  - `height = 2048`
  - negative integer fields
  - probability values below `0.0` or above `1.0`
- [ ] Add a seed program entry.
- [ ] Confirm fields exist for `x`, `y`, `code`, `free_energy`, and `free_mass`.
- [ ] Enter invalid bytecode like `999` and confirm validation appears.
- [ ] Remove the seed program and confirm it disappears.
- [ ] Save the config locally, change a few fields, then load it back and confirm the saved values are restored.

## Simulation Lifecycle

- [ ] Click `Create` with default config.
- [ ] Confirm the grid becomes available.
- [ ] Confirm the sim enters `created`.
- [ ] Confirm the config editor becomes locked.
- [ ] Click `Start`.
- [ ] Confirm the sim enters `running`.
- [ ] Confirm tick values increase.
- [ ] Confirm the grid updates visibly.
- [ ] Click `Pause`.
- [ ] Confirm the sim enters `paused`.
- [ ] Confirm tick values stop increasing.
- [ ] Confirm the last rendered grid remains visible immediately after pause and does not disappear.
- [ ] Enter step count `5` and click `Step`.
- [ ] Confirm tick advances by exactly 5.
- [ ] Click `Resume`.
- [ ] Confirm the simulation resumes updating.
- [ ] Click `Reset`, confirm the dialog, and verify tick returns to `0` with state `created`.
- [ ] Click `Destroy`, confirm the dialog, and verify the sim is removed, the config editor unlocks, and the grid returns to blank.
- [ ] After `Destroy`, confirm the last edited config values are still present instead of resetting to defaults.

## Control State Machine

- [ ] In `none`, confirm only `Create` is enabled.
- [ ] In `created`, confirm `Start`, `Reset`, and `Destroy` are enabled.
- [ ] In `running`, confirm `Pause`, `Reset`, and `Destroy` are enabled.
- [ ] In `paused`, confirm `Resume`, `Step`, `Reset`, and `Destroy` are enabled.

## Target Tick Rate Shim

- [ ] Confirm the target tick rate slider exposes exactly `1, 2, 5, 10, 30, 60, 120, max`.
- [ ] With target set below `max`, click `Start` and confirm the simulation advances without using backend native running mode.
- [ ] While a non-`max` target is active, confirm `Pause` stops the frontend stepping loop cleanly.
- [ ] While paused, click `Resume` and confirm the frontend stepping loop restarts.
- [ ] Change the target from one finite value to another while active and confirm the simulation speed changes without resetting.
- [ ] Change the target from a finite value to `max` while active and confirm control returns to backend-native running mode.
- [ ] Note that this is intentional tech debt and a temporary divergence from the current spec until backend speed control exists.

## Grid Rendering

- [ ] Create and start a simulation.
- [ ] Switch through all 8 color maps.
- [ ] Confirm recoloring is immediate and does not require a refetch.
- [ ] Zoom in and out with the mouse wheel.
- [ ] Confirm cells remain crisp and not blurred.
- [ ] Confirm zoom keeps the point under the cursor anchored instead of drifting away.
- [ ] Confirm minimum zoom is about `0.5x`.
- [ ] Confirm maximum zoom is about `64x`.
- [ ] Click-drag to pan and confirm movement feels natural and does not outrun the cursor.
- [ ] Double-click and confirm the grid fits the viewport.

## Cell Inspector

- [ ] Click a populated cell and confirm the sidebar switches to `Inspector`.
- [ ] Confirm coordinates and flat index are shown.
- [ ] Confirm resource values are always shown.
- [ ] For a populated cell, confirm ID, size, age, IP, registers, stack, and disassembly are shown.
- [ ] Confirm `dir` shows both numeric value and human-readable label.
- [ ] Confirm the current IP row is highlighted in the disassembly.
- [ ] Click an empty cell and confirm no program section is shown.
- [ ] While paused, click `Step` and confirm the inspector refreshes.
- [ ] While running, observe for at least 2 seconds and confirm auto-refresh occurs.
- [ ] If possible, confirm stale-data indication appears only when refresh falls behind.

## Status Bar

- [ ] While running, confirm updates for tick, population, energy, mass, and TPS.
- [ ] Confirm population includes live/inert breakdown.
- [ ] Confirm TPS updates roughly every 2 seconds.
- [ ] Confirm the color map selector works from the status bar.
- [ ] Confirm the charts toggle button opens and closes the metrics drawer.

## Metrics Drawer

- [ ] Let the simulation run for at least 10 seconds.
- [ ] Open the drawer and confirm it expands upward.
- [ ] Confirm the grid area shrinks rather than overlapping badly.
- [ ] Confirm all 5 charts render:
  - Population
  - Energy & Mass
  - Births / Deaths / Mutations
  - Program Size
  - Diversity
- [ ] Confirm charts advance with simulation tick history rather than wall-clock timestamps.
- [ ] Leave the simulation running and confirm data keeps accumulating.
- [ ] Pause and resume and confirm chart history continues rather than resetting.

## Sidebar and Layout

- [ ] Confirm the sidebar is a fixed right-side panel when open.
- [ ] Confirm the status bar remains visible at the bottom.
- [ ] Collapse the sidebar and confirm the grid expands.
- [ ] Re-open the sidebar and confirm layout restores cleanly.
- [ ] Switch between `Controls` and `Inspector` tabs several times.
- [ ] Open the metrics drawer with the sidebar both open and collapsed.
- [ ] Resize the browser window to smaller desktop sizes and confirm the layout remains usable.
- [ ] Confirm the controls summary panels stay visible while the config editor has its own internal scrollbar.
- [ ] Confirm the config editor can be collapsed and expanded independently of the rest of the controls tab.
- [ ] Confirm the config editor collapsed/expanded state is preserved when switching between `Controls` and `Inspector`.
- [ ] Confirm the target tick rate labels are visually aligned under the slider track rather than the full control row.
- [ ] Confirm the bottom status/metrics area remains visible while the Controls tab contains tall content and that the sidebar scrolls internally instead of pushing the page taller.

## Additional Frontend Checks

- [ ] With the backend stopped, confirm the config area shows an explicit backend-unreachable message instead of a raw browser `NetworkError`.
- [ ] With the backend running but no sim created yet, confirm the grid area explains that a simulation must be created before any grid appears.
- [ ] Confirm the default seed program bytecode matches the looked-up lithotroph opcode sequence.
- [ ] Set the grid smaller than the seeded organism position and confirm the UI surfaces a clear seed-out-of-bounds validation message instead of `[object Object]`.
- [ ] Drag the target tick rate slider across `max` and non-`max` values and confirm the helper text stays mounted without layout jitter.

## Snapshot Placeholder

- [ ] Confirm the snapshot section appears in Controls.
- [ ] Confirm it is clearly marked unavailable and exposes no working snapshot actions.

## Reconnect and Refresh Regression Checks

- [ ] Refresh the page while a simulation is already running and confirm the frontend reconnects cleanly.
- [ ] After refreshing with an existing simulation, confirm pan and cursor-anchored zoom still behave correctly and do not regress.
- [ ] Refresh while paused and confirm the paused sim is reflected correctly.
- [ ] Destroy the simulation, refresh, and confirm the app returns to no-sim state.
- [ ] Stop the backend while the page is open and confirm the UI degrades without crashing.

## Suggested Test Matrix

- [ ] Default config with no seed programs.
- [ ] Small grid such as `32x32`.
- [ ] Medium grid such as `256x256`.
- [ ] One manual seed program entry to validate seed handling.

## Record During Testing

- [ ] Browser console errors.
- [ ] REST actions that leave the UI in the wrong state.
- [ ] Button enablement mismatches.
- [ ] Rendering blur, stutter, or hit-testing issues.
- [ ] Missing or stale inspector fields.
- [ ] Charts that fail to populate or stop advancing.
