import { useCallback, useEffect, useRef, useState } from 'react';
import { Canvas2DRenderer } from '../lib/renderer';
import { useSimContext } from '../context/SimContext';
import type { ViewportTransform } from '../types';
import styles from './GridCanvas.module.css';

const MIN_SCALE = 0.5;
const MAX_SCALE = 64;
const ZOOM_SENSITIVITY = 1 / 320;

export function GridCanvas(): JSX.Element {
  const { latestFrameRef, selectCell, state } = useSimContext();
  const rendererRef = useRef<Canvas2DRenderer | null>(null);
  const frameTickRef = useRef<number | null>(null);
  const animationRef = useRef<number | null>(null);
  const activePointerIdRef = useRef<number | null>(null);
  const dragStartRef = useRef<{ x: number; y: number } | null>(null);
  const draggedRef = useRef(false);
  const [canvasElement, setCanvasElement] = useState<HTMLCanvasElement | null>(null);
  const [dragging, setDragging] = useState(false);
  const [viewport, setViewport] = useState<ViewportTransform>({ offsetX: 0, offsetY: 0, scale: 1 });
  const [size, setSize] = useState({ width: 400, height: 300 });
  const attachCanvasRef = useCallback((node: HTMLCanvasElement | null) => {
    setCanvasElement(node);
  }, []);
  const resetView = useCallback(() => {
    const renderer = rendererRef.current;
    if (!renderer) {
      return;
    }

    setViewport(renderer.fit(state.gridWidth, state.gridHeight));
    frameTickRef.current = null;
  }, [state.gridHeight, state.gridWidth]);

  useEffect(() => {
    const canvas = canvasElement;
    if (!canvas || rendererRef.current) {
      return;
    }

    const renderer = new Canvas2DRenderer();
    renderer.attach(canvas);
    renderer.resize(size.width, size.height);
    rendererRef.current = renderer;

    return () => {
      renderer.destroy();
      rendererRef.current = null;
      frameTickRef.current = null;
    };
  }, [canvasElement]);

  useEffect(() => {
    const element = canvasElement?.parentElement;
    if (!element) {
      return;
    }

    const syncSize = () => {
      const rect = element.getBoundingClientRect();
      setSize({
        width: Math.max(400, Math.floor(rect.width)),
        height: Math.max(300, Math.floor(rect.height)),
      });
    };

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      const nextWidth = Math.max(400, Math.floor(entry.contentRect.width));
      const nextHeight = Math.max(300, Math.floor(entry.contentRect.height));
      setSize({ width: nextWidth, height: nextHeight });
    });

    syncSize();
    observer.observe(element);
    return () => observer.disconnect();
  }, [canvasElement]);

  useEffect(() => {
    rendererRef.current?.resize(size.width, size.height);
  }, [size]);

  useEffect(() => {
    if (!canvasElement) {
      return;
    }

    const handleWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = canvasElement.getBoundingClientRect();
      setViewport((current) => {
        const zoomFactor = Math.exp(-event.deltaY * ZOOM_SENSITIVITY);
        const nextScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, current.scale * zoomFactor));
        return updateViewportForZoom(current, nextScale, event.clientX, event.clientY, rect);
      });
      frameTickRef.current = null;
    };

    canvasElement.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      canvasElement.removeEventListener('wheel', handleWheel);
    };
  }, [canvasElement]);

  useEffect(() => {
    if (!rendererRef.current || state.gridWidth === 0 || state.gridHeight === 0) {
      return;
    }

    setViewport(rendererRef.current.fit(state.gridWidth, state.gridHeight));
  }, [state.gridHeight, state.gridWidth]);

  useEffect(() => {
    frameTickRef.current = null;
  }, [size.height, size.width, state.colorMap, state.selectedCell, viewport]);

  useEffect(() => {
    const render = () => {
      const renderer = rendererRef.current;
      const frame = latestFrameRef.current;
      if (renderer && frame) {
        const shouldRender = frameTickRef.current !== frame.tick || dragging;
        if (shouldRender) {
          frameTickRef.current = frame.tick;
          renderer.render(frame, viewport, state.colorMap, state.selectedCell);
        }
      }

      animationRef.current = window.requestAnimationFrame(render);
    };

    animationRef.current = window.requestAnimationFrame(render);
    return () => {
      if (animationRef.current !== null) {
        window.cancelAnimationFrame(animationRef.current);
      }
    };
  }, [dragging, latestFrameRef, state.colorMap, state.selectedCell, viewport]);

  const updateViewportForZoom = (
    current: ViewportTransform,
    nextScale: number,
    clientX: number,
    clientY: number,
    rect: DOMRect,
  ): ViewportTransform => {
    const canvasX = clientX - rect.left;
    const canvasY = clientY - rect.top;
    const worldX = (canvasX - current.offsetX) / current.scale;
    const worldY = (canvasY - current.offsetY) / current.scale;
    return {
      scale: nextScale,
      offsetX: canvasX - worldX * nextScale,
      offsetY: canvasY - worldY * nextScale,
    };
  };

  if (state.simStatus === 'none') {
    return (
      <section className={styles.canvasWrap}>
        <div className={styles.emptyState}>
          {state.apiError ? 'Backend unreachable. Start the server, then create a simulation.' : 'Create a simulation to render the grid.'}
        </div>
      </section>
    );
  }

  return (
    <section className={styles.canvasWrap}>
      <canvas
        ref={attachCanvasRef}
        className={`${styles.canvas} ${dragging ? styles.canvasDragging : ''}`}
        onPointerDown={(event) => {
          activePointerIdRef.current = event.pointerId;
          dragStartRef.current = { x: event.clientX, y: event.clientY };
          draggedRef.current = false;
          setDragging(true);
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          if (activePointerIdRef.current !== event.pointerId || !dragStartRef.current) {
            return;
          }

          const dx = event.clientX - dragStartRef.current.x;
          const dy = event.clientY - dragStartRef.current.y;
          if (Math.abs(dx) > 1 || Math.abs(dy) > 1) {
            draggedRef.current = true;
          }

          dragStartRef.current = { x: event.clientX, y: event.clientY };
          setViewport((current) => ({
            ...current,
            offsetX: current.offsetX + dx,
            offsetY: current.offsetY + dy,
          }));
          frameTickRef.current = null;
        }}
        onPointerUp={(event) => {
          const renderer = rendererRef.current;
          if (!renderer || activePointerIdRef.current !== event.pointerId) {
            return;
          }

          if (!draggedRef.current) {
            const rect = event.currentTarget.getBoundingClientRect();
            const hit = renderer.hitTest(event.clientX - rect.left, event.clientY - rect.top);
            if (hit) {
              selectCell(hit);
            }
          }

          event.currentTarget.releasePointerCapture(event.pointerId);
          activePointerIdRef.current = null;
          dragStartRef.current = null;
          setDragging(false);
        }}
        onPointerCancel={(event) => {
          if (activePointerIdRef.current === event.pointerId && event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId);
          }
          activePointerIdRef.current = null;
          dragStartRef.current = null;
          setDragging(false);
        }}
        onPointerLeave={() => {
          if (activePointerIdRef.current !== null) {
            return;
          }
          dragStartRef.current = null;
          setDragging(false);
        }}
        onDoubleClick={() => {
          resetView();
        }}
      />
      <div className={styles.hud}>
        <span className={styles.chip}>
          {state.gridWidth} × {state.gridHeight}
        </span>
        <span className={styles.chip}>Zoom {viewport.scale.toFixed(2)}×</span>
        <button className={styles.buttonChip} type="button" onClick={resetView}>
          Reset View
        </button>
        {state.selectedCell ? (
          <span className={styles.chip}>
            Cell ({state.selectedCell.x}, {state.selectedCell.y})
          </span>
        ) : null}
      </div>
    </section>
  );
}
