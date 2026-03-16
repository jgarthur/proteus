//! Runs the single-simulation controller and its background worker thread.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::model::{Direction, Program};
use crate::observe::{
    collect_metrics, encode_grid_frame, inspect_cell, inspect_region, CellInspection,
    MetricsSnapshot,
};
use crate::random::cell_rng;
use crate::{Simulation, SimulationError, TickReport};

use super::types::{
    CreateSimulationResponse, SimulationConfig, SimulationLifecycle, SimulationStatusResponse,
};

const SEED_PROGRAM_SALT: u64 = 0x0d7e_8ef0_4268_33c1;
const TPS_WINDOW: Duration = Duration::from_millis(250);

/// Carries one binary grid frame through the watch channel.
#[derive(Clone, Debug)]
pub struct FramePayload {
    pub tick: u64,
    pub bytes: Arc<Vec<u8>>,
}

/// Carries one metrics snapshot through the watch channel.
#[derive(Clone, Debug)]
pub struct MetricsPayload {
    pub tick: u64,
    pub snapshot: Arc<MetricsSnapshot>,
}

/// Exposes async commands for creating, driving, and observing one simulation.
#[derive(Clone)]
pub struct SimulationController {
    command_tx: mpsc::Sender<Command>,
    frame_rx: watch::Receiver<Option<FramePayload>>,
    metrics_rx: watch::Receiver<Option<MetricsPayload>>,
    destroy_tx: broadcast::Sender<()>,
}

impl Default for SimulationController {
    /// Builds a new controller with its background worker thread.
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationController {
    /// Builds a new controller with fresh worker channels.
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel(64);
        let (frame_tx, frame_rx) = watch::channel(None);
        let (metrics_tx, metrics_rx) = watch::channel(None);
        let (destroy_tx, _) = broadcast::channel(16);

        let worker_destroy_tx = destroy_tx.clone();
        thread::Builder::new()
            .name("proteus-sim-controller".to_owned())
            .spawn(move || worker_loop(command_rx, frame_tx, metrics_tx, worker_destroy_tx))
            .expect("simulation controller thread should start");

        Self {
            command_tx,
            frame_rx,
            metrics_rx,
            destroy_tx,
        }
    }

    /// Subscribes to the latest binary frame stream.
    pub fn frame_receiver(&self) -> watch::Receiver<Option<FramePayload>> {
        self.frame_rx.clone()
    }

    /// Subscribes to the latest metrics stream.
    pub fn metrics_receiver(&self) -> watch::Receiver<Option<MetricsPayload>> {
        self.metrics_rx.clone()
    }

    /// Subscribes to destroy notifications for active clients.
    pub fn destroy_receiver(&self) -> broadcast::Receiver<()> {
        self.destroy_tx.subscribe()
    }

    /// Returns the most recently published frame, if any.
    pub fn current_frame(&self) -> Option<FramePayload> {
        self.frame_rx.borrow().clone()
    }

    /// Returns the most recently published metrics snapshot, if any.
    pub fn current_metrics(&self) -> Option<MetricsPayload> {
        self.metrics_rx.borrow().clone()
    }

    /// Creates a new managed simulation from the supplied API config.
    pub async fn create(
        &self,
        config: SimulationConfig,
    ) -> Result<CreateSimulationResponse, ControllerError> {
        self.request(|response_tx| Command::Create {
            config,
            response_tx,
        })
        .await
    }

