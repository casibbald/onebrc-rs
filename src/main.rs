use dashmap::DashMap;
use generator::{Generator, Gn, done};
use memmap2::Mmap;
use rayon::prelude::*;
use serde_json::{Value, json};
use std::env;
use std::fs::File;
use std::io;
use std::time::Instant;

fn calculate_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        let sum: f64 = values.iter().sum();
        let mean = sum / values.len() as f64;
        (mean * 10.0).ceil() / 10.0
    }
}

fn chunk_lines(mmap: &[u8], start: usize, end: usize) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        let mut current_pos = start;

        if start > 0 {
            while current_pos < end && mmap[current_pos] != b'\n' {
                current_pos += 1;
            }
            current_pos += 1;
        }

        while current_pos < end {
            let line_start = current_pos;
            while current_pos < end && mmap[current_pos] != b'\n' {
                current_pos += 1;
            }

            let line = String::from_utf8_lossy(&mmap[line_start..current_pos]).to_string();
            s.yield_with(line);

            current_pos += 1;
        }
        done!()
    })
}

fn process_chunk(mmap: &[u8], start: usize, end: usize) -> DashMap<String, Vec<f64>> {
    let map = DashMap::new();
    let lines = chunk_lines(mmap, start, end);

    for line in lines.into_iter() {
        if let Some((city, temp)) = parse_line(&line) {
            map.entry(city).or_insert_with(Vec::new).push(temp);
        }
    }

    map
}

fn reduce_results(combined_map: &DashMap<String, Vec<f64>>) -> Generator<(), (String, Value)> {
    Gn::new_scoped(move |mut s| {
        for entry in combined_map.iter() {
            let city = entry.key().clone();
            let temps = entry.value().clone();

            if !temps.is_empty() {
                let min = temps.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let mean = calculate_mean(&temps);

                s.yield_with((city, json!({ "Min": min, "Mean": mean, "Max": max })));
            }
        }
        done!();
    })
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug = args.contains(&"--debug".to_string());
    let file_path = args.iter().find(|arg| arg.starts_with("--file=")).map_or(
        "data/measurements.txt".to_string(),
        |arg| {
            arg.split('=')
                .nth(1)
                .unwrap_or("data/measurements.txt")
                .to_string()
        },
    );

    let debug_print = |message: &str| {
        if debug {
            println!("{}", message);
        }
    };

    let file = File::open(&file_path)?;
    let file_size = file.metadata()?.len() as usize;

    let mmap = unsafe { Mmap::map(&file)? };

    let start = Instant::now();
    debug_print(&format!("Start time: {:?}", start));

    let high_performance_cores = 8;
    let chunk_size = file_size / high_performance_cores;

    debug_print(&format!("File size: {} bytes", file_size));
    debug_print(&format!("Number of processors: {}", high_performance_cores));
    debug_print(&format!("Chunk size: {} bytes", chunk_size));

    let results: Vec<_> = (0..high_performance_cores)
        .into_par_iter()
        .map(|i| {
            let start = i * chunk_size;
            let end = if i == high_performance_cores - 1 {
                file_size
            } else {
                (i + 1) * chunk_size
            };
            process_chunk(&mmap, start, end)
        })
        .collect();

    let combined_map = DashMap::new();
    for map in results {
        for entry in map.iter() {
            combined_map
                .entry(entry.key().clone())
                .or_insert_with(Vec::new)
                .extend(entry.value().clone());
        }
    }

    let after_processing = Instant::now();
    debug_print(&format!("Time after processing: {:?}", after_processing));

    let mut result = serde_json::Map::new();
    let mut generator = reduce_results(&combined_map);

    while let Some((city, stats)) = generator.resume() {
        result.insert(city, stats);
    }

    let json_output = Value::Object(result);
    let output_path = "weather_stations.json";

    println!(
        "Total execution time (before writing to disk): {:?}",
        start.elapsed()
    );

    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&json_output).unwrap(),
    )?;

    Ok(())
}

fn parse_line(line: &str) -> Option<(String, f64)> {
    let parts: Vec<&str> = line.split(';').collect();
    if parts.len() == 2 {
        let city = parts[0].trim().to_string();
        let temp = parts[1].trim().parse::<f64>().ok()?;
        Some((city, temp))
    } else {
        None
    }
}
