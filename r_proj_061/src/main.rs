//! Simple RAW image viewer (tested target: Nikon Z9 NEF files).
//!
//! Usage: cargo run --release -- <path/to/image.NEF>
//!
//! Pipeline:
//!   1. `rawler` decodes the RAW sensor data (Bayer mosaic) and metadata.
//!   2. `RawDevelop` turns it into an RGB image: scaling, demosaic,
//!      white balance, color calibration, crop and sRGB gamma.
//!   3. `image` converts to 8 bit and downscales to fit the screen.
//!   4. `minifb` shows the pixels in a window (Esc or close to quit).
//!
//! Note: Nikon "High Efficiency" / "High Efficiency*" NEFs (TicoRAW) are
//! not supported by rawler (nor any other open source decoder). Use
//! "Lossless compressed" (or uncompressed) NEFs.

use std::{env, path::Path, time::Instant};

use anyhow::{Context, Result, bail};
use image::{DynamicImage, imageops::FilterType};
use minifb::{Key, Window, WindowOptions};
use rawler::imgop::develop::RawDevelop;

/// Maximum window size; the image is scaled down to fit inside.
const MAX_WIDTH: u32 = 1600;
const MAX_HEIGHT: u32 = 1000;

fn main() -> Result<()> {
    let path = env::args().nth(1).context("Usage: r_proj_061 <raw-file>")?;
    let path = Path::new(&path);
    if !path.is_file() {
        bail!("File not found: {}", path.display());
    }

    // 1. Decode RAW data
    let t = Instant::now();
    let raw = rawler::decode_file(path).with_context(|| format!("Cannot decode RAW file {}", path.display()))?;
    println!(
        "Camera: {} {}  |  Sensor: {}x{}  |  bps: {}  |  decoded in {:.2?}",
        raw.camera.clean_make,
        raw.camera.clean_model,
        raw.width,
        raw.height,
        raw.bps,
        t.elapsed()
    );

    // 2. Develop into an RGB image (default processing pipeline)
    let t = Instant::now();
    let developed = RawDevelop::default()
        .develop_intermediate(&raw)
        .context("Developing the RAW image failed")?
        .to_dynamic_image()
        .context("Cannot convert developed image")?;
    println!("Developed image: {}x{} in {:.2?}", developed.width(), developed.height(), t.elapsed());

    // 3. Convert to 8 bit RGB and fit to the screen
    let rgb = fit_to_window(developed).to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);

    // minifb expects one u32 per pixel in 0x00RRGGBB format
    let buffer: Vec<u32> = rgb
        .pixels()
        .map(|p| (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]))
        .collect();

    // 4. Show the image in a window
    let title = format!("{} ({}x{}) - Esc to close", path.file_name().unwrap_or_default().to_string_lossy(), w, h);
    let mut window = Window::new(&title, w, h, WindowOptions::default()).context("Cannot open window")?;
    window.set_target_fps(30);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&buffer, w, h)?;
    }
    Ok(())
}

/// Scale the image down (keeping aspect ratio) if it is larger than the window limits.
fn fit_to_window(img: DynamicImage) -> DynamicImage {
    if img.width() <= MAX_WIDTH && img.height() <= MAX_HEIGHT {
        img
    } else {
        img.resize(MAX_WIDTH, MAX_HEIGHT, FilterType::Triangle)
    }
}
