use std::collections::HashSet;

use crate::grid::Grid;
use crate::model::{Cell, Direction, Program};
use crate::opcode::Opcode;
use crate::simulation::TickReport;

#[cfg_attr(feature = "web", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct MetricsSnapshot {
    pub tick: u64,
    pub population: u32,
    pub live_count: u32,
    pub inert_count: u32,
    pub total_energy: u64,
    pub total_mass: u64,
    pub mean_program_size: f64,
    pub max_program_size: u32,
    pub unique_genomes: u32,
    pub births: u32,
    pub deaths: u32,
    pub mutations: u32,
}

#[cfg_attr(feature = "web", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellInspection {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
    pub program: Option<ProgramInspection>,
}

#[cfg_attr(feature = "web", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramInspection {
    pub code: Vec<u8>,
    pub disassembly: Vec<String>,
    pub size: u16,
    pub live: bool,
    pub age: u32,
    pub ip: u16,
    pub src: u16,
    pub dst: u16,
    pub dir: u8,
    pub flag: bool,
    pub msg: i16,
    pub id: u8,
    pub lc: i16,
    pub stack: Vec<i16>,
    pub abandonment_timer: Option<u32>,
}

pub fn collect_metrics(grid: &Grid, tick: u64, report: TickReport) -> MetricsSnapshot {
    let mut live_count = 0_u32;
    let mut inert_count = 0_u32;
    let mut total_energy = 0_u64;
    let mut total_mass = 0_u64;
    let mut live_program_size_sum = 0_u64;
    let mut max_program_size = 0_u32;
    let mut genomes = HashSet::<&[u8]>::new();

    for cell in grid.cells() {
        total_energy += u64::from(cell.free_energy) + u64::from(cell.bg_radiation);
        total_mass += u64::from(cell.free_mass) + u64::from(cell.bg_mass);

        let Some(program) = cell.program.as_ref() else {
            continue;
        };

        let size = u64::from(program.size());
        total_mass += size;

        if program.live {
            live_count += 1;
            live_program_size_sum += size;
            max_program_size = max_program_size.max(size as u32);
            let _ = genomes.insert(program.code.as_slice());
        } else {
            inert_count += 1;
        }
    }

    let mean_program_size = if live_count == 0 {
        0.0
    } else {
        live_program_size_sum as f64 / f64::from(live_count)
    };

    MetricsSnapshot {
        tick,
        population: live_count + inert_count,
        live_count,
        inert_count,
        // API-SPEC §10 currently defines totals over cell-local pools only.
        total_energy,
        total_mass,
        mean_program_size,
        max_program_size,
        unique_genomes: genomes.len() as u32,
        births: report.births,
        deaths: report.deaths,
        mutations: report.mutations,
    }
}

pub fn encode_grid_frame(grid: &Grid, tick: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + (grid.len() * 8));
    bytes.extend_from_slice(&tick.to_le_bytes());
    bytes.extend_from_slice(&grid.width().to_le_bytes());
    bytes.extend_from_slice(&grid.height().to_le_bytes());

    for cell in grid.cells() {
        let (flags, program_id, program_size) = cell_view_program_fields(cell);
        bytes.push(flags);
        bytes.push(program_id);
        bytes.push(program_size);
        bytes.push(clamp_to_u8(cell.free_energy));
        bytes.push(clamp_to_u8(cell.free_mass));
        bytes.push(clamp_to_u8(cell.bg_radiation));
        bytes.push(clamp_to_u8(cell.bg_mass));
        bytes.push(0);
    }

    bytes
}

pub fn inspect_cell(grid: &Grid, index: usize) -> CellInspection {
    let cell = grid.get(index).expect("cell index must be in bounds");

    CellInspection {
        index,
        x: grid.x(index),
        y: grid.y(index),
        free_energy: cell.free_energy,
        free_mass: cell.free_mass,
        bg_radiation: cell.bg_radiation,
        bg_mass: cell.bg_mass,
        program: cell.program.as_ref().map(program_inspection),
    }
}

pub fn inspect_region(grid: &Grid, x: u32, y: u32, w: u32, h: u32) -> Vec<CellInspection> {
    let mut cells = Vec::with_capacity((w * h) as usize);

    for row in y..(y + h) {
        for column in x..(x + w) {
            let index = grid.index(column, row);
            cells.push(inspect_cell(grid, index));
        }
    }

    cells
}

