use std::env;
use std::fs;
use std::path::Path;
use dashmap::DashMap;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

fn calculate_median(values: &[f64]) -> f64 {
    let mut sorted_values = values.to_vec();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = sorted_values.len();
    if len % 2 == 0 {
        // Calculate the median for even-length datasets
        let mid1 = sorted_values[len / 2 - 1];
        let mid2 = sorted_values[len / 2];
        (mid1 + mid2) / 2.0
    } else {
        // Calculate the median for odd-length datasets
        sorted_values[len / 2]
    }
}

fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    let debug = args.contains(&"--debug".to_string());
    let file_path = args.iter().find(|arg| arg.starts_with("--file=")).map_or(
        "data/weather_stations.csv".to_string(),
        |arg| arg.split('=').nth(1).unwrap_or("data/weather_stations.csv").to_string(),
    );

    // Function to print debug information
    let debug_print = |message: &str| {
        if debug {
            println!("{}", message);
        }
    };

    let start = Instant::now();
    debug_print(&format!("Start time: {:?}", start));

    // Time before reading the file
    let before_open = Instant::now();
    debug_print(&format!(
        "Time before reading the file: {:?}",
        before_open
    ));

    // Read entire file into memory
    let file_content = match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Failed to read file {}: {}", file_path, e);
            return;
        }
    };

    // Time after reading the file
    let after_read = Instant::now();
    debug_print(&format!("Time after reading the file: {:?}", after_read));

    // Split content into lines
    let lines = file_content.lines();

    // Create a concurrent hash map to store city -> Vec<f64> (temperatures)
    let weather_map = Arc::new(DashMap::new());

    for (line_number, line) in lines.enumerate() {
        debug_print(&format!("Processing line {}: {}", line_number + 1, line));

        // Skip comments or empty lines
        if line.starts_with('#') || line.trim().is_empty() {
            debug_print(&format!("Skipping line {}: {}", line_number + 1, line));
            continue;
        }

        // Parse the line
        if let Some((city, temp)) = parse_line(line) {
            debug_print(&format!(
                "Parsed line {}: City = {}, Temp = {}",
                line_number + 1,
                city,
                temp
            ));
            weather_map.entry(city).or_insert_with(Vec::new).push(temp);
        } else {
            debug_print(&format!("Failed to parse line {}: {}", line_number + 1, line));
        }
    }

    // Time after processing lines
    let after_parse = Instant::now();
    debug_print(&format!("Time after processing lines: {:?}", after_parse));

    // Map-reduce: Calculate Min, Median, Max for each city
    let mut result = serde_json::Map::new();

    for entry in weather_map.iter() {
        let city = entry.key();
        let temps = entry.value();

        if !temps.is_empty() {
            let min = temps.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let median = calculate_median(temps);

            result.insert(
                city.to_string(),
                json!({ "Min": min, "Median": median, "Max": max }),
            );
        }
    }

    // Serialize the result as JSON
    let json_output = Value::Object(result);
    let output_path = "weather_stations.json";

    // Write JSON output to file
    if let Err(e) = fs::write(output_path, serde_json::to_string_pretty(&json_output).unwrap()) {
        println!("Failed to write to file: {}", e);
    } else {
        // Time after completing the write to disk
        let after_write = Instant::now();
        debug_print(&format!(
            "Time after completing the write to disk: {:?}",
            after_write
        ));
    }

    println!("Total execution time: {:?}", start.elapsed());
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
