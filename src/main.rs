use std::env;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::prelude::*;
use std::path::Path;

use bmp::Image;
use bmp::Pixel;
use byteorder::LE;
use byteorder::ReadBytesExt;

fn main() {
    let file_location = env::args()
        .nth(1)
        .expect("No file provided.");

    println!("Decoding file: {}", &file_location);

    fs::create_dir_all("./output")
        .expect("Failed to create output directory");

    decode_file(file_location)
        .expect("File decoding failed.");
}

fn decode_file(file_location: String) -> io::Result<()> {
    let file = File::open(&file_location)?;
    let mut b = BufReader::new(file);

    let map = Map::parse(&mut b)
        .expect("Failed to parse the map");

    println!("{:#?}", &map.header);
    println!("Points: {}", &map.points.len());
    println!("Enabled: {}", &map.enabled.len());
    println!("Map Size: {}", &map.header.w * &map.header.h);

    // Let's generate a bmp.
    let file_stem = Path::new(&file_location)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap();

    create_map_image(file_stem, &map);

    Ok(())
}

fn create_map_image(file_stem: &str, map: &Map) {
    let map_size = (map.header.w * map.header.h) as usize;
    let mut img = Image::new(map.header.w, map.header.h);

    // Loop over all the pixels.
    // Then check if they are enabled.
    // If they are enabled, scale them and write the pixels.

    let mut offset = 0usize;
    let height_diff = map.header.max_height - map.header.min_height;

    (0..map_size)
        .filter(|&index| { map.enabled[index] > 0u8 })
        .for_each(|index| {
            // Let's write all enabled pixels.
            let position = get_position(&index, &map.header.w, &map.header.h);
            let point = &map.points[offset];

            let height_offset = (point.h - map.header.min_height) / height_diff;
            let pixel = (255f32 * height_offset) as u8;

            img.set_pixel(position.0, position.1, Pixel::new(pixel, pixel, pixel));

            offset += 1;
        });


    let save_path = String::from("./output/") + file_stem + ".bmp";
    img.save(save_path).unwrap();
}

fn get_position(index: &usize, width: &u32, height: &u32) -> (u32, u32) {
    let i = *index as u32;
    let x = i % width;
    let y = height - 1 - (i / width);

    (x, y)
}

#[derive(Debug)]
struct MapHeader {
    signature: u32,
    unk: u32,
    u1: f32,
    u2: f32,
    min_height: f32,
    max_height: f32,
    w: u32,
    h: u32,
    u5: f32,
    u6: f32,
    u7: f32,
    u8: f32,
    u9: f32,
    us1: u16,
    us2: u16,
    u10: f32,
    u11: f32,
    name: String,
}

impl MapHeader {
    fn parse(file: &mut BufReader<File>) -> io::Result<MapHeader> {
        Ok(MapHeader {
            signature: file.read_u32::<LE>()?,
            unk: file.read_u32::<LE>()?,
            u1: file.read_f32::<LE>()?,
            u2: file.read_f32::<LE>()?,
            min_height: file.read_f32::<LE>()?,
            max_height: file.read_f32::<LE>()?,
            w: file.read_u32::<LE>()?,
            h: file.read_u32::<LE>()?,
            u5: file.read_f32::<LE>()?, // Scale?
            u6: file.read_f32::<LE>()?,
            u7: file.read_f32::<LE>()?,
            u8: file.read_f32::<LE>()?,
            u9: file.read_f32::<LE>()?,
            us1: file.read_u16::<LE>()?,
            us2: file.read_u16::<LE>()?,
            u10: file.read_f32::<LE>()?,
            u11: file.read_f32::<LE>()?,
            name: read_fixed_string(file, 0x20),
        })
    }
}

#[derive(Debug)]
struct Map {
    header: MapHeader,
    points: Vec<TilePoint>,
    enabled: Vec<u8>,
}

#[derive(Debug)]
struct TilePoint {
    h: f32,
    unk: u8,
    r: u8,
    g: u8,
    b: u8,
}

impl TilePoint {
    fn parse(file: &mut BufReader<File>) -> io::Result<TilePoint> {
        Ok(TilePoint {
            h: file.read_f32::<LE>()?,
            unk: file.read_u8()?,
            r: file.read_u8()?,
            g: file.read_u8()?,
            b: file.read_u8()?,
        })
    }
}

fn parse_points(header: &MapHeader, b: &mut BufReader<File>) -> io::Result<(Vec<u8>, Vec<TilePoint>)> {
    let total = header.w * header.h;
    let mut counter = 0u32;

    let size = total as usize;

    let mut points = Vec::with_capacity(size);
    let mut enabled_points: Vec<u8> = Vec::with_capacity(size);

    while counter < total {
        let n = b.read_i8()? as i32;

        // Negative values = skip |n|
        // Positive value = read n + 1

        let enabled = n >= 0;
        let amount = if n >= 0 {
            let read_size = 1 + n as u32;

            (0..read_size).for_each(|_| {
                points.push(TilePoint::parse(b).expect("Failed to parse point"));
            });

            read_size
        } else {
            n.abs() as u32
        };

        enabled_points.extend(vec![if enabled { 1 } else { 0 }; amount as usize]);
        counter += amount;
    }

    Ok((enabled_points, points))
}

impl Map {
    fn parse(file: &mut BufReader<File>) -> io::Result<Map> {
        let header = MapHeader::parse(file)
            .expect("Invalid map header");

        let point_data = parse_points(&header, file)
            .unwrap_or_default();

        let enabled = point_data.0;
        let points = point_data.1;

        Ok(Map { header, points, enabled })
    }
}

fn read_fixed_string(file: &mut BufReader<File>, size: usize) -> String {
    let mut buf = vec![0u8; size];

    file.read_exact(&mut buf).unwrap();

    String::from_utf8(buf)
        .unwrap_or_default()
        .trim_matches(char::from(0))
        .to_string()
}
