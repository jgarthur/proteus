//! Runs one world and prints how it changes.

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use proteus_mini::observe::{picture, Census};
use proteus_mini::{code, Laws, Start, Tally, World};

const USAGE: &str = "\
Usage: proteus-mini [options]

Laws of the world:
  --width N           grid width in cells                            [64]
  --height N          grid height in cells                           [64]
  --energy-rate R     mean energy arriving per cell per tick         [0.25]
  --mass-rate R       mean mass arriving per cell per tick           [0.05]
  --decay-log2 K      loose energy and mass dissipate at 2^-K        [7]
  --upkeep-log2 K     each instruction costs 1 energy at 2^-K        [7]
  --slip-log2 K       an executed instruction mutates at 2^-K        [10]
  --miscopy-log2 K    a copy goes wrong at 2^-K                      [7]
  --seed N            all randomness                                 [1]
A rate K may also be `off`.

How it begins (instructions: A absorb, C collect, K copy, T turn, L look,
W weigh, D dec, S skip, . no-op):
  --start CODE        one program in the middle of the world         [ACKLST]
  --soup N            instead, random code of length N in every cell

Running and watching:
  --ticks N           ticks to run                                   [50000]
  --sample N          ticks between measurements                     [2500]
  --frames DIR        write a PNG of the grid at each measurement
  --scale N           pixels per cell in those PNGs                  [6]
";

struct Options {
    laws: Laws,
    start: Start,
    ticks: u64,
    sample: u64,
    frames: Option<PathBuf>,
    scale: usize,
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
        laws: Laws::default(),
        start: Start::Seed(code::parse("ACKLST")?),
        ticks: 50_000,
        sample: 2_500,
        frames: None,
        scale: 6,
    };
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or(format!("{flag} needs a value\n\n{USAGE}"))?;
        let laws = &mut options.laws;
        match flag.as_str() {
            "--width" => laws.width = number(&flag, &value)?,
            "--height" => laws.height = number(&flag, &value)?,
            "--energy-rate" => laws.energy_rate = number(&flag, &value)?,
            "--mass-rate" => laws.mass_rate = number(&flag, &value)?,
            "--decay-log2" => laws.decay_log2 = rate(&flag, &value)?,
            "--upkeep-log2" => laws.upkeep_log2 = rate(&flag, &value)?,
            "--slip-log2" => laws.slip_log2 = rate(&flag, &value)?,
            "--miscopy-log2" => laws.miscopy_log2 = rate(&flag, &value)?,
            "--seed" => laws.seed = number(&flag, &value)?,
            "--start" => options.start = Start::Seed(code::parse(&value)?),
            "--soup" => options.start = Start::Soup(number(&flag, &value)?),
            "--ticks" => options.ticks = number(&flag, &value)?,
            "--sample" => options.sample = number::<u64>(&flag, &value)?.max(1),
            "--frames" => options.frames = Some(value.into()),
            "--scale" => options.scale = number::<usize>(&flag, &value)?.max(1),
            _ => return Err(format!("unknown option {flag}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn number<T: FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} cannot take the value {value:?}"))
}

fn rate(flag: &str, value: &str) -> Result<Option<u32>, String> {
    match value {
        "off" => Ok(None),
        _ => number(flag, value).map(Some),
    }
}

fn run(options: Options) -> Result<(), String> {
    let mut world = World::new(options.laws.clone(), options.start.clone())?;
    if let Some(dir) = &options.frames {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    println!("# {:?}", options.laws);
    println!(
        "tick\tlive\tunfinished\twhole\tsquatters\tstarving\tbirths\tdeaths\t\
         lifespan\tchildren\tsize\tgeneration\tspecies\ttop_share"
    );
    let mut window = Tally::default();
    let census = loop {
        let census = Census::of(&world);
        let per_death = |total: u64| total as f64 / window.deaths.max(1) as f64;
        let top = census.species.first().map_or(0, |(_, count)| *count);
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.2}\t{:.2}\t{:.1}\t{}\t{:.3}",
            world.tick(),
            census.live,
            census.unfinished,
            census.whole,
            census.squatters,
            census.starving,
            window.births,
            window.deaths,
            per_death(window.lifespans),
            per_death(window.offspring),
            census.size,
            census.generation,
            census.species.len(),
            top as f64 / census.whole.max(1) as f64,
        );
        if let Some(dir) = &options.frames {
            let path = dir.join(format!("{:08}.png", world.tick()));
            std::fs::write(&path, picture(&world, options.scale))
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
        if world.tick() >= options.ticks {
            break census;
        }

        window = Tally::default();
        for _ in 0..options.sample.min(options.ticks - world.tick()) {
            window += world.step();
        }
    };

    println!("# most common whole programs, as share of whole programs:");
    for (spelling, count) in census.species.iter().take(8) {
        let share = *count as f64 / census.whole.max(1) as f64;
        println!("#   {share:.3}  {spelling}");
    }
    println!("# digest {:016x}", world.digest());
    Ok(())
}
