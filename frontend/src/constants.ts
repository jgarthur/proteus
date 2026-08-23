import { DEFAULT_LITHOTROPH_CODE } from './lib/seedLibrary';
import type { AppState, ColorMapMode, SimConfig, TargetTpsOption } from './types';

export const API_BASE_URL =
  import.meta.env.VITE_PROTEUS_API_URL?.replace(/\/$/, '') ?? 'http://localhost:3000';

export const DEFAULT_MAX_FPS = 30;
export const DEFAULT_EVERY_N_TICKS = 1;
export const TARGET_TPS_OPTIONS: readonly TargetTpsOption[] = [1, 2, 5, 10, 30, 60, 120, 'max'];
export const DEFAULT_TARGET_TPS: TargetTpsOption = 10;
export const METRICS_CAPACITY = 10_000;
export const INSPECTOR_REFRESH_MS = 500;
export const TPS_POLL_MS = 2_000;
export const FRONTEND_TICKER_INTERVAL_MS = 16;
export const FRONTEND_STEP_BATCH_LIMIT = 120;

// Temporary local testing defaults. Reconcile these with the spec-backed defaults later.
// The genome itself now lives in the seed library, next to the other built-in organisms.
export { DEFAULT_LITHOTROPH_CODE };

export const COLOR_MAP_OPTIONS: Array<{ value: ColorMapMode; label: string }> = [
  { value: 'occupancy', label: 'Occupancy' },
  { value: 'programId', label: 'Program ID' },
  { value: 'programSize', label: 'Program Size' },
  { value: 'freeEnergy', label: 'Free Energy' },
  { value: 'freeMass', label: 'Free Mass' },
  { value: 'bgRadiation', label: 'Bg Radiation' },
  { value: 'bgMass', label: 'Bg Mass' },
  { value: 'combined', label: 'Combined' },
];

// Keep probability-exponent defaults aligned with rust/src/config.rs default_* functions.
export const DEFAULT_CONFIG: SimConfig = {
  width: 64,
  height: 64,
  seed: 1,
  r_energy: 0.25,
  r_mass: 1.0,
  d_energy_log2: 7,
  d_mass_log2: 7,
  t_cap: 4.0,
  maintenance_rate_log2: 7,
  maintenance_exponent: 1.0,
  local_action_exponent: 1.0,
  n_synth: 1,
  inert_grace_ticks: 10,
  p_spawn_log2: null,
  mutation_base_log2: 16,
  mutation_background_log2: 8,
  seed_programs: [
    {
      x: 32,
      y: 32,
      code: DEFAULT_LITHOTROPH_CODE,
      free_energy: 20,
      free_mass: 12,
    },
  ],
  seed_environment: [],
};

export const INITIAL_STATE: AppState = {
  wsStatus: 'connecting',
  simStatus: 'none',
  tick: 0,
  gridWidth: 0,
  gridHeight: 0,
  frameSubscribed: false,
  metricsSubscribed: false,
  maxFps: DEFAULT_MAX_FPS,
  everyNTicks: DEFAULT_EVERY_N_TICKS,
  targetTps: DEFAULT_TARGET_TPS,
  frontendTickerActive: false,
  selectedCell: null,
  colorMap: 'occupancy',
  sidebarOpen: true,
  sidebarTab: 'controls',
  controlsConfigOpen: true,
  metricsDrawerOpen: false,
  ticksPerSecond: null,
  apiError: null,
  simEpoch: 0,
};
