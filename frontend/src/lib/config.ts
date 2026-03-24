import type { SeedProgram, SimConfig } from '../types';

export type ConfigErrors = Record<string, string>;

const CONFIG_STORAGE_KEY = 'proteus.frontend.saved-config';

export function parseCode(value: string): number[] {
  if (!value.trim()) {
    return [];
  }

  return value
    .split(',')
    .map((part) => Number(part.trim()))
    .filter((valuePart) => Number.isFinite(valuePart));
}

export function validateConfig(config: SimConfig): ConfigErrors {
  const errors: ConfigErrors = {};
  const intFields: Array<keyof SimConfig> = [
    'width',
    'height',
    'n_synth',
    'inert_grace_ticks',
    'mutation_base_log2',
    'mutation_background_log2',
  ];

  intFields.forEach((field) => {
    if (!Number.isInteger(config[field])) {
      errors[String(field)] = 'Must be an integer';
    }
  });

  if (config.width < 1 || config.width > 1024) errors.width = 'Width must be 1-1024';
  if (config.height < 1 || config.height > 1024) errors.height = 'Height must be 1-1024';

  const nonNegativeRateFields: Array<keyof SimConfig> = [
    'r_energy',
    'r_mass',
  ];

  nonNegativeRateFields.forEach((field) => {
    const value = config[field] as number;
    if (value < 0) errors[String(field)] = 'Must be non-negative';
  });

  const probabilityFields: Array<keyof SimConfig> = [
    'd_energy',
    'd_mass',
    'maintenance_rate',
    'p_spawn',
  ];

  probabilityFields.forEach((field) => {
    const value = config[field] as number;
    if (value < 0 || value > 1) errors[String(field)] = 'Must be between 0.0 and 1.0';
  });

  ['t_cap', 'maintenance_exponent', 'local_action_exponent'].forEach((field) => {
    const value = config[field as keyof SimConfig] as number;
    if (value <= 0) {
      errors[field] = 'Must be greater than 0';
    }
  });

  ['n_synth', 'inert_grace_ticks', 'mutation_base_log2', 'mutation_background_log2'].forEach((field) => {
    const value = config[field as keyof SimConfig] as number;
    if (value < 0) {
      errors[field] = 'Must be non-negative';
    }
  });

  config.seed_programs.forEach((seed, index) => {
    if (!Number.isInteger(seed.x) || seed.x < 0 || seed.x >= config.width) {
      errors[`seed_programs.${index}.x`] = 'Must be within grid width';
    }
    if (!Number.isInteger(seed.y) || seed.y < 0 || seed.y >= config.height) {
      errors[`seed_programs.${index}.y`] = 'Must be within grid height';
    }
    if (seed.free_energy < 0) {
      errors[`seed_programs.${index}.free_energy`] = 'Must be non-negative';
    }
    if (seed.free_mass < 0) {
      errors[`seed_programs.${index}.free_mass`] = 'Must be non-negative';
    }
    if (seed.code.some((code) => !Number.isInteger(code) || code < 0 || code > 255)) {
      errors[`seed_programs.${index}.code`] = 'Use comma-separated bytes 0-255';
    }
  });

  return errors;
}

export function getFirstConfigError(errors: ConfigErrors, config: SimConfig): string | null {
  if (errors.width) return errors.width;
  if (errors.height) return errors.height;

  const seedProgramIndex = config.seed_programs.findIndex(
    (_, index) => Boolean(errors[`seed_programs.${index}.x`] || errors[`seed_programs.${index}.y`]),
  );
  if (seedProgramIndex >= 0) {
    const seed = config.seed_programs[seedProgramIndex]!;
    return `Seed program ${seedProgramIndex + 1} at (${seed.x}, ${seed.y}) is outside the ${config.width}×${config.height} grid. Move it or resize the grid.`;
  }

  const firstMessage = Object.values(errors)[0];
  return firstMessage ?? null;
}

export function saveConfigToStorage(config: SimConfig): void {
  window.localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(config));
}

function isValidSeedProgram(value: unknown): value is SeedProgram {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as Partial<SeedProgram>;
  return (
    typeof candidate.x === 'number' &&
    typeof candidate.y === 'number' &&
    Array.isArray(candidate.code) &&
    candidate.code.every((code) => typeof code === 'number') &&
    typeof candidate.free_energy === 'number' &&
    typeof candidate.free_mass === 'number'
  );
}

export function loadConfigFromStorage(): SimConfig | null {
  const raw = window.localStorage.getItem(CONFIG_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  const parsed = JSON.parse(raw) as Partial<SimConfig>;
  if (
    typeof parsed.width !== 'number' ||
    typeof parsed.height !== 'number' ||
    typeof parsed.seed !== 'number' ||
    typeof parsed.r_energy !== 'number' ||
    typeof parsed.r_mass !== 'number' ||
    typeof parsed.d_energy !== 'number' ||
    typeof parsed.d_mass !== 'number' ||
    typeof parsed.t_cap !== 'number' ||
    typeof parsed.maintenance_rate !== 'number' ||
    typeof parsed.maintenance_exponent !== 'number' ||
    typeof parsed.local_action_exponent !== 'number' ||
    typeof parsed.n_synth !== 'number' ||
    typeof parsed.inert_grace_ticks !== 'number' ||
    typeof parsed.p_spawn !== 'number' ||
    typeof parsed.mutation_base_log2 !== 'number' ||
    typeof parsed.mutation_background_log2 !== 'number' ||
    !Array.isArray(parsed.seed_programs) ||
    !parsed.seed_programs.every(isValidSeedProgram)
  ) {
    throw new Error('Saved config is malformed.');
  }

  return {
    width: parsed.width,
    height: parsed.height,
    seed: parsed.seed,
    r_energy: parsed.r_energy,
    r_mass: parsed.r_mass,
    d_energy: parsed.d_energy,
    d_mass: parsed.d_mass,
    t_cap: parsed.t_cap,
    maintenance_rate: parsed.maintenance_rate,
    maintenance_exponent: parsed.maintenance_exponent,
    local_action_exponent: parsed.local_action_exponent,
    n_synth: parsed.n_synth,
    inert_grace_ticks: parsed.inert_grace_ticks,
    p_spawn: parsed.p_spawn,
    mutation_base_log2: parsed.mutation_base_log2,
    mutation_background_log2: parsed.mutation_background_log2,
    seed_programs: parsed.seed_programs,
  };
}
