import { startTransition, useEffect, useRef, useState } from "react";

const DEFAULT_VIEWPORT = {
  origin_x: 0,
  origin_y: 0,
  width: 64,
  height: 64,
  overlay: "hash"
};

const SPEED_PRESETS = [0.5, 1, 4, 12, 30];
const OVERLAYS = ["occupancy", "energy", "mass", "background", "size", "hash"];
const IMMEDIATE_MNEMONICS = new Set([
  "dup", "drop", "swap", "over", "rand",
  "add", "sub", "neg", "eq", "lt", "gt", "not", "and", "or",
  "for", "next", "jmp", "jmpnz", "jmpz",
  "cw", "ccw", "getsize", "getip", "getflag", "getmsg", "getid", "getsrc", "getdst",
  "setdir", "setsrc", "setdst", "setid", "gete", "getm"
]);
const ONE_TICK_BASE_COSTS = {
  nop: 0,
  absorb: 0,
  emit: 1,
  read: 0,
  write: 1,
  del: 1,
  synthesize: 1,
  readadj: 0,
  writeadj: 1,
  appendadj: 1,
  deladj: 1,
  sensesize: 0,
  sensee: 0,
  sensem: 0,
  senseid: 0,
  givee: 0,
  givem: 1,
  takee: 1,
  takem: 1,
  move: 1,
  boot: 0
};
const FIELD_HELP = {
  world_width: "World width in cells. Toroidal wrapping means edges connect left-to-right.",
  world_height: "World height in cells. Toroidal wrapping means edges connect top-to-bottom.",
  rng_seed: "Seed for deterministic randomness. Reusing the same config and RNG seed reproduces the same run.",
  seed_x: "Anchor position for the first seed. Additional seeds are placed into random empty cells.",
  seed_y: "Anchor position for the first seed. Additional seeds are placed into random empty cells.",
  seed_count: "How many copies of the selected seed program to place at startup.",
  randomize_additional_seeds: "If enabled, only the first seed uses the selected preset/code. The remaining startup seeds are chosen randomly from the built-in seed presets.",
  seed_preset: "Built-in seed program for the first seed. The assembly editor below always shows the selected seed's code.",
  seed_energy: "Free energy placed in the seed's own cell at tick 0.",
  seed_mass: "Free mass placed in the seed's own cell at tick 0.",
  neighbor_energy: "Free energy added to each of the four neighbor cells around every seeded organism.",
  neighbor_mass: "Free mass added to each of the four neighbor cells around every seeded organism.",
  viewport_overlay: "Cell coloring mode for the viewport.",
  viewport_zoom: "Canvas pixel size per world cell.",
  viewport_width: "Viewport width in cells.",
  viewport_height: "Viewport height in cells.",
  R_energy: "Probability that a cell gains 1 unit of background radiation each tick.",
  R_mass: "Probability that a cell gains 1 unit of free mass each tick.",
  P_spawn: "Probability that a background-mass arrival in an empty cell nucleates a live single-nop program.",
  D_energy: "Per-unit decay probability for background radiation and excess free energy each tick.",
  D_mass: "Per-unit decay probability for excess free mass each tick.",
  T_cap: "Storage threshold multiplier. A program can hold T_cap × program_size free energy and mass without decay.",
  M: "Per-instruction maintenance probability each tick.",
  inert_grace_ticks: "An inert program pays no maintenance while it has received a write within the last inert_grace_ticks ticks. After that grace window, normal maintenance resumes.",
  N_synth: "Additional energy cost paid by synthesize to convert energy into 1 free mass.",
  mutation_base_log2: "Baseline mutation probability is specified as 2^(-mutation_base_log2) per executed 1-tick instruction.",
  mutation_background_log2: "When paying with background radiation, mutation probability is min(background_amount / 2^(mutation_background_log2), 1)."
};

function formatSpeed(value) {
  return value == null ? "unlimited" : `${value.toFixed(1)} tps`;
}

function formatNumber(value) {
  if (Number.isNaN(value)) {
    return "invalid";
  }
  if (!Number.isFinite(value)) {
    return value > 0 ? "∞" : "—";
  }
  if (Number.isInteger(value)) {
    return value.toLocaleString();
  }
  if (Math.abs(value) >= 1) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 3 });
  }
  return value.toLocaleString(undefined, { maximumSignificantDigits: 4 });
}

