import { type ReactNode, useEffect, useState } from 'react';
import {
  type ConfigErrors,
  getDyadicExponentError,
  loadConfigFromStorage,
  MAX_DYADIC_EXPONENT,
  saveConfigToStorage,
  validateConfig,
} from '../../lib/config';
import { fetchSimulationConfig } from '../../lib/api';
import { formatDyadicProbability } from '../../lib/format';
import { useSimContext } from '../../context/SimContext';
import type { SeedProgram, SimConfig } from '../../types';
import styles from './ConfigEditor.module.css';

const OPTIONAL_EXPONENT_FIELDS = new Set<keyof SimConfig>([
  'd_energy_log2',
  'd_mass_log2',
  'maintenance_rate_log2',
  'p_spawn_log2',
]);

const EXPONENT_FIELDS = new Set<keyof SimConfig>([
  ...OPTIONAL_EXPONENT_FIELDS,
  'mutation_base_log2',
  'mutation_background_log2',
]);

const INTEGER_FIELDS = new Set<keyof SimConfig>([
  'width',
  'height',
  'seed',
  'n_synth',
  'inert_grace_ticks',
  'mutation_base_log2',
  'mutation_background_log2',
]);

/// Tooltip copy for the config fields. Applied to the whole `<label>`, so it
/// covers the label text and its input. The dyadic `*_log2` exponent fields
/// deliberately have none.
const FIELD_TITLES: Partial<Record<keyof SimConfig, string>> = {
  width: 'Grid size in cells.',
  height: 'Grid size in cells.',
  seed: 'World RNG seed. Same seed + config reproduces the run exactly.',
  r_energy:
    'Mean background-radiation units arriving per cell per tick. Higher = more energy income.',
  r_mass: 'Mean background-mass units arriving per cell per tick. Higher = more building material.',
  t_cap: 'Free energy/mass above t_cap \u00d7 program size decays. Higher = programs can hoard more.',
  maintenance_exponent:
    'Maintenance quanta per tick = size^\u03b2. Above 1, big programs pay disproportionately more upkeep.',
  local_action_exponent:
    'Instructions per tick = max(1, floor(size^\u03b1)). Above 1, big programs run faster.',
  n_synth: 'Extra energy consumed per mass produced by `synthesize`. Higher = mass is costlier.',
  inert_grace_ticks:
    'Ticks an inert (dead) program is exempt from maintenance after its last incoming write.',
  d_energy_log2:
    'Per-tick decay probability for each background-radiation unit and for free energy above t_cap \u00d7 size. Higher k = slower decay; empty = never decays.',
  d_mass_log2:
    'Per-tick decay probability for each background-mass unit and for free mass above t_cap \u00d7 size. Higher k = slower decay; empty = never decays.',
  maintenance_rate_log2:
    'Each of a program\u2019s size^\u03b2 maintenance quanta is charged with this probability per tick (paid in energy, then mass, then instructions). Higher k = cheaper upkeep; empty = no maintenance.',
  p_spawn_log2:
    'Probability that an empty cell which just received background mass nucleates a new single-nop program at end of tick. Higher k = rarer spontaneous life; empty = never.',
  mutation_base_log2: 'Baseline mutation probability per program per tick. Higher k = rarer mutations.',
  mutation_background_log2:
    'Each unit of background radiation spent on instruction base costs triggers a mutation with this probability; at most one mutation per tick. Higher k = paying with radiation is safer.',
};

const SEED_PROGRAM_CELL_TITLE = '0-indexed cell of the seed program.';
const SEED_PROGRAM_RESOURCE_TITLE = 'Starting free energy/mass in the seed cell.';

/// A config field row.
///
/// Renders exactly three row children -- label text, input, hint/error slot --
/// so `.grid > .field` can subgrid onto the parent grid's rows and keep the
/// inputs of neighbouring columns on a shared top edge. `children` must supply
/// the input and the hint slot (see `HintSlot` / `ExponentInput`).
function Field({
  children,
  label,
  title,
}: {
  children: ReactNode;
  label: string;
  title?: string;
}): JSX.Element {
  return (
    <label className={styles.field} title={title}>
      <span>{label}</span>
      {children}
    </label>
  );
}

