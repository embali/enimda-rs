use std::collections::{HashSet, HashMap};
use std::iter::FromIterator;
use std::cmp::min;
use std::error::Error;
use std::path::Path;
use std::fs::File;
use rand::{thread_rng, Rng};
use image::{GenericImage, ImageRgba8, DynamicImage, ImageBuffer, Luma, FilterType};
use image::imageops::rotate270;
use image::imageops::colorops::grayscale;
use gif::{Decoder, SetParameter, ColorOutput};
use gif_dispose::Screen;

fn paginate(total: u32, ppt: f32, lim: u32) -> Result<HashSet<u32>, Box<Error>> {
    let count = (1.0 / ppt).round() as u32;
    let (int, rem) = (total / count, total % count);

    let mut indexes = Vec::new();
    let mut rng = thread_rng();
    for page in 0..int {
        indexes.push(rng.gen_range(page * count, (page + 1) * count));
    }
    if rem != 0 {
        indexes.push(rng.gen_range(int * count, total));
    }
    rng.shuffle(&mut indexes);
    let len = indexes.len();
    indexes.truncate(min(len, lim as usize));

    Ok(HashSet::from_iter(indexes.iter().cloned()))
}

pub fn decompose(path: &Path,
                 width: u32,
                 height: u32,
                 frames: u32,
                 ppt: f32,
                 lim: u32)
                 -> Result<Vec<DynamicImage>, Box<Error>> {
    if ppt < 0.0 || ppt > 1.0 {
        panic!("0.0 <= ppt <= 1.0 expected");
    }
    let frames = paginate(frames, ppt, lim)?;

    let mut decoder = Decoder::new(File::open(path)?);
    decoder.set(ColorOutput::Indexed);
    let mut reader = decoder.read_info().unwrap();
    let mut screen = Screen::new(&reader);

    let mut i = 0;
    let mut ims = Vec::new();
    while let Some(frame) = reader.read_next_frame().unwrap() {
        if ppt == 1.0 || lim == 0 || frames.contains(&i) {
            screen.blit(&frame)?;
            let mut buf: Vec<u8> = Vec::new();
            for pixel in screen.pixels.iter() {
                buf.push(pixel.r);
                buf.push(pixel.g);
                buf.push(pixel.b);
                buf.push(pixel.a);
            }
            ims.push(ImageRgba8(ImageBuffer::from_raw(width, height, buf).unwrap()));
        }

        i += 1;
    }

    Ok(ims)
}

fn convert(im: &DynamicImage,
           size: u32)
           -> Result<(f32, ImageBuffer<Luma<u8>, Vec<u8>>), Box<Error>> {
    let mut conv = im.clone();
    let (w, h) = conv.dimensions();

    let mul = match w > size || h > size {
        true => {
            match w > h {
                true => w as f32 / size as f32,
                false => h as f32 / size as f32,
            }
        }
        false => 1.0,
    };

    if mul != 1.0 {
        conv = conv.resize(size, size, FilterType::Lanczos3);
    }

    Ok((mul, grayscale(&conv)))
}

fn chop(conv: &mut ImageBuffer<Luma<u8>, Vec<u8>>,
        ppt: f32,
        lim: u32)
        -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, Box<Error>> {
    if ppt < 0.0 || ppt > 1.0 {
        panic!("0.0 <= ppt <= 1.0 expected");
    }

    if ppt == 1.0 || lim == 0 {
        return Ok(conv.clone());
    }

    let (w, h) = conv.dimensions();
    let rows = paginate(w, ppt, lim)?;
    let mut strips: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::new(rows.len() as u32, h);
    for (i, row) in rows.iter().enumerate() {
        strips.copy_from(&conv.sub_image(*row, 0, 1, h), i as u32, 0);
    }

    Ok(strips)
}

fn entropy(strip: &mut ImageBuffer<Luma<u8>, Vec<u8>>,
           x: u32,
           y: u32,
           width: u32,
           height: u32)
           -> Result<f32, Box<Error>> {
    let sub = strip.sub_image(x, y, width, height);
    let (w, h) = sub.dimensions();
    let len = (w * h) as f32;

    let hm = sub.pixels().fold(HashMap::new(), |mut acc, e| {
        *acc.entry(e.2.data[0]).or_insert(0) += 1;
        acc
    });

    Ok(hm.values().fold(0f32, |acc, &x| {
        let f = x as f32 / len;
        acc - (f * f.log2())
    }))
}

pub fn scan(im: &DynamicImage,
            size: u32,
            depth: f32,
            thres: f32,
            ppt: f32,
            lim: u32,
            deep: bool)
            -> Result<Vec<u32>, Box<Error>> {
    let (mul, mut conv) = convert(im, size)?;
    let mut borders = Vec::new();

    for side in 0..4 {
        let mut strips = chop(&mut conv, ppt, lim)?;
        let (w, h) = strips.dimensions();
        let height = (depth * h as f32).round() as u32;
        let mut border = 0;

        loop {
            let mut start = border + 1;
            for center in (border + 1)..height {
                if entropy(&mut strips, 0, border, w, center)? > 0.0 {
                    start = center;
                    break;
                }
            }

            let mut sub = 0;
            let mut delta = thres;
            for center in (start..height).rev() {
                let upper = entropy(&mut strips, 0, border, w, center - border)?;
                let lower = entropy(&mut strips, 0, center, w, center - border)?;
                let diff = match lower != 0.0 {
                    true => upper as f32 / lower as f32,
                    false => delta,
                };
                if diff < delta && diff < thres {
                    delta = diff;
                    sub = center;
                }
            }

            if sub == 0 || border == sub {
                break;
            }

            border = sub;

            if !deep {
                break;
            }
        }

        borders.push((border as f32 * mul) as u32);

        if side != 3 {
            conv = rotate270(&conv);
        }
    }

    Ok(borders)
}
