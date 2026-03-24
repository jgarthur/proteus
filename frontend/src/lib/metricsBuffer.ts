import { METRICS_CAPACITY } from '../constants';
import type { MetricsBufferSnapshot, MetricsMessage } from '../types';

type BufferKey = keyof Omit<MetricsBufferSnapshot, 'count'>;

export class MetricsBuffer {
  private readonly capacity = METRICS_CAPACITY;
  private count = 0;
  private readonly data: Record<BufferKey, Float64Array> = {
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

  push(message: MetricsMessage): void {
    let index = this.count < this.capacity ? this.count : this.capacity - 1;

    if (this.count > 0 && this.data.tick[this.count - 1] === message.tick) {
      index = this.count - 1;
    } else if (this.count >= this.capacity) {
      Object.values(this.data).forEach((series) => series.copyWithin(0, 1));
    } else {
      this.count += 1;
    }

    this.data.tick[index] = message.tick;
    this.data.population[index] = message.population;
    this.data.live_count[index] = message.live_count;
    this.data.inert_count[index] = message.inert_count;
    this.data.total_energy[index] = message.total_energy;
    this.data.total_mass[index] = message.total_mass;
    this.data.births[index] = message.births;
    this.data.deaths[index] = message.deaths;
    this.data.mutations[index] = message.mutations;
    this.data.mean_program_size[index] = message.mean_program_size;
    this.data.max_program_size[index] = message.max_program_size;
    this.data.unique_genomes[index] = message.unique_genomes;
  }

  clear(): void {
    this.count = 0;
    Object.values(this.data).forEach((series) => series.fill(0));
  }

  snapshot(): MetricsBufferSnapshot {
    return {
      count: this.count,
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
