# it's currently kind of hack to place the smaller type polymorphic decls. before larger types
# this situation should be ameliorated later
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

fun subtract(a: i64, b: i64) -> i64 @{
entry:
    %tmp = sub i64 %a, %b
    ret i64 %tmp
}

fun subtract(a: f64, b: f64) -> f64 @{
entry:
    %tmp = fsub double %a, %b
    ret double %tmp
}

fun multiply(a: i64, b: i64) -> i64 @{
entry:
    %tmp = mul i64 %a, %b
    ret i64 %tmp
}

fun multiply(a: f64, b: f64) -> f64 @{
entry:
    %tmp = fmul double %a, %b
    ret double %tmp
}

fun int(a: f32) -> i32 @{
entry:
    %tmp = fptosi float %a to i32
    ret i32 %tmp
}

fun int(a: f64) -> i64 @{
entry:
    %tmp = fptosi double %a to i64
    ret i64 %tmp
}

fun trunc(a: f64) -> f32 @{
    %tmp = fptrunc double %a to float
    ret float %tmp
}

fun trunc(a: i64) -> i32 @{
    %tmp = trunc i64 %a to i32
    ret i32 %tmp
}

