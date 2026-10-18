use redox_mc_loader::bench::jvm_host::find_jvm_library;
use redox_mc_loader::bench::jvm_host::JvmHost;
use redox_mc_loader::bench::runner::{run_benchmarks, BenchmarkConfig};
use redox_mc_loader::bench::stats::compute_stats;

#[test]
fn test_stats_calculation() {
    // Generate samples: 1.0, 2.0, ..., 100.0
    let samples: Vec<f64> = (1..=100).map(|x| x as f64).collect();
    let stats = compute_stats("test_metric", samples, 100);

    assert_eq!(stats.name, "test_metric");
    assert_eq!(stats.iterations, 100);
    assert!((stats.min_ns - 1.0).abs() < 1e-6);
    assert!((stats.max_ns - 100.0).abs() < 1e-6);
    assert!((stats.avg_ns - 50.5).abs() < 1e-6);
    // p50 should be around 50.0 - 51.0
    assert!(stats.p50_ns >= 50.0 && stats.p50_ns <= 51.0);
    // p90 should be around 90.0
    assert!(stats.p90_ns >= 90.0 && stats.p90_ns <= 91.0);
    // p99 should be around 99.0
    assert!(stats.p99_ns >= 99.0 && stats.p99_ns <= 100.0);
    assert!(stats.std_dev_ns > 0.0);
}

#[test]
fn test_jvm_locator() {
    let jvm_path = find_jvm_library();
    assert!(
        jvm_path.is_some(),
        "find_jvm_library should discover libjvm on systems with JDK installed"
    );
    let path = jvm_path.unwrap();
    assert!(
        path.exists(),
        "Discovered libjvm path '{:?}' must exist",
        path
    );
}

#[test]
fn test_jvm_host_lifecycle() {
    let host = JvmHost::new().expect("Failed to create embedded JVM");
    unsafe {
        let env = host.env();
        assert!(!env.is_null(), "JNIEnv pointer must not be null");

        let version = ((**env).GetVersion)(env);
        assert!(
            version >= 0x00010008,
            "JVM version must be at least JNI 1.8, got 0x{:08x}",
            version
        );
    }
}

#[test]
fn test_microbench_suite_execution() {
    let host = JvmHost::new().expect("Failed to create embedded JVM");
    let config = BenchmarkConfig {
        warmup_iterations: 100,
        batch_size: 50,
        num_batches: 10,
    };

    let results = run_benchmarks(&host, &config).expect("Benchmark execution failed");
    assert_eq!(
        results.len(),
        4,
        "Should produce 4 benchmark results (no-arg, primitives, string, object)"
    );

    for stat in &results {
        assert!(stat.iterations > 0);
        assert!(stat.avg_ns > 0.0, "avg_ns must be positive");
        assert!(stat.p50_ns > 0.0, "p50_ns must be positive");
        assert!(stat.p99_ns >= stat.p50_ns, "p99 must be >= p50");
    }
}
