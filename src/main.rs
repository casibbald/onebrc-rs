use dashmap::DashMap;
use generator::{Generator, Gn, done};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::sync::Arc;
use std::time::Instant;

fn calculate_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn read_lines(file_path: String) -> Generator<'static, (), String> {
    Gn::new_scoped(move |mut s| {
        let file = File::open(&file_path).expect("Failed to open file");
        let reader = BufReader::new(file);

        for line in reader.lines() {
            if let Ok(line) = line {
                s.yield_with(line);
            }
        }
        done!()
    })
}

fn parse_lines(input: Generator<'static, (), String>) -> Generator<'static, (), (String, f64)> {
    Gn::new_scoped(move |mut s| {
        for line in input {
            if let Some((city, temp)) = parse_line(&line) {
                s.yield_with((city, temp));
            }
        }
        done!()
    })
}

fn aggregate_data(
    input: Generator<(), (String, f64)>,
    combined_map: Arc<DashMap<String, Vec<f64>>>,
) {
    for (city, temp) in input {
        combined_map.entry(city).or_insert_with(Vec::new).push(temp);
    }
}

fn reduce_results_to_string(combined_map: &DashMap<String, Vec<f64>>) -> String {
    let mut results = combined_map
        .iter()
        .map(|entry| {
            let city = entry.key();
            let temps = entry.value();

            let min = temps.iter().copied().fold(f64::INFINITY, f64::min);
            let max = temps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let mean = calculate_mean(&temps);

            format!("{city}={:.1}/{:.1}/{:.1}", min, mean, max)
        })
        .collect::<Vec<_>>();

    results.sort();
    format!("{{{}}}", results.join(", "))
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path: String = args
        .iter()
        .find(|arg| arg.starts_with("--file="))
        .map_or_else(
            || "data/measurements.txt".to_string(),
            |arg| {
                arg.split('=')
                    .nth(1)
                    .unwrap_or("data/measurements.txt")
                    .to_string()
            },
        );

    let timer = Instant::now();
    let combined_map = Arc::new(DashMap::new());

    let lines = read_lines(file_path);

    let parsed_lines = parse_lines(lines);

    aggregate_data(parsed_lines, combined_map.clone());

    println!(
        "Processing time (including aggregation): {:.2?}",
        timer.elapsed()
    );

    let output = reduce_results_to_string(&combined_map);
    let output_path = "weather_stations.txt";

    println!(
        "Total execution time (before writing to disk): {:.2?}",
        timer.elapsed()
    );

    std::fs::write(output_path, output)?;

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
