use crate::config::SimConfig;
use crate::grid::Grid;
use crate::model::{Cell, Direction, Packet, Program};
use crate::random::{binomial, cell_rng};

const LISTEN_CAPTURE_SALT: u64 = 0x5d17_2ef3_94ab_c881;
const BG_RADIATION_SALT: u64 = 0x1f03_86da_b9c7_e251;
const BG_MASS_SALT: u64 = 0x2c69_4ab1_78de_3f44;
const MAINTENANCE_SALT: u64 = 0x78d2_0a45_4ecb_911f;
const DECAY_SALT: u64 = 0x42f5_c1a9_203d_b665;
const SPAWN_SALT: u64 = 0xbfd1_6a70_531c_2e84;
const MUTATION_SALT: u64 = 0xe3b9_1d8c_7a4f_5012;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass3AmbientOutput {
    pub spawn_candidates: Vec<bool>,
}

impl Pass3AmbientOutput {
    pub fn new(cell_count: usize) -> Self {
        Self {
            spawn_candidates: vec![false; cell_count],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pass3TailContext<'a> {
    pub existed_set: &'a [bool],
    pub live_set: &'a [bool],
    pub incoming_writes: &'a [bool],
    pub spawn_candidates: &'a [bool],
    pub config: &'a SimConfig,
    pub tick: u64,
    pub seed: u64,
}

pub fn pass3_packets(grid: &mut Grid, packets: &mut Vec<Packet>, tick: u64, seed: u64) {
    if packets.is_empty() {
        return;
    }

    for packet in packets.iter_mut() {
        packet.position = grid.neighbor(packet.position, packet.direction);
    }

    let mut buckets = vec![Vec::<Packet>::new(); grid.len()];
    for packet in packets.drain(..) {
        buckets[packet.position].push(packet);
    }

    let mut survivors = Vec::new();
    for (cell_index, bucket) in buckets.into_iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }

        let cell = grid.get_mut(cell_index).expect("cell should exist");
        if program(cell).is_some_and(|program| program.tick.did_listen) {
            apply_listen_capture(cell, cell_index, &bucket, tick, seed);
            continue;
        }

        if bucket.len() >= 2 {
            cell.free_energy +=
                u32::try_from(bucket.len()).expect("packet count should fit in u32");
            continue;
        }

        survivors.push(bucket[0]);
    }

    *packets = survivors;
}

pub fn pass3_ambient(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
) -> Pass3AmbientOutput {
    let mut output = Pass3AmbientOutput::new(grid.len());

    resolve_absorb(grid);
    resolve_background_radiation(grid, config, tick, seed);
    resolve_collect(grid);
    resolve_background_mass(grid, config, tick, seed, &mut output);

    output
}

pub fn pass3_tail(grid: &mut Grid, context: Pass3TailContext<'_>) {
    assert_eq!(
        grid.len(),
        context.existed_set.len(),
        "existed-set length must match grid size"
    );
    assert_eq!(
        grid.len(),
        context.live_set.len(),
        "live-set length must match grid size"
    );
    assert_eq!(
        grid.len(),
        context.incoming_writes.len(),
        "incoming-write length must match grid size"
    );
    assert_eq!(
        grid.len(),
        context.spawn_candidates.len(),
        "spawn-candidate length must match grid size"
    );

    resolve_inert_lifecycle(grid, context.incoming_writes);
    resolve_maintenance(
        grid,
        context.existed_set,
        context.config,
        context.tick,
        context.seed,
    );
    resolve_free_resource_decay(grid, context.config, context.tick, context.seed);
    resolve_age_update(grid, context.live_set);
    resolve_spontaneous_creation(
        grid,
        context.spawn_candidates,
        context.config,
        context.tick,
        context.seed,
    );
}

pub fn mutate_end_of_tick(
    grid: &mut Grid,
    live_set: &[bool],
    config: &SimConfig,
    tick: u64,
    seed: u64,
) {
    assert_eq!(
        grid.len(),
        live_set.len(),
        "live-set length must match grid size"
    );

    for (cell_index, is_live_at_tick_start) in live_set.iter().copied().enumerate() {
        if !is_live_at_tick_start {
            continue;
        }

        let Some(program) = grid
            .get(cell_index)
            .expect("cell should exist")
            .program
            .as_ref()
        else {
            continue;
        };

        let probability = mutation_probability(program, config);
        let mut rng = cell_rng(seed ^ MUTATION_SALT, tick, cell_index as u64);
        if !rng.bernoulli(probability) {
            continue;
        }

        let program = grid
            .get_mut(cell_index)
            .expect("cell should exist")
            .program
            .as_mut()
            .expect("program should still exist");
        let instruction_index = (rng.next_u64() % program.code.len() as u64) as usize;
        let bit_index = (rng.next_u64() % 8) as u8;
        program.code[instruction_index] ^= 1_u8 << bit_index;
    }
}

