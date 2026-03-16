import { useState } from 'react';
import { TARGET_TPS_OPTIONS } from '../../constants';
import { useSimContext } from '../../context/SimContext';
import { ConfigEditor } from './ConfigEditor';
import styles from './ControlsTab.module.css';

export function ControlsTab(): JSX.Element {
  const {
    configErrorSummary,
    configIsValid,
    createFromConfig,
    destroy,
    pause,
    reset,
    resume,
    setControlsConfigOpen,
    setEveryNTicks,
    setMaxFps,
    setTargetTps,
    start,
    state,
    step,
  } = useSimContext();
  const [stepCount, setStepCount] = useState(1);
  const targetTpsIndex = TARGET_TPS_OPTIONS.findIndex((option) => option === state.targetTps);
  const isRunning = state.simStatus === 'running' || state.frontendTickerActive;
  const isPaused = state.simStatus === 'paused' && !state.frontendTickerActive;
  const targetTpsLabel = state.targetTps === 'max' ? 'max' : `${state.targetTps} TPS`;

  return (
    <div className={styles.stack}>
      <div className={styles.topPanels}>
        <section className={styles.panel}>
          <h2 className={styles.title}>Lifecycle</h2>
          <div className={styles.row}>
            <button
              className={styles.button}
              type="button"
              disabled={state.simStatus !== 'none' || !configIsValid}
              onClick={() => void createFromConfig()}
            >
              Create
            </button>
            <button className={styles.button} type="button" disabled={state.simStatus !== 'created'} onClick={() => void start()}>
              Start
            </button>
            <button className={styles.button} type="button" disabled={!isRunning} onClick={() => void pause()}>
              Pause
            </button>
            <button className={styles.button} type="button" disabled={!isPaused} onClick={() => void resume()}>
              Resume
            </button>
          </div>
          {!configIsValid ? <p className={styles.note}>{configErrorSummary}</p> : null}
          <div className={styles.row} style={{ marginTop: 10 }}>
            <input
              className={styles.input}
              type="number"
              min={1}
              value={stepCount}
              disabled={!isPaused}
              onChange={(event) => setStepCount(Math.max(1, Number(event.target.value) || 1))}
            />
            <button className={styles.button} type="button" disabled={!isPaused} onClick={() => void step(stepCount)}>
              Step
            </button>
            <button
              className={styles.button}
              type="button"
              disabled={state.simStatus === 'none'}
              onClick={() => {
                if (window.confirm('Reset the simulation to tick 0 using the existing config?')) {
                  void reset();
                }
              }}
            >
              Reset
            </button>
            <button
              className={`${styles.button} ${styles.danger}`}
              type="button"
              disabled={state.simStatus === 'none'}
              onClick={() => {
                if (window.confirm('Destroy the current simulation?')) {
                  void destroy();
                }
              }}
            >
              Destroy
            </button>
          </div>
        </section>

        <section className={styles.panel}>
          <h2 className={styles.title}>Streaming</h2>
          <label className={styles.fieldRow}>
            <span>Target tick rate</span>
            <div className={styles.sliderBlock}>
              <div className={styles.rangeRow}>
                <input
                  type="range"
                  min={0}
                  max={TARGET_TPS_OPTIONS.length - 1}
                  step={1}
                  value={targetTpsIndex}
                  onChange={(event) => void setTargetTps(TARGET_TPS_OPTIONS[Number(event.target.value)]!)}
                />
                <span>{targetTpsLabel}</span>
              </div>
              <div className={styles.scaleLabels}>
                {TARGET_TPS_OPTIONS.map((option) => (
                  <span key={String(option)}>{String(option)}</span>
                ))}
              </div>
            </div>
          </label>
          <p className={styles.noteStable}>
            {state.targetTps !== 'max'
              ? 'Temporary frontend shim: the backend stays paused and the UI issues repeated step requests to approximate the selected tick rate.'
              : 'Backend-native running mode: target tick rate is unlimited and the frontend shim is idle.'}
          </p>

          <label className={styles.fieldRow}>
            <span>Frame rate</span>
            <div className={styles.rangeRow}>
              <input
                type="range"
                min={1}
                max={60}
                value={state.maxFps}
                onChange={(event) => setMaxFps(Number(event.target.value))}
              />
              <span>{state.maxFps} fps</span>
            </div>
          </label>

          <label className={styles.fieldRow} style={{ marginTop: 14 }}>
            <span>Metrics every N ticks</span>
            <input
              className={styles.input}
              type="number"
              min={1}
              value={state.everyNTicks}
              onChange={(event) => setEveryNTicks(Math.max(1, Number(event.target.value) || 1))}
            />
          </label>
        </section>

        <section className={styles.panel}>
          <h2 className={styles.title}>Snapshots</h2>
          <p className={styles.muted}>Unavailable — backend support pending.</p>
        </section>
      </div>

      <div className={styles.configWrap}>
        <div className={styles.configHeader}>
          <h2 className={styles.title}>Config</h2>
          <button
            className={styles.configButton}
            type="button"
            onClick={() => setControlsConfigOpen(!state.controlsConfigOpen)}
          >
            {state.controlsConfigOpen ? 'Collapse' : 'Expand'}
          </button>
        </div>
        <div className={state.controlsConfigOpen ? styles.configBodyOpen : styles.configBodyClosed}>
          <ConfigEditor />
        </div>
      </div>
    </div>
  );
}
