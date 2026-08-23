/**
 * Built-in seed organisms offered by the config editor's population composer.
 *
 * `code` is stored as raw bytes — the same thing `SimConfig.seed_programs`
 * carries — and the human-readable listing is derived with `disassemble` so
 * there is exactly one source of truth for each genome.
 *
 * The seven verified organisms come from the seed-organism ladder in
 * `docs/analysis/2026-08-23_seed-organisms.md`, which derived each one by hand
 * (budget, then Pass trace), ran it, and reported a five-seed horizon result at
 * tick 20,000. Their `free_energy` / `free_mass` are the root preloads those
 * runs actually used, so picking one and scattering it reproduces the tested
 * starting condition rather than an invented one.
 */

/**
 * Structural grouping used by the composer.
 *
 * The three ladder families come from the analysis; `Classic` holds the
 * hand-designed and evolved organisms that predate it and carry no verified
 * regime.
 */
export type SeedFamily = 'Classic' | 'Chain' | 'Synthesis' | 'Mobile';

/** Family display order in the composer and the library picker. */
export const SEED_FAMILIES: readonly SeedFamily[] = ['Classic', 'Chain', 'Synthesis', 'Mobile'];

export interface SeedOrganism {
  id: string;
  name: string;
  description: string;
  provenance: string;
  family: SeedFamily;
  /** How well this genome is evidenced — verification result or its absence. */
  evidence: string;
  /** The parameter regime it was verified in, or why it has none. */
  regime: string;
  code: number[];
  free_energy: number;
  free_mass: number;
}

/**
 * The frontend's long-standing default organism: four `absorb`s up front to
 * saturate the `absorb_count` cap of 4 (a 5-cell harvest footprint), then the
 * spec replicator's copy loop.
 */
export const DEFAULT_LITHOTROPH_CODE = [0x51, 0x51, 0x51, 0x51, 0x53, 0x40, 0x42, 0x30, 0x55, 0x5f, 0x31, 0x64];

/** `docs/SPEC.md` "Seed Replicator", also the `code` array in API-SPEC §Cell. */
const TURNCHAIN_CODE = [81, 83, 64, 0, 74, 66, 48, 85, 95, 49, 100, 80];

/**
 * The 39-instruction evolved genome listed in
 * `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`,
 * transcribed in index order 0..38 from that document's three-column table.
 */
const DRIFTKIN_CODE = [
  81, 64, 0, 66, 83, 48, 0, 48, 81, 85, 0, 93, 81, 49, 64, 49, 81, 100, 81, 99, 81, 99, 81, 81, 81, 81, 81, 81, 49,
  64, 85, 66, 95, 48, 66, 85, 48, 95, 81,
];

const LADDER_DOC = 'docs/analysis/2026-08-23_seed-organisms.md';

