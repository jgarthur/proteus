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
