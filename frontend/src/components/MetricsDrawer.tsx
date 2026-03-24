import { useEffect, useRef, useState } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import { useSimContext } from '../context/SimContext';
import type { MetricsBufferSnapshot } from '../types';
import styles from './MetricsDrawer.module.css';

const CHART_HEIGHT = 180;
const ZERO_BASED_RANGE: uPlot.Range.MinMax = [0, null];
const MIN_DRAWER_HEIGHT = 240;
const DEFAULT_DRAWER_HEIGHT = 360;
const MAX_DRAWER_HEIGHT_RATIO = 0.78;

function clampDrawerHeight(height: number): number {
  if (typeof window === 'undefined') {
    return Math.max(MIN_DRAWER_HEIGHT, Math.round(height));
  }

  const maxHeight = Math.max(MIN_DRAWER_HEIGHT, Math.floor(window.innerHeight * MAX_DRAWER_HEIGHT_RATIO));
  return Math.min(Math.max(MIN_DRAWER_HEIGHT, Math.round(height)), maxHeight);
}

interface ChartDef {
  id: string;
  title: string;
  series: Array<{
    key: keyof Omit<MetricsBufferSnapshot, 'count'>;
    label: string;
    color: string;
    axis?: 'left' | 'right';
  }>;
}

const CHARTS: ChartDef[] = [
  {
    id: 'population',
    title: 'Population',
    series: [
      { key: 'live_count', label: 'Live', color: '#6ce3b5' },
      { key: 'inert_count', label: 'Inert', color: '#9aa6b5' },
      { key: 'population', label: 'Population', color: '#f7c75d' },
    ],
  },
  {
    id: 'energy-mass',
    title: 'Energy & Mass',
    series: [
      { key: 'total_energy', label: 'Energy', color: '#66df9c', axis: 'left' },
      { key: 'total_mass', label: 'Mass', color: '#5fb8ff', axis: 'right' },
    ],
  },
  {
    id: 'birth-death',
    title: 'Births / Deaths / Mutations',
    series: [
      { key: 'births', label: 'Births', color: '#f7c75d' },
      { key: 'deaths', label: 'Deaths', color: '#ff8a7a' },
      { key: 'mutations', label: 'Mutations', color: '#d28cff' },
    ],
  },
  {
    id: 'program-size',
    title: 'Program Size',
    series: [
      { key: 'mean_program_size', label: 'Mean', color: '#8bd1ff', axis: 'left' },
      { key: 'max_program_size', label: 'Max', color: '#ffce74', axis: 'right' },
    ],
  },
  {
    id: 'diversity',
    title: 'Diversity',
    series: [{ key: 'unique_genomes', label: 'Unique Genomes', color: '#87f2dd' }],
  },
];

export function MetricsDrawer(): JSX.Element {
  const { metricsBufferRef, metricsVersion, state } = useSimContext();
  const chartSession = state.simStatus === 'none' ? 'empty' : 'active';
  const [drawerHeight, setDrawerHeight] = useState(() =>
    typeof window === 'undefined' ? DEFAULT_DRAWER_HEIGHT : clampDrawerHeight(window.innerHeight * 0.42),
  );
  const [resizing, setResizing] = useState(false);
  const resizeRef = useRef<{ startY: number; startHeight: number } | null>(null);

  useEffect(() => {
    const handleResize = () => {
      setDrawerHeight((current) => clampDrawerHeight(current));
    };

    window.addEventListener('resize', handleResize);
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, []);

  useEffect(() => {
    const stopResize = () => {
      if (!resizeRef.current) {
        return;
      }

      resizeRef.current = null;
      setResizing(false);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };

    const handlePointerMove = (event: PointerEvent) => {
      const resizeState = resizeRef.current;
      if (!resizeState) {
        return;
      }

      event.preventDefault();
      setDrawerHeight(clampDrawerHeight(resizeState.startHeight + (resizeState.startY - event.clientY)));
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);

    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', stopResize);
      window.removeEventListener('pointercancel', stopResize);
      stopResize();
    };
  }, []);

  return (
    <section
      className={`${state.metricsDrawerOpen ? styles.drawerOpen : styles.drawer} ${resizing ? styles.resizing : ''}`}
      style={state.metricsDrawerOpen ? { height: drawerHeight } : undefined}
    >
      <div
        className={styles.resizeHandle}
        role="separator"
        aria-label="Resize metrics drawer"
        aria-orientation="horizontal"
        onPointerDown={(event) => {
          if (!state.metricsDrawerOpen) {
            return;
          }

          resizeRef.current = { startY: event.clientY, startHeight: drawerHeight };
          setResizing(true);
          document.body.style.cursor = 'ns-resize';
          document.body.style.userSelect = 'none';
          event.preventDefault();
        }}
      />
      <div className={styles.content}>
        {CHARTS.map((chart) => (
          <MetricsChart
            key={`${chartSession}-${chart.id}`}
            chart={chart}
            metricsBufferRef={metricsBufferRef}
            metricsVersion={metricsVersion}
          />
        ))}
      </div>
    </section>
  );
}

