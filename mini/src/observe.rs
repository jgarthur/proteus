//! Read-only measurements and pictures of a world. Nothing here affects a run.

use std::collections::HashMap;
use std::io::Write;

use crate::World;

/// A point-in-time summary of a world's tapes.
#[derive(Clone, Debug)]
pub struct Census {
    /// Shannon entropy of the byte distribution, in bits per byte.
    pub shannon: f64,
    /// Size of the world after brotli compression, in bits per byte.
    pub compressed: f64,
    /// Fraction of bytes that are instructions.
    pub op_density: f64,
    /// Each distinct skeleton with the number of cells that carry it, most
    /// common first. A tape's skeleton is its instructions with data removed.
    pub kinds: Vec<(String, usize)>,
}

impl Census {
    /// Measures a world.
    pub fn of(world: &World) -> Self {
        let bytes = world.bytes();
        let mut counts = [0u64; 256];
        for &byte in bytes {
            counts[byte as usize] += 1;
        }
        let total = bytes.len() as f64;
        let shannon = counts
            .iter()
            .filter(|&&count| count > 0)
            .map(|&count| count as f64 / total)
            .map(|p| -p * p.log2())
            .sum();
        let ops: u64 = (0..=255u8)
            .filter(|&byte| world.table().is_op(byte))
            .map(|byte| counts[byte as usize])
            .sum();

        let mut tally: HashMap<String, usize> = HashMap::new();
        for cell in 0..world.cells() {
            *tally.entry(skeleton(world, cell)).or_default() += 1;
        }
        let mut kinds: Vec<_> = tally.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        Self {
            shannon,
            compressed: brotli_bits(bytes) / total,
            op_density: ops as f64 / total,
            kinds,
        }
    }

    /// Returns BFF's "high-order entropy": the bits per byte that a compressor
    /// saves beyond what the byte frequencies alone explain.
    ///
    /// It stays near zero in a random soup and rises when tapes come to share
    /// structure, which is the signature of replicators taking hold.
    pub fn high_order(&self) -> f64 {
        self.shannon - self.compressed
    }

    /// Returns the share of cells that carry the most common skeleton.
    pub fn top_share(&self) -> f64 {
        let cells: usize = self.kinds.iter().map(|(_, count)| count).sum();
        self.kinds.first().map_or(0.0, |(_, count)| *count as f64) / cells as f64
    }
}

/// Returns a cell's tape with everything but its instructions removed.
pub fn skeleton(world: &World, cell: usize) -> String {
    let table = world.table();
    world
        .tape(cell)
        .iter()
        .filter(|&&byte| table.is_op(byte))
        .map(|&byte| byte as char)
        .collect()
}

/// Returns the brotli-compressed size of `bytes` in bits, at the quality BFF's
/// authors used (`brotli -q2`).
fn brotli_bits(bytes: &[u8]) -> f64 {
    let mut compressed = Vec::new();
    brotli::CompressorWriter::new(&mut compressed, 1 << 16, 2, 24)
        .write_all(bytes)
        .expect("writing to a Vec cannot fail");
    compressed.len() as f64 * 8.0
}

/// Renders a world as a PNG, one `scale`-pixel square per cell.
///
/// A cell's hue names its skeleton, so cells running the same program share a
/// colour, and its brightness grows with how much of its tape is instructions.
pub fn picture(world: &World, scale: usize) -> Vec<u8> {
    let rules = world.rules();
    let (width, height) = (rules.width * scale, rules.height * scale);
    let colours: Vec<[u8; 3]> = (0..world.cells())
        .map(|cell| colour(&skeleton(world, cell)))
        .collect();

    // Each scanline is a filter byte (0 = none) followed by RGB pixels.
    let mut raw = Vec::with_capacity(height * (1 + 3 * width));
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            raw.extend(colours[(y / scale) * rules.width + x / scale]);
        }
    }
    png(width as u32, height as u32, &raw)
}

fn colour(skeleton: &str) -> [u8; 3] {
    let hash = skeleton.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let hue = (hash >> 32) as f64 / (1u64 << 32) as f64 * 6.0;
    let value = 0.15 + 0.85 * (skeleton.len() as f64 / 12.0).min(1.0);
    let channel = |shift: f64| {
        let k = (hue + shift) % 6.0;
        let level = value * (1.0 - 0.8 * k.min(4.0 - k).clamp(0.0, 1.0));
        (level * 255.0) as u8
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
