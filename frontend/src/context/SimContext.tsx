import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from 'react';
import {
  DEFAULT_CONFIG,
  DEFAULT_TARGET_TPS,
  FRONTEND_STEP_BATCH_LIMIT,
  FRONTEND_TICKER_INTERVAL_MS,
  INITIAL_STATE,
  INSPECTOR_REFRESH_MS,
  TPS_POLL_MS,
} from '../constants';
import {
  createSimulation,
  destroySimulation,
  fetchCell,
  fetchMetrics,
  getSimStatus,
  postSimulationAction,
  stepSimulation,
} from '../lib/api';
import { getFirstConfigError, validateConfig, type ConfigErrors } from '../lib/config';
import { createComposerSession, type ComposerSession } from '../lib/populate';
import { SEED_LIBRARY } from '../lib/seedLibrary';
import { parseFrame } from '../lib/frame';
import { MetricsBuffer } from '../lib/metricsBuffer';
import { randomPlacementSeed, randomSeed } from '../lib/random';
import { useWebSocketContext } from './WebSocketContext';
import type {
  AppState,
  CellResponse,
  InspectionStamp,
  ColorMapMode,
  GridFrame,
  MetricsMessage,
  MetricsSnapshot,
  SimConfig,
  SimStatus,
  SimStatusResponse,
  SidebarTab,
  TargetTpsOption,
} from '../types';

type Action =
  | { type: 'SET_WS_STATUS'; wsStatus: AppState['wsStatus'] }
  | { type: 'SET_SIM_STATUS'; payload: SimStatusResponse | null }
  | { type: 'SET_FRAME_SUBSCRIBED'; value: boolean }
  | { type: 'SET_METRICS_SUBSCRIBED'; value: boolean }
  | { type: 'SET_MAX_FPS'; value: number }
  | { type: 'SET_EVERY_N_TICKS'; value: number }
  | { type: 'SET_TARGET_TPS'; value: TargetTpsOption }
  | { type: 'SET_FRONTEND_TICKER_ACTIVE'; value: boolean }
  | { type: 'SET_SELECTED_CELL'; value: { x: number; y: number } | null }
  | { type: 'SET_COLOR_MAP'; value: ColorMapMode }
  | { type: 'SET_SIDEBAR_OPEN'; value: boolean }
  | { type: 'SET_SIDEBAR_TAB'; value: SidebarTab }
  | { type: 'SET_CONTROLS_CONFIG_OPEN'; value: boolean }
  | { type: 'SET_METRICS_DRAWER_OPEN'; value: boolean }
  | { type: 'SET_TPS'; value: number | null }
  | { type: 'SET_API_ERROR'; value: string | null }
  | { type: 'SET_TICK'; value: number }
  | { type: 'BUMP_SIM_EPOCH' }
  | { type: 'CLEAR_SIM' };

