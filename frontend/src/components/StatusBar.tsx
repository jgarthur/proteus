import { COLOR_MAP_OPTIONS } from '../constants';
import { useSimContext } from '../context/SimContext';
import { formatDecimal, formatInteger } from '../lib/format';
import styles from './StatusBar.module.css';

export function StatusBar(): JSX.Element {
  const { latestMetrics, setColorMap, setMetricsDrawerOpen, state } = useSimContext();
  const displayedTps =
    state.frontendTickerActive && state.targetTps !== 'max' ? state.targetTps : state.ticksPerSecond;

  return (
    <footer className={styles.bar}>
      <div className={styles.metrics}>
        <span>Tick {formatInteger(latestMetrics?.tick ?? state.tick)}</span>
        <span>
          Population {formatInteger(latestMetrics?.population)} ({formatInteger(latestMetrics?.live_count)} /{' '}
          {formatInteger(latestMetrics?.inert_count)})
        </span>
        <span>Energy {formatInteger(latestMetrics?.total_energy)}</span>
        <span>Mass {formatInteger(latestMetrics?.total_mass)}</span>
        <span>TPS {formatDecimal(displayedTps, 1)}</span>
      </div>
      <div className={styles.controls}>
        <select
          className={styles.select}
          value={state.colorMap}
          onChange={(event) => setColorMap(event.target.value as typeof state.colorMap)}
        >
          {COLOR_MAP_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <button
          className={styles.button}
          type="button"
          onClick={() => setMetricsDrawerOpen(!state.metricsDrawerOpen)}
        >
          Charts {state.metricsDrawerOpen ? 'Down' : 'Up'}
        </button>
      </div>
      <div className={styles.status}>WS {state.wsStatus}</div>
    </footer>
  );
}
