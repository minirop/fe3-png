#![allow(unused)]

use byteorder::LittleEndian;
use byteorder::ReadBytesExt;
use clap::Parser;
use clap_num::maybe_hex;
use image::Rgb;
use image::RgbImage;
use memchr::memmem::rfind;
use std::fs::File;
use std::io::Write;
use std::io::{self, Seek};
use std::io::{Cursor, Read};

const COLOURS: [(u8, u8, u8); 16] = [
    (0, 0, 14),
    (31, 30, 21),
    (31, 24, 8),
    (30, 20, 7),
    (29, 14, 0),
    (26, 9, 0),
    (21, 7, 0),
    (19, 5, 0),
    (16, 4, 0),
    (14, 2, 0),
    (11, 3, 0),
    (9, 3, 0),
    (8, 2, 0),
    (14, 8, 7),
    (10, 5, 5),
    (8, 2, 0),
];

#[derive(Debug, Parser)]
struct Args {
    filename: String,

    #[arg(short, long, value_parser=maybe_hex::<u64>, default_value="0")]
    address: u64,

    #[arg(short, long)]
    output: String,

    #[arg(short, long)]
    palette: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    extract_graphics(&args.filename, args.address, &args.output, &args.palette)?;

    // not working
    // compress_graphics("output.bin", "output.comp.bin")?;

    Ok(())
}

fn compress_graphics(filename: &str, output_filename: &str) -> io::Result<()> {
    let mut file = File::open(filename)?;
    let file_size = file.metadata()?.len();
    let mut buffer = vec![0u8; file_size as usize];
    file.read(&mut buffer)?;
    let buffer = buffer;

    let mut output = vec![];
    let mut length_index = 0usize;
    let mut length_length = 0u8;

    let mut iter = buffer.windows(3);
    let mut window_opt = iter.next();
    while window_opt.is_some() {
        let window = window_opt.unwrap();
        println!("{window:?} inside {output:?}");

        if window == &[0xff, 0x00, 0xff] && output.len() > 180 {
            panic!("LOL");
        }

        if window.iter().all(|w| window[0] == *w) {
            let mut length = 3u8;
            let value = window[0];

            window_opt = iter.next();

            while let Some(window) = window_opt {
                if window[2] == value {
                    length += 1;
                } else {
                    break;
                }
            }
            iter.next();
            iter.next();

            assert!(length <= 0x1f);
            output.push(0x20 + (length - 1));
            output.push(value);
        } else if let Some(pos) = rfind(&output, window) {
            let offset = (output.len() - pos) as u8;

            println!("POS AT {pos} // {offset:#X}");
            let mut length = 3u8;
            window_opt = iter.next();

            println!("{:#X} vs {:#X}", output[pos], window[0]);

            while let Some(window) = window_opt {
                if window[2] == output[pos + length as usize] {
                    length += 1;
                    //panic!("matching");
                } else {
                    break;
                }
            }
            iter.next();
            iter.next();

            assert!(length <= 0x1f);
            output.push(0xC0 + (length - 1));
            output.push(offset);
            println!("{output:?}");
        } else {
            if length_length == 0 {
                length_index = output.len();
            }

            println!("push {:#X}", window[0]);
            output.push(window[0]);
            window_opt = iter.next();

            length_length += 1;
        }

        if output.len() > 500 {
            break;
        }
    }

    Ok(())
}

