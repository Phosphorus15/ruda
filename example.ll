; ModuleID = 'canoe'

declare i32 @scalarAdd(i32 addrspace(1)* addrspace(1)* %a, double %b)

declare double @sum(double addrspace(4)* %arr)

declare void @arrCpy(double* %dest, double* %src)

declare i32 @add(i32 %a, i32 %b)

!nvvm.annotations = !{!0, !1}

!0 = !{i32 (i32 addrspace(1)* addrspace(1)*, double)* @scalarAdd, !"kernel", i32 1}
!1 = !{double (double addrspace(4)*)* @sum, !"kernel", i32 1}