function formatReciprocalPowerOfTwo(log2) {
  if (log2 === 0) {
    return "1";
  }
  return `1 / ${formatNumber(2 ** log2)}`;
}

function normalizeInstructionPointer(ip, size) {
  if (size <= 0) {
    return null;
  }
  return ((ip % size) + size) % size;
}

function formatDisassemblyWithPointer(disassembly, ip) {
  const pointer = normalizeInstructionPointer(ip, disassembly.length);
  return disassembly.map((line, index) => `${index === pointer ? "->" : "  "} ${index.toString().padStart(3, " ")}  ${line}`).join("\n");
}

function nextRandomSeed() {
  if (globalThis.crypto?.getRandomValues) {
    const values = new Uint32Array(1);
    globalThis.crypto.getRandomValues(values);
    return Number(values[0] & 0x7fffffff);
  }
  return Math.floor(Math.random() * 0x7fffffff);
}

function HelpLabel({ label, description }) {
  return (
    <span className="field-label" title={description}>
      <span>{label}</span>
      <span className="help-badge" aria-hidden="true">?</span>
    </span>
  );
}

function parseAssemblySize(source) {
  let instructionCount = 0;
  for (const rawLine of source.split("\n")) {
    const line = rawLine.split(";", 1)[0].trim();
    if (!line) {
      continue;
    }
    const parts = line.split(/\s+/);
    const mnemonic = parts[0];
    const lowered = mnemonic.toLowerCase();
    if (lowered === "push") {
      if (parts.length !== 2) {
        return { error: "push requires one literal operand." };
      }
      instructionCount += 1;
      continue;
    }
    if (lowered === ".byte") {
      if (parts.length !== 2) {
        return { error: ".byte requires one numeric operand." };
      }
      instructionCount += 1;
      continue;
    }
    if (!IMMEDIATE_MNEMONICS.has(lowered) && !(lowered in ONE_TICK_BASE_COSTS)) {
      return { error: `Unknown instruction: ${mnemonic}` };
    }
    if (parts.length !== 1) {
      return { error: `${mnemonic} does not take operands.` };
    }
    instructionCount += 1;
  }
  if (instructionCount === 0) {
    return { error: "Program is empty." };
  }
  return { instructionCount };
}

function computeRefill(rate, decay, ticks) {
  if (ticks <= 0 || rate <= 0) {
    return 0;
  }
  if (decay <= 0) {
    return rate * ticks;
  }
  return rate * (1 - (1 - decay) ** ticks) / decay;
}

function computeSteadyState(rate, decay) {
  if (rate <= 0) {
    return 0;
  }
  if (decay <= 0) {
    return Infinity;
  }
  return rate / decay;
}