function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case 'SET_WS_STATUS':
      return { ...state, wsStatus: action.wsStatus };
    case 'SET_SIM_STATUS':
      if (!action.payload) {
        return { ...state, simStatus: 'none', tick: 0, gridWidth: 0, gridHeight: 0, ticksPerSecond: null };
      }

      return {
        ...state,
        simStatus: action.payload.status,
        tick: action.payload.tick,
        gridWidth: action.payload.grid_width,
        gridHeight: action.payload.grid_height,
        ticksPerSecond: action.payload.ticks_per_second ?? state.ticksPerSecond,
      };
    case 'SET_FRAME_SUBSCRIBED':
      return { ...state, frameSubscribed: action.value };
    case 'SET_METRICS_SUBSCRIBED':
      return { ...state, metricsSubscribed: action.value };
    case 'SET_MAX_FPS':
      return { ...state, maxFps: action.value };
    case 'SET_EVERY_N_TICKS':
      return { ...state, everyNTicks: action.value };
    case 'SET_TARGET_TPS':
      return { ...state, targetTps: action.value };
    case 'SET_FRONTEND_TICKER_ACTIVE':
      return { ...state, frontendTickerActive: action.value };
    case 'SET_SELECTED_CELL':
      return { ...state, selectedCell: action.value };
    case 'SET_COLOR_MAP':
      return { ...state, colorMap: action.value };
    case 'SET_SIDEBAR_OPEN':
      return { ...state, sidebarOpen: action.value };
    case 'SET_SIDEBAR_TAB':
      return { ...state, sidebarTab: action.value };
    case 'SET_CONTROLS_CONFIG_OPEN':
      return { ...state, controlsConfigOpen: action.value };
    case 'SET_METRICS_DRAWER_OPEN':
      return { ...state, metricsDrawerOpen: action.value };
    case 'SET_TPS':
      return { ...state, ticksPerSecond: action.value };
    case 'SET_API_ERROR':
      return { ...state, apiError: action.value };
    case 'SET_TICK':
      return { ...state, tick: action.value };
    case 'BUMP_SIM_EPOCH':
      return { ...state, simEpoch: state.simEpoch + 1 };
    case 'CLEAR_SIM':
      return {
        ...state,
        simStatus: 'none',
        tick: 0,
        gridWidth: 0,
        gridHeight: 0,
        frameSubscribed: false,
        metricsSubscribed: false,
        frontendTickerActive: false,
        selectedCell: null,
        ticksPerSecond: null,
      };
  }
}

interface SimContextValue {
  state: AppState;
  latestFrameRef: React.MutableRefObject<GridFrame | null>;
  metricsBufferRef: React.MutableRefObject<MetricsBuffer>;
  latestMetrics: MetricsSnapshot | null;
  metricsVersion: number;
  config: SimConfig;
  configErrors: ConfigErrors;
  configErrorSummary: string | null;
  configIsValid: boolean;
  setConfig: React.Dispatch<React.SetStateAction<SimConfig>>;
  composer: ComposerSession;
  setComposer: React.Dispatch<React.SetStateAction<ComposerSession>>;
  resetComposer(): void;
  selectedCellData: CellResponse | null;
  /** Null until a cell inspection lands; identifies what that data describes. */
  selectedCellStamp: InspectionStamp | null;
  selectedCellLoading: boolean;
  selectedCellError: string | null;
  selectedCellFetchedAt: number | null;
  refreshSelectedCell(): Promise<void>;
  createFromConfig(): Promise<void>;
  start(): Promise<void>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  step(count: number): Promise<void>;
  reset(): Promise<void>;
  destroy(): Promise<void>;
  setMaxFps(value: number): void;
  setEveryNTicks(value: number): void;
  setTargetTps(value: TargetTpsOption): Promise<void>;
  setColorMap(value: ColorMapMode): void;
  setSidebarOpen(value: boolean): void;
  setSidebarTab(value: SidebarTab): void;
  setControlsConfigOpen(value: boolean): void;
  setMetricsDrawerOpen(value: boolean): void;
  selectCell(cell: { x: number; y: number } | null): void;
  randomizeSeed(): void;
}

const SimContext = createContext<SimContextValue | null>(null);

interface FrontendTickerState {
  active: boolean;
  timeoutId: number | null;
  lastTimestampMs: number;
  accumulator: number;
  inFlight: boolean;
}

