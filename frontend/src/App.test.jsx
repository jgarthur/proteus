import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import App from "./App";

beforeEach(() => {
  global.fetch = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({
      config: {
        width: 64,
        height: 64,
        rng_seed: 1,
        system_params: {
          R_energy: 0.25,
          R_mass: 0.05,
          P_spawn: 0.0,
          D_energy: 0.01,
          D_mass: 0.01,
          T_cap: 4,
          M: 1 / 128,
          inert_grace_ticks: 10,
          N_synth: 1,
          mutation_base_log2: 16,
          mutation_background_log2: 8
        },
        seeds: [
          {
            assembly_source: "absorb\ntakeM\ncw\npush 0\nsetSrc\ngetSize\nfor\nread\nappendAdj\nnext\nboot\n",
            x: 32,
            y: 32,
            count: 20,
            preset_key: "basic",
            randomize_additional_seeds: false,
            initial_dir: null,
            initial_id: null,
            initial_free_energy: 20,
            initial_free_mass: 11,
            neighbor_free_energy: 20,
            neighbor_free_mass: 11,
            live: true
          }
        ]
      },
      seed_assembly: "nop\n",
      seed_presets: [
        {
          key: "basic",
          label: "Basic",
          description: "Baseline",
          assembly_source: "nop\n"
        }
      ]
    })
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

test("renders the workbench title", async () => {
  render(<App />);
  expect(await screen.findByText("Artificial life workbench")).toBeInTheDocument();
});

test("computes viability math for built-in seed instructions", async () => {
  render(<App />);
  expect(await screen.findByText("Genome size S")).toBeInTheDocument();
  expect(screen.queryByText(/Cannot compute formulas/)).not.toBeInTheDocument();
});
