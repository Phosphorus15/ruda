use crate::parser::*;
use std::ffi::CString;
use std::collections::HashMap;

use llvm::analysis::LLVMVerifyFunction;
use llvm::analysis::LLVMVerifierFailureAction::{ LLVMAbortProcessAction};
use std::ptr::{null_mut};
use llvm::prelude::{LLVMContextRef, LLVMModuleRef, LLVMValueRef, LLVMBuilderRef};
use llvm::core::*;
use llvm::LLVMType;
use std::ops::DerefMut;

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

fn build_recurse_expr(expr: BaseExpr, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, val_context: &mut Vec<HashMap<String, LLVMValueRef>>) -> LLVMValueRef {
    match expr {
        BaseExpr::ConstantFloat(v) => unsafe { LLVMConstReal(LLVMDoubleTypeInContext(context), v) },
        BaseExpr::ConstantInt(v) => unsafe { LLVMConstInt(LLVMInt64TypeInContext(context), v as u64, 1) },
        BaseExpr::Return(ret) => {
            unsafe { LLVMBuildRet(builder, build_recurse_expr(*ret, context, module, builder, val_context)) }
        }
        BaseExpr::RetNull => {
            unsafe { LLVMBuildRetVoid(builder) }
        }
        BaseExpr::LetDecl(id, _mutate, value) => {
            let value = build_recurse_expr(*value, context, module, builder, val_context);
            let state: &mut _ = val_context.last_mut().unwrap();
            state.insert(id, value);
            null_mut()
        }
        BaseExpr::Ident(id) => {
            *val_context.iter_mut().find(|map|
                map.contains_key(&id[..])).expect(format!("Could not find variable `{}` in current context !", id).as_str()).get_mut(&id[..]).unwrap()
        }
        BaseExpr::FuncCall(ident, params) => {
            let mut resolved: Vec<_> = params.into_iter()
                .map(|v| build_recurse_expr(v, context, module, builder, val_context)).collect();
            if ident.starts_with("@") {
                build_intrinsics(ident, resolved, context, module, builder)
            } else {
                let cstring = CString::new(ident);
                let func_ptr = unsafe { LLVMGetNamedFunction(module, cstring.unwrap().as_ptr()) };
                unsafe { LLVMBuildCall(builder, func_ptr, resolved.deref_mut().as_mut_ptr(), resolved.len() as u32, b"calltmp\0".as_ptr() as *mut _) }
            }
        }
        _ => {
            null_mut()
        }
    }
}

pub enum AddressSpace {
    Generic = 0,
    Global = 1,
    Constant = 4,
}

fn build_intrinsics(id: String, mut params: Vec<LLVMValueRef>, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef) -> LLVMValueRef {
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

fn build_trivial_body(decl: Vec<BaseExpr>, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, val_context: &mut Vec<HashMap<String, LLVMValueRef>>) {
    for expr in decl {
        build_recurse_expr(expr, context, module, builder, val_context);
    }
}

pub(crate) fn llvm_declare_par_func(decl: BaseExpr, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, nv: &NVIntrinsics) -> *mut LLVMType {
    if let BaseExpr::FuncDecl {
        ident, para_in, is_par, params, ret, body
    } = decl {
        let mut val_context = vec![HashMap::new()];
        let ret_type = map_type(&ret, context, false, is_par);
        let mut param_types: Vec<_> = params.iter().map(|p| map_type(&p.1, context, false, is_par)).collect();
        let func = unsafe { LLVMFunctionType(ret_type, param_types.deref_mut().as_mut_ptr(), param_types.len() as u32, 0) };
        let ident_str = CString::new(ident);
        let func_obj = unsafe { LLVMAddFunction(module, ident_str.unwrap().as_ptr(), func) };
        let mut base_var = HashMap::<String, LLVMValueRef>::new();
        for i in 0..params.len() {
            let param_str = CString::new(params[i].0.clone());
            unsafe {
                let val = LLVMGetParam(func_obj, i as u32);
                base_var.insert(params[i].0.clone(), val);
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
                        base_var.insert(par, val);
                    }
                    cnt = cnt + 1;
                }
            }
        }
        val_context.push(base_var);
        build_trivial_body(body, context, module, builder, &mut val_context);
        unsafe { LLVMVerifyFunction(func_obj, LLVMAbortProcessAction); };
        return func;
    } else {
        panic!("Unable to resolve function")
    }
}

pub struct NVIntrinsics {
    thread_x: LLVMValueRef,
    thread_y: LLVMValueRef,
    thread_z: LLVMValueRef,
    sync_thread: LLVMValueRef,
}

pub(crate) fn init_nvptx_intrinsics(context: LLVMContextRef, module: LLVMModuleRef) -> NVIntrinsics {
    unsafe {
        let type1 = LLVMFunctionType(LLVMInt32TypeInContext(context), [].as_mut_ptr(), 0, 0);
        let type_barrier = LLVMFunctionType(LLVMVoidTypeInContext(context), [].as_mut_ptr(), 0, 0);
        NVIntrinsics {
            thread_x: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.x\0".as_ptr() as *mut _, type1),
            thread_y: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.y\0".as_ptr() as *mut _, type1),
            thread_z: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.z\0".as_ptr() as *mut _, type1),
            sync_thread: LLVMAddFunction(module, b"llvm.nvvm.barrier0\0".as_ptr() as *mut _, type_barrier),
        }
    }
}