/// The third row of a field. Always rendered, empty when there is nothing to
/// say, so every field keeps its three-row shape.
function HintSlot({ error }: { error?: string }): JSX.Element {
  return <span className={error ? styles.error : styles.hint}>{error ?? ''}</span>;
}

function parseBufferedNumber(value: string, integer: boolean): number | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const pattern = integer ? /^[+-]?\d+$/ : /^[+-]?(?:\d+\.?\d*|\.\d+)$/;
  if (!pattern.test(trimmed)) {
    return null;
  }

  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) {
    return null;
  }

  return integer && !Number.isInteger(parsed) ? null : parsed;
}

function parseCodeDraft(value: string): number[] | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return [];
  }

  const parts = value.split(',');
  const parsed: number[] = [];
  for (let index = 0; index < parts.length; index += 1) {
    const token = parts[index]!.trim();
    if (!token) {
      if (index === parts.length - 1) {
        continue;
      }
      return null;
    }

    if (!/^\d+$/.test(token)) {
      return null;
    }

    const byte = Number(token);
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
      return null;
    }

    parsed.push(byte);
  }

  return parsed;
}

function updateSeedProgram(
  seedPrograms: SeedProgram[],
  index: number,
  patch: Partial<SeedProgram>,
): SeedProgram[] {
  return seedPrograms.map((program, currentIndex) =>
    currentIndex === index ? { ...program, ...patch } : program,
  );
}

interface BufferedNumberInputProps {
  className: string;
  disabled: boolean;
  integer?: boolean;
  onCommit(value: number): void;
  value: number;
}

function BufferedNumberInput({
  className,
  disabled,
  integer = false,
  onCommit,
  value,
}: BufferedNumberInputProps): JSX.Element {
  const [draft, setDraft] = useState<string | null>(null);

  useEffect(() => {
    if (disabled) {
      setDraft(null);
    }
  }, [disabled]);

  const commit = () => {
    if (draft === null) {
      return;
    }

    const parsed = parseBufferedNumber(draft, integer);
    if (parsed !== null) {
      onCommit(parsed);
    }

    setDraft(null);
  };

  return (
    <input
      className={className}
      type="text"
      inputMode={integer ? 'numeric' : 'decimal'}
      disabled={disabled}
      value={draft ?? String(value)}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          commit();
          event.currentTarget.blur();
        }

        if (event.key === 'Escape') {
          event.preventDefault();
          setDraft(null);
        }
      }}
    />
  );
}

interface ExponentInputProps {
  allowNever: boolean;
  className: string;
  disabled: boolean;
  error?: string;
  onCommit(value: number | null): void;
  value: number | null;
}

/// Renders the exponent input plus its two-line probability preview -- exactly
/// two row children, so it slots straight into `Field`.
function ExponentInput({
  allowNever,
  className,
  disabled,
  error,
  onCommit,
  value,
}: ExponentInputProps): JSX.Element {
  const [draft, setDraft] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);

  useEffect(() => {
    if (disabled) {
      setDraft(null);
      setDraftError(null);
    }
  }, [disabled]);

  const commit = () => {
    if (draft === null) {
      return;
    }

    const trimmed = draft.trim();
    if (!trimmed) {
      const error = getDyadicExponentError(null, allowNever);
      if (error) {
        setDraftError(error);
        return;
      }
      onCommit(null);
      setDraft(null);
      setDraftError(null);
      return;
    }

    const parsed = Number(draft);
    const error = getDyadicExponentError(parsed, allowNever);
    if (error) {
      setDraftError(error);
      return;
    }
    onCommit(parsed);
    setDraft(null);
    setDraftError(null);
  };

  const displayed = draft ?? (value === null ? '' : String(value));
  const previewExponent = draft === null ? value : parseBufferedNumber(draft, true);
  const isNever = allowNever && displayed.trim() === '';
  const hasValidExponent =
    previewExponent !== null && previewExponent >= 0 && previewExponent <= MAX_DYADIC_EXPONENT;
  // Line 2 of the preview. Errors take over the line rather than adding a
  // fourth row child, so the field keeps its three-row shape for subgrid.
  const detail = draftError
    ? draftError
    : isNever
      ? 'never'
      : hasValidExponent
        ? `2^-${String(previewExponent)} = ${formatDyadicProbability(previewExponent!)}`
        : error ?? 'Enter an integer from 0 to 63';
  const detailIsError = Boolean(draftError) || (!isNever && !hasValidExponent);

  return (
    <>
      <input
        className={className}
        type="number"
        inputMode="numeric"
        min={0}
        max={MAX_DYADIC_EXPONENT}
        step={1}
        disabled={disabled}
        placeholder={allowNever ? 'never' : undefined}
        value={displayed}
        onChange={(event) => {
          setDraft(event.target.value);
          setDraftError(null);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault();
            commit();
            event.currentTarget.blur();
          }
          if (event.key === 'Escape') {
            event.preventDefault();
            setDraft(null);
            setDraftError(null);
          }
        }}
      />
      <span className={styles.preview}>
        <span>Probability:</span>
        <span className={detailIsError ? styles.previewError : undefined}>{detail}</span>
      </span>
    </>
  );
}

