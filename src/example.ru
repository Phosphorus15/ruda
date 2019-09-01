parfun<x, y> scalarAdd(a: mut [[i32]], b: f64) -> i32{
    let mut line = @load(a, x);
    @store(line, y, @add(@load(line, y), b));
    return @mul(x, y);
}

parfun<x> sum(arr: [f64]) -> f64 {
    return @sum(arr);
}

fun arrCpy(dest: mut [f64], src: [f64]) {
    return;
}

fun add(a: i32, b: i32) -> i32 {
    let c = a;
    c = @add(c, b);
    return c;
}

fun dump_one() -> i32 {
    return 1;
}