pub fn disassemble(code: &[u8]) -> Vec<String> {
    code.iter()
        .map(|byte| opcode_mnemonic(Opcode::decode(*byte)))
        .collect()
}

fn cell_view_program_fields(cell: &Cell) -> (u8, u8, u8) {
    match cell.program.as_ref() {
        Some(program) => {
            let mut flags = 0b001;
            if program.live {
                flags |= 0b010;
            }
            if program.tick.is_open {
                flags |= 0b100;
            }

            (
                flags,
                program.registers.id,
                ((u32::from(program.size()) / 128).min(255)) as u8,
            )
        }
        None => (0b100, 0, 0),
    }
}

fn clamp_to_u8(value: u32) -> u8 {
    value.min(u32::from(u8::MAX)) as u8
}

fn program_inspection(program: &Program) -> ProgramInspection {
    ProgramInspection {
        code: program.code.clone(),
        disassembly: disassemble(&program.code),
        size: program.size(),
        live: program.live,
        age: program.age,
        ip: program.registers.ip,
        src: program.registers.src,
        dst: program.registers.dst,
        dir: api_direction(program.registers.dir),
        flag: program.registers.flag,
        msg: program.registers.msg,
        id: program.registers.id,
        lc: program.registers.lc,
        stack: program.stack.clone(),
        abandonment_timer: if program.live {
            None
        } else {
            Some(program.abandonment_timer)
        },
    }
}

fn api_direction(direction: Direction) -> u8 {
    // API-SPEC §12 uses North/East/South/West numbering, while the engine stores
    // Right/Up/Left/Down. Keep the transport mapping local to the API surface.
    match direction {
        Direction::Up => 0,
        Direction::Right => 1,
        Direction::Down => 2,
        Direction::Left => 3,
    }
}

