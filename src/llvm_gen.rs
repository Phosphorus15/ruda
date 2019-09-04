use crate::parser::*;
use std::ffi::CString;
use std::collections::HashMap;

use llvm::analysis::LLVMVerifyFunction;
use llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction;
use std::ptr::null_mut;
use llvm::prelude::{LLVMContextRef, LLVMModuleRef, LLVMValueRef, LLVMBuilderRef};
use llvm::core::*;
use llvm::LLVMType;
use std::ops::DerefMut;

/*
Current primitives approach :
    i32 :< i64
    f32 :< f64
    i64 :< f64
    i32 :< f32
*/
fn subtype_check(t: &TyName, s: &TyName) -> bool {
    println!("subtype check src: {:?} dest: {:?}", t, s);
    if *s == *t {
        return true;
    }
    if let (TyName::NameBind(src), TyName::NameBind(dest)) = (t, s) {
        println!("name bind comparison src: {} dest: {}", src, dest);
        if src[..].eq("i32") {
            return subtype_check(&TyName::NameBind(String::from("i64")), s)
                || subtype_check(&TyName::NameBind(String::from("f32")), s);
        }
        if src[..].eq("f32") {
            return subtype_check(&TyName::NameBind(String::from("f64")), s);
        }
        if src[..].eq("i64") {
            return subtype_check(&TyName::NameBind(String::from("f64")), s);
        }
    }
    false
}

fn gen_subtype_cast(src: &TyName, dest: &TyName, src_val: LLVMValueRef, context: LLVMContextRef, builder: LLVMBuilderRef) -> LLVMValueRef {
    if src == dest { return src_val; }
    if let (TyName::NameBind(src_name), TyName::NameBind(dest_name)) = (src, dest) {
        match (&src_name[..], &dest_name[..]) {
            ("i32", "i64") => {
                return unsafe { LLVMBuildSExtOrBitCast(builder, src_val, LLVMInt64TypeInContext(context), b"casttmp\0".as_ptr() as *mut _) };
            }
            ("i32", "f32") => {
                return unsafe { LLVMBuildBitCast(builder, src_val, LLVMFloatTypeInContext(context), b"casttmp\0".as_ptr() as *mut _) };
            }
            ("i32", "f64") => {
                return unsafe { LLVMBuildBitCast(builder, src_val, LLVMDoubleTypeInContext(context), b"casttmp\0".as_ptr() as *mut _) };
            }
            ("i64", "f64") => {
                return unsafe { LLVMBuildBitCast(builder, src_val, LLVMDoubleTypeInContext(context), b"casttmp\0".as_ptr() as *mut _) };
            }
            ("f32", "f64") => {
                return unsafe { LLVMBuildFPExt(builder, src_val, LLVMDoubleTypeInContext(context), b"casttmp\0".as_ptr() as *mut _) };
            }
            _ => {}
        }
    }
    src_val // This should raise an error instead
    // But we just slide it to llvm to raise error for now
}


fn map_type(ty: &TyName, context: LLVMContextRef, set_mut: bool, device_side: bool) -> *mut LLVMType {
    match ty {
        TyName::NameBind(name) => {
            unsafe {
                match &name[..] {
                    "i32" => LLVMInt32TypeInContext(context),
                    "i64" => LLVMInt64TypeInContext(context),
                    "f32" => LLVMFloatTypeInContext(context),
                    "f64" => LLVMDoubleTypeInContext(context),
                    _ => LLVMVoidTypeInContext(context)
                }
            }
        }
        TyName::Array(ty) => {
// set to constant address space if necessary
            unsafe { LLVMPointerType(map_type(&**ty, context, set_mut, device_side), if device_side { if set_mut { 1 } else { 4 } } else { 0 }) }
        }
        TyName::MutBind(ty) => {
            map_type(&**ty, context, true, device_side)
        }
        _ => unsafe { LLVMVoidTypeInContext(context) }
    }
}

