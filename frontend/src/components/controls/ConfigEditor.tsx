import { useState } from 'react';
import {
  type ConfigErrors,
  loadConfigFromStorage,
  parseCode,
  saveConfigToStorage,
  validateConfig,
} from '../../lib/config';
import { useSimContext } from '../../context/SimContext';
import type { SeedProgram, SimConfig } from '../../types';
import styles from './ConfigEditor.module.css';

function updateSeedProgram(
  seedPrograms: SeedProgram[],
  index: number,
  patch: Partial<SeedProgram>,
): SeedProgram[] {
  return seedPrograms.map((program, currentIndex) =>
    currentIndex === index ? { ...program, ...patch } : program,
  );
}

export function ConfigEditor(): JSX.Element {
  const { config, configErrorSummary, configIsValid, randomizeSeed, setConfig, state } = useSimContext();
  const [storageMessage, setStorageMessage] = useState<string | null>(null);

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
            <input
              className={styles.input}
              type="number"
              value={config.width}
              disabled={!isEditable}
              onChange={(event) => setField('width', Number(event.target.value))}
            />
            {errors.width ? <span className={styles.error}>{errors.width}</span> : null}
          </label>
          <label className={styles.field}>
            <span>Height</span>
            <input
              className={styles.input}
              type="number"
              value={config.height}
              disabled={!isEditable}
              onChange={(event) => setField('height', Number(event.target.value))}
            />
            {errors.height ? <span className={styles.error}>{errors.height}</span> : null}
          </label>
        </div>
        <label className={styles.field}>
          <span>Seed</span>
          <input
            className={styles.input}
            type="number"
            value={config.seed}
            disabled={!isEditable}
            onChange={(event) => setField('seed', Number(event.target.value))}
          />
        </label>
        <div className={styles.buttonRow}>
          <button className={styles.buttonSecondary} type="button" disabled={!isEditable} onClick={() => randomizeSeed()}>
            Randomize Seed
          </button>
          <button
            className={styles.buttonSecondary}
            type="button"
            disabled={!isEditable}
            onClick={() => {
              saveConfigToStorage(config);
              setStorageMessage('Saved current config to this browser.');
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
                  setStorageMessage('No saved config found in this browser yet.');
                  return;
                }
                setConfig(savedConfig);
                setStorageMessage('Loaded saved config from this browser.');
              } catch (error) {
                setStorageMessage(error instanceof Error ? error.message : 'Failed to load saved config.');
              }
            }}
          >
            Load Config
          </button>
        </div>
        {storageMessage ? <p className={styles.muted}>{storageMessage}</p> : null}
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
          const codeValue = seedProgram.code.join(', ');
          return (
            <div key={`${index}-${seedProgram.x}-${seedProgram.y}`} className={styles.seedCard}>
              <div className={styles.grid}>
                <label className={styles.field}>
                  <span>X</span>
                  <input
                    className={styles.input}
                    type="number"
                    value={seedProgram.x}
                    disabled={!isEditable}
                    onChange={(event) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, { x: Number(event.target.value) }),
                      )
                    }
                  />
                  {errors[`seed_programs.${index}.x`] ? (
                    <span className={styles.error}>{errors[`seed_programs.${index}.x`]}</span>
                  ) : null}
                </label>
                <label className={styles.field}>
                  <span>Y</span>
                  <input
                    className={styles.input}
                    type="number"
                    value={seedProgram.y}
                    disabled={!isEditable}
                    onChange={(event) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, { y: Number(event.target.value) }),
                      )
                    }
                  />
                  {errors[`seed_programs.${index}.y`] ? (
                    <span className={styles.error}>{errors[`seed_programs.${index}.y`]}</span>
                  ) : null}
                </label>
                <label className={styles.field}>
                  <span>Free Energy</span>
                  <input
                    className={styles.input}
                    type="number"
                    value={seedProgram.free_energy}
                    disabled={!isEditable}
                    onChange={(event) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, {
                          free_energy: Number(event.target.value),
                        }),
                      )
                    }
                  />
                </label>
                <label className={styles.field}>
                  <span>Free Mass</span>
                  <input
                    className={styles.input}
                    type="number"
                    value={seedProgram.free_mass}
                    disabled={!isEditable}
                    onChange={(event) =>
                      setField(
                        'seed_programs',
                        updateSeedProgram(config.seed_programs, index, {
                          free_mass: Number(event.target.value),
                        }),
                      )
                    }
                  />
                </label>
              </div>
              <label className={styles.field}>
                <span>Code (comma-separated decimals)</span>
                <textarea
                  className={styles.textarea}
                  value={codeValue}
                  disabled={!isEditable}
                  onChange={(event) =>
                    setField(
                      'seed_programs',
                      updateSeedProgram(config.seed_programs, index, {
                        code: parseCode(event.target.value),
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
            <input
              className={styles.input}
              type="number"
              step="any"
              disabled={!isEditable}
              value={config[field] as number}
              onChange={(event) => setField(field, Number(event.target.value) as SimConfig[typeof field])}
            />
            {errors[String(field)] ? <span className={styles.error}>{errors[String(field)]}</span> : null}
          </label>
        ))}
      </div>
    </div>
  );
}
