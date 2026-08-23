export function randomSeed(): number {
  return Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
}

/**
 * A random u32, for the composer's placement seed: it feeds a 32-bit PRNG
 * directly, so there is nothing above 2^32 for it to say.
 */
export function randomPlacementSeed(): number {
  return Math.floor(Math.random() * 2 ** 32) >>> 0;
}
