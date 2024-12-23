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

    // Create simulation parameters
    let params = WorldParams {
        grid_width: args.grid_size,
        grid_height: args.grid_size,
        ..Default::default()
    };

    // Initialize simulation
    info!("Initializing Simulation struct");
    let mut simulation = Simulation::new(params);

    // Calculate tick duration if TPS is specified
    let tick_duration = if args.tps > 0 {
        Some(Duration::from_secs_f64(1.0 / args.tps as f64))
    } else {
        None
    };

    // Main simulation loop
    let mut tick_count = 0;
    loop {
        simulation.tick();
        tick_count += 1;

        if tick_count % 1000 == 0 {
            debug!("Tick count: {}", tick_count);
        }

        // Apply TPS limiting if specified
        if let Some(duration) = tick_duration {
            sleep(duration);
        }
    }
}
