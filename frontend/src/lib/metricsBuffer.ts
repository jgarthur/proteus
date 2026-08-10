import { METRICS_CAPACITY } from '../constants';
import type { EventTotals, MetricsBufferSnapshot, MetricsSnapshot } from '../types';

type BufferKey = keyof Omit<MetricsBufferSnapshot, 'count'>;

export class MetricsBuffer {
  private readonly capacity = METRICS_CAPACITY;
  private count = 0;
  private eventBaseline: { epoch: number; tick: number; totals: EventTotals } | null = null;
  private readonly data: Record<BufferKey, Float64Array> = {
    epoch: new Float64Array(this.capacity),
    tick: new Float64Array(this.capacity),
    population: new Float64Array(this.capacity),
    live_count: new Float64Array(this.capacity),
    inert_count: new Float64Array(this.capacity),
    total_energy: new Float64Array(this.capacity),
    total_mass: new Float64Array(this.capacity),
    births: new Float64Array(this.capacity),
    deaths: new Float64Array(this.capacity),
    mutations: new Float64Array(this.capacity),
    mean_program_size: new Float64Array(this.capacity),
    max_program_size: new Float64Array(this.capacity),
    unique_genomes: new Float64Array(this.capacity),
  };

  push(message: MetricsSnapshot): void {
    const previous = this.eventBaseline;
    const isSameSample =
      previous !== null && message.epoch === previous.epoch && message.tick === previous.tick;
    const totalsRegressed =
      previous !== null &&
      (message.event_totals.births < previous.totals.births ||
        message.event_totals.boot_births < previous.totals.boot_births ||
        message.event_totals.spawn_births < previous.totals.spawn_births ||
        message.event_totals.deaths < previous.totals.deaths ||
        message.event_totals.mutations < previous.totals.mutations);
    const observationRegressed =
      previous !== null &&
      (message.epoch !== previous.epoch || message.tick < previous.tick || totalsRegressed);

    if (observationRegressed) {
      this.clear();
    }

    const baseline = this.eventBaseline;
    const elapsedTicks =
      baseline !== null && message.epoch === baseline.epoch && message.tick > baseline.tick
        ? message.tick - baseline.tick
        : 0;
    const lastIndex = this.count - 1;
    const birthsPerTick =
      elapsedTicks > 0
        ? (message.event_totals.births - baseline!.totals.births) / elapsedTicks
        : isSameSample && lastIndex >= 0
          ? this.data.births[lastIndex]
          : 0;
    const deathsPerTick =
      elapsedTicks > 0
        ? (message.event_totals.deaths - baseline!.totals.deaths) / elapsedTicks
        : isSameSample && lastIndex >= 0
          ? this.data.deaths[lastIndex]
          : 0;
    const mutationsPerTick =
      elapsedTicks > 0
        ? (message.event_totals.mutations - baseline!.totals.mutations) / elapsedTicks
        : isSameSample && lastIndex >= 0
          ? this.data.mutations[lastIndex]
          : 0;

    let index = this.count < this.capacity ? this.count : this.capacity - 1;

    if (
      this.count > 0 &&
      this.data.epoch[this.count - 1] === message.epoch &&
      this.data.tick[this.count - 1] === message.tick
    ) {
      index = this.count - 1;
    } else if (this.count >= this.capacity) {
      Object.values(this.data).forEach((series) => series.copyWithin(0, 1));
    } else {
      this.count += 1;
    }

    this.data.epoch[index] = message.epoch;
    this.data.tick[index] = message.tick;
    this.data.population[index] = message.population;
    this.data.live_count[index] = message.live_count;
    this.data.inert_count[index] = message.inert_count;
    this.data.total_energy[index] = message.total_energy;
    this.data.total_mass[index] = message.total_mass;
    this.data.births[index] = birthsPerTick;
    this.data.deaths[index] = deathsPerTick;
    this.data.mutations[index] = mutationsPerTick;
    this.data.mean_program_size[index] = message.mean_program_size;
    this.data.max_program_size[index] = message.max_program_size;
    this.data.unique_genomes[index] = message.unique_genomes;
    this.eventBaseline = {
      epoch: message.epoch,
      tick: message.tick,
      totals: { ...message.event_totals },
    };
  }

  clear(): void {
    this.count = 0;
    this.eventBaseline = null;
    Object.values(this.data).forEach((series) => series.fill(0));
  }

  snapshot(): MetricsBufferSnapshot {
    return {
      count: this.count,
      epoch: this.data.epoch.slice(0, this.count),
      tick: this.data.tick.slice(0, this.count),
      population: this.data.population.slice(0, this.count),
      live_count: this.data.live_count.slice(0, this.count),
      inert_count: this.data.inert_count.slice(0, this.count),
      total_energy: this.data.total_energy.slice(0, this.count),
      total_mass: this.data.total_mass.slice(0, this.count),
      births: this.data.births.slice(0, this.count),
      deaths: this.data.deaths.slice(0, this.count),
      mutations: this.data.mutations.slice(0, this.count),
      mean_program_size: this.data.mean_program_size.slice(0, this.count),
      max_program_size: this.data.max_program_size.slice(0, this.count),
      unique_genomes: this.data.unique_genomes.slice(0, this.count),
    };
  }
}
