parfun<x, y> scalaAdd(a: mut [[i32]], b: f64) -> i32{
    @set(a, x, y, @add(@get(a, x, y), b));
    return @mul(x, y);
}

parfun<x> sum(arr: mut [f64]) -> f64 {
    return @sum(arr);
}

fun arrCpy(dest: mut [f64], src: [f64]) {
}

fun add(a: i32, b: i32) -> i32 {
    return @add(a, b);
}
