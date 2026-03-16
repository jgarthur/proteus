export type WsStatus = 'disconnected' | 'connecting' | 'connected';
export type SimStatus = 'none' | 'created' | 'running' | 'paused';
export type SidebarTab = 'controls' | 'inspector';
export type TargetTpsOption = 1 | 2 | 5 | 10 | 30 | 60 | 120 | 'max';

export type ColorMapMode =
  | 'occupancy'
  | 'programId'
  | 'programSize'
  | 'freeEnergy'
  | 'freeMass'
  | 'bgRadiation'
  | 'bgMass'
  | 'combined';

export interface AppState {
  wsStatus: WsStatus;
  simStatus: SimStatus;
  tick: number;
  gridWidth: number;
  gridHeight: number;
  frameSubscribed: boolean;
  metricsSubscribed: boolean;
  maxFps: number;
  everyNTicks: number;
  targetTps: TargetTpsOption;
  frontendTickerActive: boolean;
  selectedCell: { x: number; y: number } | null;
  colorMap: ColorMapMode;
  sidebarOpen: boolean;
  sidebarTab: SidebarTab;
  controlsConfigOpen: boolean;
  metricsDrawerOpen: boolean;
  ticksPerSecond: number | null;
  apiError: string | null;
}

export interface SimStatusResponse {
  status: Exclude<SimStatus, 'none'>;
  tick: number;
  grid_width: number;
  grid_height: number;
  population?: number;
  total_energy?: number;
  total_mass?: number;
  ticks_per_second?: number;
}

export interface MetricsMessage {
  type: 'metrics';
  tick: number;
  population: number;
  live_count: number;
  inert_count: number;
  total_energy: number;
  total_mass: number;
  mean_program_size: number;
  max_program_size: number;
  unique_genomes: number;
  births: number;
  deaths: number;
  mutations: number;
}

export interface ErrorMessage {
  type: 'error';
  code?: string;
  message: string;
}

export interface GridFrame {
  tick: number;
  width: number;
  height: number;
  cells: DataView;
}

export interface SeedProgram {
  x: number;
  y: number;
  code: number[];
  free_energy: number;
  free_mass: number;
}

export interface SimConfig {
  width: number;
  height: number;
  seed: number;
  r_energy: number;
  r_mass: number;
  d_energy: number;
  d_mass: number;
  t_cap: number;
  maintenance_rate: number;
  maintenance_exponent: number;
  local_action_exponent: number;
  n_synth: number;
  inert_grace_ticks: number;
  p_spawn: number;
  mutation_base_log2: number;
  mutation_background_log2: number;
  seed_programs: SeedProgram[];
}

export interface CellProgram {
  code: number[];
  disassembly: string[];
  size: number;
  live: boolean;
  age: number;
  ip: number;
  src: number;
  dst: number;
  dir: number;
  flag: boolean;
  msg: number;
  id: number;
  lc: number;
  stack: number[];
  abandonment_timer: number | null;
}

export interface CellResponse {
  index: number;
  x: number;
  y: number;
  free_energy: number;
  free_mass: number;
  bg_radiation: number;
  bg_mass: number;
  program: CellProgram | null;
}

export interface MetricsSnapshot extends Omit<MetricsMessage, 'type'> {}

export interface MetricsBufferSnapshot {
  count: number;
  tick: Float64Array;
  population: Float64Array;
  live_count: Float64Array;
  inert_count: Float64Array;
  total_energy: Float64Array;
  total_mass: Float64Array;
  births: Float64Array;
  deaths: Float64Array;
  mutations: Float64Array;
  mean_program_size: Float64Array;
  max_program_size: Float64Array;
  unique_genomes: Float64Array;
}

export interface ViewportTransform {
  offsetX: number;
  offsetY: number;
  scale: number;
}

export interface GridRenderer {
  attach(canvas: HTMLCanvasElement): void;
  resize(width: number, height: number): void;
  render(
    frame: GridFrame,
    viewport: ViewportTransform,
    colorMode: ColorMapMode,
    selectedCell: { x: number; y: number } | null,
  ): void;
  hitTest(canvasX: number, canvasY: number): { x: number; y: number } | null;
  fit(gridWidth: number, gridHeight: number): ViewportTransform;
  destroy(): void;
}
