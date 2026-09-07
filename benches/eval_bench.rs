use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tianshu_formula::{Formula, FormulaCache, MapContext};

fn bench_simple(c: &mut Criterion) {
    let f = Formula::parse("a + b * c").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("a", 1.0).insert("b", 2.0).insert("c", 3.0);
    c.bench_function("eval_simple_a+b*c", |b| {
        b.iter(|| f.eval(black_box(&ctx)).unwrap())
    });
}

fn bench_medium(c: &mut Criterion) {
    let f = Formula::parse("clamp(atk * (1 + crit) + flat, 0, if(cap > 0, cap, 99999))").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("atk", 120.0)
        .insert("crit", 0.25)
        .insert("flat", 15.0)
        .insert("cap", 200.0);
    c.bench_function("eval_medium_clamp_if", |b| {
        b.iter(|| f.eval(black_box(&ctx)).unwrap())
    });
}

fn bench_long(c: &mut Criterion) {
    // >= 50 nodes worth of ops
    let expr = (0..60)
        .map(|i| format!("x{i}"))
        .collect::<Vec<_>>()
        .join("+");
    let f = Formula::parse(&expr).unwrap();
    let mut ctx = MapContext::new();
    for i in 0..60 {
        ctx.insert(format!("x{i}"), 1.0);
    }
    c.bench_function("eval_long_60_adds", |b| {
        b.iter(|| f.eval(black_box(&ctx)).unwrap())
    });
}

fn bench_batch_10k(c: &mut Criterion) {
    let f = Formula::parse("a * 2 + b").unwrap();
    let contexts: Vec<MapContext> = (0..10_000)
        .map(|i| {
            let mut ctx = MapContext::new();
            ctx.insert("a", i as f64).insert("b", 1.0);
            ctx
        })
        .collect();
    c.bench_function("eval_batch_10k", |b| {
        b.iter(|| f.eval_batch(black_box(&contexts)).unwrap())
    });
}

fn bench_parse_every_time_vs_cache(c: &mut Criterion) {
    const N: usize = 200;
    let src = "clamp(atk * (1 + crit) + flat, 0, if(cap > 0, cap, 99999))";
    let mut ctx = MapContext::new();
    ctx.insert("atk", 120.0)
        .insert("crit", 0.25)
        .insert("flat", 15.0)
        .insert("cap", 200.0);

    c.bench_function("parse_every_time_n200", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for _ in 0..N {
                let f = Formula::parse(black_box(src)).unwrap();
                sum += f.eval(black_box(&ctx)).unwrap();
            }
            sum
        })
    });

    c.bench_function("cached_parse_then_eval_n200", |b| {
        b.iter(|| {
            let mut cache = FormulaCache::new();
            let mut sum = 0.0;
            for _ in 0..N {
                let f = cache.get_or_parse(black_box(src)).unwrap();
                sum += f.eval(black_box(&ctx)).unwrap();
            }
            sum
        })
    });
}

criterion_group!(
    benches,
    bench_simple,
    bench_medium,
    bench_long,
    bench_batch_10k,
    bench_parse_every_time_vs_cache
);
criterion_main!(benches);