fn apply_listen_capture(
    cell: &mut Cell,
    cell_index: usize,
    packets: &[Packet],
    tick: u64,
    seed: u64,
) {
    let count = u32::try_from(packets.len()).expect("packet count should fit in u32");
    cell.free_energy += count;

    if packets.is_empty() {
        return;
    }

    let mut rng = cell_rng(seed ^ LISTEN_CAPTURE_SALT, tick, cell_index as u64);
    let choice = (rng.next_u64() % packets.len() as u64) as usize;
    let chosen = packets[choice];

    let program = program_mut(cell).expect("listening cell should contain a program");
    program.registers.msg = chosen.message;
    program.registers.dir = chosen.direction.opposite();
    program.registers.flag = true;
}

fn resolve_absorb(grid: &mut Grid) {
    let mut buckets = vec![Vec::<usize>::new(); grid.len()];
    for source in 0..grid.len() {
        let Some(program) = grid
            .get(source)
            .expect("cell should exist")
            .program
            .as_ref()
        else {
            continue;
        };
        if program.tick.absorb_count == 0 {
            continue;
        }

        let dir = program.tick.absorb_dir.unwrap_or(program.registers.dir);
        for footprint_cell in absorb_footprint(grid, source, program.tick.absorb_count, dir) {
            buckets[footprint_cell].push(source);
        }
    }

    let mut gains = vec![0_u32; grid.len()];
    for (cell_index, absorbers) in buckets.into_iter().enumerate() {
        if absorbers.is_empty() {
            continue;
        }

        let bg = grid
            .get(cell_index)
            .expect("cell should exist")
            .bg_radiation;
        if bg == 0 {
            continue;
        }

        let share = bg / u32::try_from(absorbers.len()).expect("absorber count should fit in u32");
        let remainder =
            bg % u32::try_from(absorbers.len()).expect("absorber count should fit in u32");
        if share > 0 {
            for absorber in absorbers {
                gains[absorber] += share;
            }
        }

        grid.get_mut(cell_index)
            .expect("cell should exist")
            .bg_radiation = remainder;
    }

    for (cell_index, gain) in gains.into_iter().enumerate() {
        if gain == 0 {
            continue;
        }
        grid.get_mut(cell_index)
            .expect("cell should exist")
            .free_energy += gain;
    }
}

fn resolve_background_radiation(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) {
    for cell_index in 0..grid.len() {
        let mut rng = cell_rng(seed ^ BG_RADIATION_SALT, tick, cell_index as u64);
        let current = grid
            .get(cell_index)
            .expect("cell should exist")
            .bg_radiation;
        let decayed = binomial(&mut rng, current, config.d_energy);
        let mut remaining = current - decayed;
        if rng.bernoulli(config.r_energy) {
            remaining += 1;
        }
        grid.get_mut(cell_index)
            .expect("cell should exist")
            .bg_radiation = remaining;
    }
}

fn resolve_collect(grid: &mut Grid) {
    for cell in grid.cells_mut() {
        if cell
            .program
            .as_ref()
            .is_some_and(|program| program.tick.did_collect)
        {
            cell.free_mass += cell.bg_mass;
            cell.bg_mass = 0;
        }
    }
}

fn resolve_background_mass(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    output: &mut Pass3AmbientOutput,
) {
    for cell_index in 0..grid.len() {
        let mut rng = cell_rng(seed ^ BG_MASS_SALT, tick, cell_index as u64);
        let current = grid.get(cell_index).expect("cell should exist").bg_mass;
        let decayed = binomial(&mut rng, current, config.d_mass);
        let mut remaining = current - decayed;
        let arrival = rng.bernoulli(config.r_mass);
        if arrival {
            remaining += 1;
        }

        let cell = grid.get_mut(cell_index).expect("cell should exist");
        cell.bg_mass = remaining;
        if arrival && !cell.has_program() {
            output.spawn_candidates[cell_index] = true;
        }
    }
}

fn resolve_inert_lifecycle(grid: &mut Grid, incoming_writes: &[bool]) {
    for (cell_index, cell) in grid.cells_mut().iter_mut().enumerate() {
        let Some(program) = cell.program.as_mut() else {
            continue;
        };
        if !program.is_inert() {
            continue;
        }

        if incoming_writes[cell_index] {
            program.abandonment_timer = 0;
        } else {
            program.abandonment_timer = program.abandonment_timer.wrapping_add(1);
        }
        program.tick.is_open = true;
    }
}

