# New multi-dispatch rule make previous hack no-longer necessary
# But we might need parametric polymorphism for refinement
fun add(a: i64, b: i64) -> i64 @{
entry:
    %tmp = add i64 %a, %b
    ret i64 %tmp
}

fun add(a: i32, b: i32) -> i32 @{
entry:
    %tmp = add i32 %a, %b
    ret i32 %tmp
}

fun add(a: f64, b: f64) -> f64 @{
entry:
    %tmp = fadd double %a, %b
    ret double %tmp
}

fun add(a: f32, b: f32) -> f32 @{
entry:
    %tmp = fadd float %a, %b
    ret float %tmp
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

fun subtract(a: i32, b: i32) -> i32 @{
entry:
    %tmp = sub i32 %a, %b
    ret i32 %tmp
}

fun subtract(a: f32, b: f32) -> f32 @{
entry:
    %tmp = fsub float %a, %b
    ret float %tmp
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

fun multiply(a: i32, b: i32) -> i32 @{
entry:
    %tmp = mul i32 %a, %b
    ret i32 %tmp
}

fun multiply(a: f32, b: f32) -> f32 @{
entry:
    %tmp = fmul float %a, %b
    ret float %tmp
}

fun divide(a: i64, b: i64) -> i64 @{
entry:
    %tmp = sdiv i64 %a, %b
    ret i64 %tmp
}

fun divide(a: f64, b: f64) -> f64 @{
entry:
    %tmp = fdiv double %a, %b
    ret double %tmp
}

fun divide(a: i32, b: i32) -> i32 @{
entry:
    %tmp = sdiv i32 %a, %b
    ret i32 %tmp
}

fun divide(a: f32, b: f32) -> f32 @{
entry:
    %tmp = fdiv float %a, %b
    ret float %tmp
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