    /// Returns the current simulation status.
    pub async fn status(&self) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(Command::Status).await
    }

    /// Returns the current simulation config.
    pub async fn config(&self) -> Result<SimulationConfig, ControllerError> {
        self.request(Command::Config).await
    }

    /// Starts a created simulation.
    pub async fn start(&self) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(Command::Start).await
    }

    /// Pauses a running simulation.
    pub async fn pause(&self) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(Command::Pause).await
    }

    /// Resumes a paused simulation.
    pub async fn resume(&self) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(Command::Resume).await
    }

    /// Steps a paused simulation forward by a fixed tick count.
    pub async fn step(&self, count: u64) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(|response_tx| Command::Step { count, response_tx })
            .await
    }

    /// Resets the current simulation back to its original config.
    pub async fn reset(&self) -> Result<SimulationStatusResponse, ControllerError> {
        self.request(Command::Reset).await
    }

    /// Destroys the current simulation and clears observers.
    pub async fn destroy(&self) -> Result<(), ControllerError> {
        self.request(Command::Destroy).await
    }

    /// Returns the latest metrics snapshot from the worker.
    pub async fn metrics(&self) -> Result<MetricsSnapshot, ControllerError> {
        self.request(Command::Metrics).await
    }

    /// Returns one inspected cell by flat index.
    pub async fn inspect_cell_by_index(
        &self,
        index: usize,
    ) -> Result<CellInspection, ControllerError> {
        self.request(|response_tx| Command::InspectCellByIndex { index, response_tx })
            .await
    }

    /// Returns one inspected cell by grid coordinates.
    pub async fn inspect_cell_by_coordinates(
        &self,
        x: u32,
        y: u32,
    ) -> Result<CellInspection, ControllerError> {
        self.request(|response_tx| Command::InspectCellByCoordinates { x, y, response_tx })
            .await
    }

    /// Returns a bounded inspected region of cells.
    pub async fn inspect_region(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<Vec<CellInspection>, ControllerError> {
        self.request(|response_tx| Command::InspectRegion {
            x,
            y,
            w,
            h,
            response_tx,
        })
        .await
    }

    /// Sends one command to the worker thread and awaits its response.
    async fn request<T, F>(&self, command: F) -> Result<T, ControllerError>
    where
        T: Send + 'static,
        F: FnOnce(oneshot::Sender<Result<T, ControllerError>>) -> Command,
    {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(command(response_tx))
            .await
            .map_err(|_| {
                ControllerError::Internal("controller command channel closed".to_owned())
            })?;
        response_rx.await.map_err(|_| {
            ControllerError::Internal("controller response channel closed".to_owned())
        })?
    }
}

/// Describes the controller-layer failures exposed to the API.
#[derive(Debug)]
pub enum ControllerError {
    NoSim,
    SimAlreadyExists,
    SimNotRunning,
    SimNotPaused,
    SimNotCreated,
    InvalidConfig(String),
    CellOutOfBounds,
    RegionTooLarge { area: u32 },
    BadRequest(String),
    Internal(String),
}

/// Enumerates the commands the worker thread can execute.
enum Command {
    Create {
        config: SimulationConfig,
        response_tx: oneshot::Sender<Result<CreateSimulationResponse, ControllerError>>,
    },
    Status(oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>),
    Config(oneshot::Sender<Result<SimulationConfig, ControllerError>>),
    Start(oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>),
    Pause(oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>),
    Resume(oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>),
    Step {
        count: u64,
        response_tx: oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>,
    },
    Reset(oneshot::Sender<Result<SimulationStatusResponse, ControllerError>>),
    Destroy(oneshot::Sender<Result<(), ControllerError>>),
    Metrics(oneshot::Sender<Result<MetricsSnapshot, ControllerError>>),
    InspectCellByIndex {
        index: usize,
        response_tx: oneshot::Sender<Result<CellInspection, ControllerError>>,
    },
    InspectCellByCoordinates {
        x: u32,
        y: u32,
        response_tx: oneshot::Sender<Result<CellInspection, ControllerError>>,
    },
    InspectRegion {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        response_tx: oneshot::Sender<Result<Vec<CellInspection>, ControllerError>>,
    },
}

/// Stores the live simulation plus its latest observer-facing projections.
struct ManagedSimulation {
    config: SimulationConfig,
    simulation: Simulation,
    lifecycle: SimulationLifecycle,
    latest_metrics: MetricsSnapshot,
    latest_frame: Arc<Vec<u8>>,
    ticks_per_second: f64,
    tps_window_start: Instant,
    tps_window_tick: u64,
}

impl ManagedSimulation {
    /// Builds a managed simulation and publishes its initial observation state.
    fn new(config: SimulationConfig) -> Result<Self, ControllerError> {
        config.validate().map_err(ControllerError::InvalidConfig)?;

        let mut simulation =
            Simulation::new(config.to_engine_config()).map_err(simulation_error)?;
        apply_seed_programs(&mut simulation, &config).map_err(ControllerError::InvalidConfig)?;

        let latest_metrics =
            collect_metrics(simulation.grid(), simulation.tick(), TickReport::default());
        let latest_frame = Arc::new(encode_grid_frame(simulation.grid(), simulation.tick()));

        Ok(Self {
            config,
            simulation,
            lifecycle: SimulationLifecycle::Created,
            latest_metrics,
            latest_frame,
            ticks_per_second: 0.0,
            tps_window_start: Instant::now(),
            tps_window_tick: 0,
        })
    }