fn resolve_maintenance(
    grid: &mut Grid,
    existed_set: &[bool],
    config: &SimConfig,
    tick: u64,
    seed: u64,
) {
    for (cell_index, existed_at_tick_start) in existed_set.iter().copied().enumerate() {
        if !existed_at_tick_start {
            continue;
        }

        let Some(program) = grid
            .get(cell_index)
            .expect("cell should exist")
            .program
            .as_ref()
        else {
            continue;
        };
        if program.tick.is_newborn {
            continue;
        }

        let rate = if program.live {
            config.maintenance_rate
        } else if program.abandonment_timer < config.inert_grace_ticks {
            0.0
        } else {
            config.maintenance_rate
        };
        if rate <= 0.0 {
            continue;
        }

        let q = f64::from(program.size()).powf(config.maintenance_exponent);
        let whole = q.floor() as u32;
        let fractional = (q - f64::from(whole)) * rate;
        let mut rng = cell_rng(seed ^ MAINTENANCE_SALT, tick, cell_index as u64);
        let mut quanta = binomial(&mut rng, whole, rate);
        quanta += u32::from(rng.bernoulli(fractional));

        if quanta == 0 {
            continue;
        }

        apply_maintenance(grid.get_mut(cell_index).expect("cell should exist"), quanta);
    }
}

fn apply_maintenance(cell: &mut Cell, mut quanta: u32) {
    let energy_paid = cell.free_energy.min(quanta);
    cell.free_energy -= energy_paid;
    quanta -= energy_paid;

    let mass_paid = cell.free_mass.min(quanta);
    cell.free_mass -= mass_paid;
    quanta -= mass_paid;

    if quanta == 0 {
        return;
    }

    let Some(program) = cell.program.as_mut() else {
        return;
    };
    let destroy = usize::try_from(quanta).expect("maintenance quanta should fit in usize");
    let new_len = program.code.len().saturating_sub(destroy);
    program.code.truncate(new_len);
    if program.code.is_empty() {
        cell.program = None;
    }
}

fn resolve_free_resource_decay(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) {
    for cell_index in 0..grid.len() {
        let mut rng = cell_rng(seed ^ DECAY_SALT, tick, cell_index as u64);
        let threshold =
            resource_threshold(grid.get(cell_index).expect("cell should exist"), config);

        let cell = grid.get_mut(cell_index).expect("cell should exist");
        let energy_excess = (f64::from(cell.free_energy) - threshold).max(0.0).floor() as u32;
        let energy_decay = binomial(&mut rng, energy_excess, config.d_energy);
        cell.free_energy -= energy_decay;

        let mass_excess = (f64::from(cell.free_mass) - threshold).max(0.0).floor() as u32;
        let mass_decay = binomial(&mut rng, mass_excess, config.d_mass);
        cell.free_mass -= mass_decay;
    }
}

fn resolve_age_update(grid: &mut Grid, live_set: &[bool]) {
    for (cell_index, was_live_at_tick_start) in live_set.iter().copied().enumerate() {
        if !was_live_at_tick_start {
            continue;
        }

        let Some(program) = grid
            .get_mut(cell_index)
            .expect("cell should exist")
            .program
            .as_mut()
        else {
            continue;
        };
        program.age = program.age.wrapping_add(1);
    }
}

fn resolve_spontaneous_creation(
    grid: &mut Grid,
    spawn_candidates: &[bool],
    config: &SimConfig,
    tick: u64,
    seed: u64,
) {
    for (cell_index, is_spawn_candidate) in spawn_candidates.iter().copied().enumerate() {
        if !is_spawn_candidate {
            continue;
        }

        let cell = grid.get(cell_index).expect("cell should exist");
        if cell.has_program() {
            continue;
        }

        let mut rng = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
        if !rng.bernoulli(config.p_spawn) {
            continue;
        }

        let dir = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
        let id = rng.next_u32() as u8;
        let mut program =
            Program::new_live(vec![0x50], dir, id).expect("spawned program should be valid");
        program.tick.is_newborn = true;

        let cell = grid.get_mut(cell_index).expect("cell should exist");
        cell.program = Some(program);
        cell.free_energy += cell.bg_radiation;
        cell.free_mass += cell.bg_mass;
        cell.bg_radiation = 0;
        cell.bg_mass = 0;
    }
}

fn resource_threshold(cell: &Cell, config: &SimConfig) -> f64 {
    let size = cell
        .program
        .as_ref()
        .map_or(0.0, |program| f64::from(program.size()));
    config.t_cap * size
}

fn mutation_probability(program: &Program, config: &SimConfig) -> f64 {
    if program.tick.bg_radiation_consumed > 0 {
        let denominator = 2_f64.powi(config.mutation_background_log2 as i32);
        (f64::from(program.tick.bg_radiation_consumed) / denominator).min(1.0)
    } else {
        2_f64.powi(-(config.mutation_base_log2 as i32))
    }
}