function buildDerivedMetrics(seed, systemParams) {
  const parsed = parseAssemblySize(seed.assembly_source);
  if (parsed.error) {
    return { error: parsed.error };
  }
  const S = parsed.instructionCount;
  const storageThreshold = systemParams.T_cap * S;
  const maintenancePerTick = S * systemParams.M;
  const startingRunway = maintenancePerTick > 0 ? seed.initial_free_energy / maintenancePerTick : Infinity;
  const backgroundSteadyState = computeSteadyState(systemParams.R_energy, systemParams.D_energy);
  const backgroundRefillOverGenome = computeRefill(systemParams.R_energy, systemParams.D_energy, S);
  const solitaryAbsorbIntake = 5 * backgroundRefillOverGenome;
  const neighborMassRefillOverGenome = computeRefill(systemParams.R_mass, systemParams.D_mass, S);
  const synthesizeEnergyPerMass = systemParams.N_synth + 1;
  const spawnChancePerTick = systemParams.R_mass * systemParams.P_spawn;
  const baselineMutationRate = 2 ** (-systemParams.mutation_base_log2);
  const oneUnitBackgroundMutationRate = Math.min(1 / (2 ** systemParams.mutation_background_log2), 1);
  const bootstrapCrossEnergy = seed.initial_free_energy + (4 * seed.neighbor_free_energy);
  const bootstrapCrossMass = seed.initial_free_mass + (4 * seed.neighbor_free_mass);
  const totalBootstrapEnergy = seed.count * bootstrapCrossEnergy;
  const totalBootstrapMass = seed.count * bootstrapCrossMass;
  const initialStrength = Math.min(S, seed.initial_free_energy);
  const seedEnergyAboveThreshold = Math.max(0, seed.initial_free_energy - storageThreshold);
  const seedMassAboveThreshold = Math.max(0, seed.initial_free_mass - storageThreshold);
  return {
    assumption: "Formulas use S = parsed seed instruction count as the time horizon where the spec requires T. Total bootstrap ignores overlap between seeded neighborhoods.",
    cards: [
      { label: "Genome size S", value: formatNumber(S), formula: "parsed instruction count" },
      { label: "Storage threshold", value: formatNumber(storageThreshold), formula: "T_cap × S" },
      { label: "Maintenance / tick", value: formatNumber(maintenancePerTick), formula: "S × M" },
      { label: "Inert grace window", value: systemParams.inert_grace_ticks > 0 ? `${formatNumber(systemParams.inert_grace_ticks)} ticks` : "none", formula: "inert_grace_ticks" },
      { label: "Inert maintenance while active", value: "0", formula: "active inert programs do not pay maintenance during grace" },
      { label: "Starting energy runway", value: `${formatNumber(startingRunway)} ticks`, formula: "initial_free_energy / (S × M)" },
      { label: "Starting strength", value: formatNumber(initialStrength), formula: "min(S, initial_free_energy)" },
      { label: "Background steady state", value: formatNumber(backgroundSteadyState), formula: "R_energy / D_energy" },
      { label: `Background refill over ${S} ticks`, value: formatNumber(backgroundRefillOverGenome), formula: "R_energy × (1 − (1 − D_energy)^S) / D_energy" },
      { label: `Solitary absorb intake over ${S} ticks`, value: formatNumber(solitaryAbsorbIntake), formula: "5 × refill(S)" },
      { label: `Empty-neighbor mass refill over ${S} ticks`, value: formatNumber(neighborMassRefillOverGenome), formula: "R_mass × (1 − (1 − D_mass)^S) / D_mass" },
      { label: "Synthesize energy / mass", value: formatNumber(synthesizeEnergyPerMass), formula: "N_synth + 1" },
      { label: "Spawn chance / empty cell / tick", value: formatNumber(spawnChancePerTick), formula: "R_mass × P_spawn" },
      { label: "Baseline mutation rate", value: formatNumber(baselineMutationRate), formula: `2^(-${systemParams.mutation_base_log2}) = ${formatReciprocalPowerOfTwo(systemParams.mutation_base_log2)}` },
      { label: "Background mutation rate at x=1", value: formatNumber(oneUnitBackgroundMutationRate), formula: `min(1 / 2^${systemParams.mutation_background_log2}, 1)` },
      { label: "Bootstrap energy / seed cross", value: formatNumber(bootstrapCrossEnergy), formula: "initial_free_energy + 4 × neighbor_free_energy" },
      { label: "Bootstrap mass / seed cross", value: formatNumber(bootstrapCrossMass), formula: "initial_free_mass + 4 × neighbor_free_mass" },
      { label: "Total bootstrap energy", value: formatNumber(totalBootstrapEnergy), formula: "seed_count × bootstrap_cross_energy" },
      { label: "Total bootstrap mass", value: formatNumber(totalBootstrapMass), formula: "seed_count × bootstrap_cross_mass" },
      { label: "Seed-cell energy above threshold", value: formatNumber(seedEnergyAboveThreshold), formula: "max(0, initial_free_energy − T_cap × S)" },
      { label: "Seed-cell mass above threshold", value: formatNumber(seedMassAboveThreshold), formula: "max(0, initial_free_mass − T_cap × S)" }
    ]
  };
}

function api(path, options) {
  return fetch(path, {
    headers: {
      "Content-Type": "application/json",
      ...(options?.headers ?? {})
    },
    ...options
  }).then(async (response) => {
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
      throw new Error(payload?.detail ?? `Request failed: ${response.status}`);
    }
    return payload;
  });
}

function createApiBase() {
  if (window.location.port === "5173") {
    return `${window.location.protocol}//${window.location.hostname}:8000`;
  }
  return `${window.location.protocol}//${window.location.host}`;
}

function buildWebSocketUrl() {
  const base = createApiBase();
  const protocol = base.startsWith("https") ? "wss" : "ws";
  return `${protocol}://${base.replace(/^https?:\/\//, "")}/api/runs/current/stream`;
}

