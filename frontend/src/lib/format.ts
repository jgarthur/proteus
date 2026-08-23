export function formatInteger(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return '—';
  }

  return Math.round(value).toLocaleString();
}

export function formatDecimal(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return '—';
  }

  return value.toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

/**
 * Formats the dyadic probability 2^-k for the config editor preview.
 *
 * Values at or above 1e-4 render as trimmed 6-decimal fixed point; smaller ones
 * switch to a compact exponential (3 significant figures, bare negative
 * exponent) so the preview never widens the config column.
 *
 * Expected strings:
 *   k =  0 -> "1"
 *   k =  1 -> "0.5"
 *   k =  7 -> "0.007813"
 *   k =  9 -> "0.001953"
 *   k = 13 -> "0.000122"
 *   k = 14 -> "6.10e-5"
 *   k = 20 -> "9.54e-7"
 *   k = 63 -> "1.08e-19"
 */
export function formatDyadicProbability(k: number): string {
  if (k === 0) {
    return '1';
  }

  const value = 2 ** -k;
  if (value >= 1e-4) {
    return value.toFixed(6).replace(/\.?0+$/, '');
  }

  const [mantissa, exponent] = value.toExponential(2).split('e');
  const sign = exponent!.startsWith('-') ? '-' : '';
  const magnitude = String(Number(exponent!.replace(/^[+-]/, '')));
  return `${mantissa}e${sign}${magnitude}`;
}

export function directionLabel(dir: number): string {
  switch (dir) {
    case 0:
      return 'right';
    case 1:
      return 'up';
    case 2:
      return 'left';
    case 3:
      return 'down';
    default:
      return 'unknown';
  }
}
