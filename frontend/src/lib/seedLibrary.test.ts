import { describe, expect, it } from 'vitest';
import { disassemble } from './opcodes';
import {
  DEFAULT_LITHOTROPH_CODE,
  findLibraryOrganismByCode,
  SEED_FAMILIES,
  SEED_LIBRARY,
  seedLibraryByFamily,
} from './seedLibrary';

/** The `disassembly` array shown for this genome in `docs/API-SPEC.md`. */
const API_SPEC_REPLICATOR_DISASSEMBLY = [
  'absorb',
  'collect',
  'cw',
  'push 0',
  'setSrc',
  'getSize',
  'for',
  'read',
  'appendAdj',
  'next',
  'boot',
  'nop',
];

describe('SEED_LIBRARY', () => {
  it('has the documented entries in family order with unique ids', () => {
    expect(SEED_LIBRARY.map((organism) => organism.id)).toEqual([
      'spec-replicator',
      'lithotroph-4x',
      'seed1-glider',
      'radchain',
      'sunchain',
      'freesynth',
      'frontforge',
      'mixedforge',
      'squarestep',
      'wildstep',
    ]);
    expect(new Set(SEED_LIBRARY.map((organism) => organism.id)).size).toBe(SEED_LIBRARY.length);
    expect(new Set(SEED_LIBRARY.map((organism) => organism.name)).size).toBe(SEED_LIBRARY.length);
  });

  it('renames the three originals without changing their genomes or ids', () => {
    const byId = new Map(SEED_LIBRARY.map((organism) => [organism.id, organism]));
    expect(byId.get('spec-replicator')!.name).toBe('Turnchain');
    expect(byId.get('lithotroph-4x')!.name).toBe('Quadsorb');
    expect(byId.get('seed1-glider')!.name).toBe('Driftkin');
    expect(byId.get('spec-replicator')!.code).toHaveLength(12);
    expect(byId.get('lithotroph-4x')!.code).toEqual(DEFAULT_LITHOTROPH_CODE);
    expect(byId.get('seed1-glider')!.code).toHaveLength(39);
  });

  it('carries the ladder genomes exactly as the analysis lists them', () => {
    const byId = new Map(SEED_LIBRARY.map((organism) => [organism.id, organism.code]));
    expect(byId.get('radchain')).toEqual([83, 66, 48, 85, 95, 49, 100]);
    expect(byId.get('sunchain')).toEqual([81, 83, 66, 48, 85, 95, 49, 100]);
    expect(byId.get('freesynth')).toEqual([66, 48, 88, 85, 95, 49, 100]);
    expect(byId.get('frontforge')).toEqual([81, 81, 66, 48, 88, 85, 95, 49, 100]);
    expect(byId.get('mixedforge')).toEqual([81, 81, 83, 66, 48, 88, 85, 95, 49, 100]);
    expect(byId.get('squarestep')).toEqual([83, 66, 48, 85, 95, 49, 100, 64, 99]);
    expect(byId.get('wildstep')).toEqual([81, 83, 66, 48, 85, 95, 49, 100, 20, 73, 99]);
  });

  it('matches the sizes the analysis summary table quotes', () => {
    const sizes = new Map(SEED_LIBRARY.map((organism) => [organism.id, organism.code.length]));
    expect(sizes.get('radchain')).toBe(7);
    expect(sizes.get('sunchain')).toBe(8);
    expect(sizes.get('freesynth')).toBe(7);
    expect(sizes.get('frontforge')).toBe(9);
    expect(sizes.get('mixedforge')).toBe(10);
    expect(sizes.get('squarestep')).toBe(9);
    expect(sizes.get('wildstep')).toBe(11);
  });

  it('preloads each ladder organism with the root resources its run used', () => {
    const byId = new Map(SEED_LIBRARY.map((organism) => [organism.id, organism]));
    // Primary rows and Frontforge start the root at 0E/0M.
    ['radchain', 'freesynth', 'frontforge', 'squarestep'].forEach((id) => {
      expect([byId.get(id)!.free_energy, byId.get(id)!.free_mass]).toEqual([0, 0]);
    });
    expect([byId.get('sunchain')!.free_energy, byId.get('sunchain')!.free_mass]).toEqual([16, 16]);
    expect([byId.get('mixedforge')!.free_energy, byId.get('mixedforge')!.free_mass]).toEqual([32, 0]);
    expect([byId.get('wildstep')!.free_energy, byId.get('wildstep')!.free_mass]).toEqual([16, 16]);
  });

  it('gives every entry a family, evidence note, and regime note', () => {
    SEED_LIBRARY.forEach((organism) => {
      expect(SEED_FAMILIES).toContain(organism.family);
      expect(organism.evidence.length).toBeGreaterThan(0);
      expect(organism.regime.length).toBeGreaterThan(0);
    });
  });

  it('assigns the ladder families the analysis uses', () => {
    const families = new Map(SEED_LIBRARY.map((organism) => [organism.id, organism.family]));
    expect(families.get('radchain')).toBe('Chain');
    expect(families.get('sunchain')).toBe('Chain');
    expect(families.get('freesynth')).toBe('Synthesis');
    expect(families.get('frontforge')).toBe('Synthesis');
    expect(families.get('mixedforge')).toBe('Synthesis');
    expect(families.get('squarestep')).toBe('Mobile');
    expect(families.get('wildstep')).toBe('Mobile');
    ['spec-replicator', 'lithotroph-4x', 'seed1-glider'].forEach((id) => {
      expect(families.get(id)).toBe('Classic');
    });
  });

  it('carries valid bytes and non-empty prose for every entry', () => {
    SEED_LIBRARY.forEach((organism) => {
      expect(organism.code.length).toBeGreaterThan(0);
      organism.code.forEach((byte) => {
        expect(Number.isInteger(byte)).toBe(true);
        expect(byte).toBeGreaterThanOrEqual(0);
        expect(byte).toBeLessThanOrEqual(255);
      });
      expect(organism.name.length).toBeGreaterThan(0);
      expect(organism.description.length).toBeGreaterThan(0);
      expect(organism.provenance.length).toBeGreaterThan(0);
      expect(organism.free_energy).toBeGreaterThanOrEqual(0);
      expect(organism.free_mass).toBeGreaterThanOrEqual(0);
    });
  });

  it('contains no undefined bytes: nothing disassembles to a noop', () => {
    SEED_LIBRARY.forEach((organism) => {
      disassemble(organism.code).forEach((instruction) => {
        expect(instruction).not.toMatch(/^noop /);
      });
    });
  });

  it('disassembles the spec replicator to exactly the API-SPEC listing', () => {
    const replicator = SEED_LIBRARY.find((organism) => organism.id === 'spec-replicator')!;
    expect(replicator.code).toEqual([81, 83, 64, 0, 74, 66, 48, 85, 95, 49, 100, 80]);
    expect(disassemble(replicator.code)).toEqual(API_SPEC_REPLICATOR_DISASSEMBLY);
  });

  it('gives Quadsorb four leading absorbs', () => {
    const quadsorb = SEED_LIBRARY.find((organism) => organism.id === 'lithotroph-4x')!;
    expect(disassemble(quadsorb.code).slice(0, 4)).toEqual(['absorb', 'absorb', 'absorb', 'absorb']);
  });

  it('keeps Driftkin at 39 instructions with two moves', () => {
    const glider = SEED_LIBRARY.find((organism) => organism.id === 'seed1-glider')!;
    const listing = disassemble(glider.code);
    expect(listing).toHaveLength(39);
    expect(listing.filter((instruction) => instruction === 'move')).toHaveLength(2);
    expect(listing[17]).toBe('boot');
    expect(listing.slice(30, 33)).toEqual(['read', 'getSize', 'appendAdj']);
  });
});

