use chrono::prelude::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vapix::v3::system_log::*;

pub fn criterion_benchmark(c: &mut Criterion) {
    let now = Local::now().into();

    c.bench_function("parse 5.51.7", |b| {
        let log = include_str!("system_log/5.51.7.log");
        b.iter(|| {
            let entries = Entries::new(black_box(String::from(log)), now);
            black_box(entries.iter().collect::<Vec<_>>());
        })
    });

    c.bench_function("parse 9.80.2.2", |b| {
        let log = include_str!("system_log/9.80.2.2.log");
        b.iter(|| {
            let entries = Entries::new(black_box(String::from(log)), now);
            black_box(entries.iter().collect::<Vec<_>>());
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