fn build_recurse_expr(expr: BaseExpr, module_decl: &HashMap<String, Vec<(LLVMValueRef, TyName)>>, expected_ret: TyName, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, val_context: &mut Vec<HashMap<String, (LLVMValueRef, TyName)>>) -> (LLVMValueRef, TyName) {
    match expr {
        BaseExpr::ConstantFloat(v) => unsafe { (LLVMConstReal(LLVMDoubleTypeInContext(context), v), TyName::NameBind(String::from("f64"))) },
        BaseExpr::ConstantInt(v) => unsafe { (LLVMConstInt(LLVMInt64TypeInContext(context), v as u64, 1), TyName::NameBind(String::from("i64"))) },
        BaseExpr::Return(ret) => {
            let val = build_recurse_expr(ret.0, module_decl, expected_ret.clone(), context, module, builder, val_context);
            assert!(subtype_check(dbg!(&val.1), dbg!(&expected_ret)));
            unsafe { (LLVMBuildRet(builder, val.0), val.1) }
        }
        BaseExpr::RetNull => {
            unsafe { (LLVMBuildRetVoid(builder), TyName::Unit) }
        }
        BaseExpr::LetDecl(id, _mutate, value) => {
            let value = build_recurse_expr(value.0, module_decl, expected_ret, context, module, builder, val_context);
            let state: &mut _ = val_context.last_mut().unwrap();
            state.insert(id, value.clone());
            value
        }
        BaseExpr::Ident(id) => {
            val_context.iter_mut().find(|map|
                map.contains_key(&id[..])).expect(format!("Could not find variable `{}` in current context !", id).as_str()).get_mut(&id[..]).unwrap().clone()
        }
        BaseExpr::FuncCall(ident, params) => {
            let mut resolved: Vec<_> = params.into_iter()
                .map(|v| build_recurse_expr(v.0, module_decl, expected_ret.clone(), context, module, builder, val_context)).collect();
            if ident.starts_with("@") {
                (build_intrinsics(ident, resolved, context, module, builder), TyName::NameBind(String::from("i64")))
            } else {
                let empty = vec![];
                let func_decls = module_decl.get(&ident[..]).unwrap_or(&empty);
                //let cstring = CString::new(ident);
                let mut target_ref: LLVMValueRef = null_mut();
                let mut ret_val = TyName::Unit;
                for (func_ref, ty) in func_decls {
                    println!("decl {:?}", ty);
                    if let TyName::Arrow(box_params, ret) = ty {
                        if let TyName::Tuple(params) = (**box_params).clone() {
                            if params.len() == resolved.len() {
                                if params.iter().zip(resolved.iter().map(|v| &v.1))
                                    .all(|(t1, t2)| dbg!(subtype_check(t2, t1))) {
                                    target_ref = *func_ref;
                                    ret_val = (**ret).clone();
                                    for i in 0..params.len() {
                                        resolved[i].0 = gen_subtype_cast(&resolved[i].1.clone(), &params[i].clone(), resolved[i].0, context, builder);
                                        resolved[i].1 = params[i].clone();
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                assert!(!target_ref.is_null());
                unsafe {
                    let param_len = resolved.len();
                    (LLVMBuildCall(builder, target_ref,
                                   resolved.into_iter().map(|v| v.0).collect::<Vec<_>>().deref_mut().as_mut_ptr(), param_len as u32, b"calltmp\0".as_ptr() as *mut _), ret_val)
                }
            }
        }
        _ => {
            (null_mut(), TyName::Unit)
        }
    }
}

pub enum AddressSpace {
    Generic = 0,
    Global = 1,
    Constant = 4,
}

fn build_intrinsics(id: String, mut typed_params: Vec<(LLVMValueRef, TyName)>, _context: LLVMContextRef, _module: LLVMModuleRef, builder: LLVMBuilderRef) -> LLVMValueRef {
    let mut params = typed_params.into_iter().map(|v| v.0).collect::<Vec<_>>();
    println!("{}", id);
    match &id[..] {
        "@add" | "@+" => unsafe { LLVMBuildAdd(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@mul" | "@*" => unsafe { LLVMBuildMul(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@div" | "@/" => unsafe { LLVMBuildExactSDiv(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@sub" | "@-" => unsafe { LLVMBuildSub(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@load" => unsafe {
            let ptr = LLVMBuildGEP(builder, params[0], &mut (params[1].clone()) as *mut _, 1, b"loadtmp\0".as_ptr() as *mut _);
            LLVMBuildLoad(builder, ptr, b"tmp\0".as_ptr() as *mut _)
        }
        "@store" => unsafe {
            assert!(LLVMGetPointerAddressSpace(LLVMTypeOf(params[0])) != AddressSpace::Constant as u32);
            let ptr = LLVMBuildGEP(builder, params[0], &mut (params[1].clone()) as *mut _, 1, b"loadtmp\0".as_ptr() as *mut _);
            LLVMBuildStore(builder, params[2], ptr)
        }
        _ => null_mut()
    }
}

fn build_trivial_body(decl: Vec<BaseExpr>, module_decl: &HashMap<String, Vec<(LLVMValueRef, TyName)>>, expected_ret: TyName, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, val_context: &mut Vec<HashMap<String, (LLVMValueRef, TyName)>>) {
    for expr in decl {
        build_recurse_expr(expr, module_decl, expected_ret.clone(), context, module, builder, val_context);
    }
}

pub(crate) fn llvm_declare_func(decl: BaseExpr, context: LLVMContextRef, module: LLVMModuleRef, _builder: LLVMBuilderRef) -> (LLVMValueRef, String) {
    if let BaseExpr::FuncDecl {
        ident, para_in: _, is_par, params, ret, body: _
    } = decl {
        let ret_type = map_type(&ret, context, false, is_par);
        let mut param_types: Vec<_> = params.iter().map(|p| map_type(&p.1, context, false, is_par)).collect();
        let func = unsafe { LLVMFunctionType(ret_type, param_types.deref_mut().as_mut_ptr(), param_types.len() as u32, 0) };
        let ident_str = CString::new(ident.clone());
        let func_obj = unsafe { LLVMAddFunction(module, ident_str.unwrap().as_ptr(), func) };
        for i in 0..params.len() {
            let param_str = CString::new(params[i].0.clone());
            unsafe {
                let val = LLVMGetParam(func_obj, i as u32);
                LLVMSetValueName(val, param_str.unwrap().as_ptr());
            }
        }
        if is_par {
            unsafe {
                let node = LLVMMDNodeInContext(context,
                                               [func_obj, LLVMMDString(b"kernel\0".as_ptr() as *mut _, 6),
                                                   LLVMConstInt(LLVMInt32Type(), 1, 1)].as_mut_ptr(), 3);
                LLVMAddNamedMetadataOperand(module, b"nvvm.annotations\0".as_ptr() as *mut i8, node);
            }
        }
        return (func_obj, ident);
    } else {
        panic!("Unable to resolve function")
    }
}

pub(crate) fn llvm_define_func(decl: TypedExpr, func_ref: LLVMValueRef, module_decl: &HashMap<String, Vec<(LLVMValueRef, TyName)>>, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, nv: &NVIntrinsics) -> LLVMValueRef {
    if let BaseExpr::FuncDecl {
        ident: _, para_in, is_par, params, ret, body
    } = decl.0 {
        let mut val_context = vec![HashMap::new()];
        let func_obj = func_ref;
        let mut base_var = HashMap::<String, (LLVMValueRef, TyName)>::new();
        for i in 0..params.len() {
            let param_str = CString::new(params[i].0.clone());
            unsafe {
                let val = LLVMGetParam(func_obj, i as u32);
                base_var.insert(params[i].0.clone(), (val, params[i].1.clone()));
                LLVMSetValueName(val, param_str.unwrap().as_ptr());
            }
        }
        unsafe { LLVMPositionBuilderAtEnd(builder, LLVMAppendBasicBlockInContext(context, func_obj, b"entry\0".as_ptr() as *const _)); }
        if is_par {
            unsafe {
                let node = LLVMMDNodeInContext(context,
                                               [func_obj, LLVMMDString(b"kernel\0".as_ptr() as *mut _, 6),
                                                   LLVMConstInt(LLVMInt32Type(), 1, 1)].as_mut_ptr(), 3);
                LLVMAddNamedMetadataOperand(module, b"nvvm.annotations\0".as_ptr() as *mut i8, node);
                // Load index from intrinsics
                let mut cnt = 0;
                for par in para_in {
                    if par.ne(&String::from("_")) {
                        let cstring = CString::new(par.clone());
                        let val = LLVMBuildCall(builder, match cnt {
                            0 => nv.thread_x,
                            1 => nv.thread_y,
                            2 => nv.thread_z,
                            _ => nv.thread_x
                        }, [].as_mut_ptr(), 0, cstring.unwrap().as_ptr());
                        base_var.insert(par, (val, TyName::NameBind(String::from("i64"))));
                    }
                    cnt = cnt + 1;
                }
            }
        }
        val_context.push(base_var);
        build_trivial_body(body.into_iter().map(|v| v.0).collect(), module_decl, ret, context, module, builder, &mut val_context);
        unsafe { LLVMVerifyFunction(func_obj, LLVMAbortProcessAction); };
        return func_obj;
    } else {
        panic!("Unable to resolve function")
    }
}

#[allow(dead_code)]
pub struct NVIntrinsics {
    thread_x: LLVMValueRef,
    thread_y: LLVMValueRef,
    thread_z: LLVMValueRef,
    sync_thread: LLVMValueRef,
}

pub(crate) fn init_nvptx_intrinsics(context: LLVMContextRef, module: LLVMModuleRef) -> NVIntrinsics {
    unsafe {
        let type_cu_index = LLVMFunctionType(LLVMInt32TypeInContext(context), [].as_mut_ptr(), 0, 0);
        let type_barrier = LLVMFunctionType(LLVMVoidTypeInContext(context), [].as_mut_ptr(), 0, 0);
        NVIntrinsics {
            thread_x: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.x\0".as_ptr() as *mut _, type_cu_index),
            thread_y: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.y\0".as_ptr() as *mut _, type_cu_index),
            thread_z: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.z\0".as_ptr() as *mut _, type_cu_index),
            sync_thread: LLVMAddFunction(module, b"llvm.nvvm.barrier0\0".as_ptr() as *mut _, type_barrier),
        }
    }
}