fn absorb_footprint(
    grid: &Grid,
    source: usize,
    count: u8,
    dir: crate::model::Direction,
) -> Vec<usize> {
    let mut cells = Vec::with_capacity(5);
    push_unique(&mut cells, source);
    if count >= 2 {
        push_unique(&mut cells, grid.neighbor(source, dir));
    }
    if count >= 3 {
        push_unique(&mut cells, grid.neighbor(source, dir.clockwise()));
        push_unique(&mut cells, grid.neighbor(source, dir.counterclockwise()));
    }
    if count >= 4 {
        push_unique(&mut cells, grid.neighbor(source, dir.opposite()));
    }
    cells
}

fn push_unique(cells: &mut Vec<usize>, value: usize) {
    if !cells.contains(&value) {
        cells.push(value);
    }
}

fn program(cell: &Cell) -> Option<&Program> {
    cell.program.as_ref()
}

fn program_mut(cell: &mut Cell) -> Option<&mut Program> {
    cell.program.as_mut()
}

#[cfg(test)]
mod tests {
    use crate::config::SimConfig;
    use crate::grid::Grid;
    use crate::model::{Cell, Direction, Packet, Program};

    use super::{pass3_ambient, pass3_packets, Pass3AmbientOutput};

    #[test]
    fn packet_phase_propagates_and_persists_single_packets() {
        let mut grid = Grid::new(3, 1).expect("grid should build");
        let mut packets = vec![Packet {
            position: 0,
            direction: Direction::Right,
            message: 7,
        }];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        assert_eq!(
            packets,
            vec![Packet {
                position: 1,
                direction: Direction::Right,
                message: 7,
            }]
        );
        assert_eq!(grid.get(1).expect("cell should exist").free_energy, 0);
    }

    #[test]
    fn collisions_convert_packets_to_free_energy() {
        let mut grid = Grid::new(3, 1).expect("grid should build");
        let mut packets = vec![
            Packet {
                position: 0,
                direction: Direction::Right,
                message: 1,
            },
            Packet {
                position: 2,
                direction: Direction::Left,
                message: 2,
            },
        ];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        assert!(packets.is_empty());
        assert_eq!(grid.get(1).expect("cell should exist").free_energy, 2);
    }

    #[test]
    fn listening_captures_packets_and_sets_message_and_arrival_direction() {
        let mut cell = Cell::with_program(
            Program::new_live(vec![0x52], Direction::Up, 4).expect("program should build"),
        );
        cell.program
            .as_mut()
            .expect("program should exist")
            .tick
            .did_listen = true;

        let mut grid =
            Grid::from_cells(2, 1, vec![Cell::default(), cell]).expect("grid should build");
        let mut packets = vec![Packet {
            position: 0,
            direction: Direction::Right,
            message: 9,
        }];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        let listener = grid.get(1).expect("cell should exist");
        let program = listener.program.as_ref().expect("program should exist");
        assert!(packets.is_empty());
        assert_eq!(listener.free_energy, 1);
        assert_eq!(program.registers.msg, 9);
        assert_eq!(program.registers.dir, Direction::Left);
        assert!(program.registers.flag);
    }

    #[test]
    fn absorb_distribution_splits_background_radiation_and_leaves_remainder() {
        let mut left = Cell::with_program(
            Program::new_live(vec![0x51], Direction::Right, 1).expect("program should build"),
        );
        let mut right = Cell::with_program(
            Program::new_live(vec![0x51], Direction::Left, 2).expect("program should build"),
        );
        left.program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_count = 2;
        left.program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_dir = Some(Direction::Right);
        right
            .program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_count = 2;
        right
            .program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_dir = Some(Direction::Left);

        let center = Cell {
            bg_radiation: 5,
            ..Cell::default()
        };

        let mut grid =
            Grid::from_cells(3, 1, vec![left, center, right]).expect("grid should build");
        let config = SimConfig {
            d_energy: 0.0,
            r_energy: 0.0,
            d_mass: 0.0,
            r_mass: 0.0,
            ..SimConfig::default()
        };

        let output = pass3_ambient(&mut grid, &config, 0, 5);

        assert_eq!(
            output,
            Pass3AmbientOutput {
                spawn_candidates: vec![false, false, false]
            }
        );
        assert_eq!(grid.get(0).expect("cell should exist").free_energy, 2);
        assert_eq!(grid.get(1).expect("cell should exist").bg_radiation, 1);
        assert_eq!(grid.get(2).expect("cell should exist").free_energy, 2);
    }
}
