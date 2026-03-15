#![allow(dead_code, unused_macros)]

use proteus::{Cell, Direction, Grid, Program, SimConfig, Simulation, TickState};

#[derive(Clone, Debug)]
pub struct ProgramBuilder {
    code: Vec<u8>,
    ip: u16,
    dir: Direction,
    src: u16,
    dst: u16,
    flag: bool,
    msg: i16,
    id: u8,
    lc: i16,
    stack: Vec<i16>,
    live: bool,
    age: u32,
    abandonment_timer: u32,
    tick: TickState,
    free_energy: u32,
    free_mass: u32,
    bg_radiation: u32,
    bg_mass: u32,
}

impl Default for ProgramBuilder {
    fn default() -> Self {
        Self {
            code: vec![0x50],
            ip: 0,
            dir: Direction::Right,
            src: 0,
            dst: 0,
            flag: false,
            msg: 0,
            id: 0,
            lc: 0,
            stack: Vec::new(),
            live: true,
            age: 0,
            abandonment_timer: 0,
            tick: TickState::default(),
            free_energy: 0,
            free_mass: 0,
            bg_radiation: 0,
            bg_mass: 0,
        }
    }
}

impl ProgramBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn code(mut self, code: &[u8]) -> Self {
        self.code = code.to_vec();
        self
    }

    pub fn ip(mut self, ip: u16) -> Self {
        self.ip = ip;
        self
    }

    pub fn dir(mut self, dir: Direction) -> Self {
        self.dir = dir;
        self
    }

    pub fn src(mut self, src: u16) -> Self {
        self.src = src;
        self
    }

    pub fn dst(mut self, dst: u16) -> Self {
        self.dst = dst;
        self
    }

    pub fn flag(mut self, flag: bool) -> Self {
        self.flag = flag;
        self
    }

    pub fn msg(mut self, msg: i16) -> Self {
        self.msg = msg;
        self
    }

    pub fn id(mut self, id: u8) -> Self {
        self.id = id;
        self
    }

    pub fn lc(mut self, lc: i16) -> Self {
        self.lc = lc;
        self
    }

    pub fn stack(mut self, stack: &[i16]) -> Self {
        self.stack = stack.to_vec();
        self
    }

    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    pub fn age(mut self, age: u32) -> Self {
        self.age = age;
        self
    }

    pub fn abandonment_timer(mut self, abandonment_timer: u32) -> Self {
        self.abandonment_timer = abandonment_timer;
        self
    }

    pub fn newborn(mut self, is_newborn: bool) -> Self {
        self.tick.is_newborn = is_newborn;
        self
    }

    pub fn open(mut self, is_open: bool) -> Self {
        self.tick.is_open = is_open;
        self
    }

    pub fn absorb_count(mut self, absorb_count: u8) -> Self {
        self.tick.absorb_count = absorb_count;
        self
    }

    pub fn absorb_dir(mut self, absorb_dir: Direction) -> Self {
        self.tick.absorb_dir = Some(absorb_dir);
        self
    }

    pub fn did_listen(mut self, did_listen: bool) -> Self {
        self.tick.did_listen = did_listen;
        self
    }

    pub fn did_collect(mut self, did_collect: bool) -> Self {
        self.tick.did_collect = did_collect;
        self
    }

    pub fn did_nop(mut self, did_nop: bool) -> Self {
        self.tick.did_nop = did_nop;
        self
    }

    pub fn free_energy(mut self, free_energy: u32) -> Self {
        self.free_energy = free_energy;
        self
    }

    pub fn free_mass(mut self, free_mass: u32) -> Self {
        self.free_mass = free_mass;
        self
    }

    pub fn bg_radiation(mut self, bg_radiation: u32) -> Self {
        self.bg_radiation = bg_radiation;
        self
    }

    pub fn bg_mass(mut self, bg_mass: u32) -> Self {
        self.bg_mass = bg_mass;
        self
    }

    pub fn build(self) -> Cell {
        let mut program = if self.live {
            Program::new_live(self.code, self.dir, self.id).expect("test program should be valid")
        } else {
            Program::new_inert(self.code, self.dir, self.id).expect("test program should be valid")
        };

        program.registers.ip = self.ip;
        program.registers.src = self.src;
        program.registers.dst = self.dst;
        program.registers.flag = self.flag;
        program.registers.msg = self.msg;
        program.registers.lc = self.lc;
        program.stack = self.stack;
        program.age = self.age;
        program.abandonment_timer = self.abandonment_timer;
        program.tick = self.tick;

        Cell {
            program: Some(program),
            free_energy: self.free_energy,
            free_mass: self.free_mass,
            bg_radiation: self.bg_radiation,
            bg_mass: self.bg_mass,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorldBuilder {
    config: SimConfig,
    cells: Vec<Cell>,
}

impl WorldBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        let cell_count = usize::try_from(width)
            .expect("width should fit in usize")
            .checked_mul(usize::try_from(height).expect("height should fit in usize"))
            .expect("test grid should fit in usize");

        let config = SimConfig {
            width,
            height,
            ..SimConfig::default()
        };

        Self {
            config,
            cells: vec![Cell::default(); cell_count],
        }
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    pub fn configure<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut SimConfig),
    {
        configure(&mut self.config);
        self
    }

    pub fn at(mut self, x: u32, y: u32, program: ProgramBuilder) -> Self {
        let index = self.index(x, y);
        self.cells[index] = program.build();
        self
    }

    pub fn free_energy_at(mut self, x: u32, y: u32, free_energy: u32) -> Self {
        let index = self.index(x, y);
        self.cells[index].free_energy = free_energy;
        self
    }

    pub fn free_mass_at(mut self, x: u32, y: u32, free_mass: u32) -> Self {
        let index = self.index(x, y);
        self.cells[index].free_mass = free_mass;
        self
    }

    pub fn bg_radiation_at(mut self, x: u32, y: u32, bg_radiation: u32) -> Self {
        let index = self.index(x, y);
        self.cells[index].bg_radiation = bg_radiation;
        self
    }

    pub fn bg_mass_at(mut self, x: u32, y: u32, bg_mass: u32) -> Self {
        let index = self.index(x, y);
        self.cells[index].bg_mass = bg_mass;
        self
    }

    pub fn build(self) -> (Grid, SimConfig) {
        let grid = Grid::from_cells(self.config.width, self.config.height, self.cells)
            .expect("world builder should produce a valid grid");
        (grid, self.config)
    }

    pub fn build_simulation(self) -> Simulation {
        let (grid, config) = self.build();
        Simulation::from_grid(config, grid)
            .expect("world builder should produce a valid simulation")
    }

    fn index(&self, x: u32, y: u32) -> usize {
        let width = usize::try_from(self.config.width).expect("width should fit in usize");
        (usize::try_from(y).expect("y should fit in usize") * width)
            + usize::try_from(x).expect("x should fit in usize")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridDiff {
    pub x: u32,
    pub y: u32,
    pub left: String,
    pub right: String,
}

pub fn diff_grids(left: &Grid, right: &Grid) -> Vec<GridDiff> {
    if left.width() != right.width() || left.height() != right.height() {
        return vec![GridDiff {
            x: 0,
            y: 0,
            left: format!("dimensions {}x{}", left.width(), left.height()),
            right: format!("dimensions {}x{}", right.width(), right.height()),
        }];
    }

    let mut diffs = Vec::new();
    for index in 0..left.len() {
        let left_cell = left.get(index).expect("left cell should exist");
        let right_cell = right.get(index).expect("right cell should exist");
        if left_cell != right_cell {
            diffs.push(GridDiff {
                x: left.x(index),
                y: left.y(index),
                left: format!("{left_cell:?}"),
                right: format!("{right_cell:?}"),
            });
        }
    }

    diffs
}

pub fn run_ticks(simulation: &mut Simulation, ticks: u32) {
    for _ in 0..ticks {
        simulation.run_tick();
    }
}

macro_rules! __cell_field {
    ($cell:expr, free_energy) => {
        $cell.free_energy
    };
    ($cell:expr, free_mass) => {
        $cell.free_mass
    };
    ($cell:expr, bg_radiation) => {
        $cell.bg_radiation
    };
    ($cell:expr, bg_mass) => {
        $cell.bg_mass
    };
    ($cell:expr, has_program) => {
        $cell.program.is_some()
    };
}

macro_rules! assert_cell {
    ($grid:expr, ($x:expr, $y:expr), $($field:ident == $expected:expr),+ $(,)?) => {{
        let __grid = &$grid;
        let __cell = __grid
            .get(__grid.index($x, $y))
            .expect("cell should exist for assertion");
        $(
            assert_eq!(
                __cell_field!(__cell, $field),
                $expected,
                "cell assertion failed at ({}, {}) for {}",
                $x,
                $y,
                stringify!($field)
            );
        )+
    }};
}

macro_rules! __program_field {
    ($program:expr, ip) => {
        $program.registers.ip
    };
    ($program:expr, dir) => {
        $program.registers.dir
    };
    ($program:expr, src) => {
        $program.registers.src
    };
    ($program:expr, dst) => {
        $program.registers.dst
    };
    ($program:expr, flag) => {
        $program.registers.flag
    };
    ($program:expr, msg) => {
        $program.registers.msg
    };
    ($program:expr, id) => {
        $program.registers.id
    };
    ($program:expr, lc) => {
        $program.registers.lc
    };
    ($program:expr, stack) => {
        &$program.stack
    };
    ($program:expr, code) => {
        &$program.code
    };
    ($program:expr, live) => {
        $program.live
    };
    ($program:expr, age) => {
        $program.age
    };
    ($program:expr, abandonment_timer) => {
        $program.abandonment_timer
    };
    ($program:expr, is_open) => {
        $program.tick.is_open
    };
    ($program:expr, is_newborn) => {
        $program.tick.is_newborn
    };
    ($program:expr, absorb_count) => {
        $program.tick.absorb_count
    };
    ($program:expr, absorb_dir) => {
        $program.tick.absorb_dir
    };
    ($program:expr, did_listen) => {
        $program.tick.did_listen
    };
    ($program:expr, did_collect) => {
        $program.tick.did_collect
    };
    ($program:expr, did_nop) => {
        $program.tick.did_nop
    };
    ($program:expr, bg_radiation_consumed) => {
        $program.tick.bg_radiation_consumed
    };
}

macro_rules! assert_program {
    ($grid:expr, ($x:expr, $y:expr), $($field:ident == $expected:expr),+ $(,)?) => {{
        let __grid = &$grid;
        let __cell = __grid
            .get(__grid.index($x, $y))
            .expect("cell should exist for program assertion");
        let __program = __cell
            .program
            .as_ref()
            .expect("program should exist for assertion");
        $(
            assert_eq!(
                __program_field!(__program, $field),
                $expected,
                "program assertion failed at ({}, {}) for {}",
                $x,
                $y,
                stringify!($field)
            );
        )+
    }};
}
