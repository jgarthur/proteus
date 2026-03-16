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
  getSimStatus,
  postSimulationAction,
  stepSimulation,
} from '../lib/api';
import { getFirstConfigError, validateConfig, type ConfigErrors } from '../lib/config';
import { parseFrame } from '../lib/frame';
import { MetricsBuffer } from '../lib/metricsBuffer';
import { randomSeed } from '../lib/random';
import { useWebSocketContext } from './WebSocketContext';
import type {
  AppState,
  CellResponse,
  ColorMapMode,
  GridFrame,
  MetricsMessage,
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
  latestMetrics: MetricsMessage | null;
  metricsVersion: number;
  config: SimConfig;
  configErrors: ConfigErrors;
  configErrorSummary: string | null;
  configIsValid: boolean;
  setConfig: React.Dispatch<React.SetStateAction<SimConfig>>;
  selectedCellData: CellResponse | null;
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
  const [latestMetrics, setLatestMetrics] = useState<MetricsMessage | null>(null);
  const [metricsVersion, setMetricsVersion] = useState(0);
  const [selectedCellData, setSelectedCellData] = useState<CellResponse | null>(null);
  const [selectedCellLoading, setSelectedCellLoading] = useState(false);
  const [selectedCellError, setSelectedCellError] = useState<string | null>(null);
  const [selectedCellFetchedAt, setSelectedCellFetchedAt] = useState<number | null>(null);
  const latestFrameRef = useRef<GridFrame | null>(null);
  const metricsBufferRef = useRef(new MetricsBuffer());
  const stateRef = useRef(state);
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

  const configErrors = useMemo(() => validateConfig(config), [config]);
  const configErrorSummary = useMemo(
    () => getFirstConfigError(configErrors, config),
    [config, configErrors],
  );
  const configIsValid = useMemo(
    () => Object.keys(configErrors).length === 0,
    [configErrors],
  );

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
    }
  }, [state.simStatus, status, subscribeFrames, subscribeMetrics]);

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
          setLatestMetrics(parsed);
          metricsBufferRef.current.push(parsed);
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
    dispatch({ type: 'SET_SELECTED_CELL', value: cell });
    if (cell) {
      dispatch({ type: 'SET_SIDEBAR_OPEN', value: true });
      dispatch({ type: 'SET_SIDEBAR_TAB', value: 'inspector' });
    } else {
      setSelectedCellData(null);
      setSelectedCellFetchedAt(null);
      setSelectedCellError(null);
    }
  }, []);

  const refreshSelectedCell = useCallback(async () => {
    if (!state.selectedCell || state.simStatus === 'none') {
      return;
    }

    setSelectedCellLoading(true);
    try {
      const response = await fetchCell(state.selectedCell.x, state.selectedCell.y);
      setSelectedCellData(response);
      setSelectedCellFetchedAt(Date.now());
      setSelectedCellError(null);
    } catch (error) {
      setSelectedCellError(error instanceof Error ? error.message : 'Failed to load cell');
    } finally {
      setSelectedCellLoading(false);
    }
  }, [state.selectedCell, state.simStatus]);

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
      } catch (error) {
        dispatch({
          type: 'SET_API_ERROR',
          value: error instanceof Error ? error.message : 'Request failed',
        });
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
    await runAction(async () => createSimulation(config));
  }, [config, configErrorSummary, configIsValid, runAction]);

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
    await runAction(async () => postSimulationAction('reset'));
  }, [runAction, stopFrontendTicker]);

  const destroy = useCallback(async () => {
    stopFrontendTicker();
    unsubscribeFrames();
    unsubscribeMetrics();
    latestFrameRef.current = null;
    metricsBufferRef.current.clear();
    setLatestMetrics(null);
    setMetricsVersion(0);
    setSelectedCellData(null);
    setSelectedCellFetchedAt(null);
    await runAction(async () => destroySimulation(), 'none');
  }, [runAction, stopFrontendTicker, unsubscribeFrames, unsubscribeMetrics]);

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
      selectedCellData,
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
      config,
      createFromConfig,
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