export const SEED_LIBRARY: readonly SeedOrganism[] = [
  {
    id: 'spec-replicator',
    name: 'Turnchain',
    description:
      'The twelve-byte hand-designed replicator from the spec: absorb, collect, rotate a quarter turn, then copy the whole body into the neighbour and boot it. One absorb, so it harvests its own cell only.',
    provenance: 'docs/SPEC.md "Seed Replicator"',
    family: 'Classic',
    evidence: 'Hand-designed reference organism; never run to a horizon.',
    regime: 'No verified regime; shipped with the spec default preload of 20E/12M.',
    code: TURNCHAIN_CODE,
    free_energy: 20,
    free_mass: 12,
  },
  {
    id: 'lithotroph-4x',
    name: 'Quadsorb',
    description:
      'Turnchain with its register setup traded for four leading absorbs, which saturates the absorb_count cap and buys the full five-cell harvest footprint. This is the frontend default seed.',
    provenance: 'frontend default; saturates the absorb_count cap of 4',
    family: 'Classic',
    evidence: 'Frontend default; the genome behind the 2026-08-13 emergence sweeps and the 2026-08-21 ambient-rebalance sweep (takeoff in most seeds at the defaults).',
    regime: 'Spec defaults (r_energy=0.25, d*=7, mutation 16/8); see the 2026-08-21 sweep for the rate response.',
    code: DEFAULT_LITHOTROPH_CODE,
    free_energy: 20,
    free_mass: 12,
  },
  {
    id: 'seed1-glider',
    name: 'Driftkin',
    description:
      'Evolved, not hand-designed: a thirty-nine-byte genome observed running around the grid and shedding short-lived offspring. Roughly a third of it is absorb, and the two move instructions are what make it graze rather than sit in its own depletion halo — removing them makes the behaviour much less interesting.',
    provenance: 'docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md, "The seed-1 program"',
    family: 'Classic',
    evidence: 'Observed in a run, not verified against a criterion. Its copy loop is misaligned.',
    regime: 'No verified regime; observed at the defaults of that analysis.',
    code: DRIFTKIN_CODE,
    free_energy: 20,
    free_mass: 12,
  },

  {
    id: 'radchain',
    name: 'Radchain',
    description:
      'A seven-byte stationary chain replicator that collects ambient mass and pays copy energy straight from background radiation.',
    provenance: `${LADDER_DOC}, rung 1`,
    family: 'Chain',
    evidence: 'Verified 3/5 seeds; exact genome retained.',
    regime: 'At d*=7, solitary mean rE>=.802, rM>=.809; verified at 2/1.25 with cap=4, maintenance_log2=10, alpha=.60.',
    code: [83, 66, 48, 85, 95, 49, 100],
    free_energy: 0,
    free_mass: 0,
  },
  {
    id: 'sunchain',
    name: 'Sunchain',
    description: 'An eight-byte Radchain that absorbs its own-cell radiation into an energy buffer before copying.',
    provenance: `${LADDER_DOC}, rung 1 captured-energy counterpart`,
    family: 'Chain',
    evidence: 'Verified 3/5 seeds; exact genome retained.',
    regime:
      'At d*=7, conservative rE>=.908, rM>=.899; verified at 1.25/1.25 with a 16E/16M root preload, cap=4, maintenance_log2=10, alpha=.55.',
    code: [81, 83, 66, 48, 85, 95, 49, 100],
    free_energy: 16,
    free_mass: 16,
  },

  {
    id: 'freesynth',
    name: 'Freesynth',
    description:
      'A seven-byte copier for massless worlds that synthesizes each offspring byte just in time with zero synthesis surcharge.',
    provenance: `${LADDER_DOC}, rung 2`,
    family: 'Synthesis',
    evidence: 'Verified 3/5 seeds; requires disabled maintenance in this minimal form.',
    regime: 'At d*=7, mean rE>=1.605 with rM=0, n_synth=0 and maintenance null; verified at rE=2, cap=4, alpha=.75.',
    code: [66, 48, 88, 85, 95, 49, 100],
    free_energy: 0,
    free_mass: 0,
  },
  {
    id: 'frontforge',
    name: 'Frontforge',
    description:
      'A nine-byte zero-mass builder that absorbs its own and front cells, then converts captured energy into each offspring byte.',
    provenance: `${LADDER_DOC}, rung 2 captured-energy counterpart`,
    family: 'Synthesis',
    evidence: 'Verified 3/5 seeds; genome-unstable long-term.',
    regime:
      'At d*=7, conservative rE>=1.401 with rM=0; verified at 2/0 with cap=4, maintenance_log2=10, alpha=.65, n_synth=1.',
    code: [81, 81, 66, 48, 88, 85, 95, 49, 100],
    free_energy: 0,
    free_mass: 0,
  },
  {
    id: 'mixedforge',
    name: 'Mixedforge',
    description:
      'A ten-byte Frontforge that also collects ambient mass into a surplus buffer when both mass pathways exist.',
    provenance: `${LADDER_DOC}, rung 2 captured-energy counterpart`,
    family: 'Synthesis',
    evidence: 'Verified 4/5 seeds — the best of the stationary forms; genome-unstable long-term.',
    regime: 'At d*=7, conservative rE>=1.445 with rM=0; verified at 2/0 with a 32E root preload, otherwise Frontforge regime.',
    code: [81, 81, 83, 66, 48, 88, 85, 95, 49, 100],
    free_energy: 32,
    free_mass: 0,
  },

  {
    id: 'squarestep',
    name: 'Squarestep',
    description:
      'A nine-byte mobile collector that births forward, turns clockwise, and forages a repeatable four-cell orbit.',
    provenance: `${LADDER_DOC}, rung 3`,
    family: 'Mobile',
    evidence: 'Verified 5/5 seeds as a colonizer — it fills a 64x64 grid; genome-unstable when dense.',
    regime:
      'At d*=7, established-orbit means rE>=.249, rM>=.224; verified at .5/.3 with cap=4, maintenance_log2=14, alpha=.55.',
    code: [83, 66, 48, 85, 95, 49, 100, 64, 99],
    free_energy: 0,
    free_mass: 0,
  },
  {
    id: 'wildstep',
    name: 'Wildstep',
    description:
      'An eleven-byte absorber/collector that chooses a random heading after each birth to colonize fresh cells.',
    provenance: `${LADDER_DOC}, rung 3 captured-energy analogue`,
    family: 'Mobile',
    evidence: 'Verified 5/5 seeds as a colonizer; genome-unstable when dense.',
    regime:
      'At d*=7, conservative fresh-cell rE>=.105, rM>=.0965; verified at .25/.2 with a 16E/16M root preload, cap=4, maintenance_log2=14, alpha=.5.',
    code: [81, 83, 66, 48, 85, 95, 49, 100, 20, 73, 99],
    free_energy: 16,
    free_mass: 16,
  },
];

/** Finds the library organism whose bytes match `code` exactly, if any. */
export function findLibraryOrganismByCode(code: readonly number[]): SeedOrganism | null {
  return (
    SEED_LIBRARY.find(
      (organism) =>
        organism.code.length === code.length && organism.code.every((byte, index) => byte === code[index]),
    ) ?? null
  );
}

/** Groups the library by family, in `SEED_FAMILIES` order, skipping empties. */
export function seedLibraryByFamily(
  library: readonly SeedOrganism[] = SEED_LIBRARY,
): Array<{ family: SeedFamily; organisms: SeedOrganism[] }> {
  return SEED_FAMILIES.map((family) => ({
    family,
    organisms: library.filter((organism) => organism.family === family),
  })).filter((group) => group.organisms.length > 0);
}
