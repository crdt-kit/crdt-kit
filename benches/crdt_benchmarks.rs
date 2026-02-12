use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crdt_kit::prelude::*;

fn bench_gcounter_increment(c: &mut Criterion) {
    c.bench_function("GCounter::increment x1000", |b| {
        b.iter(|| {
            let mut counter = GCounter::new("bench");
            for _ in 0..1000 {
                counter.increment();
            }
            black_box(counter.value())
        })
    });
}

fn bench_gcounter_merge(c: &mut Criterion) {
    let counters: Vec<GCounter> = (0..10)
        .map(|i| {
            let mut c = GCounter::new(format!("node-{i}"));
            c.increment_by(100);
            c
        })
        .collect();

    c.bench_function("GCounter::merge 10 replicas", |b| {
        b.iter(|| {
            let mut merged = counters[0].clone();
            for other in &counters[1..] {
                merged.merge(other);
            }
            black_box(merged.value())
        })
    });

    // Also benchmark with many more replicas
    let many_counters: Vec<GCounter> = (0..100)
        .map(|i| {
            let mut c = GCounter::new(format!("node-{i}"));
            c.increment_by(100);
            c
        })
        .collect();

    c.bench_function("GCounter::merge 100 replicas", |b| {
        b.iter(|| {
            let mut merged = many_counters[0].clone();
            for other in &many_counters[1..] {
                merged.merge(other);
            }
            black_box(merged.value())
        })
    });
}

fn bench_pncounter(c: &mut Criterion) {
    c.bench_function("PNCounter::inc+dec x1000", |b| {
        b.iter(|| {
            let mut counter = PNCounter::new("bench");
            for _ in 0..500 {
                counter.increment();
                counter.decrement();
            }
            black_box(counter.value())
        })
    });
}

fn bench_orset_insert(c: &mut Criterion) {
    c.bench_function("ORSet::insert x1000", |b| {
        b.iter(|| {
            let mut set = ORSet::new("bench");
            for i in 0..1000u32 {
                set.insert(i);
            }
            black_box(set.len())
        })
    });
}

fn bench_orset_merge(c: &mut Criterion) {
    let mut s1 = ORSet::new("a");
    let mut s2 = ORSet::new("b");

    for i in 0..500u32 {
        s1.insert(i);
        s2.insert(i + 250); // 50% overlap
    }

    c.bench_function("ORSet::merge 500+500 elements", |b| {
        b.iter(|| {
            let mut merged = s1.clone();
            merged.merge(&s2);
            black_box(merged.len())
        })
    });
}

fn bench_gset_merge(c: &mut Criterion) {
    let mut s1 = GSet::new();
    let mut s2 = GSet::new();

    for i in 0..1000u32 {
        s1.insert(i);
        s2.insert(i + 500);
    }

    c.bench_function("GSet::merge 1000+1000 elements", |b| {
        b.iter(|| {
            let mut merged = s1.clone();
            merged.merge(&s2);
            black_box(merged.len())
        })
    });
}

fn bench_lww_register_merge(c: &mut Criterion) {
    let registers: Vec<LWWRegister<String>> = (0..100)
        .map(|i| LWWRegister::with_timestamp(format!("node-{i}"), format!("value-{i}"), i))
        .collect();

    c.bench_function("LWWRegister::merge 100 replicas", |b| {
        b.iter(|| {
            let mut merged = registers[0].clone();
            for other in &registers[1..] {
                merged.merge(other);
            }
            black_box(merged.value().clone())
        })
    });
}

criterion_group!(
    benches,
    bench_gcounter_increment,
    bench_gcounter_merge,
    bench_pncounter,
    bench_orset_insert,
    bench_orset_merge,
    bench_gset_merge,
    bench_lww_register_merge,
);
criterion_main!(benches);
