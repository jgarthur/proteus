//! Runs one world and prints how it changes.

use std::path::PathBuf;
use std::process::ExitCode;

use proteus_mini::observe::{picture, Census};
use proteus_mini::{Head1, Rules, Schedule, Tick, World};

const USAGE: &str = "\
Usage: proteus-mini [options]

Laws of the world (defaults are BFF on a nearest-neighbour grid):
  --width N           grid width in cells                          [64]
  --height N          grid height in cells                         [64]
  --tape N            bytes per cell                               [64]
  --ops CHARS         enabled instructions, from <>{}-+.,[]:       [<>{}-+.,[]]
  --steps N           bytes a pair may read as code per tick       [8192]
  --light N           speed of light in cells per tick; 0 = none   [1]
  --schedule KIND     pairing: random | tiling                     [random]
  --head1 WHERE       head 1 starts on tape: first | second       [first]
  --mutation-log2 K   per-byte mutation chance 2^-K, or off        [12]
  --seed N            initial soup and all later randomness        [1]

Running and watching:
  --ticks N           ticks to run                                 [20000]
  --sample N          ticks between measurements                   [500]
  --after N           stop N ticks after takeoff instead
  --takeoff BITS      high-order entropy that counts as takeoff    [1.0]
  --frames DIR        write a PNG of the grid at each measurement
  --scale N           pixels per cell in those PNGs                [4]
  --dump DIR          write the raw tapes at each measurement
";

struct Options {
    rules: Rules,
    ticks: u64,
    sample: u64,
    after: Option<u64>,
    takeoff: f64,
    frames: Option<PathBuf>,
    scale: usize,
    dump: Option<PathBuf>,
}

fn main() -> ExitCode {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse(std::env::args().skip(1)).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        rules: Rules::default(),
        ticks: 20_000,
        sample: 500,
        after: None,
        takeoff: 1.0,
        frames: None,
        scale: 4,
        dump: None,
    };
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or(format!("{flag} needs a value\n\n{USAGE}"))?;
        let rules = &mut options.rules;
        match flag.as_str() {
            "--width" => rules.width = number(&flag, &value)?,
            "--height" => rules.height = number(&flag, &value)?,
            "--tape" => rules.tape = number(&flag, &value)?,
            "--ops" => rules.ops = value,
            "--steps" => rules.steps = number(&flag, &value)?,
            "--light" => rules.light = number(&flag, &value)?,
            "--schedule" => {
                rules.schedule = match value.as_str() {
                    "random" => Schedule::Random,
                    "tiling" => Schedule::Tiling,
                    _ => return Err(format!("--schedule is random or tiling, not {value:?}")),
                }
            }
            "--head1" => {
                rules.head1 = match value.as_str() {
                    "first" => Head1::First,
                    "second" => Head1::Second,
                    _ => return Err(format!("--head1 is first or second, not {value:?}")),
                }
            }
            "--mutation-log2" => {
                rules.mutation_log2 = match value.as_str() {
                    "off" => None,
                    _ => Some(number(&flag, &value)?),
                }
            }
            "--seed" => rules.seed = number(&flag, &value)?,
            "--ticks" => options.ticks = number(&flag, &value)?,
            "--sample" => options.sample = number::<u64>(&flag, &value)?.max(1),
            "--after" => options.after = Some(number(&flag, &value)?),
            "--takeoff" => options.takeoff = number(&flag, &value)?,
            "--frames" => options.frames = Some(value.into()),
            "--scale" => options.scale = number::<usize>(&flag, &value)?.max(1),
            "--dump" => options.dump = Some(value.into()),
            _ => return Err(format!("unknown option {flag}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} cannot take the value {value:?}"))
}

fn run(options: Options) -> Result<(), String> {
    let mut world = World::new(options.rules.clone())?;
    for dir in options.frames.iter().chain(&options.dump) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    println!("# {:?}", options.rules);
    println!("tick\tpairs\treads\tops\tshannon\tbrotli\thigh_order\tkinds\ttop_share");
    let mut takeoff = None;
    let (mut since_sample, mut window) = (Tick::default(), 1);
    let census = loop {
        let census = Census::of(&world);
        let per_pair = |count: u64| count as f64 / since_sample.pairs.max(1) as f64;
        println!(
            "{}\t{}\t{:.1}\t{:.1}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.3}",
            world.tick(),
            since_sample.pairs / window,
            per_pair(since_sample.reads),
            per_pair(since_sample.ops),
            census.shannon,
            census.compressed,
            census.high_order(),
            census.kinds.len(),
            census.top_share(),
        );
        let name = format!("{:08}", world.tick());
        if let Some(dir) = &options.frames {
            write(
                dir.join(format!("{name}.png")),
                &picture(&world, options.scale),
            )?;
        }
        if let Some(dir) = &options.dump {
            write(dir.join(format!("{name}.bin")), world.bytes())?;
        }

        if takeoff.is_none() && census.high_order() >= options.takeoff {
            takeoff = Some(world.tick());
        }
        let stop = match (options.after, takeoff) {
            (Some(after), Some(takeoff)) => (takeoff + after).min(options.ticks),
            _ => options.ticks,
        };
        if world.tick() >= stop {
            break census;
        }

        since_sample = Tick::default();
        window = options.sample.min(stop - world.tick());
        for _ in 0..window {
            let tick = world.step();
            since_sample.pairs += tick.pairs;
            since_sample.reads += tick.reads;
            since_sample.ops += tick.ops;
        }
    };

    match takeoff {
        Some(tick) => println!("# takeoff by tick {tick}"),
        None => println!("# no takeoff"),
    }
    println!("# most common programs, as share of cells and instruction skeleton:");
    for (skeleton, count) in census.kinds.iter().take(5) {
        let share = *count as f64 / world.cells() as f64;
        println!("#   {share:.3}  {skeleton}");
    }
    println!("# digest {:016x}", world.digest());
    Ok(())
}

fn write(path: PathBuf, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