function MetricsChart({
  chart,
  metricsBufferRef,
  metricsVersion,
}: {
  chart: ChartDef;
  metricsBufferRef: React.MutableRefObject<{ snapshot(): MetricsBufferSnapshot }>;
  metricsVersion: number;
}): JSX.Element {
  const plotHostRef = useRef<HTMLDivElement | null>(null);
  const legendHostRef = useRef<HTMLDivElement | null>(null);
  const plotRef = useRef<uPlot | null>(null);
  const hasData = metricsVersion > 0;

  useEffect(() => {
    const plotHost = plotHostRef.current;
    if (!hasData || !plotHost) {
      return;
    }

    const resizePlot = () => {
      if (!plotRef.current) {
        return;
      }

      plotRef.current.setSize({
        width: Math.max(1, Math.floor(plotHost.clientWidth)),
        height: CHART_HEIGHT,
      });
    };

    resizePlot();
    const observer = new ResizeObserver(() => {
      resizePlot();
    });
    observer.observe(plotHost);

    return () => {
      observer.disconnect();
    };
  }, [hasData]);

  useEffect(() => {
    const plotHost = plotHostRef.current;
    const legendHost = legendHostRef.current;
    if (!plotHost || !legendHost) {
      return;
    }

    const snapshot = metricsBufferRef.current.snapshot();
    if (snapshot.count === 0) {
      plotRef.current?.destroy();
      plotRef.current = null;
      return;
    }

    const data = [snapshot.tick, ...chart.series.map((series) => snapshot[series.key])] as uPlot.AlignedData;
    const options: uPlot.Options = {
      width: Math.max(1, Math.floor(plotHost.clientWidth)),
      height: CHART_HEIGHT,
      scales: {
        x: { time: false },
        y: { range: ZERO_BASED_RANGE },
      },
      axes: [
        { stroke: '#90a5bb', grid: { stroke: 'rgba(157, 176, 196, 0.14)' } },
        { stroke: '#90a5bb', grid: { stroke: 'rgba(157, 176, 196, 0.08)' } },
      ],
      series: [
        { label: 'Tick' },
        ...chart.series.map((series) => ({
          label: series.label,
          stroke: series.color,
          width: 2,
        })),
      ],
      legend: {
        show: true,
        mount: (_self: uPlot, legendTable: HTMLElement) => {
          if (legendTable.parentElement !== legendHost) {
            legendHost.replaceChildren(legendTable);
          }
        },
      },
    };

    if (!plotRef.current) {
      plotRef.current = new uPlot(options, data, plotHost);
    } else {
      plotRef.current.setData(data);
    }

    return () => {
      // Keep the instance for incremental updates until component unmount.
    };
  }, [chart, metricsBufferRef, metricsVersion]);

  useEffect(() => {
    return () => {
      plotRef.current?.destroy();
      plotRef.current = null;
    };
  }, []);

  return (
    <article className={styles.chartCard}>
      <h2 className={styles.title}>{chart.title}</h2>
      {hasData ? (
        <div className={styles.chartBody}>
          <div ref={legendHostRef} className={styles.legendRail} />
          <div ref={plotHostRef} className={styles.plotSurface} style={{ height: CHART_HEIGHT }} />
        </div>
      ) : (
        <div className={styles.empty}>Metrics will appear after the sim runs.</div>
      )}
    </article>
  );
}
