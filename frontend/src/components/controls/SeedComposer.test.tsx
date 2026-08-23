// @vitest-environment jsdom

/**
 * Component tests for the population composer's live write-through.
 *
 * The composer's interesting behaviour is not in `src/lib/populate.ts` — that
 * module is pure and already covered — but in how `ConfigEditor` decides *when*
 * to rebuild the scatter and what it treats as its own to replace. That
 * decision lives in an effect over derived state (`armed`, the hand-placed cell
 * key, the live-generated set), so it only exists once the editor is mounted
 * inside the real `SimProvider`, which is where the composer session lives.
 *
 * Everything the provider talks to is mocked: `lib/api` (no backend), the
 * WebSocket context (no socket), and `lib/random`, so placement seeds and world
 * seeds come from a counter instead of `Math.random` and every layout below is
 * reproducible.
 */

import { StrictMode, useState } from 'react';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const random = vi.hoisted(() => {
  let placementCalls = 0;
  let seedCalls = 0;
  return {
    reset(): void {
      placementCalls = 0;
      seedCalls = 0;
    },
    randomPlacementSeed(): number {
      placementCalls += 1;
      return 0x1000 * placementCalls + 7;
    },
    randomSeed(): number {
      seedCalls += 1;
      return 900_000 + seedCalls;
    },
  };
});

const webSocket = vi.hoisted(() => ({
  status: 'disconnected' as const,
  addMessageListener: () => () => {},
  sendJson: () => {},
}));

vi.mock('../../lib/random', () => ({
  randomPlacementSeed: random.randomPlacementSeed,
  randomSeed: random.randomSeed,
}));

vi.mock('../../lib/api', () => ({
  createSimulation: vi.fn(),
  destroySimulation: vi.fn(),
  fetchCell: vi.fn(),
  fetchMetrics: vi.fn(),
  fetchSimulationConfig: vi.fn(async () => null),
  getSimStatus: vi.fn(async () => null),
  postSimulationAction: vi.fn(),
  stepSimulation: vi.fn(),
}));

vi.mock('../../context/WebSocketContext', () => ({
  useWebSocketContext: () => webSocket,
}));

import { SimProvider } from '../../context/SimContext';
import { DEFAULT_CONFIG } from '../../constants';
import { SEED_LIBRARY } from '../../lib/seedLibrary';
import { ConfigEditor } from './ConfigEditor';

/** One seed-program card, read back off the DOM. */
interface Card {
  x: number;
  y: number;
  scattered: boolean;
}

const STATUS_TEXT = /^(scattered|hand-placed) · \d+ bytes$/;

/**
 * Reads every seed-program card in document order.
 *
 * The three per-card queries are index-aligned because each card renders
 * exactly one X input, one Y input, and one status line, and the queries return
 * document order. `X`/`Y` are matched as prefixes because a card whose cell
 * falls outside the grid appends its validation error to the same label.
 */
function cards(): Card[] {
  const xs = screen.queryAllByLabelText(/^X/) as HTMLInputElement[];
  const ys = screen.queryAllByLabelText(/^Y/) as HTMLInputElement[];
  const statuses = screen.queryAllByText(STATUS_TEXT);
  expect(ys).toHaveLength(xs.length);
  expect(statuses).toHaveLength(xs.length);

  return xs.map((input, index) => ({
    x: Number(input.value),
    y: Number(ys[index]!.value),
    scattered: (statuses[index]!.textContent ?? '').startsWith('scattered'),
  }));
}

function scatteredCards(): Card[] {
  return cards().filter((card) => card.scattered);
}

function cells(entries: readonly Card[]): string[] {
  return entries.map((entry) => `${entry.x},${entry.y}`);
}

/** Commits a buffered text field (grid size, world seed, a card's X/Y). */
async function commit(user: UserEvent, input: HTMLElement, value: number): Promise<void> {
  await user.clear(input);
  await user.type(input, `${value}{Enter}`);
}

/** Sets one of the composer's own number inputs, which push on every keystroke. */
async function setNumberCell(user: UserEvent, input: HTMLElement, value: number): Promise<void> {
  await user.clear(input);
  await user.type(input, String(value));
  await user.tab();
}

async function setTotal(user: UserEvent, total: number): Promise<void> {
  await setNumberCell(user, screen.getByLabelText('Total'), total);
}

function weightInput(organismName: string): HTMLInputElement {
  const row = screen.getByRole('checkbox', { name: `Include ${organismName}` }).closest('tr');
  expect(row).not.toBeNull();
  return within(row!).getByRole('spinbutton') as HTMLInputElement;
}

async function setWeight(user: UserEvent, organismName: string, weight: number): Promise<void> {
  await setNumberCell(user, weightInput(organismName), weight);
}

