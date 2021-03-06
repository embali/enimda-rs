//! Crate to detect image borders and whitespace using entropy-based image border detection
//! algorithm.
//!
//! # Example
//!
//! ```rust,ignore
//! extern crate enimda;
//!
//! use std::path::Path;
//! use enimda::enimda;
//!
//! let path = Path::new("test.jpg");
//! let borders = enimda(&path, Some(10), Some(512), Some(50), Some(0.25), Some(0.5), Some(false))?;
//!
//! println!("{:?}", borders);
//! ```

#![deny(missing_docs)]

extern crate rand;
extern crate image;
extern crate gif;
extern crate gif_dispose;
extern crate image_utils;

use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use std::error::Error;
use image::{ImageRgba8, ImageBuffer, ImageFormat};
use image_utils::info;
use gif::{Decoder, SetParameter, ColorOutput};
use gif_dispose::Screen;

mod utils;

use utils::{slice, scan};

/// Borders location
#[derive(Debug, PartialEq)]
pub struct Borders {
    /// Border offset from the top
    pub top: u32,
    /// Border offset from the right
    pub right: u32,
    /// Border offset from the bottom
    pub bottom: u32,
    /// Border offset from the left
    pub left: u32,
}

/// Scan image and find its borders
///
/// `path` - path to image file
///
/// `frames` - frame limit to use in case of animated image, optimization parameter, no limit by
/// default, if set then random frames will be used for scan
///
/// `size` - fit image to this size in pixels to improve performance, optimization parameter, no
/// resize by default
///
/// `columns` - column limit to use for scan, optimization parameter, no limit by default, if set
/// then random columns will be used for scan
///
/// `depth` - percent of pixels of image height to use for scan, 0.25 by default
///
/// `threshold` - threshold, aggressiveness of algorithm, 0.5 by default
///
/// `deep` - iteratively find deep borders, true by default (less performant, but more accurate)
///
/// Returns Borders struct
pub fn enimda(path: &Path,
              frames: Option<u32>,
              size: Option<u32>,
              columns: Option<u32>,
              depth: Option<f32>,
              threshold: Option<f32>,
              deep: Option<bool>)
              -> Result<Borders, Box<Error>> {
    let inf = info(path)?;

    let borders = match inf.format {
        ImageFormat::GIF => {
            let frames = frames.unwrap_or(0);
            let frameset = slice(inf.frames, frames)?;

            let mut decoder = Decoder::new(File::open(path)?);
            decoder.set(ColorOutput::Indexed);
            let mut reader = decoder.read_info().unwrap();
            let mut screen = Screen::new(&reader);

            let mut index = 0;
            let mut variants = Vec::new();
            while let Some(frame) = reader.read_next_frame().unwrap() {
                if frames == 0 || frameset.contains(&index) {
                    screen.blit(&frame)?;
                    let mut buf: Vec<u8> = Vec::new();
                    for pixel in screen.pixels.iter() {
                        buf.push(pixel.r);
                        buf.push(pixel.g);
                        buf.push(pixel.b);
                        buf.push(pixel.a);
                    }
                    let im = ImageRgba8(ImageBuffer::from_raw(inf.width, inf.height, buf).unwrap());
                    let sub = scan(&im, size, columns, depth, threshold, deep)?;
                    variants.push(sub);
                }

                index += 1;
            }

            let mut borders = vec![0, 0, 0, 0];
            for (index, variant) in variants.iter().enumerate() {
                for side in 0..borders.len() {
                    if index == 0 || variant[side] < borders[side] {
                        borders[side] = variant[side];
                    }
                }
            }

            borders
        }
        _ => {
            let im = image::load(BufReader::new(File::open(path)?), inf.format)?;
            scan(&im, size, columns, depth, threshold, deep)?
        }
    };

    Ok(Borders {
        top: borders[0],
        right: borders[1],
        bottom: borders[2],
        left: borders[3],
    })
}
