fun test(a: f32, b: i64) -> i32 {
    return int(trunc(a + b));
}

parfun<x> run(a: [f32], b: [i64], c: mut [i32]) {
    @store(c, x, test(@load(a, x), @load(b, x)));
    return;
}