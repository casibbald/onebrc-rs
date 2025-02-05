use std::sync::Arc;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::time::Instant;
use std::arch::x86_64::*;
use rustc_hash::FxHashMap;
use std::ptr;

#[derive(Default)]
struct Stats {
    min: i32,
    max: i32,
    sum: i64,
    count: u32,
}

#[inline(always)]
unsafe fn find_line_end(ptr: *const u8) -> *const u8 {
    let mut current = ptr;
    loop {
        let chunk = _mm_loadu_si128(current as _);
        let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n')));
        if mask != 0 {
            return current.add(mask.trailing_zeros() as usize);
        }
        current = current.add(16);
    }
}

#[inline(always)]
unsafe fn parse_temperature(mut p: *const u8) -> i32 {
    let sign = if *p == b'-' {
        p = p.add(1);
        -1
    } else {
        1
    };

    let mut val = 0i32;
    let mut dot_pos = 0i32;

    // Process 4 bytes at a time using SIMD
    let chunk = _mm_loadu_si128(p as _);
    let digits = _mm_sub_epi8(chunk, _mm_set1_epi8(b'0'));
    let is_digit = _mm_movemask_epi8(_mm_cmpgt_epi8(digits, _mm_set1_epi8(9)));

    let mut i = 0;
    while i < 8 {
        let b = *p.add(i);
        if b == b'.' {
            dot_pos = i as i32;
            break;
        }
        val = val * 10 + (b - b'0') as i32;
        i += 1;
    }

    // Last digit after dot
    val = val * 10 + (*p.add(dot_pos as usize + 1) - b'0') as i32;
    val * sign
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data/measurements.txt")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data = mmap.as_ptr();
    let len = mmap.len();

    let start = Instant::now();

    let result = Arc::new(FxHashMap::default());
    let num_threads = rayon::current_num_threads();
    let chunk_size = (len + num_threads - 1) / num_threads;

    (0..num_threads).into_par_iter().for_each(|i| {
        let mut local_map = FxHashMap::default();
        let start = i * chunk_size;
        let end = (start + chunk_size).min(len);

        let mut ptr = unsafe { data.add(start) };
        let end_ptr = unsafe { data.add(end) };

        // Align to line start
        if i != 0 {
            while ptr < end_ptr && *ptr != b'\n' {
                ptr = ptr.add(1);
            }
            ptr = ptr.add(1);
        }

        while ptr < end_ptr {
            let line_start = ptr;
            let line_end = unsafe { find_line_end(ptr) };

            // Find semicolon using SIMD
            let mut semi = line_start;
            while semi < line_end {
                let chunk = _mm_loadu_si128(semi as _);
                let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, _mm_set1_epi8(b';')));
                if mask != 0 {
                    semi = semi.add(mask.trailing_zeros() as usize);
                    break;
                }
                semi = semi.add(16);
            }

            // Parse temperature
            let temp = unsafe { parse_temperature(semi.add(1)) };

            // Update stats
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

        // Merge results
        let global = Arc::clone(&result);
        let mut global = global.lock().unwrap();
        for (city, stats) in local_map {
            global.entry(city)
                .and_modify(|s| {
                    s.min = s.min.min(stats.min);
                    s.max = s.max.max(stats.max);
                    s.sum += stats.sum;
                    s.count += stats.count;
                })
                .or_insert(stats);
        }
    });

    // Generate output
    let map = Arc::try_unwrap(result).unwrap().into_inner().unwrap();
    let mut output = String::with_capacity(map.len() * 40);
    // ... output generation ...

    println!("Processing time: {:?}", start.elapsed());
    Ok(())
}