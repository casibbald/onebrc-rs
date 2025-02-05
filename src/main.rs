use std::sync::Arc;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::time::Instant;
use memchr::memchr;
use rustc_hash::FxHashMap;

#[derive(Default)]
struct CityStats {
    min: i32,
    max: i32,
    sum: i64,
    count: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data/measurements.txt")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data = Arc::new(mmap);

    let elapsed = Instant::now();

    let results: Vec<_> = data.par_chunks(data.len() / rayon::current_num_threads())
        .map(|chunk| {
            let mut map = FxHashMap::default();
            let mut pos = 0;

            while pos < chunk.len() {
                let Some(line_end) = memchr(b'\n', &chunk[pos..]) else { break };
                let line = &chunk[pos..pos + line_end];
                pos += line_end + 1;

                let Some(semi) = memchr(b';', line) else { continue };
                let city = &line[..semi];
                let temp = parse_temp(&line[semi + 1..]).unwrap_or(0);

                map.entry(city)
                    .and_modify(|s: &mut CityStats| {
                        s.min = s.min.min(temp);
                        s.max = s.max.max(temp);
                        s.sum += temp as i64;
                        s.count += 1;
                    })
                    .or_insert(CityStats {
                        min: temp,
                        max: temp,
                        sum: temp as i64,
                        count: 1,
                    });
            }
            map
        })
        .collect();

    let global_map = results.into_iter()
        .reduce(|mut a, b| {
            for (city, stats) in b {
                a.entry(city)
                    .and_modify(|s| {
                        s.min = s.min.min(stats.min);
                        s.max = s.max.max(stats.max);
                        s.sum += stats.sum;
                        s.count += stats.count;
                    })
                    .or_insert(stats);
            }
            a
        })
        .unwrap();

    let output = global_map.par_iter()
        .map(|(city, stats)| {
            let min = stats.min as f32 / 10.0;
            let mean = (stats.sum as f32 / stats.count as f32) / 10.0;
            let max = stats.max as f32 / 10.0;
            format!("{}={:.1}/{:.1}/{:.1}", String::from_utf8_lossy(city), min, mean, max)
        })
        .collect::<Vec<_>>()
        .join(", ");

    println!("Processing time: {:?}", elapsed.elapsed());
    std::fs::write("weather_stations.txt", output)?;

    Ok(())
}

#[inline(always)]
fn parse_temp(bytes: &[u8]) -> Option<i32> {
    let mut sign = 1;
    let mut i = 0;
    let len = bytes.len();

    if len == 0 {
        return None;
    }

    if bytes[0] == b'-' {
        sign = -1;
        i += 1;
        if len == 1 {
            return None;
        }
    }

    let mut val = 0i32;
    let mut seen_dot = false;
    let mut digits_after_dot = 0;

    while i < len {
        let b = bytes[i];
        match b {
            b'0'..=b'9' => {
                if seen_dot {
                    digits_after_dot += 1;
                    if digits_after_dot > 1 {
                        return None;
                    }
                }
                val = val * 10 + (b - b'0') as i32;
            },
            b'.' => {
                if seen_dot || i == 0 || i == len - 1 {
                    return None;
                }
                seen_dot = true;
            },
            _ => return None,
        }
        i += 1;
    }

    if !seen_dot || digits_after_dot != 1 {
        None
    } else {
        Some(sign * val)
    }
}