import { Fragment, useState } from 'react';
import { disassemble } from '../../lib/opcodes';
import { MAX_COMPOSER_TOTAL, MAX_COMPOSER_WEIGHT, type ComposerRow } from '../../lib/populate';
import { seedLibraryByFamily, type SeedOrganism } from '../../lib/seedLibrary';
import styles from './ConfigEditor.module.css';

const COLUMN_TITLES = {
  active: 'Include this organism in the mix.',
  organism: 'Built-in seed organism. Click the name to read its evidence, regime, and listing.',
  weight: 'Relative share of the total. Two organisms at 1 and 3 split the total 25/75.',
  count: 'Exact number placed, apportioned from the weights by largest remainder.',
  bytes: 'Program size in instructions.',
} as const;

const FAMILY_TITLES: Record<string, string> = {
  Classic: 'Hand-designed and evolved organisms that predate the ladder. No verified regime.',
  Chain: 'Stationary chain replicators: collect mass, copy the body, boot the child.',
  Synthesis: 'Builders for worlds with no ambient mass — each offspring byte is synthesized.',
  Mobile: 'Foragers that move between cycles instead of sitting in their own depletion halo.',
};

const TOTAL_TITLE = 'How many organisms to place in all, split across the active rows by weight.';
const EQUALIZE_TITLE = 'Set every weight back to 1 for an even mix.';
const PLACEMENT_SEED_TITLE =
  'Which free cells the mix lands on, combined with the config seed. Same seeds, grid, and counts always give the same layout.';
const NEW_PLACEMENT_TITLE =
  'Re-roll which cells the mix lands on without changing the simulation seed.';

/** Placement seeds are u32: the placement PRNG consumes exactly 32 bits. */
const MAX_PLACEMENT_SEED = 2 ** 32 - 1;

interface NumberCellProps {
  className: string;
  disabled: boolean;
  integer?: boolean;
  max: number;
  min: number;
  step: number;
  value: number;
  onChange(value: number): void;
}

/**
 * A number input that keeps whatever the user is typing while still pushing
 * every valid intermediate value upward, so half-typed text is never clobbered
 * by a re-render. Values are clamped to `max` before they leave, so no amount
 * of typing can push a weight or total into a range the arithmetic downstream
 * cannot represent.
 */
function NumberCell({
  className,
  disabled,
  integer = false,
  max,
  min,
  step,
  value,
  onChange,
}: NumberCellProps): JSX.Element {
  const [draft, setDraft] = useState<string | null>(null);

  return (
    <input
      className={className}
      type="number"
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      value={draft ?? String(value)}
      onChange={(event) => {
        const text = event.target.value;
        setDraft(text);
        const parsed = Number(text);
        if (text.trim() === '' || !Number.isFinite(parsed) || parsed < min) {
          return;
        }
        if (integer && !Number.isInteger(parsed)) {
          return;
        }
        onChange(Math.min(parsed, max));
      }}
      onBlur={() => setDraft(null)}
    />
  );
}

/** The expanded body of one organism row: what it is, how well evidenced, and its listing. */
function OrganismDetail({ organism }: { organism: SeedOrganism }): JSX.Element {
  return (
    <div className={styles.organismDetail}>
      <p className={styles.hint}>{organism.description}</p>
      <dl className={styles.detailList}>
        <dt>Evidence</dt>
        <dd>{organism.evidence}</dd>
        <dt>Regime</dt>
        <dd>{organism.regime}</dd>
        <dt>Source</dt>
        <dd>{organism.provenance}</dd>
        <dt>Start</dt>
        <dd>
          {organism.free_energy} free energy, {organism.free_mass} free mass
        </dd>
      </dl>
      <pre className={styles.listing}>{disassemble(organism.code).join('\n')}</pre>
    </div>
  );
}

interface SeedComposerProps {
  clampedTo: number | null;
  counts: Map<string, number>;
  disabled: boolean;
  library: readonly SeedOrganism[];
  placementSeed: number;
  requested: number;
  rows: readonly ComposerRow[];
  total: number;
  onEqualize(): void;
  onNewPlacementSeed(): void;
  onPlacementSeedChange(placementSeed: number): void;
  onRowChange(organismId: string, patch: Partial<ComposerRow>): void;
  onTotalChange(total: number): void;
}

/**
 * Population composer: a total plus per-organism weights, resolved into exact
 * counts and scattered onto distinct random cells. The composer's own state
 * lives in app state, not in `SimConfig`; every change here is written straight
 * through into `seed_programs`, replacing what the composer placed last time
 * and leaving hand-placed entries alone.
 *
 * Organisms are grouped by structural family, and each row expands in place to
 * show what the organism is, how well it is evidenced, the regime it was
 * verified in, and its disassembly.
 */
