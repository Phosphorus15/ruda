
fun test(a: i64, b: i64) -> i64 {
    let c = 1 + a * 5 - b / 2;
    return c;
}

fun wrap(a: i32) -> i64 {
    return test(a, 3);
}

#fun @add(a: i64, b: i64) -> i64 @{
#    %tmp = add i64 %a, %b
#    ret i64 %tmp
#}
