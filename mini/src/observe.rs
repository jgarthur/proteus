//! Read-only measurements and pictures of a world. Nothing here affects a run.

use std::collections::HashMap;

use crate::code::{self, Op};
use crate::{Body, World};

/// What a live program's code says about its prospects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Has `absorb`, `collect` and `copy`: everything reproduction needs.
    Whole,
    /// Has `absorb` but lacks `collect` or `copy`: it feeds and cannot breed.
    Squatter,
    /// Has no `absorb`: it cannot pay its upkeep for long.
    Starving,
}

impl Kind {
    /// Classifies code by the instructions it contains.
    ///
    /// This is a necessary condition only. Code that has all three
    /// instructions may still fail to reproduce.
    pub fn of(code: &[u8]) -> Self {
        let has = |op| code::has(code, op);
        match (has(Op::Absorb), has(Op::Collect) && has(Op::Copy)) {
            (true, true) => Self::Whole,
            (true, false) => Self::Squatter,
            (false, _) => Self::Starving,
        }
    }
}

/// A point-in-time summary of a world.
#[derive(Clone, Debug, Default)]
pub struct Census {
    /// Live programs.
    pub live: usize,
    /// Children under construction.
    pub unfinished: usize,
    /// Live programs that have everything reproduction needs.
    pub whole: usize,
    /// Live programs that feed but cannot breed.
    pub squatters: usize,
    /// Live programs that cannot feed.
    pub starving: usize,
    /// Mean instructions per live program.
    pub size: f64,
    /// Mean generation of live programs.
    pub generation: f64,
    /// Each distinct code among whole programs with the number of cells that
    /// carry it, most common first.
    pub species: Vec<(String, usize)>,
}

impl Census {
    /// Measures a world.
    pub fn of(world: &World) -> Self {
        let mut census = Self::default();
        let mut tally: HashMap<String, usize> = HashMap::new();
        let (mut sizes, mut generations) = (0, 0u64);
        for body in world.cells().iter().filter_map(|cell| cell.body.as_ref()) {
            if !body.live {
                census.unfinished += 1;
                continue;
            }
            census.live += 1;
            sizes += body.code.len();
            generations += u64::from(body.generation);
            match Kind::of(&body.code) {
                Kind::Whole => {
                    census.whole += 1;
                    *tally.entry(code::spell(&body.code)).or_default() += 1;
                }
                Kind::Squatter => census.squatters += 1,
                Kind::Starving => census.starving += 1,
            }
        }
        let live = census.live.max(1) as f64;
        census.size = sizes as f64 / live;
        census.generation = generations as f64 / live;
        census.species = tally.into_iter().collect();
        census
            .species
            .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        census
    }
}

/// Renders a world as a PNG, one `scale`-pixel square per cell.
///
/// Empty cells are black and unfinished children dark grey. A whole program's
/// hue names its code. Squatters are light grey and starving programs dark red.
pub fn picture(world: &World, scale: usize) -> Vec<u8> {
    let laws = world.laws();
    let (width, height) = (laws.width * scale, laws.height * scale);
    let colours: Vec<[u8; 3]> = world
        .cells()
        .iter()
        .map(|cell| cell.body.as_ref().map_or([0; 3], colour))
        .collect();

    // Each scanline is a filter byte (0 = none) followed by RGB pixels.
    let mut raw = Vec::with_capacity(height * (1 + 3 * width));
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            raw.extend(colours[(y / scale) * laws.width + x / scale]);
        }
    }
    png(width as u32, height as u32, &raw)
}

fn colour(body: &Body) -> [u8; 3] {
    if !body.live {
        return [48; 3];
    }
    match Kind::of(&body.code) {
        Kind::Squatter => return [150; 3],
        Kind::Starving => return [110, 20, 20],
        Kind::Whole => {}
    }
    let hash = body.code.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, &b| {
        (hash ^ u64::from(code::decode(b) as u8)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let hue = (hash >> 32) as f64 / (1u64 << 32) as f64 * 6.0;
    let channel = |shift: f64| {
        let k = (hue + shift) % 6.0;
        ((1.0 - 0.8 * k.min(4.0 - k).clamp(0.0, 1.0)) * 255.0) as u8
    };
    [channel(5.0), channel(3.0), channel(1.0)]
}

/// Wraps raw scanlines in a minimal PNG container with stored (uncompressed) data.
fn png(width: u32, height: u32, raw: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xffff).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        zlib.extend((block.len() as u16).to_le_bytes());
        zlib.extend((!(block.len() as u16)).to_le_bytes());
        zlib.extend(block);
    }
    let (a, b) = raw.iter().fold((1u32, 0u32), |(a, b), &byte| {
        let a = (a + u32::from(byte)) % 65521;
        (a, (b + a) % 65521)
    });
    zlib.extend((b << 16 | a).to_be_bytes());

    let mut header = Vec::new();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8 bits per channel, RGB

    let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", &header), (b"IDAT", &zlib), (b"IEND", &Vec::new())] {
        file.extend((data.len() as u32).to_be_bytes());
        let start = file.len();
        file.extend(kind);
        file.extend(data);
        file.extend(crc32(&file[start..]).to_be_bytes());
    }
    file
}

fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg())
        })
    })
}
