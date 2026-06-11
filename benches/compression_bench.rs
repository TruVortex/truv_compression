use criterion::{Criterion, black_box, criterion_group, criterion_main};
use truv_compression::compress::compress;

fn bench_compression(c: &mut Criterion) {
    let mut mock_data = Vec::new();
    for _ in 0..10_000 {
        mock_data.extend_from_slice(b"ALGORITHM_TEST_DATA_PATTERN_");
    }

    c.bench_function("compress_mock_data", |b| {
        b.iter(|| {
            let mut out_buffer = Vec::new();
            compress(black_box(&mock_data), &mut out_buffer).unwrap();
        })
    });
}

criterion_group!(benches, bench_compression);
criterion_main!(benches);
