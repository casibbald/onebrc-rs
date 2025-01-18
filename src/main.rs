use dashmap::DashMap;
use serde_json::{Value, json};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::Arc;

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
    let file_path = "data/weather_stations.csv";

    let weather_map = Arc::new(DashMap::new());

    if let Ok(lines) = read_lines(file_path) {
        for (line_number, line) in lines.flatten().enumerate() {
            println!("Reading line {}: {}", line_number + 1, line);

            if line.starts_with('#') || line.trim().is_empty() {
                println!("Skipping line {}: {}", line_number + 1, line);
                continue;
            }

            if let Some((city, temp)) = parse_line(&line) {
                println!(
                    "Parsed line {}: City = {}, Temp = {}",
                    line_number + 1,
                    city,
                    temp
                );
                weather_map.entry(city).or_insert_with(Vec::new).push(temp);
            } else {
                println!("Failed to parse line {}: {}", line_number + 1, line);
            }
        }
    } else {
        println!("Failed to open file: {}", file_path);
    }

    if weather_map.is_empty() {
        println!("No data was parsed into the map!");
    }

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
        println!("Output written to {}", output_path);
    }

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