export function SeedComposer({
  clampedTo,
  counts,
  disabled,
  library,
  placementSeed,
  requested,
  rows,
  total,
  onEqualize,
  onNewPlacementSeed,
  onPlacementSeedChange,
  onRowChange,
  onTotalChange,
}: SeedComposerProps): JSX.Element {
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const groups = seedLibraryByFamily(library);

  const toggle = (organismId: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (!next.delete(organismId)) {
        next.add(organismId);
      }
      return next;
    });
  };

  return (
    <div className={styles.composer}>
      <div className={styles.composerHead}>
        <h4 className={styles.subTitle}>Population Composer</h4>
        <span className={styles.hint}>
          Weights set the mix; it lands on random free cells as you edit.
        </span>
      </div>

      <table className={styles.composerTable}>
        <thead>
          <tr>
            <th scope="col" title={COLUMN_TITLES.active}>
              Use
            </th>
            <th scope="col" title={COLUMN_TITLES.organism}>
              Organism
            </th>
            <th scope="col" title={COLUMN_TITLES.weight}>
              Weight
            </th>
            <th scope="col" title={COLUMN_TITLES.count}>
              Count
            </th>
            <th scope="col" title={COLUMN_TITLES.bytes}>
              Bytes
            </th>
          </tr>
        </thead>
        {groups.map((group) => (
          <tbody key={group.family}>
            <tr className={styles.familyRow}>
              <th colSpan={5} scope="rowgroup" title={FAMILY_TITLES[group.family]}>
                {group.family}
              </th>
            </tr>
            {group.organisms.map((organism) => {
              const row = rows.find((candidate) => candidate.organismId === organism.id);
              const active = row?.active ?? false;
              const weight = row?.weight ?? 1;
              const isOpen = expanded.has(organism.id);
              return (
                <Fragment key={organism.id}>
                  <tr>
                    <td>
                      <input
                        type="checkbox"
                        disabled={disabled}
                        checked={active}
                        aria-label={`Include ${organism.name}`}
                        onChange={(event) => onRowChange(organism.id, { active: event.target.checked })}
                      />
                    </td>
                    <td>
                      <button
                        className={styles.disclosure}
                        type="button"
                        aria-expanded={isOpen}
                        title={organism.description}
                        onClick={() => toggle(organism.id)}
                      >
                        <span aria-hidden="true">{isOpen ? '▾' : '▸'}</span> {organism.name}
                      </button>
                    </td>
                    <td>
                      <NumberCell
                        className={styles.numberCell}
                        disabled={disabled || !active}
                        max={MAX_COMPOSER_WEIGHT}
                        min={0}
                        step={0.5}
                        value={weight}
                        onChange={(value) => onRowChange(organism.id, { weight: value })}
                      />
                    </td>
                    <td className={styles.numeric}>{counts.get(organism.id) ?? 0}</td>
                    <td className={styles.numeric}>{organism.code.length}</td>
                  </tr>
                  {isOpen ? (
                    <tr className={styles.detailRow}>
                      <td colSpan={5}>
                        <OrganismDetail organism={organism} />
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        ))}
      </table>

      <div className={styles.composerControls}>
        <label className={styles.inlineField} title={TOTAL_TITLE}>
          <span>Total</span>
          <NumberCell
            className={styles.numberCell}
            disabled={disabled}
            integer
            max={MAX_COMPOSER_TOTAL}
            min={0}
            step={1}
            value={total}
            onChange={onTotalChange}
          />
        </label>
        <button
          className={`${styles.button} ${styles.buttonSecondary}`}
          type="button"
          title={EQUALIZE_TITLE}
          disabled={disabled}
          onClick={onEqualize}
        >
          Equalize
        </button>
        <label className={styles.inlineField} title={PLACEMENT_SEED_TITLE}>
          <span>Placement seed</span>
          <NumberCell
            className={styles.seedCell}
            disabled={disabled}
            integer
            max={MAX_PLACEMENT_SEED}
            min={0}
            step={1}
            value={placementSeed}
            onChange={onPlacementSeedChange}
          />
        </label>
        <button
          className={`${styles.button} ${styles.buttonSecondary}`}
          type="button"
          title={NEW_PLACEMENT_TITLE}
          disabled={disabled}
          onClick={onNewPlacementSeed}
        >
          New
        </button>
      </div>

      {clampedTo !== null ? (
        <p className={styles.warning}>
          Only {clampedTo} free cells; placed {clampedTo} of {requested}.
        </p>
      ) : null}
    </div>
  );
}
