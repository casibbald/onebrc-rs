#![feature(asm_const)]
#![feature(portable_simd)]

use std::arch::asm;
use dashmap::DashMap;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::ops::Add;
use std::time::Instant;
// use std::ptr;
use std::simd::u8x16;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
struct Stats {
    min: i32,
    max: i32,
    sum: i64,
    count: u32,
}

// ARM64 assembly implementation of critical path
unsafe fn find_newline_neon(start: *const u8, end: *const u8) -> *const u8 {
    let mut current = start;

    asm!(
    "1:",
    "ld1 {{v0.16b}}, [{current}]",
    "cmeq v1.16b, v0.16b, {newline}",
    "umaxv b2, v1.16b",
    "fmov {mask:w}, s2",
    "cbnz {mask}, 2f",
    "add {current}, {current}, 16",
    "cmp {current}, {end}",
    "b.lo 1b",
    "b 3f",
    "2:",
    "rbit {mask}, {mask}",
    "clz {mask}, {mask}",
    "add {current}, {current}, {mask}",
    "3:",
    newline = in(vreg) u8x16::splat(b'\n'),
    current = inout(reg) current => _,
    end = in(reg) end,
    mask = out(reg) _,
    options(nostack, preserves_flags)
    );

    current
}

unsafe fn parse_temperature_asm(ptr: u8) -> i32 {
    let mut temp = 0i32;
    let mut sign = 1i32;
    let mut dot_pos = 0usize;

    asm!(
    "// Sign check",
    "ldrb {sign:w}, [{ptr}]",
    "cmp {sign}, '-'",
    "csel {sign}, {neg_one}, {one}, eq",
    "cinc {ptr}, {ptr}, eq",

    "// NEON validation",
    "ld1 {{v0.16b}}, [{ptr}]",
    "sub v1.16b, v0.16b, {ascii_zero}",
    "cmgt v2.16b, v1.16b, {nine}",
    "umaxv b3, v2.16b",
    "fmov {mask:w}, s3",
    "cbnz {mask}, 2f",

    "// Integer parsing",
    "mov {dot_pos}, 0",
    "1:",
    "ldrb {char:w}, [{ptr}, {dot_pos}]",
    "cmp {char}, '.'",
    "b.eq 3f",
    "sub {char}, {char}, '0'",
    "madd {temp}, {temp}, {ten}, {char}",
    "add {dot_pos}, {dot_pos}, 1",
    "b 1b",

    "3:",
    "add {ptr}, {ptr}, {dot_pos}",
    "ldrb {char:w}, [{ptr}, 1]",
    "sub {char}, {char}, '0'",
    "madd {temp}, {temp}, {ten}, {char}",
    "mul {temp}, {temp}, {sign}",
    "b 4f",

    "2:",
    "mov {temp}, 0",
    "4:",
    ptr = inout(reg) ptr => _,
    sign = inout(reg) sign,
    temp = inout(reg) temp,
    dot_pos = out(reg) dot_pos,
    char = out(reg) _,
    ascii_zero = in(vreg) u8x16::splat(b'0'),
    nine = in(vreg) u8x16::splat(9),
    ten = in(reg) 10i32,
    one = in(reg) 1i32,
    neg_one = in(reg) -1i32,
    mask = out(reg) _,
    options(nostack, preserves_flags)
    );

    temp
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data/measurements.txt")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data = Arc::new(Mutex::new(mmap.as_ptr()));
    let len = mmap.len();

    let start = Instant::now();

    let result = DashMap::new();
    let num_threads = rayon::current_num_threads();
    let chunk_size = (len + num_threads - 1) / num_threads;

    (0..num_threads).into_par_iter().for_each(|i| unsafe {
        let data = Arc::clone(&data);
        let mut local_map = DashMap::new();
        let start_ptr = unsafe { *data.lock().unwrap().add(i * chunk_size) };
        let end_ptr = unsafe { *data.lock().unwrap().add((i + 1) * chunk_size).min(data.lock().unwrap().add(len)) };

        let mut ptr = start_ptr;
        if i != 0 {
            // Align to line start
            while ptr < end_ptr && { ptr } != b'\n' {
                ptr = ptr.add(1);
            }
            ptr = ptr.add(1);
        }

        while ptr < end_ptr {
            let line_start = ptr;
            let line_end = { find_newline_neon(*ptr, *end_ptr) };

            // Find semicolon using same NEON pattern
            let mut semi = line_start;
            while semi < *line_end {
                let chunk =  { std::slice::from_raw_parts(*semi, 16.min(*line_end as usize - *semi as usize)) };
                if let Some(pos) = chunk.iter().position(|&b| b == b';') {
                    semi = unsafe { semi.add(&pos) };
                    break;
                }
                semi = unsafe { semi.add(16) };
            }

            if semi >= *line_end {
                ptr = unsafe { *line_end.add(1) };
                continue;
            }

            // Parse temperature with assembly
            let temp =  { parse_temperature_asm(*semi.add(1)) };

            // Update stats
            let city = { std::slice::from_raw_parts(*line_start, *semi as usize - *line_start as usize) };
            local_map.entry(city.to_vec())
                .and_modify(|s: &mut Stats| {

                    s.min = s.min.min(temp);
                    s.max = s.max.max(temp);
                    s.sum += temp as i64;
                    s.count += 1;
                })
                .or_insert(Stats {
                    min: temp,
                    max: temp,
                    sum: temp as i64,
                    count: 1,
                });

            ptr = unsafe { *line_end.add(1) };
        }

        // Merge results
        for (city, stats) in local_map {
            result.entry(city)
                .and_modify(|s: &mut Stats| {
                    s.min = s.min.min(stats.min);
                    s.max = s.max.max(stats.max);
                    s.sum += stats.sum;
                    s.count += stats.count;
                })
                .or_insert(stats);
        }
    });

    // Generate output
    let mut results: Vec<_> = result.into_iter().collect();
    results.par_sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let output = results.into_par_iter()
        .map(|(city, stats)| {
            let city = String::from_utf8_lossy(&city).to_string();
            let min = stats.min as f32 / 10.0;
            let mean = (stats.sum as f32 / stats.count as f32) / 10.0;
            let max = stats.max as f32 / 10.0;
            format!("{}={:.1}/{:.1}/{:.1}", city, min, mean, max)
        })
        .collect::<Vec<_>>()
        .join(", ");

    println!("Processing time: {:?}", start.elapsed());
    std::fs::write("weather_stations.txt", output)?;

    Ok(())
}