interface BufferedCodeTextareaProps {
  className: string;
  disabled: boolean;
  onCommit(value: number[]): void;
  value: number[];
}

function BufferedCodeTextarea({
  className,
  disabled,
  onCommit,
  value,
}: BufferedCodeTextareaProps): JSX.Element {
  const [draft, setDraft] = useState<string | null>(null);

  useEffect(() => {
    if (disabled) {
      setDraft(null);
    }
  }, [disabled]);

  const commit = () => {
    if (draft === null) {
      return;
    }

    const parsed = parseCodeDraft(draft);
    if (parsed !== null) {
      onCommit(parsed);
    }

    setDraft(null);
  };

  return (
    <textarea
      className={className}
      disabled={disabled}
      value={draft ?? value.join(', ')}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          setDraft(null);
        }
      }}
    />
  );
}

export function ConfigEditor(): JSX.Element {
  const { config, configErrorSummary, configIsValid, randomizeSeed, setConfig, state } = useSimContext();
  const [toolMessage, setToolMessage] = useState<string | null>(null);

  const errors = validateConfig(config);
  const isEditable = state.simStatus === 'none';
  const hasSeedPlacementError = Object.keys(errors).some(
    (key) => key.startsWith('seed_programs.') && (key.endsWith('.x') || key.endsWith('.y')),
  );

  const setField = <K extends keyof SimConfig>(field: K, value: SimConfig[K]) => {
    setConfig((current) => ({
      ...current,
      [field]: value,
    }));
  };

  const copyConfigAsJson = async () => {
    if (!window.navigator.clipboard?.writeText) {
      setToolMessage('Clipboard API unavailable in this browser.');
      return;
    }

    let sourceConfig = config;
    let copiedLiveConfig = false;

    if (state.simStatus !== 'none') {
      try {
        const liveConfig = await fetchSimulationConfig();
        if (liveConfig) {
          sourceConfig = liveConfig;
          copiedLiveConfig = true;
        }
      } catch {
        // Fall back to the local editor config if the live config request fails.
      }
    }

    try {
      await window.navigator.clipboard.writeText(JSON.stringify(sourceConfig, null, 2));
      setToolMessage(
        copiedLiveConfig
          ? 'Copied current simulation config JSON to clipboard.'
          : state.simStatus !== 'none'
            ? 'Copied local editor config JSON to clipboard.'
            : 'Copied config JSON to clipboard.',
      );
    } catch (error) {
      setToolMessage(error instanceof Error ? error.message : 'Failed to copy config JSON.');
    }
  };

  return (
    <section className={styles.panel}>
      {!isEditable ? <div className={styles.banner}>Config is locked while a simulation exists.</div> : null}
      {state.apiError ? <div className={styles.banner}>{state.apiError}</div> : null}
      {isEditable && !configIsValid && configErrorSummary ? (
        <div className={hasSeedPlacementError ? styles.bannerStrong : styles.banner}>
          {hasSeedPlacementError ? `Seed Program Outside Grid: ${configErrorSummary}` : configErrorSummary}
        </div>
      ) : null}
      {isEditable && !state.apiError ? (
        <p className={styles.muted}>The grid stays blank until the backend is reachable and you create a simulation.</p>
      ) : null}

      <div className={styles.group}>
        <h3 className={styles.groupTitle}>Grid</h3>
        <div className={styles.grid}>
          <Field label="Width" title={FIELD_TITLES.width}>
            <BufferedNumberInput
              className={styles.input}
              integer
              value={config.width}
              disabled={!isEditable}
              onCommit={(value) => setField('width', value)}
            />
            <HintSlot error={errors.width} />
          </Field>
          <Field label="Height" title={FIELD_TITLES.height}>
            <BufferedNumberInput
              className={styles.input}
              integer
              value={config.height}
              disabled={!isEditable}
              onCommit={(value) => setField('height', value)}
            />
            <HintSlot error={errors.height} />
          </Field>
        </div>
        <Field label="Seed" title={FIELD_TITLES.seed}>
          <BufferedNumberInput
            className={styles.input}
            integer
            value={config.seed}
            disabled={!isEditable}
            onCommit={(value) => setField('seed', value)}
          />
          <HintSlot error={errors.seed} />
        </Field>
        <div className={styles.buttonRow}>
          <button
            className={styles.buttonSecondary}
            type="button"
            disabled={!isEditable}
            onClick={() => {
              randomizeSeed();
              setToolMessage(null);
            }}
          >
            Randomize Seed
          </button>
          <button className={styles.buttonSecondary} type="button" onClick={() => void copyConfigAsJson()}>
            Copy JSON
          </button>
          <button
            className={styles.buttonSecondary}
            type="button"
            disabled={!isEditable}
            onClick={() => {
              saveConfigToStorage(config);
              setToolMessage('Saved current config to this browser.');
            }}
          >
            Save Config
          </button>
          <button
            className={styles.buttonSecondary}
            type="button"
            disabled={!isEditable}
            onClick={() => {
              try {
                const savedConfig = loadConfigFromStorage();
                if (!savedConfig) {
                  setToolMessage('No saved config found in this browser yet.');
                  return;
                }
                setConfig(savedConfig);
                setToolMessage('Loaded saved config from this browser.');
              } catch (error) {
                setToolMessage(error instanceof Error ? error.message : 'Failed to load saved config.');
              }
            }}
          >
            Load Config
          </button>
        </div>
        <p className={styles.muted}>Config edits apply on blur or Enter. Press Escape to revert the active field.</p>
        {toolMessage ? <p className={styles.muted}>{toolMessage}</p> : null}
      </div>

      <ConfigGroup
        title="Resource Rates"
        fields={[
          ['r_energy', 'R Energy'],
          ['r_mass', 'R Mass'],
          ['d_energy_log2', 'D Energy Log2'],
          ['d_mass_log2', 'D Mass Log2'],
          ['t_cap', 'T Cap'],
        ]}
        config={config}
        errors={errors}
        isEditable={isEditable}
        setField={setField}
      />

      <ConfigGroup
        title="Program Dynamics"
        fields={[
          ['maintenance_rate_log2', 'Maintenance Rate Log2'],
          ['maintenance_exponent', 'Maintenance Exponent'],
          ['local_action_exponent', 'Local Action Exponent'],
          ['n_synth', 'N Synth'],
          ['inert_grace_ticks', 'Inert Grace Ticks'],
          ['p_spawn_log2', 'P Spawn Log2'],
        ]}
        config={config}
        errors={errors}
        isEditable={isEditable}
        setField={setField}
      />

      <ConfigGroup
        title="Mutation"
        fields={[
          ['mutation_base_log2', 'Mutation Base Log2'],
          ['mutation_background_log2', 'Mutation Background Log2'],
        ]}
        config={config}
        errors={errors}
        isEditable={isEditable}
        setField={setField}
      />

      <div className={styles.group}>
        <h3 className={styles.groupTitle}>Seed Programs</h3>
        {config.seed_programs.length === 0 ? <p className={styles.muted}>No seed programs configured.</p> : null}
        {config.seed_programs.map((seedProgram, index) => {
          return (
            <div key={`${index}-${seedProgram.x}-${seedProgram.y}`} className={styles.seedCard}>
              <div className={styles.grid}>
                <Field label="X" title={SEED_PROGRAM_CELL_TITLE}>
                  <BufferedNumberInput
                    className={styles.input}
                    integer
                    value={seedProgram.x}
                    disabled={!isEditable}
                    onCommit={(value) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, { x: value }),
                      )
                    }
                  />
                  <HintSlot error={errors[`seed_programs.${index}.x`]} />
                </Field>
                <Field label="Y" title={SEED_PROGRAM_CELL_TITLE}>
                  <BufferedNumberInput
                    className={styles.input}
                    integer
                    value={seedProgram.y}
                    disabled={!isEditable}
                    onCommit={(value) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, { y: value }),
                      )
                    }
                  />
                  <HintSlot error={errors[`seed_programs.${index}.y`]} />
                </Field>
                <Field label="Free Energy" title={SEED_PROGRAM_RESOURCE_TITLE}>
                  <BufferedNumberInput
                    className={styles.input}
                    integer
                    value={seedProgram.free_energy}
                    disabled={!isEditable}
                    onCommit={(value) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, {
                          free_energy: value,
                        }),
                      )
                    }
                  />
                  <HintSlot />
                </Field>
                <Field label="Free Mass" title={SEED_PROGRAM_RESOURCE_TITLE}>
                  <BufferedNumberInput
                    className={styles.input}
                    integer
                    value={seedProgram.free_mass}
                    disabled={!isEditable}
                    onCommit={(value) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, {
                          free_mass: value,
                        }),
                      )
                    }
                  />
                  <HintSlot />
                </Field>
              </div>
              <Field label="Code (comma-separated decimals)">
                <BufferedCodeTextarea
                  className={styles.textarea}
                  value={seedProgram.code}
                  disabled={!isEditable}
                  onCommit={(value) =>
                    setField(
                      'seed_programs',
                      updateSeedProgram(config.seed_programs, index, {
                        code: value,
                      }),
                    )
                  }
                />
                <HintSlot error={errors[`seed_programs.${index}.code`]} />
              </Field>
              <div className={styles.buttonRow}>
                <button
                  className={styles.buttonSecondary}
                  type="button"
                  disabled={!isEditable}
                  onClick={() =>
                    setField(
                      'seed_programs',
                      config.seed_programs.filter((_, currentIndex) => currentIndex !== index),
                    )
                  }
                >
                  Remove
                </button>
              </div>
            </div>
          );
        })}
        <div className={styles.buttonRow}>
          <button
            className={styles.button}
            type="button"
            disabled={!isEditable}
            onClick={() =>
              setField('seed_programs', [
                ...config.seed_programs,
                { x: 0, y: 0, code: [], free_energy: 0, free_mass: 0 },
              ])
            }
          >
            Add Seed Program
          </button>
        </div>
      </div>
    </section>
  );
}

