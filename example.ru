import src/arith.ru

fun test(a: f32, b: i64) -> i32 {
    return int(trunc(a + b));
}

fun calc(x: f32, b: i64, c: i32) -> f64 {
    let d = x * b + 3;
    return d - c * x / 5;
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
