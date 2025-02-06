use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::time::Instant;
use std::arch::aarch64::*;
use dashmap::DashMap;
use rustc_hash::FxHasher;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, RwLock};

type FastHasher = BuildHasherDefault<FxHasher>;
const IMM5: i32 = 0;

#[derive(Default, Clone)]
struct Stats {
    min: i32,
    max: i32,
    sum: i64,
    count: u32,
}

#[inline(always)]
unsafe fn neon_find_newline<const IMM5: i32>(ptr: *const u8) -> *const u8 {
    let mut current = ptr;
    let vnewline = vdupq_n_u8(b'\n');

    loop {
        let chunk = vld1q_u8(current);
        let matches = vceqq_u8(chunk, vnewline);
        let mask = vgetq_lane_u64::<{ IMM5 }>(vreinterpretq_u64_u8(matches));

        if mask != 0 {
            return current.add(mask.trailing_zeros() as usize);
        }
        current = current.add(16);
    }
}

#[inline(always)]
unsafe fn neon_find_semicolon(ptr: *const u8) -> *const u8 {
    let vsemi = vdupq_n_u8(b';');
    let mut current = ptr;

    loop {
        let chunk = vld1q_u8(current);
        let matches = vceqq_u8(chunk, vsemi);
        let mask = vgetq_lane_u64(vreinterpretq_u64_u8(matches), 0);

        if mask != 0 {
            return current.add(mask.trailing_zeros() as usize);
        }
        current = current.add(16);
    }
}

#[inline(always)]
unsafe fn parse_temp_neon(p: *const u8) -> i32 {
    let sign = if *p == b'-' { -1 } else { 1 };
    let start = if sign == -1 { p.add(1) } else { p };

    let chunk = vld1q_u8(start);
    let digits = vsubq_u8(chunk, vdupq_n_u8(b'0'));
    let is_valid = vcltq_u8(digits, vdupq_n_u8(10));

    if vgetq_lane_u64(vreinterpretq_u64_u8(is_valid), 0) != 0xFFFFFFFFFFFFFFFF {
        return 0;
    }

    let dot_pos = {
        let mut pos = 0;
        while pos < 16 {
            if *start.add(pos) == b'.' {
                break;
            }
            pos += 1;
        }
        pos
    };

    let mut val = 0i32;
    for i in 0..dot_pos {
        val = val * 10 + (*start.add(i) - b'0') as i32;
    }
    val = val * 10 + (*start.add(dot_pos + 1) - b'0') as i32;

    sign * val
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data/measurements.txt")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data = Arc::new(RwLock::new(mmap.as_ptr() as usize));
    let len = mmap.len();

    let start = Instant::now();

    let result = Arc::new(DashMap::with_hasher(FastHasher::default()));
    let num_cores = rayon::current_num_threads();
    let chunk_size = (len + num_cores - 1) / num_cores;

    (0..num_cores).into_par_iter().for_each(|i| {
        let mut local_map = DashMap::with_hasher(FastHasher::default());
        let start_ptr = unsafe { (*data.read().unwrap() + i * chunk_size) as *const u8 };
        let end_ptr = unsafe { ((*data.read().unwrap() + (i + 1) * chunk_size).min(*data.read().unwrap() + len)) as *const u8 };

        let mut ptr = start_ptr;
        if i != 0 {
            while ptr < end_ptr && unsafe { *ptr } != b'\n' {
                ptr = unsafe { ptr.add(1) };
            }
            ptr = unsafe { ptr.add(1) };
        }

        while ptr < end_ptr {
            let line_start = ptr;
            let line_end = unsafe { neon_find_newline::<0>(ptr) };


            let semi = unsafe { neon_find_semicolon(line_start) };
            if semi >= line_end {
                ptr = unsafe { line_end.add(1) };
                continue;
            }

            let temp = unsafe { parse_temp_neon(semi.add(1)) };
            let city = unsafe { std::slice::from_raw_parts(line_start, semi as usize - line_start as usize) };

            local_map.entry(city)
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

            ptr = unsafe { line_end.add(1) };
        }

        let global = result.clone();
        for (city, stats) in local_map {
            let city_str = String::from_utf8_lossy(city).to_string();
            global.entry(city_str)
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
    let mut results: Vec<_> = <DashMap<String, Stats, BuildHasherDefault<FxHasher>> as Clone>::clone(&result).into_iter().collect();
    results.par_sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let output = results.into_par_iter()
        .map(|(city, stats)| {
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