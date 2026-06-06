#![allow(clippy::unwrap_used)]

//! Bench для `delphi::filters::*` на той же synthetic data что TS-side
//! `products/delphi/tests/filterService.bench.ts:makeSynthetic(n)`.
//!
//! Запуск: `cargo bench --manifest-path core/ark/crates/ark-core/rust/Cargo.toml \
//!   --bench delphi_filters --features bench-fixtures`.

use ark_core::delphi::filters::fixtures::{make_synthetic, TODAY_ISO};
use ark_core::delphi::filters::{count_all, filter_todos};
use ark_core::delphi::types::SmartList;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("delphi_filters");
    for &n in &[1_000usize, 10_000usize] {
        let data = make_synthetic(n);
        for list in SmartList::ALL {
            group.bench_with_input(
                BenchmarkId::new(format!("filter_{}", list.as_str()), n),
                &n,
                |b, _| {
                    b.iter(|| {
                        let _ = filter_todos(list, black_box(&data), TODAY_ISO);
                    });
                },
            );
        }
        group.bench_with_input(BenchmarkId::new("count_all", n), &n, |b, _| {
            b.iter(|| {
                let _ = count_all(black_box(&data), TODAY_ISO);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_filter);
criterion_main!(benches);