export function SimProvider({ children }: PropsWithChildren): JSX.Element {
  const { addMessageListener, sendJson, status } = useWebSocketContext();
  const [state, dispatch] = useReducer(reducer, {
    ...INITIAL_STATE,
    wsStatus: status,
  });
  const [config, setConfig] = useState<SimConfig>(DEFAULT_CONFIG);
  // The composer lives here rather than in ConfigEditor because that component
  // unmounts whenever the sidebar collapses or switches to the Inspector tab,
  // which would otherwise strand every entry it had generated.
  const [composer, setComposer] = useState<ComposerSession>(() =>
    createComposerSession(SEED_LIBRARY.map((organism) => organism.id), randomPlacementSeed()),
  );
  const [latestMetrics, setLatestMetrics] = useState<MetricsSnapshot | null>(null);
  const [metricsVersion, setMetricsVersion] = useState(0);
  const [selectedCellData, setSelectedCellData] = useState<CellResponse | null>(null);
  const [selectedCellStamp, setSelectedCellStamp] = useState<InspectionStamp | null>(null);
  const [selectedCellLoading, setSelectedCellLoading] = useState(false);
  const [selectedCellError, setSelectedCellError] = useState<string | null>(null);
  const [selectedCellFetchedAt, setSelectedCellFetchedAt] = useState<number | null>(null);
  const latestFrameRef = useRef<GridFrame | null>(null);
  const metricsBufferRef = useRef(new MetricsBuffer());
  const stateRef = useRef(state);
  const selectedCellDataRef = useRef<CellResponse | null>(null);
  const selectedCellRequestRef = useRef(0);
  /**
   * Authoritative uid-epoch counter. It leads `state.simEpoch` by one commit,
   * so a fetch reads it synchronously at start rather than waiting for the
   * dispatched value to reach `stateRef`.
   */
  const simEpochRef = useRef(INITIAL_STATE.simEpoch);
  /** True while a create/reset/destroy request is in flight. */
  const boundaryInFlightRef = useRef(false);
  const frontendTickerRef = useRef<FrontendTickerState>({
    active: false,
    timeoutId: null,
    lastTimestampMs: 0,
    accumulator: 0,
    inFlight: false,
  });

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  useEffect(() => {
    selectedCellDataRef.current = selectedCellData;
  }, [selectedCellData]);

  const configErrors = useMemo(() => validateConfig(config), [config]);
  const configErrorSummary = useMemo(
    () => getFirstConfigError(configErrors, config),
    [config, configErrors],
  );
  const configIsValid = useMemo(
    () => Object.keys(configErrors).length === 0,
    [configErrors],
  );

  /** Returns the composer to its initial mix with nothing generated. */
  const resetComposer = useCallback(() => {
    setComposer(
      createComposerSession(SEED_LIBRARY.map((organism) => organism.id), randomPlacementSeed()),
    );
  }, []);

  /**
   * Forgets which entries the composer generated while keeping the user's mix.
   *
   * Called whenever the seed-program list stops being the one the composer
   * produced — a simulation is created or reset from it — so a later
   * regeneration cannot resurrect entries that are no longer the composer's to
   * replace. What it had placed stays in the config as plain hand-placed
   * entries.
   */
  const clearComposerGenerated = useCallback(() => {
    setComposer((current) =>
      current.generated.length === 0 && current.clampedTo === null
        ? current
        : { ...current, generated: [], clampedTo: null },
    );
  }, []);

  const syncStatus = useCallback(async () => {
    try {
      const payload = await getSimStatus();
      dispatch({ type: 'SET_SIM_STATUS', payload });
      dispatch({ type: 'SET_API_ERROR', value: null });
      if (payload === null) {
        latestFrameRef.current = null;
      }
    } catch (error) {
      dispatch({
        type: 'SET_API_ERROR',
        value: error instanceof Error ? error.message : 'Failed to load simulation status',
      });
    }
  }, []);

  const seedMetricsSnapshot = useCallback(async () => {
    try {
      const snapshot = await fetchMetrics();
      if (!metricsBufferRef.current.push(snapshot)) {
        return;
      }
      setLatestMetrics(snapshot);
      setMetricsVersion((value) => value + 1);
      dispatch({ type: 'SET_TICK', value: snapshot.tick });
    } catch {
      // Fall back to the streaming channel if the REST snapshot is unavailable.
    }
  }, []);

  const stopFrontendTicker = useCallback(() => {
    const ticker = frontendTickerRef.current;
    ticker.active = false;
    ticker.lastTimestampMs = 0;
    ticker.accumulator = 0;
    if (ticker.timeoutId !== null) {
      window.clearTimeout(ticker.timeoutId);
      ticker.timeoutId = null;
    }
    stateRef.current = { ...stateRef.current, frontendTickerActive: false };
    dispatch({ type: 'SET_FRONTEND_TICKER_ACTIVE', value: false });
  }, []);

  const runFrontendTickerLoop = useCallback(() => {
    // Temporary shim until the backend exposes a real target-TPS control.
    // When active, the simulation stays paused and the frontend sends timed step requests.
    const loop = async () => {
      const ticker = frontendTickerRef.current;
      if (!ticker.active) {
        return;
      }

      const target = stateRef.current.targetTps;
      if (target === 'max') {
        stopFrontendTicker();
        return;
      }

      const now = performance.now();
      if (ticker.lastTimestampMs === 0) {
        ticker.lastTimestampMs = now;
      }

      const elapsedSeconds = (now - ticker.lastTimestampMs) / 1000;
      ticker.lastTimestampMs = now;
      ticker.accumulator += elapsedSeconds * target;

      if (!ticker.inFlight) {
        const dueTicks = Math.floor(ticker.accumulator);
        if (dueTicks > 0) {
          const stepCount = Math.max(1, Math.min(FRONTEND_STEP_BATCH_LIMIT, dueTicks));
          ticker.accumulator -= stepCount;
          ticker.inFlight = true;
          try {
            const result = await stepSimulation(stepCount);
            dispatch({ type: 'SET_SIM_STATUS', payload: result });
            dispatch({ type: 'SET_API_ERROR', value: null });
          } catch (error) {
            dispatch({
              type: 'SET_API_ERROR',
              value:
                error instanceof Error ? error.message : 'Client-side target TPS loop failed',
            });
            stopFrontendTicker();
            await syncStatus();
            return;
          } finally {
            ticker.inFlight = false;
          }
        }
      }

      if (ticker.active) {
        ticker.timeoutId = window.setTimeout(loop, FRONTEND_TICKER_INTERVAL_MS);
      }
    };

    frontendTickerRef.current.timeoutId = window.setTimeout(loop, FRONTEND_TICKER_INTERVAL_MS);
  }, [stopFrontendTicker, syncStatus]);

  const startFrontendTicker = useCallback(async () => {
    const target = stateRef.current.targetTps;
    if (target === 'max') {
      return;
    }

    stopFrontendTicker();

    try {
      const currentStatus = stateRef.current.simStatus;

      if (currentStatus === 'created') {
        await postSimulationAction('start');
        const paused = await postSimulationAction('pause');
        dispatch({ type: 'SET_SIM_STATUS', payload: paused });
      } else if (currentStatus === 'running') {
        const paused = await postSimulationAction('pause');
        dispatch({ type: 'SET_SIM_STATUS', payload: paused });
      } else if (currentStatus !== 'paused') {
        throw new Error('Client-side target TPS requires a created or paused simulation.');
      }

      dispatch({ type: 'SET_FRONTEND_TICKER_ACTIVE', value: true });
      dispatch({ type: 'SET_API_ERROR', value: null });
      stateRef.current = { ...stateRef.current, frontendTickerActive: true };
      const ticker = frontendTickerRef.current;
      ticker.active = true;
      ticker.lastTimestampMs = performance.now();
      ticker.accumulator = 0;
      ticker.inFlight = false;
      runFrontendTickerLoop();
    } catch (error) {
      dispatch({
        type: 'SET_API_ERROR',
        value:
          error instanceof Error ? error.message : 'Failed to start client-side target TPS loop',
      });
      stopFrontendTicker();
      await syncStatus();
    }
  }, [runFrontendTickerLoop, stopFrontendTicker, syncStatus]);

  useEffect(() => {
    dispatch({ type: 'SET_WS_STATUS', wsStatus: status });
  }, [status]);

  useEffect(() => {
    void syncStatus();
  }, [syncStatus]);

  useEffect(() => {
    return () => {
      stopFrontendTicker();
    };
  }, [stopFrontendTicker]);

  const subscribeFrames = useCallback(
    (maxFps = state.maxFps) => {
      sendJson({ subscribe: 'frames', max_fps: maxFps });
      dispatch({ type: 'SET_FRAME_SUBSCRIBED', value: true });
    },
    [sendJson, state.maxFps],
  );

  const unsubscribeFrames = useCallback(() => {
    sendJson({ unsubscribe: 'frames' });
    dispatch({ type: 'SET_FRAME_SUBSCRIBED', value: false });
  }, [sendJson]);

  const subscribeMetrics = useCallback(
    (everyNTicks = state.everyNTicks) => {
      sendJson({ subscribe: 'metrics', every_n_ticks: everyNTicks });
      dispatch({ type: 'SET_METRICS_SUBSCRIBED', value: true });
    },
    [sendJson, state.everyNTicks],
  );

  const unsubscribeMetrics = useCallback(() => {
    sendJson({ unsubscribe: 'metrics' });
    dispatch({ type: 'SET_METRICS_SUBSCRIBED', value: false });
  }, [sendJson]);

  useEffect(() => {
    if (status !== 'connected') {
      return;
    }

    if (state.simStatus !== 'none') {
      subscribeFrames();
      subscribeMetrics();
      if (metricsBufferRef.current.snapshot().count === 0) {
        void seedMetricsSnapshot();
      }
    }
  }, [seedMetricsSnapshot, state.simStatus, status, subscribeFrames, subscribeMetrics]);

  useEffect(() => {
    if (state.simStatus === 'none' && state.frontendTickerActive) {
      stopFrontendTicker();
    }
  }, [state.frontendTickerActive, state.simStatus, stopFrontendTicker]);

  useEffect(() => {
    return addMessageListener((event) => {
      if (typeof event.data !== 'string') {
        latestFrameRef.current = parseFrame(event.data);
        return;
      }

      try {
        const parsed = JSON.parse(event.data) as MetricsMessage | { type: 'error'; message: string };
        if (parsed.type === 'metrics') {
          if (!metricsBufferRef.current.push(parsed)) {
            return;
          }
          setLatestMetrics(parsed);
          setMetricsVersion((value) => value + 1);
          dispatch({ type: 'SET_TICK', value: parsed.tick });
        } else if (parsed.type === 'error') {
          dispatch({ type: 'SET_API_ERROR', value: parsed.message });
        }
      } catch {
        dispatch({ type: 'SET_API_ERROR', value: 'Failed to parse WebSocket message' });
      }
    });
  }, [addMessageListener]);

  useEffect(() => {
    if (state.simStatus === 'none') {
      return;
    }

    const interval = window.setInterval(async () => {
      try {
        const payload = await getSimStatus();
        dispatch({ type: 'SET_TPS', value: payload?.ticks_per_second ?? null });
      } catch {
        dispatch({ type: 'SET_TPS', value: null });
      }
    }, TPS_POLL_MS);

    return () => {
      window.clearInterval(interval);
    };
  }, [state.simStatus]);

  const selectCell = useCallback((cell: { x: number; y: number } | null) => {
    selectedCellRequestRef.current += 1;
    const currentSelectedCell = stateRef.current.selectedCell;
    const isSameCellSelection =
      cell !== null &&
      currentSelectedCell !== null &&
      currentSelectedCell.x === cell.x &&
      currentSelectedCell.y === cell.y;
    dispatch({ type: 'SET_SELECTED_CELL', value: cell });
    if (cell) {
      if (!isSameCellSelection) {
        setSelectedCellData(null);
        setSelectedCellStamp(null);
        setSelectedCellFetchedAt(null);
      }
      setSelectedCellError(null);
      setSelectedCellLoading(false);
      dispatch({ type: 'SET_SIDEBAR_OPEN', value: true });
      dispatch({ type: 'SET_SIDEBAR_TAB', value: 'inspector' });
    } else {
      setSelectedCellData(null);
      setSelectedCellStamp(null);
      setSelectedCellFetchedAt(null);
      setSelectedCellError(null);
      setSelectedCellLoading(false);
    }
  }, []);

  /**
   * Opens a simulation boundary (create, reset, destroy) and drops the
   * inspection data retained across it. Each boundary starts a new uid epoch,
   * so a parent uid decoded from the old data would point into a simulation
   * that no longer exists — and after a resize the modulo decode would land in
   * range on the wrong cell. Bumping the request counter discards any
   * inspection fetch already in flight; the flag keeps the running inspector's
   * poll from starting a new one against the simulation being replaced.
   *
   * The epoch itself is bumped in `endSimBoundary`, once the request has
   * actually landed, so that a fetch which observed the pre-boundary
   * simulation carries the pre-boundary epoch and renders as inert text.
   */
  const beginSimBoundary = useCallback(() => {
    boundaryInFlightRef.current = true;
    selectedCellRequestRef.current += 1;
    setSelectedCellData(null);
    setSelectedCellStamp(null);
    setSelectedCellFetchedAt(null);
    setSelectedCellError(null);
    setSelectedCellLoading(false);
  }, []);

  /** Closes a simulation boundary: the new uid epoch starts here. */
  const endSimBoundary = useCallback(() => {
    simEpochRef.current += 1;
    boundaryInFlightRef.current = false;
    dispatch({ type: 'BUMP_SIM_EPOCH' });
  }, []);

  /**
   * Reads the selection from `stateRef` rather than a closure, so a call made
   * right after a simulation boundary refetches whatever is selected *now* —
   * including a cell picked while the boundary request was still in flight,
   * whose own effect returned early.
   */
  const refreshSelectedCell = useCallback(async () => {
    const { selectedCell, simStatus } = stateRef.current;
    if (!selectedCell || simStatus === 'none' || boundaryInFlightRef.current) {
      return;
    }

    const requestId = ++selectedCellRequestRef.current;
    // Captured at fetch start, not on response: this request observes the
    // simulation that is current now, whatever it has become by the time the
    // response lands.
    const stamp: InspectionStamp = {
      simEpoch: simEpochRef.current,
      gridWidth: stateRef.current.gridWidth,
      gridHeight: stateRef.current.gridHeight,
    };
    const existingData = selectedCellDataRef.current;
    const isRefreshingCurrentCell =
      existingData !== null && existingData.x === selectedCell.x && existingData.y === selectedCell.y;
    if (!isRefreshingCurrentCell) {
      setSelectedCellLoading(true);
    }

    try {
      const response = await fetchCell(selectedCell.x, selectedCell.y);
      if (requestId !== selectedCellRequestRef.current) {
        return;
      }

      setSelectedCellData(response);
      setSelectedCellStamp(stamp);
      setSelectedCellFetchedAt(Date.now());
      setSelectedCellError(null);
    } catch (error) {
      if (requestId !== selectedCellRequestRef.current) {
        return;
      }

      setSelectedCellError(error instanceof Error ? error.message : 'Failed to load cell');
    } finally {
      if (requestId === selectedCellRequestRef.current) {
        setSelectedCellLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    if (!state.selectedCell || state.simStatus === 'none') {
      return;
    }

    void refreshSelectedCell();
  }, [refreshSelectedCell, state.selectedCell, state.tick, state.simStatus]);

  useEffect(() => {
    if (!state.selectedCell || state.simStatus !== 'running') {
      return;
    }

    const interval = window.setInterval(() => {
      void refreshSelectedCell();
    }, INSPECTOR_REFRESH_MS);

    return () => {
      window.clearInterval(interval);
    };
  }, [refreshSelectedCell, state.selectedCell, state.simStatus]);

  const runAction = useCallback(
    async (operation: () => Promise<SimStatusResponse | void>, nextStatus?: SimStatus) => {
      try {
        const result = await operation();
        if (result && 'status' in result) {
          dispatch({ type: 'SET_SIM_STATUS', payload: result });
        } else if (nextStatus === 'none') {
          dispatch({ type: 'CLEAR_SIM' });
        }
        dispatch({ type: 'SET_API_ERROR', value: null });
        await syncStatus();
        return true;
      } catch (error) {
        dispatch({
          type: 'SET_API_ERROR',
          value: error instanceof Error ? error.message : 'Request failed',
        });
        return false;
      }
    },
    [syncStatus],
  );

  const createFromConfig = useCallback(async () => {
    if (!configIsValid) {
      dispatch({
        type: 'SET_API_ERROR',
        value: configErrorSummary ?? 'Config is invalid. Fix the highlighted fields before creating the simulation.',
      });
      return;
    }

    metricsBufferRef.current.clear();
    setLatestMetrics(null);
    setMetricsVersion(0);
    beginSimBoundary();
    try {
      // Only a successful create turns scattered entries into plain hand-placed ones;
      // a rejected request must leave the composer's ownership intact.
      if (await runAction(async () => createSimulation(config))) {
        clearComposerGenerated();
      }
      await seedMetricsSnapshot();
    } finally {
      endSimBoundary();
    }
    await refreshSelectedCell();
  }, [
    beginSimBoundary,
    clearComposerGenerated,
    config,
    configErrorSummary,
    configIsValid,
    endSimBoundary,
    refreshSelectedCell,
    runAction,
    seedMetricsSnapshot,
  ]);

  const start = useCallback(async () => {
    if (stateRef.current.targetTps === 'max') {
      await runAction(async () => postSimulationAction('start'));
      return;
    }

    await startFrontendTicker();
  }, [runAction, startFrontendTicker]);

  const pause = useCallback(async () => {
    if (stateRef.current.frontendTickerActive) {
      stopFrontendTicker();
      await syncStatus();
      return;
    }

    await runAction(async () => postSimulationAction('pause'));
  }, [runAction, stopFrontendTicker, syncStatus]);

  const resume = useCallback(async () => {
    if (stateRef.current.targetTps === 'max') {
      await runAction(async () => postSimulationAction('resume'));
      return;
    }

    await startFrontendTicker();
  }, [runAction, startFrontendTicker]);

  const step = useCallback(
    async (count: number) => {
      await runAction(async () => stepSimulation(count));
      await refreshSelectedCell();
    },
    [refreshSelectedCell, runAction],
  );

  const reset = useCallback(async () => {
    stopFrontendTicker();
    metricsBufferRef.current.clear();
    setLatestMetrics(null);
    setMetricsVersion(0);
    beginSimBoundary();
    try {
      if (await runAction(async () => postSimulationAction('reset'))) {
        clearComposerGenerated();
      }
      await seedMetricsSnapshot();
    } finally {
      endSimBoundary();
    }
    await refreshSelectedCell();
  }, [
    beginSimBoundary,
    clearComposerGenerated,
    endSimBoundary,
    refreshSelectedCell,
    runAction,
    seedMetricsSnapshot,
    stopFrontendTicker,
  ]);

  const destroy = useCallback(async () => {
    stopFrontendTicker();
    unsubscribeFrames();
    unsubscribeMetrics();
    latestFrameRef.current = null;
    metricsBufferRef.current.clear();
    setLatestMetrics(null);
    setMetricsVersion(0);
    beginSimBoundary();
    try {
      await runAction(async () => destroySimulation(), 'none');
    } finally {
      endSimBoundary();
    }
  }, [
    beginSimBoundary,
    endSimBoundary,
    runAction,
    stopFrontendTicker,
    unsubscribeFrames,
    unsubscribeMetrics,
  ]);

  const setMaxFps = useCallback(
    (value: number) => {
      dispatch({ type: 'SET_MAX_FPS', value });
      if (state.simStatus !== 'none' && status === 'connected') {
        unsubscribeFrames();
        subscribeFrames(value);
      }
    },
    [state.simStatus, status, subscribeFrames, unsubscribeFrames],
  );

  const setEveryNTicks = useCallback(
    (value: number) => {
      dispatch({ type: 'SET_EVERY_N_TICKS', value });
      if (state.simStatus !== 'none' && status === 'connected') {
        unsubscribeMetrics();
        subscribeMetrics(value);
      }
    },
    [state.simStatus, status, subscribeMetrics, unsubscribeMetrics],
  );

  const setTargetTps = useCallback(
    async (value: TargetTpsOption) => {
      stateRef.current = { ...stateRef.current, targetTps: value };
      dispatch({ type: 'SET_TARGET_TPS', value });

      if (stateRef.current.frontendTickerActive) {
        if (value === 'max') {
          stopFrontendTicker();
          await runAction(async () => postSimulationAction('resume'));
        }
        return;
      }

      if (stateRef.current.simStatus === 'running' && value !== 'max') {
        await startFrontendTicker();
      }
    },
    [runAction, startFrontendTicker, stopFrontendTicker],
  );

  const setColorMap = useCallback((value: ColorMapMode) => {
    dispatch({ type: 'SET_COLOR_MAP', value });
  }, []);

  const setSidebarOpen = useCallback((value: boolean) => {
    dispatch({ type: 'SET_SIDEBAR_OPEN', value });
  }, []);

  const setSidebarTab = useCallback((value: SidebarTab) => {
    dispatch({ type: 'SET_SIDEBAR_TAB', value });
  }, []);

  const setControlsConfigOpen = useCallback((value: boolean) => {
    dispatch({ type: 'SET_CONTROLS_CONFIG_OPEN', value });
  }, []);

  const setMetricsDrawerOpen = useCallback((value: boolean) => {
    dispatch({ type: 'SET_METRICS_DRAWER_OPEN', value });
  }, []);

  const randomizeSeedValue = useCallback(() => {
    setConfig((current) => ({
      ...current,
      seed: randomSeed(),
    }));
  }, []);

  const value = useMemo<SimContextValue>(
    () => ({
      state,
      latestFrameRef,
      metricsBufferRef,
      latestMetrics,
      metricsVersion,
      config,
      configErrors,
      configErrorSummary,
      configIsValid,
      setConfig,
      composer,
      setComposer,
      resetComposer,
      selectedCellData,
      selectedCellStamp,
      selectedCellLoading,
      selectedCellError,
      selectedCellFetchedAt,
      refreshSelectedCell,
      createFromConfig,
      start,
      pause,
      resume,
      step,
      reset,
      destroy,
      setMaxFps,
      setEveryNTicks,
      setTargetTps,
      setColorMap,
      setSidebarOpen,
      setSidebarTab,
      setControlsConfigOpen,
      setMetricsDrawerOpen,
      selectCell,
      randomizeSeed: randomizeSeedValue,
    }),
    [
      composer,
      config,
      createFromConfig,
      resetComposer,
      destroy,
      latestMetrics,
      metricsVersion,
      pause,
      configErrors,
      configErrorSummary,
      configIsValid,
      refreshSelectedCell,
      reset,
      resume,
      selectedCellData,
      selectedCellError,
      selectedCellFetchedAt,
      selectedCellLoading,
      selectedCellStamp,
      setColorMap,
      setControlsConfigOpen,
      setEveryNTicks,
      setMaxFps,
      setTargetTps,
      setMetricsDrawerOpen,
      setSidebarOpen,
      setSidebarTab,
      start,
      state,
      step,
      randomizeSeedValue,
    ],
  );

  return <SimContext.Provider value={value}>{children}</SimContext.Provider>;
}

export function useSimContext(): SimContextValue {
  const context = useContext(SimContext);
  if (!context) {
    throw new Error('useSimContext must be used within SimProvider');
  }

  return context;
}
