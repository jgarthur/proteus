import { directionLabel, formatInteger } from '../../lib/format';
import { cellIndexToCoordinates, decodeProgramSite, originLabel } from '../../lib/lineage';
import { useSimContext } from '../../context/SimContext';
import type { CellProgram } from '../../types';
import styles from './InspectorTab.module.css';

const MAX_RENDERED_STACK_ENTRIES = 64;

export function InspectorTab(): JSX.Element {
  const {
    selectCell,
    selectedCellData,
    selectedCellStamp,
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

  // A uid only decodes against the simulation instance and grid it was fetched
  // under: reset, create, and destroy start a new uid epoch, and a resize makes
  // the modulo decode land in range on the wrong cell. Retained data that
  // predates such a boundary still renders, but its parent uid is inert text.
  const inspectionMatchesCurrentSim =
    selectedCellStamp !== null &&
    selectedCellStamp.simEpoch === state.simEpoch &&
    selectedCellStamp.gridWidth === state.gridWidth &&
    selectedCellStamp.gridHeight === state.gridHeight;
  const isStale =
    state.simStatus === 'running' &&
    selectedCellFetchedAt !== null &&
    Date.now() - selectedCellFetchedAt > 1_000;
  const stack = selectedCellData?.program?.stack ?? [];
  const stackIsTruncated = stack.length > MAX_RENDERED_STACK_ENTRIES;
  const visibleStack = stackIsTruncated ? stack.slice(-MAX_RENDERED_STACK_ENTRIES) : stack;
  const stackPreview = stackIsTruncated ? `..., ${visibleStack.join(', ')}` : visibleStack.join(', ');

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
        {selectedCellLoading && !selectedCellData ? <p className={styles.muted}>Loading cell data…</p> : null}
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
              <span className={styles.mono}>[{stackPreview}]</span>
            </div>
            {stackIsTruncated ? (
              <p className={styles.muted}>
                Showing the last {MAX_RENDERED_STACK_ENTRIES} of {formatInteger(stack.length)} stack entries.
              </p>
            ) : null}
            {!selectedCellData.program.live ? (
              <KeyValue label="Abandonment timer" value={selectedCellData.program.abandonment_timer ?? '—'} />
            ) : null}
          </section>

          <section className={styles.card}>
            <h2 className={styles.title}>Lineage</h2>
            <Lineage
              program={selectedCellData.program}
              gridWidth={state.gridWidth}
              gridHeight={state.gridHeight}
              canDecodeParent={inspectionMatchesCurrentSim}
              onSelectCell={selectCell}
            />
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

function Lineage({
  program,
  gridWidth,
  gridHeight,
  canDecodeParent,
  onSelectCell,
}: {
  program: CellProgram;
  gridWidth: number;
  gridHeight: number;
  canDecodeParent: boolean;
  onSelectCell: (cell: { x: number; y: number } | null) => void;
}): JSX.Element {
  const parentUid = program.parent_uid;
  const parentSite =
    parentUid === null || !canDecodeParent ? null : decodeProgramSite(parentUid, gridWidth * gridHeight);
  const parentCell = parentSite === null ? null : cellIndexToCoordinates(parentSite.cellIndex, gridWidth, gridHeight);

  return (
    <>
      <KeyValue label="Origin" value={originLabel(program.origin)} />
      <KeyValue label="Generation" value={formatInteger(program.generation)} />
      <KeyValue label="Birth tick" value={formatInteger(program.birth_tick)} />
      <KeyValue label="Created tick" value={formatInteger(program.created_tick)} />
      <KeyValue label="uid" value={String(program.uid)} />
      <div className={styles.kv}>
        <span>Parent uid</span>
        {parentUid === null ? (
          <span className={styles.mono}>—</span>
        ) : parentCell ? (
          <button
            type="button"
            className={styles.uidLink}
            title={`Inspect the cell the parent was created in (${parentCell.x}, ${parentCell.y}) at tick ${formatInteger(parentSite?.tick)}`}
            onClick={() => onSelectCell(parentCell)}
          >
            {parentUid}
          </button>
        ) : (
          <span className={styles.mono}>{parentUid}</span>
        )}
      </div>
      {parentCell ? (
        <p className={styles.muted}>
          Parent created at ({parentCell.x}, {parentCell.y}) on tick {formatInteger(parentSite?.tick)}; it may have
          since moved or died.
        </p>
      ) : null}
    </>
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
