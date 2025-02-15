use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rand::{thread_rng, Rng};
use wnfs_nameaccumulator::{AccumulatorSetup, NameAccumulator, NameSegment};

fn name_segment_from_digest(c: &mut Criterion) {
    c.bench_function("NameSegment::new_hashed", |b| {
        b.iter_batched(
            || thread_rng().gen::<[u8; 32]>(),
            |sth| NameSegment::new_hashed("wnfs benchmarks", sth),
            BatchSize::SmallInput,
        );
    });
}

fn name_segment_rng(c: &mut Criterion) {
    c.bench_function("NameSegment::new(rng)", |b| {
        b.iter(|| NameSegment::new(&mut thread_rng()));
    });
}

fn name_accumulator_add(c: &mut Criterion) {
    let setup = &AccumulatorSetup::from_rsa_2048(&mut thread_rng());
    c.bench_function("NameAccumulator::add", |b| {
        b.iter_batched(
            || NameSegment::new(&mut thread_rng()),
            |segment| NameAccumulator::empty(setup).add(Some(&segment), setup),
            BatchSize::SmallInput,
        )
    });
}

fn name_accumulator_serialize(c: &mut Criterion) {
    let setup = &AccumulatorSetup::from_rsa_2048(&mut thread_rng());
    c.bench_function("NameAccumulator serialization", |b| {
        b.iter_batched(
            || {
                let segment = NameSegment::new(&mut thread_rng());
                let mut name = NameAccumulator::empty(setup);
                name.add(Some(&segment), setup);
                name
            },
            |name| name.into_bytes(),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    name_segment_from_digest,
    name_segment_rng,
    name_accumulator_add,
    name_accumulator_serialize,
);

criterion_main!(benches);