    /// Builds the response returned by successful create requests.
    fn create_response(&self) -> CreateSimulationResponse {
        CreateSimulationResponse {
            status: self.lifecycle,
            tick: self.simulation.tick(),
            grid_width: self.simulation.grid().width(),
            grid_height: self.simulation.grid().height(),
            config: self.config.clone(),
        }
    }

    /// Builds the response returned by status-like requests.
    fn status_response(&self) -> SimulationStatusResponse {
        SimulationStatusResponse {
            status: self.lifecycle,
            tick: self.simulation.tick(),
            grid_width: self.simulation.grid().width(),
            grid_height: self.simulation.grid().height(),
            population: self.latest_metrics.population,
            total_energy: self.latest_metrics.total_energy,
            total_mass: self.latest_metrics.total_mass,
            ticks_per_second: self.ticks_per_second,
        }
    }

    /// Publishes the latest frame and metrics to all watchers.
    fn publish(
        &self,
        frame_tx: &watch::Sender<Option<FramePayload>>,
        metrics_tx: &watch::Sender<Option<MetricsPayload>>,
    ) {
        let _ = frame_tx.send(Some(FramePayload {
            tick: self.simulation.tick(),
            bytes: Arc::clone(&self.latest_frame),
        }));
        let _ = metrics_tx.send(Some(MetricsPayload {
            tick: self.latest_metrics.tick,
            snapshot: Arc::new(self.latest_metrics.clone()),
        }));
    }

    /// Resets the rolling ticks-per-second measurement window.
    fn reset_tps(&mut self) {
        self.ticks_per_second = 0.0;
        self.tps_window_start = Instant::now();
        self.tps_window_tick = self.simulation.tick();
    }

    /// Recomputes the latest metrics snapshot and frame after a tick.
    fn refresh_observation(&mut self, report: TickReport) {
        self.latest_metrics =
            collect_metrics(self.simulation.grid(), self.simulation.tick(), report);
        self.latest_frame = Arc::new(encode_grid_frame(
            self.simulation.grid(),
            self.simulation.tick(),
        ));
    }

    /// Runs one full tick and republishes the observer streams.
    fn tick_once(
        &mut self,
        frame_tx: &watch::Sender<Option<FramePayload>>,
        metrics_tx: &watch::Sender<Option<MetricsPayload>>,
    ) {
        let report = self.simulation.run_tick_report();
        self.refresh_observation(report);
        self.update_tps_after_tick();
        self.publish(frame_tx, metrics_tx);
    }

    /// Updates the smoothed ticks-per-second estimate when the window expires.
    fn update_tps_after_tick(&mut self) {
        let elapsed = self.tps_window_start.elapsed();
        if elapsed < TPS_WINDOW {
            return;
        }

        let ticks = self.simulation.tick().saturating_sub(self.tps_window_tick);
        self.ticks_per_second = ticks as f64 / elapsed.as_secs_f64();
        self.tps_window_start = Instant::now();
        self.tps_window_tick = self.simulation.tick();
    }
}

