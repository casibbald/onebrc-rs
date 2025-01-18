use dashmap::DashMap;
use generator::{Generator, Gn, done};
use rayon::ThreadPoolBuilder;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;
use serde_json::{Value, json};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
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

fn chunk_lines(file_path: &str, start: u64, end: u64) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        let file = File::open(file_path).expect("Failed to open file");
        let mut reader = BufReader::new(file);

        // Seek to the start of the chunk
        reader.seek(SeekFrom::Start(start)).expect("Failed to seek");

        // Skip partial line at the start of the chunk
        if start > 0 {
            let mut temp_buf = Vec::new();
            reader.read_until(b'\n', &mut temp_buf).unwrap();
        }

        let mut buffer = String::new();
        let mut current_pos = start;

        // Process lines within the chunk
        while current_pos < end {
            if reader.read_line(&mut buffer).unwrap() == 0 {
                break;
            }

            s.yield_with(buffer.clone()); // Yield the line

            current_pos = reader.stream_position().unwrap();
        }
        done!()
    })
}

fn process_chunk(file_path: &str, start: u64, end: u64) -> DashMap<String, Vec<f64>> {
    let map = DashMap::new();
    let lines = chunk_lines(file_path, start, end);

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
        done!(); // Indicate the generator has completed.
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

    let start = Instant::now();
    debug_print(&format!("Start time: {:?}", start));

    let file = File::open(&file_path)?;
    let file_size = file.metadata()?.len();

    let high_performance_cores = 8; // Adjust based on your Mac's configuration
    let thread_pool = ThreadPoolBuilder::new()
        .num_threads(high_performance_cores)
        .build_global();
    if thread_pool.is_err() {
        eprintln!("Failed to set the rayon thread pool size.");
    }

    let num_cpus = high_performance_cores as u64;
    let chunk_size = file_size / num_cpus;

    debug_print(&format!("File size: {} bytes", file_size));
    debug_print(&format!("Number of processors: {}", num_cpus));
    debug_print(&format!("Chunk size: {} bytes", chunk_size));

    // Process file in parallel chunks
    let results: Vec<_> = (0..num_cpus)
        .into_par_iter()
        .map(|i| {
            let start = i * chunk_size;
            let end = if i == num_cpus - 1 {
                file_size
            } else {
                (i + 1) * chunk_size - 1
            };
            process_chunk(&file_path, start, end)
        })
        .collect();

    // Combine all results
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

    // Final map-reduce using the generator
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

    // Write results to disk
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
