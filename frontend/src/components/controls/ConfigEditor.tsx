import { useEffect, useState } from 'react';
import {
  type ConfigErrors,
  loadConfigFromStorage,
  saveConfigToStorage,
  validateConfig,
} from '../../lib/config';
import { fetchSimulationConfig } from '../../lib/api';
import { useSimContext } from '../../context/SimContext';
import type { SeedProgram, SimConfig } from '../../types';
import styles from './ConfigEditor.module.css';

const INTEGER_FIELDS = new Set<keyof SimConfig>([
  'width',
  'height',
  'seed',
  'n_synth',
  'inert_grace_ticks',
  'mutation_base_log2',
  'mutation_background_log2',
]);

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
          <label className={styles.field}>
            <span>Width</span>
            <BufferedNumberInput
              className={styles.input}
              integer
              value={config.width}
              disabled={!isEditable}
              onCommit={(value) => setField('width', value)}
            />
            {errors.width ? <span className={styles.error}>{errors.width}</span> : null}
          </label>
          <label className={styles.field}>
            <span>Height</span>
            <BufferedNumberInput
              className={styles.input}
              integer
              value={config.height}
              disabled={!isEditable}
              onCommit={(value) => setField('height', value)}
            />
            {errors.height ? <span className={styles.error}>{errors.height}</span> : null}
          </label>
        </div>
        <label className={styles.field}>
          <span>Seed</span>
          <BufferedNumberInput
            className={styles.input}
            integer
            value={config.seed}
            disabled={!isEditable}
            onCommit={(value) => setField('seed', value)}
          />
        </label>
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
          ['d_energy', 'D Energy'],
          ['d_mass', 'D Mass'],
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
          ['maintenance_rate', 'Maintenance Rate'],
          ['maintenance_exponent', 'Maintenance Exponent'],
          ['local_action_exponent', 'Local Action Exponent'],
          ['n_synth', 'N Synth'],
          ['inert_grace_ticks', 'Inert Grace Ticks'],
          ['p_spawn', 'P Spawn'],
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
                <label className={styles.field}>
                  <span>X</span>
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
                  {errors[`seed_programs.${index}.x`] ? (
                    <span className={styles.error}>{errors[`seed_programs.${index}.x`]}</span>
                  ) : null}
                </label>
                <label className={styles.field}>
                  <span>Y</span>
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
                  {errors[`seed_programs.${index}.y`] ? (
                    <span className={styles.error}>{errors[`seed_programs.${index}.y`]}</span>
                  ) : null}
                </label>
                <label className={styles.field}>
                  <span>Free Energy</span>
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
                </label>
                <label className={styles.field}>
                  <span>Free Mass</span>
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
                </label>
              </div>
              <label className={styles.field}>
                <span>Code (comma-separated decimals)</span>
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
                {errors[`seed_programs.${index}.code`] ? (
                  <span className={styles.error}>{errors[`seed_programs.${index}.code`]}</span>
                ) : null}
              </label>
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
          <label key={String(field)} className={styles.field}>
            <span>{label}</span>
            <BufferedNumberInput
              className={styles.input}
              integer={INTEGER_FIELDS.has(field)}
              disabled={!isEditable}
              value={config[field] as number}
              onCommit={(value) => setField(field, value as SimConfig[typeof field])}
            />
            {errors[String(field)] ? <span className={styles.error}>{errors[String(field)]}</span> : null}
          </label>
        ))}
      </div>
    </div>
  );
}