/// Runs the background loop that owns the managed simulation instance.
fn worker_loop(
    mut command_rx: mpsc::Receiver<Command>,
    frame_tx: watch::Sender<Option<FramePayload>>,
    metrics_tx: watch::Sender<Option<MetricsPayload>>,
    destroy_tx: broadcast::Sender<()>,
) {
    let mut simulation = None::<ManagedSimulation>;

    loop {
        if matches!(
            simulation.as_ref().map(|sim| sim.lifecycle),
            Some(SimulationLifecycle::Running)
        ) {
            match command_rx.try_recv() {
                Ok(command) => handle_command(
                    command,
                    &mut simulation,
                    &frame_tx,
                    &metrics_tx,
                    &destroy_tx,
                ),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    if let Some(sim) = simulation.as_mut() {
                        sim.tick_once(&frame_tx, &metrics_tx);
                    }
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        } else {
            let Some(command) = command_rx.blocking_recv() else {
                break;
            };
            handle_command(
                command,
                &mut simulation,
                &frame_tx,
                &metrics_tx,
                &destroy_tx,
            );
        }
    }
}

/// Applies one controller command to the managed simulation state.
fn handle_command(
    command: Command,
    simulation: &mut Option<ManagedSimulation>,
    frame_tx: &watch::Sender<Option<FramePayload>>,
    metrics_tx: &watch::Sender<Option<MetricsPayload>>,
    destroy_tx: &broadcast::Sender<()>,
) {
    match command {
        Command::Create {
            config,
            response_tx,
        } => {
            let response = if simulation.is_some() {
                Err(ControllerError::SimAlreadyExists)
            } else {
                match ManagedSimulation::new(config) {
                    Ok(sim) => {
                        let response = sim.create_response();
                        sim.publish(frame_tx, metrics_tx);
                        *simulation = Some(sim);
                        Ok(response)
                    }
                    Err(err) => Err(err),
                }
            };
            let _ = response_tx.send(response);
        }
        Command::Status(response_tx) => {
            let _ = response_tx.send(
                simulation
                    .as_ref()
                    .map(ManagedSimulation::status_response)
                    .ok_or(ControllerError::NoSim),
            );
        }
        Command::Config(response_tx) => {
            let _ = response_tx.send(
                simulation
                    .as_ref()
                    .map(|sim| sim.config.clone())
                    .ok_or(ControllerError::NoSim),
            );
        }
        Command::Start(response_tx) => {
            let result = match simulation.as_mut() {
                Some(sim) if sim.lifecycle == SimulationLifecycle::Created => {
                    sim.lifecycle = SimulationLifecycle::Running;
                    sim.reset_tps();
                    Ok(sim.status_response())
                }
                Some(sim) if sim.lifecycle == SimulationLifecycle::Running => {
                    Err(ControllerError::SimNotCreated)
                }
                Some(_) => Err(ControllerError::SimNotCreated),
                None => Err(ControllerError::NoSim),
            };
            let _ = response_tx.send(result);
        }
        Command::Pause(response_tx) => {
            let result = match simulation.as_mut() {
                Some(sim) if sim.lifecycle == SimulationLifecycle::Running => {
                    sim.lifecycle = SimulationLifecycle::Paused;
                    sim.reset_tps();
                    Ok(sim.status_response())
                }
                Some(sim) if sim.lifecycle == SimulationLifecycle::Paused => {
                    Ok(sim.status_response())
                }
                Some(_) => Err(ControllerError::SimNotRunning),
                None => Err(ControllerError::NoSim),
            };
            let _ = response_tx.send(result);
        }
        Command::Resume(response_tx) => {
            let result = match simulation.as_mut() {
                Some(sim) if sim.lifecycle == SimulationLifecycle::Paused => {
                    sim.lifecycle = SimulationLifecycle::Running;
                    sim.reset_tps();
                    Ok(sim.status_response())
                }
                Some(_) => Err(ControllerError::SimNotPaused),
                None => Err(ControllerError::NoSim),
            };
            let _ = response_tx.send(result);
        }
        Command::Step { count, response_tx } => {
            let result = match simulation.as_mut() {
                Some(_) if count == 0 => Err(ControllerError::BadRequest(
                    "step count must be greater than zero".to_owned(),
                )),
                Some(sim)
                    if sim.lifecycle == SimulationLifecycle::Paused
                        || sim.lifecycle == SimulationLifecycle::Created =>
                {
                    sim.reset_tps();
                    for _ in 0..count {
                        sim.tick_once(frame_tx, metrics_tx);
                    }
                    sim.lifecycle = SimulationLifecycle::Paused;
                    Ok(sim.status_response())
                }
                Some(_) => Err(ControllerError::SimNotPaused),
                None => Err(ControllerError::NoSim),
            };
            let _ = response_tx.send(result);
        }
        Command::Reset(response_tx) => {
            let result = match simulation.take() {
                Some(existing) => match ManagedSimulation::new(existing.config.clone()) {
                    Ok(sim) => {
                        let response = sim.status_response();
                        sim.publish(frame_tx, metrics_tx);
                        *simulation = Some(sim);
                        Ok(response)
                    }
                    Err(err) => Err(err),
                },
                None => Err(ControllerError::NoSim),
            };
            let _ = response_tx.send(result);
        }
        Command::Destroy(response_tx) => {
            let result = if simulation.take().is_some() {
                let _ = frame_tx.send(None);
                let _ = metrics_tx.send(None);
                let _ = destroy_tx.send(());
                Ok(())
            } else {
                Err(ControllerError::NoSim)
            };
            let _ = response_tx.send(result);
        }
        Command::Metrics(response_tx) => {
            let _ = response_tx.send(
                simulation
                    .as_ref()
                    .map(|sim| sim.latest_metrics.clone())
                    .ok_or(ControllerError::NoSim),
            );
        }
        Command::InspectCellByIndex { index, response_tx } => {
            let result = simulation
                .as_ref()
                .ok_or(ControllerError::NoSim)
                .and_then(|sim| inspect_cell_by_index(sim, index));
            let _ = response_tx.send(result);
        }
        Command::InspectCellByCoordinates { x, y, response_tx } => {
            let result = simulation
                .as_ref()
                .ok_or(ControllerError::NoSim)
                .and_then(|sim| inspect_cell_by_coordinates(sim, x, y));
            let _ = response_tx.send(result);
        }
        Command::InspectRegion {
            x,
            y,
            w,
            h,
            response_tx,
        } => {
            let result = simulation
                .as_ref()
                .ok_or(ControllerError::NoSim)
                .and_then(|sim| inspect_region_cells(sim, x, y, w, h));
            let _ = response_tx.send(result);
        }
    }
}

/// Inspects one cell by flat index with bounds checking.
fn inspect_cell_by_index(
    simulation: &ManagedSimulation,
    index: usize,
) -> Result<CellInspection, ControllerError> {
    if index >= simulation.simulation.grid().len() {
        return Err(ControllerError::CellOutOfBounds);
    }

    Ok(inspect_cell(simulation.simulation.grid(), index))
}

/// Inspects one cell by coordinates with bounds checking.
fn inspect_cell_by_coordinates(
    simulation: &ManagedSimulation,
    x: u32,
    y: u32,
) -> Result<CellInspection, ControllerError> {
    if x >= simulation.simulation.grid().width() || y >= simulation.simulation.grid().height() {
        return Err(ControllerError::CellOutOfBounds);
    }

    let index = simulation.simulation.grid().index(x, y);
    Ok(inspect_cell(simulation.simulation.grid(), index))
}

/// Inspects one bounded cell rectangle with validation.
fn inspect_region_cells(
    simulation: &ManagedSimulation,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Result<Vec<CellInspection>, ControllerError> {
    if w == 0 || h == 0 {
        return Err(ControllerError::BadRequest(
            "region width and height must be greater than zero".to_owned(),
        ));
    }

    let area = w
        .checked_mul(h)
        .ok_or_else(|| ControllerError::BadRequest("region dimensions overflow".to_owned()))?;
    if area > 100 {
        return Err(ControllerError::RegionTooLarge { area });
    }

    let max_x = x.checked_add(w).ok_or(ControllerError::CellOutOfBounds)?;
    let max_y = y.checked_add(h).ok_or(ControllerError::CellOutOfBounds)?;
    if max_x > simulation.simulation.grid().width() || max_y > simulation.simulation.grid().height()
    {
        return Err(ControllerError::CellOutOfBounds);
    }

    Ok(inspect_region(simulation.simulation.grid(), x, y, w, h))
}

/// Seeds the initial programs requested by the API config into the grid.
fn apply_seed_programs(
    simulation: &mut Simulation,
    config: &SimulationConfig,
) -> Result<(), String> {
    for seed_program in &config.seed_programs {
        let index = simulation.grid().index(seed_program.x, seed_program.y);
        let mut rng = cell_rng(config.seed ^ SEED_PROGRAM_SALT, 0, index as u64);
        let dir = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
        let id = rng.next_u32() as u8;

        let program =
            Program::new_live(seed_program.code.clone(), dir, id).map_err(|err| err.to_string())?;
        let cell = simulation
            .grid_mut()
            .get_mut(index)
            .expect("seeded cell should exist");
        cell.program = Some(program);
        cell.free_energy = seed_program.free_energy;
        cell.free_mass = seed_program.free_mass;
    }

    Ok(())
}

/// Converts engine construction failures into controller-layer errors.
fn simulation_error(error: SimulationError) -> ControllerError {
    match error {
        SimulationError::InvalidConfig(err) => ControllerError::InvalidConfig(err.to_string()),
        other => ControllerError::Internal(other.to_string()),
    }
}
