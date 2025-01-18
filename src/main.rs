use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use dashmap::DashMap;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

fn calculate_median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = values.len();
    if len % 2 == 0 {
        (values[len / 2 - 1] + values[len / 2]) / 2.0
    } else {
        values[len / 2]
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug = args.contains(&"--debug".to_string());

    let debug_print = |message: &str| {
        if debug {
            println!("{}", message);
        }
    };

    let start = Instant::now();
    debug_print(&format!("Start time: {:?}", start));

    let file_path = "data/weather_stations.csv";

    let before_open = Instant::now();
    debug_print(&format!("Time before opening the file: {:?}", before_open));

    let lines = match read_lines(file_path) {
        Ok(lines) => lines,
        Err(e) => {
            println!("Failed to open file: {}", e);
            return Err(e);
        }
    };

    let after_open = Instant::now();
    debug_print(&format!("Time after opening the file: {:?}", after_open));

    let weather_map = Arc::new(DashMap::new());

    for (line_number, line) in lines.flatten().enumerate() {
        debug_print(&format!("Reading line {}: {}", line_number + 1, line));

        if line.starts_with('#') || line.trim().is_empty() {
            debug_print(&format!("Skipping line {}: {}", line_number + 1, line));
            continue;
        }

        if let Some((city, temp)) = parse_line(&line) {
            debug_print(&format!("Parsed line {}: City = {}, Temp = {}", line_number + 1, city, temp));
            weather_map.entry(city).or_insert_with(Vec::new).push(temp);
        } else {
            debug_print(&format!("Failed to parse line {}: {}", line_number + 1, line));
        }
    }

    let after_map = Instant::now();
    debug_print(&format!("Time after mapping the file: {:?}", after_map));

    let mut result = serde_json::Map::new();

    for entry in weather_map.iter() {
        let city = entry.key();
        let temps = entry.value();

        if !temps.is_empty() {
            let min = temps.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let median = calculate_median(temps.clone());

            result.insert(
                city.to_string(),
                json!({ "Min": min, "Median": median, "Max": max }),
            );
        }
    }

    let json_output = Value::Object(result);
    let output_path = "weather_stations.json";

    if let Err(e) = std::fs::write(output_path, serde_json::to_string_pretty(&json_output)?) {
        println!("Failed to write to file: {}", e);
    } else {
        let after_write = Instant::now();
        debug_print(&format!("Time after completing the write to disk: {:?}", after_write));
    }

    println!("Total execution time: {:?}", start.elapsed());

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

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
