use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use qck_backend::services::short_code::{Base62Codec, ShortCodeGenerator};

fn bench_base62_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("base62_encode");

    for value in [1u64, 100, 10000, 1000000, u64::MAX / 2].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(value), value, |b, &val| {
            b.iter(|| Base62Codec::encode(black_box(val)));
        });
    }
    group.finish();
}

fn bench_base62_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("base62_decode");

    let test_strings = vec![
        ("short", "abc"),
        ("medium", "abc123XYZ"),
        ("long", "abcdefghijklmnop"),
    ];

    for (name, input) in test_strings {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, input| {
            b.iter(|| Base62Codec::decode(black_box(input)));
        });
    }
    group.finish();
}

fn bench_random_generation(c: &mut Criterion) {
    let generator = ShortCodeGenerator::new(7, 5);

    c.bench_function("random_generation", |b| {
        b.iter(|| generator.generate_random_code(7));
    });
}

fn bench_validation(c: &mut Criterion) {
    let generator = ShortCodeGenerator::new(7, 5);
    let valid_code = "abc123Z";

    c.bench_function("code_validation", |b| {
        b.iter(|| generator.is_reserved_code(black_box(valid_code)));
    });
}

fn bench_concurrent_generation(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    c.bench_function("concurrent_generation_10_threads", |b| {
        b.iter(|| {
            let generator = Arc::new(ShortCodeGenerator::new(7, 5));
            let mut handles = vec![];

            for _ in 0..10 {
                let gen = Arc::clone(&generator);
                let handle = thread::spawn(move || {
                    for _ in 0..100 {
                        gen.generate_random_code(7);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn bench_collision_check(c: &mut Criterion) {
    use std::collections::HashSet;

    c.bench_function("collision_check_1000_codes", |b| {
        b.iter(|| {
            let generator = ShortCodeGenerator::new(7, 5);
            let mut codes = HashSet::new();

            for _ in 0..1000 {
                let code = generator.generate_random_code(7);
                codes.insert(code);
            }

            codes.len() >= 990 // Allow some duplicates in random generation
        });
    });
}

criterion_group!(
    benches,
    bench_base62_encode,
    bench_base62_decode,
    bench_random_generation,
    bench_validation,
    bench_concurrent_generation,
    bench_collision_check
);
criterion_main!(benches);
