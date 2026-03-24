import { getCellColor } from './colorMaps';
import { cellOffset } from './frame';
import type { ColorMapMode, GridFrame, GridRenderer, ViewportTransform } from '../types';

const FIT_VERTICAL_PADDING_MAX = 16;
const FIT_VERTICAL_PADDING_RATIO = 0.025;

export class Canvas2DRenderer implements GridRenderer {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private offscreen: HTMLCanvasElement;
  private offscreenCtx: CanvasRenderingContext2D;
  private imageData: ImageData | null = null;
  private gridWidth = 0;
  private gridHeight = 0;
  private canvasWidth = 1;
  private canvasHeight = 1;
  private lastViewport: ViewportTransform = { offsetX: 0, offsetY: 0, scale: 1 };

  constructor() {
    this.offscreen = document.createElement('canvas');
    const context = this.offscreen.getContext('2d');
    if (!context) {
      throw new Error('Canvas 2D is not available');
    }

    this.offscreenCtx = context;
  }

  attach(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    if (!this.ctx) {
      throw new Error('Failed to acquire 2D context');
    }
  }

  resize(width: number, height: number): void {
    if (!this.canvas) {
      return;
    }

    this.canvasWidth = Math.max(1, width);
    this.canvasHeight = Math.max(1, height);
    this.canvas.width = this.canvasWidth;
    this.canvas.height = this.canvasHeight;
  }

  fit(gridWidth: number, gridHeight: number): ViewportTransform {
    const verticalPadding = Math.min(FIT_VERTICAL_PADDING_MAX, this.canvasHeight * FIT_VERTICAL_PADDING_RATIO);
    const scale = Math.max(
      0.5,
      Math.min(
        64,
        Math.min(
          this.canvasWidth / Math.max(1, gridWidth),
          Math.max(1, this.canvasHeight - verticalPadding * 2) / Math.max(1, gridHeight),
        ),
      ),
    );
    return {
      scale,
      offsetX: (this.canvasWidth - gridWidth * scale) / 2,
      offsetY: (this.canvasHeight - gridHeight * scale) / 2,
    };
  }

  render(
    frame: GridFrame,
    viewport: ViewportTransform,
    colorMode: ColorMapMode,
    selectedCell: { x: number; y: number } | null,
  ): void {
    if (!this.ctx) {
      return;
    }

    this.lastViewport = viewport;
    this.ensureFrameSurface(frame.width, frame.height);

    if (!this.imageData) {
      return;
    }

    const pixels = this.imageData.data;
    for (let y = 0; y < frame.height; y += 1) {
      for (let x = 0; x < frame.width; x += 1) {
        const sourceOffset = cellOffset(frame.width, x, y);
        const targetOffset = (y * frame.width + x) * 4;
        const color = getCellColor(frame, sourceOffset, colorMode);
        pixels[targetOffset] = (color >> 16) & 0xff;
        pixels[targetOffset + 1] = (color >> 8) & 0xff;
        pixels[targetOffset + 2] = color & 0xff;
        pixels[targetOffset + 3] = 255;
      }
    }

    this.offscreenCtx.putImageData(this.imageData, 0, 0);

    this.ctx.save();
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);
    this.ctx.clearRect(0, 0, this.canvasWidth, this.canvasHeight);
    this.ctx.fillStyle = '#081018';
    this.ctx.fillRect(0, 0, this.canvasWidth, this.canvasHeight);
    this.ctx.imageSmoothingEnabled = false;
    this.ctx.translate(viewport.offsetX, viewport.offsetY);
    this.ctx.scale(viewport.scale, viewport.scale);
    this.ctx.drawImage(this.offscreen, 0, 0);

    if (selectedCell) {
      this.ctx.lineWidth = 1 / viewport.scale;
      this.ctx.strokeStyle = '#f5b942';
      this.ctx.strokeRect(selectedCell.x, selectedCell.y, 1, 1);
    }

    this.ctx.restore();
  }

  hitTest(canvasX: number, canvasY: number): { x: number; y: number } | null {
    if (this.gridWidth === 0 || this.gridHeight === 0) {
      return null;
    }

    const gridX = Math.floor((canvasX - this.lastViewport.offsetX) / this.lastViewport.scale);
    const gridY = Math.floor((canvasY - this.lastViewport.offsetY) / this.lastViewport.scale);

    if (gridX < 0 || gridY < 0 || gridX >= this.gridWidth || gridY >= this.gridHeight) {
      return null;
    }

    return { x: gridX, y: gridY };
  }

  destroy(): void {
    this.canvas = null;
    this.ctx = null;
    this.imageData = null;
    this.offscreen.width = 1;
    this.offscreen.height = 1;
  }

  private ensureFrameSurface(width: number, height: number): void {
    if (this.gridWidth === width && this.gridHeight === height && this.imageData) {
      return;
    }

    this.gridWidth = width;
    this.gridHeight = height;
    this.offscreen.width = width;
    this.offscreen.height = height;
    this.imageData = this.offscreenCtx.createImageData(width, height);
  }
}
