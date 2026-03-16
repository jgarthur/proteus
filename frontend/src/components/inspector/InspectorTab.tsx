import { directionLabel, formatInteger } from '../../lib/format';
import { useSimContext } from '../../context/SimContext';
import styles from './InspectorTab.module.css';

export function InspectorTab(): JSX.Element {
  const {
    selectedCellData,
    selectedCellError,
    selectedCellFetchedAt,
    selectedCellLoading,
    state,
  } = useSimContext();

  if (!state.selectedCell) {
    return (
      <div className={styles.card}>
        <h2 className={styles.title}>Inspector</h2>
        <p className={styles.muted}>Click a cell to inspect.</p>
      </div>
    );
  }

  const isStale =
    state.simStatus === 'running' &&
    selectedCellFetchedAt !== null &&
    Date.now() - selectedCellFetchedAt > 1_000;

  return (
    <div className={styles.panel}>
      <section className={styles.card}>
        <h2 className={styles.title}>Cell</h2>
        <div className={styles.kv}>
          <span>Coordinates</span>
          <span className={styles.mono}>
            ({state.selectedCell.x}, {state.selectedCell.y})
          </span>
        </div>
        {selectedCellLoading ? <p className={styles.muted}>Loading cell data…</p> : null}
        {selectedCellError ? <p className={styles.error}>{selectedCellError}</p> : null}
        {selectedCellData ? (
          <>
            <div className={styles.kv}>
              <span>Flat Index</span>
              <span className={styles.mono}>{formatInteger(selectedCellData.index)}</span>
            </div>
            <div className={styles.kv}>
              <span>Status</span>
              <span>{selectedCellData.program ? (selectedCellData.program.live ? 'live' : 'inert') : 'empty'}</span>
            </div>
            <div className={styles.kv}>
              <span>Free Energy</span>
              <span>{formatInteger(selectedCellData.free_energy)}</span>
            </div>
            <div className={styles.kv}>
              <span>Free Mass</span>
              <span>{formatInteger(selectedCellData.free_mass)}</span>
            </div>
            <div className={styles.kv}>
              <span>Bg Radiation</span>
              <span>{formatInteger(selectedCellData.bg_radiation)}</span>
            </div>
            <div className={styles.kv}>
              <span>Bg Mass</span>
              <span>{formatInteger(selectedCellData.bg_mass)}</span>
            </div>
            {isStale ? <p className={styles.muted}>Data is older than 1 second.</p> : null}
          </>
        ) : null}
      </section>

      {selectedCellData?.program ? (
        <>
          <section className={styles.card}>
            <h2 className={styles.title}>Program</h2>
            <KeyValue label="ID" value={selectedCellData.program.id} />
            <KeyValue label="Size" value={selectedCellData.program.size} />
            <KeyValue label="Age" value={selectedCellData.program.age} />
            <KeyValue label="IP" value={selectedCellData.program.ip} />
            <KeyValue label="src" value={selectedCellData.program.src} />
            <KeyValue label="dst" value={selectedCellData.program.dst} />
            <KeyValue
              label="dir"
              value={`${selectedCellData.program.dir} (${directionLabel(selectedCellData.program.dir)})`}
            />
            <KeyValue label="flag" value={String(selectedCellData.program.flag)} />
            <KeyValue label="msg" value={selectedCellData.program.msg} />
            <KeyValue label="lc" value={selectedCellData.program.lc} />
            <div className={styles.kv}>
              <span>Stack</span>
              <span className={styles.mono}>[{selectedCellData.program.stack.join(', ')}]</span>
            </div>
            {!selectedCellData.program.live ? (
              <KeyValue label="Abandonment timer" value={selectedCellData.program.abandonment_timer ?? '—'} />
            ) : null}
          </section>

          <section className={styles.card}>
            <h2 className={styles.title}>Disassembly</h2>
            <div className={styles.disassembly}>
              {selectedCellData.program.disassembly.map((instruction, index) => (
                <div
                  key={`${index}-${instruction}`}
                  className={index === selectedCellData.program?.ip ? styles.instructionActive : styles.instruction}
                >
                  <span className={styles.mono}>{index}</span>
                  <span>{instruction}</span>
                  <span className={styles.mono}>{selectedCellData.program?.code[index] ?? 0}</span>
                </div>
              ))}
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}

function KeyValue({ label, value }: { label: string; value: number | string }): JSX.Element {
  return (
    <div className={styles.kv}>
      <span>{label}</span>
      <span className={styles.mono}>{value}</span>
    </div>
  );
}
