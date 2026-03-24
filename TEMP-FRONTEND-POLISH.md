# Frontend Polish Tracking

Temporary working notes for the current frontend polish pass.

## Reported Issues

- [x] Initial grid view is not centered on simulation creation, even though `Reset View` centers it correctly.
- [x] `Reset View` should leave a little vertical padding above and below the grid.
- [x] Inspector view briefly flickers a loading message before cell details settle.
- [x] Charts stop working after destroying and recreating a simulation.
- [ ] Inspector stack display should cap rendered entries to avoid browser slowdowns on huge stacks.
- [x] TPS and FPS slider rows should not resize when the right-hand value label changes width.
- [ ] Charts should resize correctly with the window and drawer width changes.
- [ ] Chart legends should render at full contrast inside each chart card.
- [ ] Inspector does not work reliably while the sim is running at `max` speed.
- [x] Config inputs should stop auto-coercing and overwriting while the user is still typing.
- [x] Controls sidebar should use a single scrollbar; expanded config should not trap its own scroll.
- [x] Metrics drawer height should be user-resizable.
- [x] Metrics plots should not stretch excessively wide on large windows.

## Commits

- `1e6ad79` `Fix grid viewport fitting`
- `f12f2ca` `Polish inspector, metrics, and controls`

## Current Chart State

- Current uncommitted chart work is only in:
  - `frontend/src/components/MetricsDrawer.tsx`
  - `frontend/src/components/MetricsDrawer.module.css`
- The current patch keeps the `ResizeObserver`-based `setSize()` path and removes the global `.uplot { width: 100% !important; }` override.
- The built-in `uPlot` legend is now mounted into a separate React host above the plot so legend width no longer contributes to the plot root's intrinsic width.
- The resize observer now attaches when chart data exists, which closes the bug where a chart that mounted empty never subscribed to later drawer/window resizes.
- The chart y-scale now explicitly restores a zero floor (`[0, null]`) after a bounds regression where axes no longer consistently included 0.
- The drawer now uses a draggable top resize handle with a clamped pixel height, and chart cards are width-capped in an auto-fit centered grid so plots do not become overly wide on large screens.
- While dragging the drawer, the drawer now disables height animation and the grid canvas redraws synchronously on resize to reduce resize-time flicker in the main viewport.
- `npm run build` passed after this chart-structure change.
- Manual browser verification is still needed for the remaining legend contrast issue.

## Current Diagnosis

- The greyed-out legend likely comes from `uPlot` applying `.u-series.u-off` state to built-in legend rows.
- The previous CSS attempted to override legend appearance, but it was not a reliable fix.
- The overflow diagnosis is now narrower:
  - only the plot area appears to overflow
  - the legend should not be treated as the primary root cause
  - the better suspects are `uPlot` root/container intrinsic sizing, canvas sizing, axis measurement, or insufficient min-width containment in the chart/grid layout

## Recommended Next Step

- Manually verify chart behavior while:
  - shrinking and expanding the window
  - opening and collapsing the sidebar
  - opening and collapsing the metrics drawer
- If plots still exceed card bounds, inspect axis measurement and plot root sizing under the new external-legend structure.
- If responsiveness is still awkward after that, revisit whether `uPlot` is worth keeping for this product surface.

## Notes

- Keep fixes in small commits.
- Check back with the user when verification is needed.
- Config editor UX should treat text entry as a short-lived draft: keep the raw string while focused, commit on blur or Enter, and let Escape revert the active field.
- Avoid live coercion in config inputs. The old `Number(...)`/CSV parsing on each keystroke made decimal entry, temporary empties, and seed-code edits feel broken because the UI rewrote the field before the user finished the thought.
- Inspector-at-`max` issue is not critical. If a real fix introduces substantial complexity, prefer leaving behavior as-is and adding a clear UI note that inspection is unreliable or unsupported while running at `max` speed.
- Practically, the sim is currently running too fast at `max` for inspector interaction to be very useful anyway.
- `npm run build` passed after the current batch of non-viewport changes.
- `STATUS.md` now includes backlog item `FRONTEND-CHARTS` for re-evaluating `uPlot` if responsive/layout friction continues.
- The worktree currently has unrelated non-frontend changes in `docs/API-SPEC.md`, `rust/src/observe.rs`, `rust/src/simulation.rs`, and `rust/tests/tick_driver.rs`. Do not revert them during frontend polish work.
