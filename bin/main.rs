use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use log::{debug, info};
use proteus::simulation::Simulation;
use proteus::world::WorldParams;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Size of the simulation grid (width and height)
    #[arg(long, default_value_t = WorldParams::default().grid_width)]
    grid_size: i32,

    /// Ticks per second. 0 means no limiting.
    #[arg(long, default_value_t = 0)]
    tps: u32,

    /// Random number generator seed. 0 means use default.
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Maximum number of ticks to run. 0 means unlimited.
    #[arg(long, default_value_t = 0)]
    max_ticks: u64,

    /// Number of threads to use for parallel processing. Defaults to the number of logical cores.
    #[arg(long)]
    threads: Option<usize>,
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        if cfg!(debug_assertions) {
            std::env::set_var("RUST_LOG", "info");
        } else {
            std::env::set_var("RUST_LOG", "warn");
        }
    }
    env_logger::init();

    let args = Args::parse();

    let params = WorldParams {
        grid_width: args.grid_size,
        grid_height: args.grid_size,
        rng_seed: args.seed,
        ..Default::default()
    };
    let tick_duration = if args.tps > 0 {
        Some(Duration::from_secs_f64(1.0 / args.tps as f64))
    } else {
        None
    };

    match args.threads {
        Some(threads) => rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Failed to initialize thread pool"),
        None => rayon::ThreadPoolBuilder::new()
            .build_global()
            .expect("Failed to initialize thread pool"),
    };
    info!(
        "Rayon thread pool initialized with {} threads",
        rayon::current_num_threads(),
    );

    info!("Initializing Simulation struct");
    let mut simulation = Simulation::new(params);
    let mut tick_count = 0;

    info!("Starting simulation loop");
    loop {
        simulation.tick();

        tick_count += 1;
        if tick_count % 1000 == 0 {
            debug!("Tick count: {}", tick_count);
        }
        if args.max_ticks > 0 && tick_count >= args.max_ticks {
            info!("Reached maximum tick count of {}", args.max_ticks);
            break;
        }
        if let Some(duration) = tick_duration {
            sleep(duration);
        }
    }
}
