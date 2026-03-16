import type { GridFrame } from '../types';

export function parseFrame(buffer: ArrayBuffer): GridFrame {
  const view = new DataView(buffer);
  const tick = Number(view.getBigUint64(0, true));
  const width = view.getUint32(8, true);
  const height = view.getUint32(12, true);
  const cells = new DataView(buffer, 16);
  return { tick, width, height, cells };
}

export function cellOffset(width: number, x: number, y: number): number {
  return (y * width + x) * 8;
}