fn opcode_mnemonic(opcode: Opcode) -> String {
    match opcode {
        Opcode::PushLiteral(value) => format!("push {value}"),
        Opcode::Dup => "dup".to_owned(),
        Opcode::Drop => "drop".to_owned(),
        Opcode::Swap => "swap".to_owned(),
        Opcode::Over => "over".to_owned(),
        Opcode::Rand => "rand".to_owned(),
        Opcode::Add => "add".to_owned(),
        Opcode::Sub => "sub".to_owned(),
        Opcode::Neg => "neg".to_owned(),
        Opcode::Eq => "eq".to_owned(),
        Opcode::Lt => "lt".to_owned(),
        Opcode::Gt => "gt".to_owned(),
        Opcode::Not => "not".to_owned(),
        Opcode::And => "and".to_owned(),
        Opcode::Or => "or".to_owned(),
        Opcode::For => "for".to_owned(),
        Opcode::Next => "next".to_owned(),
        Opcode::Jmp => "jmp".to_owned(),
        Opcode::JmpNz => "jmpNz".to_owned(),
        Opcode::JmpZ => "jmpZ".to_owned(),
        Opcode::Cw => "cw".to_owned(),
        Opcode::Ccw => "ccw".to_owned(),
        Opcode::GetSize => "getSize".to_owned(),
        Opcode::GetIp => "getIp".to_owned(),
        Opcode::GetFlag => "getFlag".to_owned(),
        Opcode::GetMsg => "getMsg".to_owned(),
        Opcode::GetId => "getId".to_owned(),
        Opcode::GetSrc => "getSrc".to_owned(),
        Opcode::GetDst => "getDst".to_owned(),
        Opcode::SetDir => "setDir".to_owned(),
        Opcode::SetSrc => "setSrc".to_owned(),
        Opcode::SetDst => "setDst".to_owned(),
        Opcode::SetId => "setId".to_owned(),
        Opcode::GetE => "getE".to_owned(),
        Opcode::GetM => "getM".to_owned(),
        Opcode::Nop => "nop".to_owned(),
        Opcode::Absorb => "absorb".to_owned(),
        Opcode::Listen => "listen".to_owned(),
        Opcode::Collect => "collect".to_owned(),
        Opcode::Emit => "emit".to_owned(),
        Opcode::Read => "read".to_owned(),
        Opcode::Write => "write".to_owned(),
        Opcode::Del => "del".to_owned(),
        Opcode::Synthesize => "synthesize".to_owned(),
        Opcode::SenseSize => "senseSize".to_owned(),
        Opcode::SenseE => "senseE".to_owned(),
        Opcode::SenseM => "senseM".to_owned(),
        Opcode::SenseId => "senseId".to_owned(),
        Opcode::ReadAdj => "readAdj".to_owned(),
        Opcode::WriteAdj => "writeAdj".to_owned(),
        Opcode::AppendAdj => "appendAdj".to_owned(),
        Opcode::DelAdj => "delAdj".to_owned(),
        Opcode::GiveE => "giveE".to_owned(),
        Opcode::GiveM => "giveM".to_owned(),
        Opcode::Move => "move".to_owned(),
        Opcode::Boot => "boot".to_owned(),
        Opcode::NoOp(byte) => format!("noop 0x{byte:02x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_metrics, disassemble, encode_grid_frame, inspect_cell};
    use crate::model::{Cell, Direction, Program};
    use crate::simulation::TickReport;
    use crate::Grid;

    #[test]
    fn frame_encoding_uses_specified_header_and_cell_layout() {
        let mut cells = vec![Cell::default(), Cell::default()];
        let mut program =
            Program::new_live(vec![0x50, 0x64], Direction::Up, 7).expect("program should build");
        program.tick.is_open = true;
        cells[0].program = Some(program);
        cells[0].free_energy = 300;
        cells[0].bg_mass = 2;

        let grid = Grid::from_cells(2, 1, cells).expect("grid should build");
        let frame = encode_grid_frame(&grid, 9);

        assert_eq!(&frame[0..8], &9_u64.to_le_bytes());
        assert_eq!(&frame[8..12], &2_u32.to_le_bytes());
        assert_eq!(&frame[12..16], &1_u32.to_le_bytes());
        assert_eq!(frame.len(), 32);
        assert_eq!(&frame[16..24], &[0b111, 7, 0, 255, 0, 0, 2, 0]);
        assert_eq!(&frame[24..32], &[0b100, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn cell_inspection_uses_api_direction_encoding() {
        let mut grid = Grid::new(1, 1).expect("grid should build");
        let program =
            Program::new_live(vec![0x50], Direction::Up, 2).expect("program should build");
        grid.get_mut(0).expect("cell should exist").program = Some(program);

        let cell = inspect_cell(&grid, 0);
        assert_eq!(cell.program.expect("program should exist").dir, 0);
    }

    #[test]
    fn metrics_use_live_programs_for_size_statistics() {
        let mut cells = vec![Cell::default(), Cell::default()];
        cells[0].program =
            Some(Program::new_live(vec![0x50, 0x51], Direction::Right, 1).expect("live program"));
        cells[1].program =
            Some(Program::new_inert(vec![0x50], Direction::Right, 2).expect("inert program"));
        cells[0].free_energy = 3;
        cells[1].bg_radiation = 4;

        let grid = Grid::from_cells(2, 1, cells).expect("grid should build");
        let metrics = collect_metrics(
            &grid,
            5,
            TickReport {
                births: 1,
                deaths: 2,
                mutations: 3,
            },
        );

        assert_eq!(metrics.population, 2);
        assert_eq!(metrics.live_count, 1);
        assert_eq!(metrics.inert_count, 1);
        assert_eq!(metrics.total_energy, 7);
        assert_eq!(metrics.total_mass, 3);
        assert_eq!(metrics.mean_program_size, 2.0);
        assert_eq!(metrics.max_program_size, 2);
        assert_eq!(metrics.unique_genomes, 1);
        assert_eq!(metrics.births, 1);
        assert_eq!(metrics.deaths, 2);
        assert_eq!(metrics.mutations, 3);
    }

    #[test]
    fn disassembly_uses_api_facing_mnemonics() {
        assert_eq!(
            disassemble(&[0x00, 0x40, 0x4a, 0x5f, 0xff]),
            vec!["push 0", "cw", "setSrc", "appendAdj", "noop 0xff"]
        );
    }
}