function buildOverlayColor(cell, overlay) {
  if (overlay === "energy") {
    const intensity = Math.min(1, cell.free_energy / 30);
    return `rgba(${Math.round(254 - intensity * 80)}, ${Math.round(217 - intensity * 40)}, ${Math.round(72 + intensity * 110)}, 1)`;
  }
  if (overlay === "mass") {
    const intensity = Math.min(1, cell.free_mass / 20);
    return `rgba(${Math.round(204 - intensity * 100)}, ${Math.round(154 + intensity * 60)}, ${Math.round(125 - intensity * 50)}, 1)`;
  }
  if (overlay === "background") {
    const intensity = Math.min(1, cell.background_radiation / 20);
    return `rgba(${Math.round(38 + intensity * 70)}, ${Math.round(61 + intensity * 90)}, ${Math.round(74 + intensity * 110)}, 1)`;
  }
  if (overlay === "size") {
    if (!cell.occupied) {
      return "rgba(243, 236, 222, 1)";
    }
    const intensity = Math.min(1, cell.size / 32);
    return `rgba(${Math.round(244 - intensity * 98)}, ${Math.round(223 - intensity * 114)}, ${Math.round(196 - intensity * 154)}, 1)`;
  }
  if (overlay === "hash") {
    if (!cell.program_hash) {
      return "rgba(243, 236, 222, 1)";
    }
    const hue = parseInt(cell.program_hash.slice(0, 6), 16) % 360;
    return `hsl(${hue} 72% 58%)`;
  }
  if (!cell.occupied) {
    return "rgba(243, 236, 222, 1)";
  }
  if (!cell.live) {
    return "rgba(204, 132, 58, 1)";
  }
  return "rgba(28, 37, 44, 1)";
}