fn extract_graphics(
    filename: &str,
    start: u64,
    output_filename: &str,
    palette_file: &Option<String>,
) -> io::Result<()> {
    let mut palette = vec![];

    if let Some(pal) = palette_file {
        let mut pal_file = File::open(pal)?;
        while let Ok(colour) = pal_file.read_u16::<LittleEndian>() {
            let r = (colour & 0b11111) as u8;
            let g = ((colour >> 5) & 0b11111) as u8;
            let b = ((colour >> 10) & 0b11111) as u8;
            palette.push(Rgb([r * 8, g * 8, b * 8]));
        }
    } else {
        for i in 0..16 {
            palette.push(Rgb([i * 17, i * 17, i * 17]));
        }
    }

    let mut rom = File::open(filename)?;

    if start > 0 {
        rom.seek(io::SeekFrom::Start(start))?;
    }

    let mut output = vec![];
    loop {
        let id = rom.read_u8()?;
        if id == 0xFF {
            break;
        }

        let len = output.len();
        let pos = rom.seek(io::SeekFrom::Current(0))? - start;

        // aaac cccc
        // aaa = id
        // ccccc = bytes to copy
        match id >> 5 {
            0x0 => {
                let count = id as usize + 1;
                let mut buffer = vec![0u8; count];
                rom.read(&mut buffer)?;
                output.extend(buffer);
            }
            0x1 => {
                let copy_count = (id & 0x1F) as usize + 1;
                let copied_byte = rom.read_u8()?;
                let copied = vec![copied_byte; copy_count];
                output.extend(copied);
            }
            0x2 => {
                let copy_count = (id & 0x1F) as usize + 1;
                let mut double = [0u8; 2];
                rom.read(&mut double)?;
                for i in 0..copy_count {
                    output.push(double[i % 2]);
                }
            }
            0x3 => {
                let count = (id & 0x1F) + 1;
                let start = rom.read_u8()?;
                for i in 0..count {
                    output.push(start + i);
                }
            }
            0x4 => {
                let copy_count = (id & 0x1F) as usize + 1;
                let lo = rom.read_u8()? as usize;
                let hi = rom.read_u8()? as usize;
                let starting_copy = lo + (hi << 8);
                let mut copied = vec![0u8; copy_count];
                copied.copy_from_slice(&output[starting_copy..(starting_copy + copy_count)]);
                output.extend(copied);
            }
            0x5 => {
                let copy_count = (id & 0x1F) as usize + 1;
                let lo = rom.read_u8()? as usize;
                let hi = rom.read_u8()? as usize;
                let starting_copy = lo + (hi << 8);

                for id in starting_copy..(starting_copy + copy_count) {
                    let byte = output[id] ^ 0xFF;
                    output.push(byte);
                }
            }
            0x6 => {
                let copy_count = (id & 0x1F) as usize + 1;
                let back = rom.read_u8()?;
                let starting_copy = output.len() - (back as usize);

                for id in starting_copy..(starting_copy + copy_count) {
                    output.push(output[id]);
                }
            }
            0x7 => {
                let add = ((id & 0b11) as usize) << 8;
                let count = rom.read_u8()? as usize + 1 + add;

                let real_id = id & 0b11111100;

                if real_id == 0xE0 {
                    let mut buffer = vec![0u8; count];
                    rom.read(&mut buffer)?;
                    output.extend(buffer);
                } else if real_id == 0xE4 {
                    let copied_byte = rom.read_u8()?;
                    let copied = vec![copied_byte; count];
                    output.extend(copied);
                } else if real_id == 0xE8 {
                    let mut double = [0u8; 2];
                    rom.read(&mut double)?;
                    for i in 0..count {
                        output.push(double[i % 2]);
                    }
                } else if real_id == 0xF8 {
                    let back = rom.read_u8()? as usize;
                    println!("{} - {back}", output.len());
                    let starting_pos = output.len() - back;
                    for id in 0..count {
                        output.push(output[starting_pos + id]);
                    }
                } else {
                    let mut output_bin = File::create("dump.bin")?;
                    output_bin.write(&output)?;
                    println!("{id:#X} // {real_id:#X}");
                    unimplemented!();
                }
                // technically, it does "id << 3 & $e0" so "ccc0 0000"
                // then "id & 3" => 0000 00cc
                // id => 111a aabb
                // check if aaa != 0
            }
            _ => {
                println!("unhandled id: {id:#X} ({pos:#X})");
                println!("{}", len);
                println!("{:?}", &output[(len - 16)..(len - 1)]);
                break;
            }
        }
    }

    let size = rom.seek(io::SeekFrom::Current(0))? - start;
    println!("start: {start}");
    println!("compressed size: {size:#X} // {size}");
    println!(
        "uncompressed size: {:#X}, {}, {}",
        output.len(),
        16 * 8,
        (output.len() / 64)
    );

    let mut output_bin = File::create(format!("{output_filename}.bin"))?;
    output_bin.write(&output)?;

    let mut buffer = vec![0u8; 32];
    let mut cursor = Cursor::new(&output);
    let mut img = RgbImage::new(16 * 8, (output.len() / 64) as u32);

    for id in 0..(output.len() / 32) {
        cursor.read(&mut buffer)?;

        let cell_x = ((id % 16) * 8) as u32;
        let cell_y = ((id / 16) * 8) as u32;

        for y in 0..8 {
            for x in 0..8 {
                let plane1 = buffer[y * 2];
                let plane2 = buffer[y * 2 + 1];
                let plane3 = buffer[y * 2 + 16];
                let plane4 = buffer[y * 2 + 17];

                let bit1 = (plane1 >> (7 - x)) & 1;
                let bit2 = (plane2 >> (7 - x)) & 1;
                let bit3 = (plane3 >> (7 - x)) & 1;
                let bit4 = (plane4 >> (7 - x)) & 1;
                let index = bit1 + bit2 * 2 + bit3 * 4 + bit4 * 8;
                img.put_pixel(cell_x + x, cell_y + y as u32, palette[index as usize]);
            }
        }
    }

    img.save(output_filename).unwrap();

    Ok(())
}
