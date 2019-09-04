
fun test(a: f64, b: i64) -> i32 {
    return i32(int(a + b));
}

parfun<x> run(a: [f64], b: [i64], c: mut [i32]) {
    @store(c, x, test(@load(a, x), @load(b, x)));
    return;
}

fun add(a: i64, b: i64) -> i64 @{
entry:
    %tmp = add i64 %a, %b
    ret i64 %tmp
}

fun add(a: f64, b: f64) -> f64 @{
entry:
    %tmp = fadd double %a, %b
    ret double %tmp
}

fun int(a: f64) -> i64 @{
entry:
    %tmp = fptosi double %a to i64
    ret i64 %tmp
}

fun int(a: f32) -> i32 @{
entry:
    %tmp = fptosi float %a to i32
    ret i32 %tmp
}

fun f32(a: f64) -> f32 @{
    %tmp = fptrunc double %a to float
    ret float %tmp
}

fun i32(a: i64) -> i32 @{
    %tmp = trunc i64 %a to i32
    ret i32 %tmp
}

