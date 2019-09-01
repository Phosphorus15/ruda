fun add(a: i64, b: i64) -> i64 {
    return @add(a, b);
}

parfun<x> scalarAdd(arr: mut [i64], v: i64) {
    let c = @load(arr, x); # load intrinsic
    @store(arr, x, add(c, v));
    return;
}
