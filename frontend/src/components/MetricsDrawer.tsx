import { useEffect, useRef } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import { useSimContext } from '../context/SimContext';
import type { MetricsBufferSnapshot } from '../types';
import styles from './MetricsDrawer.module.css';

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

  return (
    <section className={state.metricsDrawerOpen ? styles.drawerOpen : styles.drawer}>
      <div className={styles.content}>
        {CHARTS.map((chart) => (
          <MetricsChart
            key={chart.id}
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
  const containerRef = useRef<HTMLDivElement | null>(null);
  const plotRef = useRef<uPlot | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
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
      width: Math.max(280, container.clientWidth),
      height: 180,
      scales: {
        x: { time: false },
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
      },
    };

    if (!plotRef.current) {
      plotRef.current = new uPlot(options, data, container);
    } else {
      plotRef.current.setData(data);
      plotRef.current.setSize({ width: Math.max(280, container.clientWidth), height: 180 });
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

  const hasData = metricsBufferRef.current.snapshot().count > 0;

  return (
    <article className={styles.chartCard}>
      <h2 className={styles.title}>{chart.title}</h2>
      {hasData ? <div ref={containerRef} /> : <div className={styles.empty}>Metrics will appear after the sim runs.</div>}
    </article>
  );
}