interface ConfigGroupProps {
  title: string;
  fields: Array<[keyof SimConfig, string]>;
  config: SimConfig;
  errors: ConfigErrors;
  isEditable: boolean;
  setField: <K extends keyof SimConfig>(field: K, value: SimConfig[K]) => void;
}

function ConfigGroup({ config, errors, fields, isEditable, setField, title }: ConfigGroupProps): JSX.Element {
  return (
    <div className={styles.group}>
      <h3 className={styles.groupTitle}>{title}</h3>
      <div className={styles.grid}>
        {fields.map(([field, label]) => (
          <Field key={String(field)} label={label} title={FIELD_TITLES[field]}>
            {EXPONENT_FIELDS.has(field) ? (
              <ExponentInput
                allowNever={OPTIONAL_EXPONENT_FIELDS.has(field)}
                className={styles.input}
                disabled={!isEditable}
                error={errors[String(field)]}
                value={config[field] as number | null}
                onCommit={(value) => setField(field, value as SimConfig[typeof field])}
              />
            ) : (
              <>
                <BufferedNumberInput
                  className={styles.input}
                  integer={INTEGER_FIELDS.has(field)}
                  disabled={!isEditable}
                  value={config[field] as number}
                  onCommit={(value) => setField(field, value as SimConfig[typeof field])}
                />
                <HintSlot error={errors[String(field)]} />
              </>
            )}
          </Field>
        ))}
      </div>
    </div>
  );
}
