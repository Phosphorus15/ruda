fun test(a: f32, b: i64) -> i32 {
    while a > 0 {
        return int(a);
    };
    return int(trunc(a + b));
}

parfun<x> run(a: [f32], b: [i64], c: mut [i32]) {
    @store(c, x, test(@load(a, x), @load(b, x)));
    return;
}

parfun<x> cadd(a: mut [f32], b: [f32]) {
    let v1 = @load(a, x);
    let v2 = @load(b, x);
    @store(a, x, v1 + v2);
    return;
}
