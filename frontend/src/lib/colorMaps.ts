import type { ColorMapMode, GridFrame } from '../types';

function clampByte(value: number): number {
  return Math.max(0, Math.min(255, value));
}

function rgb(r: number, g: number, b: number): number {
  return (clampByte(r) << 16) | (clampByte(g) << 8) | clampByte(b);
}

function hueToRgb(p: number, q: number, t: number): number {
  let hue = t;
  if (hue < 0) hue += 1;
  if (hue > 1) hue -= 1;
  if (hue < 1 / 6) return p + (q - p) * 6 * hue;
  if (hue < 1 / 2) return q;
  if (hue < 2 / 3) return p + (q - p) * (2 / 3 - hue) * 6;
  return p;
}

function hslToRgb(h: number, s: number, l: number): number {
  if (s === 0) {
    const value = Math.round(l * 255);
    return rgb(value, value, value);
  }

  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return rgb(
    Math.round(hueToRgb(p, q, h + 1 / 3) * 255),
    Math.round(hueToRgb(p, q, h) * 255),
    Math.round(hueToRgb(p, q, h - 1 / 3) * 255),
  );
}

function lerpColor(a: [number, number, number], b: [number, number, number], t: number): number {
  return rgb(
    a[0] + (b[0] - a[0]) * t,
    a[1] + (b[1] - a[1]) * t,
    a[2] + (b[2] - a[2]) * t,
  );
}

export function getCellColor(frame: GridFrame, offset: number, mode: ColorMapMode): number {
  const flags = frame.cells.getUint8(offset);
  const hasProgram = (flags & 0b001) !== 0;
  const isLive = (flags & 0b010) !== 0;

  switch (mode) {
    case 'occupancy':
      if (!hasProgram) return rgb(0, 0, 0);
      return isLive ? rgb(245, 247, 250) : rgb(70, 75, 85);
    case 'programId': {
      if (!hasProgram) return rgb(0, 0, 0);
      const hue = ((frame.cells.getUint8(offset + 1) * 137.508) % 360) / 360;
      return hslToRgb(hue, 0.7, 0.5);
    }
    case 'programSize': {
      if (!hasProgram) return rgb(0, 0, 0);
      const value = frame.cells.getUint8(offset + 2) / 255;
      return lerpColor([36, 58, 127], [255, 224, 96], value);
    }
    case 'freeEnergy':
      return lerpColor([0, 0, 0], [0, 235, 130], frame.cells.getUint8(offset + 3) / 255);
    case 'freeMass':
      return lerpColor([0, 0, 0], [72, 150, 255], frame.cells.getUint8(offset + 4) / 255);
    case 'bgRadiation':
      return lerpColor([0, 0, 0], [255, 95, 60], frame.cells.getUint8(offset + 5) / 255);
    case 'bgMass':
      return lerpColor([0, 0, 0], [50, 225, 245], frame.cells.getUint8(offset + 6) / 255);
    case 'combined': {
      const base = rgb(
        frame.cells.getUint8(offset + 5),
        frame.cells.getUint8(offset + 3),
        frame.cells.getUint8(offset + 4),
      );
      if (!hasProgram) {
        return base;
      }

      const outline = isLive ? 32 : 18;
      return rgb(
        Math.min(255, ((base >> 16) & 0xff) + outline),
        Math.min(255, ((base >> 8) & 0xff) + outline),
        Math.min(255, (base & 0xff) + outline),
      );
    }
  }
}