describe('findLibraryOrganismByCode', () => {
  it('matches library genomes exactly', () => {
    SEED_LIBRARY.forEach((organism) => {
      expect(findLibraryOrganismByCode([...organism.code])?.id).toBe(organism.id);
    });
  });

  it('returns null for a genome that is not in the library', () => {
    const replicator = SEED_LIBRARY[0]!;
    expect(findLibraryOrganismByCode([])).toBeNull();
    expect(findLibraryOrganismByCode(replicator.code.slice(0, -1))).toBeNull();
    expect(findLibraryOrganismByCode([...replicator.code, 0x50])).toBeNull();
  });
});

describe('seedLibraryByFamily', () => {
  it('groups in SEED_FAMILIES order and loses nobody', () => {
    const groups = seedLibraryByFamily();
    expect(groups.map((group) => group.family)).toEqual(['Classic', 'Chain', 'Synthesis', 'Mobile']);
    expect(groups.flatMap((group) => group.organisms)).toEqual([...SEED_LIBRARY]);
  });

  it('counts each family', () => {
    const sizes = Object.fromEntries(
      seedLibraryByFamily().map((group) => [group.family, group.organisms.length]),
    );
    expect(sizes).toEqual({ Classic: 3, Chain: 2, Synthesis: 3, Mobile: 2 });
  });

  it('skips families with no members', () => {
    const groups = seedLibraryByFamily(SEED_LIBRARY.filter((organism) => organism.family === 'Mobile'));
    expect(groups.map((group) => group.family)).toEqual(['Mobile']);
  });
});
