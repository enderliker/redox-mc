//! CLI executable for running JNI microbenchmarks.
//! Usage: `cargo run -p redox-mc-loader --bin jni-bench [--quick]`

use redox_mc_loader::bench::jvm_host::JvmHost;
use redox_mc_loader::bench::report::print_report;
use redox_mc_loader::bench::runner::{run_benchmarks, BenchmarkConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|arg| arg == "--quick" || arg == "-q");

    let config = if quick {
        println!("[jni-bench] Running in QUICK mode (reduced iterations)...");
        BenchmarkConfig {
            warmup_iterations: 1_000,
            batch_size: 200,
            num_batches: 50,
        }
    } else {
        println!("[jni-bench] Initializing full benchmark suite (warmup: 50,000, 200,000 measured iterations)...");
        BenchmarkConfig::default()
    };

    println!("[jni-bench] Initializing JVM via dynamic libjvm loader...");
    let host = match JvmHost::new() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[jni-bench] FATAL: Failed to initialize JVM: {}", e);
            std::process::exit(1);
        }
    };

    println!("[jni-bench] JVM initialized successfully. Running benchmark suites...");
    let results = match run_benchmarks(&host, &config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[jni-bench] FATAL: Error during benchmark execution: {}", e);
            std::process::exit(1);
        }
    };

    print_report(&results);
}