function App() {
  const [config, setConfig] = useState(null);
  const [seedPresets, setSeedPresets] = useState([]);
  const [viewport, setViewport] = useState(DEFAULT_VIEWPORT);
  const [zoom, setZoom] = useState(18);
  const [status, setStatus] = useState("idle");
  const [targetTps, setTargetTps] = useState(4);
  const [summary, setSummary] = useState(null);
  const [frame, setFrame] = useState(null);
  const [selectedCell, setSelectedCell] = useState(null);
  const [selectedDetail, setSelectedDetail] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [streamState, setStreamState] = useState("disconnected");
  const [importText, setImportText] = useState("");
  const canvasRef = useRef(null);
  const socketRef = useRef(null);

  async function loadCellDetail(x, y, { showError = true } = {}) {
    try {
      const detail = await api(`/api/runs/current/cells/${x}/${y}`);
      setSelectedDetail(detail);
    } catch (nextError) {
      if (showError) {
        setError(nextError.message);
      }
    }
  }

  useEffect(() => {
    api("/api/defaults")
      .then((payload) => {
        setConfig(payload.config);
        setSeedPresets(payload.seed_presets ?? []);
        setViewport((current) => ({
          ...current,
          origin_x: Math.max(0, Math.floor(payload.config.width / 2) - Math.floor(current.width / 2)),
          origin_y: Math.max(0, Math.floor(payload.config.height / 2) - Math.floor(current.height / 2))
        }));
      })
      .catch((nextError) => setError(nextError.message))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!frame || !canvasRef.current) {
      return;
    }
    const canvas = canvasRef.current;
    const context = canvas.getContext("2d");
    canvas.width = frame.width * zoom;
    canvas.height = frame.height * zoom;
    context.clearRect(0, 0, canvas.width, canvas.height);
    frame.cells.forEach((row, rowIndex) => {
      row.forEach((cell, columnIndex) => {
        context.fillStyle = buildOverlayColor(cell, frame.overlay);
        context.fillRect(columnIndex * zoom, rowIndex * zoom, zoom, zoom);
        if (cell.open && cell.occupied) {
          context.fillStyle = "rgba(255, 255, 255, 0.55)";
          context.fillRect(columnIndex * zoom, rowIndex * zoom, Math.max(2, zoom / 3), Math.max(2, zoom / 3));
        }
        if (selectedCell && selectedCell.x === cell.x && selectedCell.y === cell.y) {
          context.strokeStyle = "#f45d22";
          context.lineWidth = 2;
          context.strokeRect(columnIndex * zoom + 1, rowIndex * zoom + 1, zoom - 2, zoom - 2);
        }
      });
    });
  }, [frame, zoom, selectedCell]);

  useEffect(() => {
    if (!summary) {
      return;
    }
    const socket = new WebSocket(buildWebSocketUrl());
    socketRef.current = socket;
    socket.addEventListener("open", () => {
      setStreamState("connected");
      socket.send(JSON.stringify({ type: "viewport", ...viewport }));
    });
    socket.addEventListener("message", (event) => {
      const payload = JSON.parse(event.data);
      if (payload.type === "frame") {
        startTransition(() => {
          setStatus(payload.status);
          setTargetTps(payload.target_tps);
          setSummary(payload.summary);
          setFrame(payload.viewport);
        });
      }
      if (payload.type === "error") {
        setError(payload.message);
      }
    });
    socket.addEventListener("close", () => setStreamState("disconnected"));
    socket.addEventListener("error", () => setStreamState("error"));
    return () => {
      socket.close();
    };
  }, [Boolean(summary)]);

  useEffect(() => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      socketRef.current.send(JSON.stringify({ type: "viewport", ...viewport }));
    }
  }, [viewport]);

  useEffect(() => {
    if (!selectedCell || !summary) {
      return;
    }
    let cancelled = false;
    api(`/api/runs/current/cells/${selectedCell.x}/${selectedCell.y}`)
      .then((detail) => {
        if (!cancelled) {
          setSelectedDetail(detail);
        }
      })
      .catch((nextError) => {
        if (!cancelled) {
          setError(nextError.message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selectedCell, summary?.tick]);

  async function refreshRunState() {
    const current = await api("/api/runs/current");
    setStatus(current.status);
    setTargetTps(current.target_tps);
    setSummary(current.summary);
    const nextFrame = await api(
      `/api/runs/current/viewport?origin_x=${viewport.origin_x}&origin_y=${viewport.origin_y}&width=${viewport.width}&height=${viewport.height}&overlay=${viewport.overlay}`
    );
    setFrame(nextFrame);
    return current;
  }

  async function createRun() {
    setError("");
    const nextConfig = {
      ...config,
      rng_seed: nextRandomSeed()
    };
    setConfig(nextConfig);
    const payload = await api("/api/runs", {
      method: "POST",
      body: JSON.stringify(nextConfig)
    });
    setStatus(payload.status);
    setTargetTps(payload.target_tps);
    setSummary(payload.summary);
    await refreshRunState();
  }

  async function controlRun(action, extra = {}) {
    setError("");
    const payload = await api("/api/runs/current/control", {
      method: "POST",
      body: JSON.stringify({ action, ...extra })
    });
    setStatus(payload.status);
    setTargetTps(payload.target_tps);
    setSummary(payload.summary);
    if (action !== "play") {
      await refreshRunState();
    }
  }

  async function selectCell(x, y) {
    setSelectedCell({ x, y });
    await loadCellDetail(x, y);
  }

  async function exportRun() {
    const response = await fetch("/api/runs/current/export");
    if (!response.ok) {
      const payload = await response.json();
      throw new Error(payload.detail);
    }
    const text = await response.text();
    const blob = new Blob([text], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "proteus-run.json";
    link.click();
    URL.revokeObjectURL(url);
  }

  async function importRun() {
    setError("");
    const archive = JSON.parse(importText);
    const payload = await api("/api/runs/import", {
      method: "POST",
      body: JSON.stringify({ archive })
    });
    setStatus(payload.status);
    setTargetTps(payload.target_tps);
    setSummary(payload.summary);
    await refreshRunState();
  }

  function updateSystemParam(name, value) {
    setConfig((current) => ({
      ...current,
      system_params: {
        ...current.system_params,
        [name]: value
      }
    }));
  }

  function updateSeed(name, value) {
    setConfig((current) => ({
      ...current,
      seeds: current.seeds.map((seed, index) =>
        index === 0
          ? {
              ...seed,
              [name]: value
            }
          : seed
      )
    }));
  }

  function selectSeedPreset(presetKey) {
    const preset = seedPresets.find((entry) => entry.key === presetKey);
    if (!preset) {
      updateSeed("preset_key", null);
      return;
    }
    setConfig((current) => ({
      ...current,
      seeds: current.seeds.map((seed, index) =>
        index === 0
          ? {
              ...seed,
              preset_key: preset.key,
              assembly_source: preset.assembly_source
            }
          : seed
      )
    }));
  }

  function handleCanvasClick(event) {
    if (!frame) {
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const x = Math.floor((event.clientX - rect.left) / zoom);
    const y = Math.floor((event.clientY - rect.top) / zoom);
    const cell = frame.cells[y]?.[x];
    if (cell) {
      selectCell(cell.x, cell.y);
    }
  }

  if (loading || !config) {
    return <main className="app-shell"><section className="panel">Loading Proteus…</section></main>;
  }

  const seed = config.seeds[0];
  const selectedPreset = seedPresets.find((entry) => entry.key === seed.preset_key) ?? null;
  const derivedMetrics = buildDerivedMetrics(seed, config.system_params);
  const selectedProgram = selectedDetail?.program ?? null;
  const selectedInstructionPointer = selectedProgram
    ? normalizeInstructionPointer(selectedProgram.registers.IP, selectedProgram.disassembly.length)
    : null;
  const pointedInstruction = selectedProgram && selectedInstructionPointer != null
    ? selectedProgram.disassembly[selectedInstructionPointer]
    : null;

  return (
    <main className="app-shell">
      <section className="panel control-panel">
        <div className="panel-header">
          <p className="eyebrow">Proteus v0</p>
          <h1>Artificial life workbench</h1>
        </div>
        <div className="form-grid">
          <label>
            <HelpLabel label="Width" description={FIELD_HELP.world_width} />
            <input type="number" value={config.width} onChange={(event) => setConfig({ ...config, width: Number(event.target.value) })} />
          </label>
          <label>
            <HelpLabel label="Height" description={FIELD_HELP.world_height} />
            <input type="number" value={config.height} onChange={(event) => setConfig({ ...config, height: Number(event.target.value) })} />
          </label>
          <label>
            <HelpLabel label="RNG seed" description={FIELD_HELP.rng_seed} />
            <input type="number" value={config.rng_seed} onChange={(event) => setConfig({ ...config, rng_seed: Number(event.target.value) })} />
          </label>
          <label>
            <HelpLabel label="Seed X" description={FIELD_HELP.seed_x} />
            <input type="number" value={seed.x} onChange={(event) => updateSeed("x", Number(event.target.value))} />
          </label>
          <label>
            <HelpLabel label="Seed Y" description={FIELD_HELP.seed_y} />
            <input type="number" value={seed.y} onChange={(event) => updateSeed("y", Number(event.target.value))} />
          </label>
          <label>
            <HelpLabel label="Seed count" description={FIELD_HELP.seed_count} />
            <input type="number" min="1" value={seed.count ?? 1} onChange={(event) => updateSeed("count", Number(event.target.value))} />
          </label>
          <label className="checkbox-field">
            <HelpLabel label="Randomly selected seeds" description={FIELD_HELP.randomize_additional_seeds} />
            <input
              type="checkbox"
              checked={Boolean(seed.randomize_additional_seeds)}
              onChange={(event) => updateSeed("randomize_additional_seeds", event.target.checked)}
            />
          </label>
          <label>
            <HelpLabel label="Seed preset" description={FIELD_HELP.seed_preset} />
            <select value={seed.preset_key ?? "custom"} onChange={(event) => selectSeedPreset(event.target.value)}>
              {seedPresets.map((preset) => <option key={preset.key} value={preset.key}>{preset.label}</option>)}
              <option value="custom">Custom</option>
            </select>
          </label>
          <label>
            <HelpLabel label="Seed energy" description={FIELD_HELP.seed_energy} />
            <input type="number" value={seed.initial_free_energy} onChange={(event) => updateSeed("initial_free_energy", Number(event.target.value))} />
          </label>
          <label>
            <HelpLabel label="Seed mass" description={FIELD_HELP.seed_mass} />
            <input type="number" value={seed.initial_free_mass} onChange={(event) => updateSeed("initial_free_mass", Number(event.target.value))} />
          </label>
          <label>
            <HelpLabel label="Neighbor energy" description={FIELD_HELP.neighbor_energy} />
            <input type="number" value={seed.neighbor_free_energy} onChange={(event) => updateSeed("neighbor_free_energy", Number(event.target.value))} />
          </label>
          <label>
            <HelpLabel label="Neighbor mass" description={FIELD_HELP.neighbor_mass} />
            <input type="number" value={seed.neighbor_free_mass} onChange={(event) => updateSeed("neighbor_free_mass", Number(event.target.value))} />
          </label>
        </div>
        <div className="params-grid">
          {Object.entries(config.system_params).map(([name, value]) => (
            <label key={name}>
              <HelpLabel label={name} description={FIELD_HELP[name] ?? name} />
              <input type="number" step="any" value={value} onChange={(event) => updateSystemParam(name, Number(event.target.value))} />
            </label>
          ))}
        </div>
        <div className="derived-panel">
          <div className="panel-header compact">
            <p className="eyebrow">Derived</p>
            <h2>Viability math</h2>
          </div>
          {"error" in derivedMetrics ? (
            <p className="muted">Cannot compute formulas: {derivedMetrics.error}</p>
          ) : (
            <>
              <p className="muted">{derivedMetrics.assumption}</p>
              <dl className="formula-grid">
                {derivedMetrics.cards.map((card) => (
                  <div key={card.label} className="formula-card" title={card.formula}>
                    <dt>{card.label}</dt>
                    <dd>{card.value}</dd>
                    <small>{card.formula}</small>
                  </div>
                ))}
              </dl>
            </>
          )}
        </div>
        <label className="full-width">
          <HelpLabel label="Seed assembly" description="Editable source code for the selected first seed. This remains visible even when a built-in preset is selected." />
          <textarea value={seed.assembly_source} onChange={(event) => {
            updateSeed("preset_key", null);
            updateSeed("assembly_source", event.target.value);
          }} rows={14} />
        </label>
        {selectedPreset ? <p className="muted">{selectedPreset.description}</p> : <p className="muted">Custom assembly for the selected first seed. Additional startup seeds only randomize when the checkbox is enabled.</p>}
        <label className="full-width">
          Import archive JSON
          <textarea value={importText} onChange={(event) => setImportText(event.target.value)} rows={6} />
        </label>
        <div className="button-row">
          <button className="secondary" onClick={importRun}>Import</button>
          <button className="secondary" onClick={exportRun} disabled={!summary}>Export</button>
        </div>
        {error ? <p className="error-banner">{error}</p> : null}
      </section>

      <section className="panel viewport-panel">
        <div className="run-controls">
          <div className="status-strip">
            <span>Status: {status}</span>
            <span>Stream: {streamState}</span>
            <span>Speed: {formatSpeed(targetTps)}</span>
          </div>
          <div className="button-row">
            <button onClick={createRun}>Create run</button>
            <button onClick={() => controlRun(status === "playing" ? "pause" : "play")} disabled={!summary}>
              {status === "playing" ? "Pause" : "Play"}
            </button>
            <button onClick={() => controlRun("step", { steps: 1 })} disabled={!summary}>Step</button>
            <button onClick={() => controlRun("reset")} disabled={!summary}>Reset</button>
          </div>
          <div className="button-row">
            {SPEED_PRESETS.map((preset) => (
              <button key={preset} className={preset === targetTps ? "secondary active" : "secondary"} onClick={() => controlRun("set_speed", { target_tps: preset })} disabled={!summary}>
                {preset} tps
              </button>
            ))}
            <button className={targetTps == null ? "secondary active" : "secondary"} onClick={() => controlRun("set_speed", { target_tps: null })} disabled={!summary}>
              max
            </button>
          </div>
        </div>
        <div className="viewport-toolbar">
          <div>
            <p className="eyebrow">Viewport</p>
            <h2>Current world</h2>
          </div>
          <div className="toolbar-actions">
            <label>
              <HelpLabel label="Overlay" description={FIELD_HELP.viewport_overlay} />
              <select value={viewport.overlay} onChange={(event) => setViewport({ ...viewport, overlay: event.target.value })}>
                {OVERLAYS.map((overlay) => <option key={overlay}>{overlay}</option>)}
              </select>
            </label>
            <label>
              <HelpLabel label="Zoom" description={FIELD_HELP.viewport_zoom} />
              <input type="range" min="8" max="40" value={zoom} onChange={(event) => setZoom(Number(event.target.value))} />
            </label>
          </div>
        </div>
        <div className="viewport-controls">
          <button onClick={() => setViewport({ ...viewport, origin_y: viewport.origin_y - 4 })}>Up</button>
          <button onClick={() => setViewport({ ...viewport, origin_x: viewport.origin_x - 4 })}>Left</button>
          <button onClick={() => setViewport({ ...viewport, origin_x: viewport.origin_x + 4 })}>Right</button>
          <button onClick={() => setViewport({ ...viewport, origin_y: viewport.origin_y + 4 })}>Down</button>
          <label>
            <HelpLabel label="Width" description={FIELD_HELP.viewport_width} />
            <input type="number" value={viewport.width} onChange={(event) => setViewport({ ...viewport, width: Number(event.target.value) })} />
          </label>
          <label>
            <HelpLabel label="Height" description={FIELD_HELP.viewport_height} />
            <input type="number" value={viewport.height} onChange={(event) => setViewport({ ...viewport, height: Number(event.target.value) })} />
          </label>
        </div>
        <div className="canvas-frame">
          <canvas ref={canvasRef} onClick={handleCanvasClick} />
        </div>
      </section>

      <section className="panel inspector-panel">
        <div className="panel-header">
          <p className="eyebrow">Metrics</p>
          <h2>Run state</h2>
        </div>
        {summary ? (
          <>
            <dl className="metrics-grid">
              <div><dt>Tick</dt><dd>{summary.tick}</dd></div>
              <div><dt>Backend</dt><dd>{summary.engine_backend}</dd></div>
              <div><dt>Occupied</dt><dd>{summary.occupied_cells}</dd></div>
              <div><dt>Live</dt><dd>{summary.live_programs}</dd></div>
              <div><dt>Inert</dt><dd>{summary.inert_programs}</dd></div>
              <div><dt>Instructions</dt><dd>{summary.total_instructions}</dd></div>
              <div><dt>Free energy</dt><dd>{summary.total_free_energy}</dd></div>
              <div><dt>Free mass</dt><dd>{summary.total_free_mass}</dd></div>
              <div><dt>Background rad</dt><dd>{summary.total_background_radiation}</dd></div>
            </dl>
            {summary.counters ? (
              <dl className="metrics-grid">
                <div><dt>Inert created</dt><dd>{summary.counters.inert_created ?? 0}</dd></div>
                <div><dt>Abandoned</dt><dd>{summary.counters.inert_abandonments ?? 0}</dd></div>
                <div><dt>Booted active</dt><dd>{summary.counters.boot_successes_active_construction ?? 0}</dd></div>
                <div><dt>Booted abandoned</dt><dd>{summary.counters.boot_successes_abandoned ?? 0}</dd></div>
                <div><dt>Preboot removed</dt><dd>{summary.counters.inert_removed_preboot ?? 0}</dd></div>
                <div><dt>Writes</dt><dd>{summary.counters.inert_write_events ?? 0}</dd></div>
              </dl>
            ) : null}
          </>
        ) : (
          <p className="muted">Create or import a run to start streaming frames.</p>
        )}
        <div className="panel-header compact">
          <p className="eyebrow">Inspector</p>
          <h2>{selectedCell ? `${selectedCell.x}, ${selectedCell.y}` : "No cell selected"}</h2>
        </div>
        {selectedDetail ? (
          <div className="inspector-block">
            <p>Open: {selectedDetail.open ? "yes" : "no"}</p>
            <p>Free energy: {selectedDetail.free_energy}</p>
            <p>Free mass: {selectedDetail.free_mass}</p>
            <p>Background radiation: {selectedDetail.background_radiation}</p>
            {selectedDetail.program ? (
              <>
                <p>Program #{selectedDetail.program.program_id}</p>
                <p>Lifecycle: {selectedDetail.program.live ? "live" : "inert"}</p>
                {!selectedDetail.program.live ? (
                  <p>
                    Inert wait: {selectedDetail.program.inert_ticks_without_write}
                    {config.system_params.inert_grace_ticks > 0 ? ` / ${config.system_params.inert_grace_ticks}` : " (no grace)"}
                  </p>
                ) : null}
                <p>Age: {selectedDetail.program.age}</p>
                <p>Hash: {selectedDetail.program.program_hash}</p>
                <p>Strength: {selectedDetail.program.strength}</p>
                <p>Current instruction: {pointedInstruction ?? "none"}{selectedInstructionPointer != null ? ` (IP ${selectedInstructionPointer})` : ""}</p>
                <pre>{JSON.stringify(selectedDetail.program.registers, null, 2)}</pre>
                <pre>{formatDisassemblyWithPointer(selectedDetail.program.disassembly, selectedDetail.program.registers.IP)}</pre>
              </>
            ) : (
              <p className="muted">Empty cell.</p>
            )}
          </div>
        ) : (
          <p className="muted">Click a cell in the viewport for details.</p>
        )}
      </section>
    </main>
  );
}
export default App;