/**
 * The editor as the app mounts it: inside the provider that owns the composer
 * session, and behind a toggle, because the sidebar unmounts the editor.
 */
function Sidebar(): JSX.Element {
  const [open, setOpen] = useState(true);
  return (
    <>
      <button type="button" onClick={() => setOpen((value) => !value)}>
        Toggle sidebar
      </button>
      {open ? <ConfigEditor /> : null}
    </>
  );
}

async function renderEditor(): Promise<UserEvent> {
  const user = userEvent.setup();
  render(
    <StrictMode>
      <SimProvider>
        <Sidebar />
      </SimProvider>
    </StrictMode>,
  );
  // The provider fetches the simulation status on mount; flush that promise so
  // its dispatch lands inside `act`.
  await act(async () => {
    await Promise.resolve();
  });
  return user;
}

const DEFAULT_CARD: Card = {
  x: DEFAULT_CONFIG.seed_programs[0]!.x,
  y: DEFAULT_CONFIG.seed_programs[0]!.y,
  scattered: false,
};

beforeEach(() => {
  random.reset();
  window.localStorage.clear();
});

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

describe('population composer live write-through', () => {
  it('places nothing on a fresh mount', async () => {
    await renderEditor();

    // The session owns nothing and has never been touched, so it is not armed:
    // merely opening the editor must not write a population into the config.
    expect(cards()).toEqual([DEFAULT_CARD]);
    expect(screen.queryAllByText(/^scattered/)).toHaveLength(0);
    // The one card is the default hand-placed Quadsorb, unchanged.
    const status = screen.getByText(STATUS_TEXT);
    expect(status.textContent).toBe('hand-placed · 12 bytes');
    expect(within(status.parentElement!).getByText('Quadsorb')).toBeTruthy();
  });

  it('writes the whole mix through as soon as the total changes', async () => {
    const user = await renderEditor();

    await setTotal(user, 6);

    const entries = cards();
    expect(entries).toHaveLength(7);
    // The hand-placed default is untouched and still first.
    expect(entries[0]).toEqual(DEFAULT_CARD);
    expect(entries.slice(1).every((entry) => entry.scattered)).toBe(true);
    expect(new Set(cells(entries)).size).toBe(7);
    for (const entry of entries.slice(1)) {
      expect(entry.x).toBeGreaterThanOrEqual(0);
      expect(entry.x).toBeLessThan(DEFAULT_CONFIG.width);
      expect(entry.y).toBeGreaterThanOrEqual(0);
      expect(entry.y).toBeLessThan(DEFAULT_CONFIG.height);
    }

    // A later total edit must also write through: the first edit armed the
    // session, so this one is carried by the `composer.total` dependency alone.
    await setTotal(user, 4);
    expect(cards()).toHaveLength(5);
  });

  it('re-scatters into the grid when it shrinks', async () => {
    const user = await renderEditor();
    await setTotal(user, 6);
    expect(scatteredCards().some((entry) => entry.x >= 16 || entry.y >= 16)).toBe(true);

    // Width alone first: the regeneration must fire on the `config.width`
    // dependency by itself, not only once height follows.
    await commit(user, screen.getByLabelText(/^Width/), 16);
    expect(scatteredCards().every((entry) => entry.x < 16)).toBe(true);

    await commit(user, screen.getByLabelText(/^Height/), 16);

    const scattered = scatteredCards();
    expect(scattered).toHaveLength(6);
    for (const entry of scattered) {
      expect(entry.x).toBeLessThan(16);
      expect(entry.y).toBeLessThan(16);
    }
    expect(new Set(cells(cards())).size).toBe(7);
  });

  it('re-rolls the layout on New and on Randomize Seed, and reproduces it', async () => {
    const user = await renderEditor();
    await setTotal(user, 6);

    const placementSeedInput = screen.getByLabelText('Placement seed') as HTMLInputElement;
    const firstSeed = Number(placementSeedInput.value);
    const first = cards();

    await user.click(screen.getByRole('button', { name: 'New' }));

    const rerolled = cards();
    expect(Number((screen.getByLabelText('Placement seed') as HTMLInputElement).value)).not.toBe(firstSeed);
    expect(rerolled).toHaveLength(first.length);
    expect(rerolled.filter((entry) => entry.scattered)).toHaveLength(6);
    expect(rerolled[0]).toEqual(DEFAULT_CARD);
    expect(cells(rerolled)).not.toEqual(cells(first));
    expect(new Set(cells(rerolled)).size).toBe(7);

    // Same seeds, same grid, same counts: the layout comes back exactly.
    await setNumberCell(user, screen.getByLabelText('Placement seed'), firstSeed);
    expect(cards()).toEqual(first);

    await user.click(screen.getByRole('button', { name: 'Randomize Seed' }));

    const reseeded = cards();
    expect(reseeded).toHaveLength(7);
    expect(reseeded.filter((entry) => entry.scattered)).toHaveLength(6);
    expect(reseeded[0]).toEqual(DEFAULT_CARD);
    expect(cells(reseeded)).not.toEqual(cells(first));
    expect(new Set(cells(reseeded)).size).toBe(7);
  });

  it('keeps its layout across an editor remount', async () => {
    const user = await renderEditor();
    await setTotal(user, 6);
    const before = cards();

    // Collapsing the sidebar unmounts the editor; the session lives above it.
    await user.click(screen.getByRole('button', { name: 'Toggle sidebar' }));
    expect(screen.queryAllByText(STATUS_TEXT)).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: 'Toggle sidebar' }));

    expect(cards()).toEqual(before);
  });

  it('re-scatters when a generated entry is edited onto another generated cell', async () => {
    const user = await renderEditor();
    await setTotal(user, 6);

    const before = cards();
    const target = before[2]!;
    expect(target.scattered).toBe(true);

    // Index 1 is the first generated entry. Editing it replaces the object, so
    // the composer stops owning it and it becomes hand-placed; the cell it
    // lands on must then be excluded from the scatter.
    await commit(user, screen.getAllByLabelText(/^X/)[1]!, target.x);
    await commit(user, screen.getAllByLabelText(/^Y/)[1]!, target.y);

    const after = cards();
    // The finding this test exists for: nothing may end up sharing a cell.
    expect(new Set(cells(after)).size).toBe(after.length);
    expect(after).toHaveLength(8);
    expect(after.filter((entry) => entry.scattered)).toHaveLength(6);
    expect(after[0]).toEqual(DEFAULT_CARD);
    expect(after[1]).toEqual({ x: target.x, y: target.y, scattered: false });
  });

  it('regenerates correctly after a generated entry is removed', async () => {
    const user = await renderEditor();
    await setTotal(user, 6);

    // Removing an entry only shrinks what the composer owns; the mix and the
    // hand-placed cells are unchanged, so nothing is regenerated yet.
    await user.click(screen.getAllByRole('button', { name: 'Remove' })[1]!);
    expect(cards()).toHaveLength(6);
    expect(scatteredCards()).toHaveLength(5);

    // Re-entering the same weight changes nothing about the mix, so the
    // regeneration it triggers recomputes the very layout the composer
    // remembers. It must still notice that it now owns only five of those six
    // entries and place the sixth again, rather than matching its own stale
    // memory against the new placement and skipping the write.
    await setWeight(user, 'Quadsorb', 1);
    expect(cards()).toHaveLength(7);
    expect(scatteredCards()).toHaveLength(6);

    await setWeight(user, 'Quadsorb', 3);

    const after = cards();
    expect(after).toHaveLength(7);
    expect(after[0]).toEqual(DEFAULT_CARD);
    expect(after.filter((entry) => entry.scattered)).toHaveLength(6);
    expect(new Set(cells(after)).size).toBe(7);
  });

  it('resets the session and places nothing on Load Config', async () => {
    const user = await renderEditor();
    await user.click(screen.getByRole('button', { name: 'Save Config' }));

    await setTotal(user, 6);
    expect(cards()).toHaveLength(7);

    await user.click(screen.getByRole('button', { name: 'Load Config' }));

    expect(screen.getByText('Loaded saved config from this browser.')).toBeTruthy();
    // A loaded config starts a fresh, untouched session: what it holds is the
    // user's, not the composer's, and nothing is scattered over it.
    expect(cards()).toEqual([DEFAULT_CARD]);
    expect(Number((screen.getByLabelText('Total') as HTMLInputElement).value)).toBe(1);

    // Still nothing after a remount, which re-runs the write-through effect.
    await user.click(screen.getByRole('button', { name: 'Toggle sidebar' }));
    await user.click(screen.getByRole('button', { name: 'Toggle sidebar' }));
    expect(cards()).toEqual([DEFAULT_CARD]);
  });

  it('equalizes every weight, including inactive rows', async () => {
    const user = await renderEditor();

    await setWeight(user, 'Quadsorb', 3);
    await setWeight(user, 'Turnchain', 5);
    await user.click(screen.getByRole('checkbox', { name: 'Include Turnchain' }));
    expect(weightInput('Turnchain').disabled).toBe(true);
    expect(weightInput('Turnchain').value).toBe('5');

    await user.click(screen.getByRole('button', { name: 'Equalize' }));

    for (const organism of SEED_LIBRARY) {
      expect(weightInput(organism.name).value).toBe('1');
    }
    expect(weightInput('Turnchain').disabled).toBe(true);
  });
});
