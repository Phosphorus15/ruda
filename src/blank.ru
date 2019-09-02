fun add(a: i64, b: i64) -> i64 {
    return @add(a, b);
}

parfun<x> scalarAdd(arr: mut [i64], v: i64) {
    let c = @load(arr, x); # load intrinsic
    @store(arr, x, @add(c, v));
    return;
}

parfun<x, y> transpose(matrix: mut [[i64]]) {
    let l1 = @load(matrix, x);
    let l2 = @load(matrix, y);

    let v = @load(l1, y);

    @store(l2, x, v);
    return;
}
