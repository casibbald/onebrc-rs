use dashmap::DashMap;
use generator::{Generator, Gn, done};
use memmap2::Mmap;
use pollster::block_on;
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io;
use std::time::Instant;
use wgpu::util::DeviceExt;
use wgpu::{Buffer, Device, Queue};

fn calculate_mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        let sum: f32 = values.iter().sum();
        let mean = sum / values.len() as f32;
        (mean * 10.0).ceil() / 10.0
    }
}

async fn calculate_gpu(input: &[f32]) -> (f32, f32, f32) {
    use wgpu::util::DeviceExt;

    // Initialize GPU instance
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .unwrap();

    // Create input and output buffers
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Buffer"),
        contents: bytemuck::cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (3 * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Load and create shader
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    // Create pipeline and bind group
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ],
        label: Some("Bind Group"),
    });

    // Dispatch compute work
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
        });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups((input.len() as u32 + 63) / 64, 1, 1);
    }

    // Copy results to a staging buffer
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: output_buffer.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_buffer.size());
    queue.submit(Some(encoder.finish()));

    // Map and read results
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();

    // Map the buffer and use the callback
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap();
    });

    device.poll(wgpu::Maintain::Wait);

    // Wait for the mapping operation to complete
    receiver.receive().await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let result = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    staging_buffer.unmap(); // Unmap buffer after reading

    // Return min, max, and mean
    (result[0], result[1], result[2])
}




fn chunk_lines(mmap: &[u8], start: usize, end: usize) -> Generator<'_, (), &str> {
    Gn::new_scoped(move |mut s| {
        let mut current_pos = start;

        // Skip to the next full line if not at the start
        if start > 0 {
            while current_pos < end && mmap[current_pos] != b'\n' {
                current_pos += 1;
            }
            current_pos += 1; // Move past '\n'
        }

        // Process lines within the chunk
        while current_pos < end {
            let line_start = current_pos;
            while current_pos < end && mmap[current_pos] != b'\n' {
                current_pos += 1;
            }

            let line = std::str::from_utf8(&mmap[line_start..current_pos]).expect("Invalid UTF-8");
            s.yield_with(line);

            current_pos += 1; // Move past '\n'
        }
        done!()
    })
}

fn process_chunk_local(mmap: &[u8], start: usize, end: usize) -> HashMap<String, Vec<f32>> {
    let mut local_map = HashMap::with_capacity(1000);
    let lines = chunk_lines(mmap, start, end);

    for line in lines.into_iter() {
        if let Some((city, temp)) = parse_line(line) {
            local_map.entry(city).or_insert_with(Vec::new).push(temp);
        }
    }

    local_map
}

fn reduce_results(combined_map: &DashMap<String, Vec<f32>>) -> DashMap<String, Value> {
    let result = DashMap::new();

    // Convert DashMap to Vec for parallel iteration
    let entries: Vec<_> = combined_map
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();

    entries.into_par_iter().for_each(|(city, temps)| {
        if !temps.is_empty() {
            let min = temps.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = temps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            // let mean = calculate_gpu(&temps);
            let (min, max, mean) = block_on(calculate_gpu(&temps));


            result.insert(city, json!({ "Min": min, "Mean": mean, "Max": max }));
        }
    });

    result
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

    // Memory map the file
    let mmap = unsafe { Mmap::map(&file)? };

    let start = Instant::now();
    debug_print(&format!("Start time: {:?}", start));

    // let num_cpus = num_cpus::get_physical(); // Use all physical cores
    let num_cpus = 1; // Use all physical cores
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
                (i + 1) * chunk_size
            };
            process_chunk_local(&mmap, start, end)
        })
        .collect();

    // Combine all results into a DashMap
    let combined_map = DashMap::new();
    for local_map in results {
        for (city, temps) in local_map {
            combined_map
                .entry(city)
                .or_insert_with(Vec::new)
                .extend(temps);
        }
    }

    let after_processing = Instant::now();
    debug_print(&format!("Time after processing: {:?}", after_processing));

    // Final map-reduce
    let reduced_map = reduce_results(&combined_map);

    let result: serde_json::Map<String, Value> = reduced_map
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();

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

fn parse_line(line: &str) -> Option<(String, f32)> {
    let parts: Vec<&str> = line.split(';').collect();
    if parts.len() == 2 {
        let city = parts[0].trim().to_string();
        let temp = parts[1].trim().parse::<f32>().ok()?;
        Some((city, temp))
    } else {
        None
    }
}
