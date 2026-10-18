//! Statistical computation for microbenchmarks.

#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub name: String,
    pub iterations: usize,
    pub min_ns: f64,
    pub max_ns: f64,
    pub avg_ns: f64,
    pub p50_ns: f64,
    pub p90_ns: f64,
    pub p95_ns: f64,
    pub p99_ns: f64,
    pub std_dev_ns: f64,
}

/// Computes statistical metrics (min, max, mean, percentiles, std_dev) from a vector of sample timings in nanoseconds.
pub fn compute_stats(
    name: &str,
    mut samples_ns: Vec<f64>,
    total_iterations: usize,
) -> BenchmarkStats {
    if samples_ns.is_empty() {
        return BenchmarkStats {
            name: name.to_string(),
            iterations: 0,
            min_ns: 0.0,
            max_ns: 0.0,
            avg_ns: 0.0,
            p50_ns: 0.0,
            p90_ns: 0.0,
            p95_ns: 0.0,
            p99_ns: 0.0,
            std_dev_ns: 0.0,
        };
    }

    samples_ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = samples_ns.len();
    let min_ns = samples_ns[0];
    let max_ns = samples_ns[n - 1];
    let sum: f64 = samples_ns.iter().sum();
    let avg_ns = sum / n as f64;

    let percentile = |p: f64| -> f64 {
        if n == 1 {
            return samples_ns[0];
        }
        let rank = p * (n - 1) as f64;
        let low = rank.floor() as usize;
        let high = rank.ceil() as usize;
        let weight = rank - low as f64;
        samples_ns[low] * (1.0 - weight) + samples_ns[high] * weight
    };

    let p50_ns = percentile(0.50);
    let p90_ns = percentile(0.90);
    let p95_ns = percentile(0.95);
    let p99_ns = percentile(0.99);

    let variance: f64 = samples_ns
        .iter()
        .map(|&x| {
            let diff = x - avg_ns;
            diff * diff
        })
        .sum::<f64>()
        / n as f64;
    let std_dev_ns = variance.sqrt();

    BenchmarkStats {
        name: name.to_string(),
        iterations: total_iterations,
        min_ns,
        max_ns,
        avg_ns,
        p50_ns,
        p90_ns,
        p95_ns,
        p99_ns,
        std_dev_ns,
    }
}
