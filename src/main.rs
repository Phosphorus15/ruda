extern crate llvm_sys as llvm;

use std::mem;

use llvm::core::*;
use llvm::execution_engine::*;
use llvm::target::*;

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use pest::iterators::Pair;
use std::io::Read;
use llvm::LLVMType;
use std::ptr::{null, null_mut};
use llvm::prelude::{LLVMContextRef, LLVMModuleRef, LLVMValueRef, LLVMBuilderRef};
use std::ops::{Deref, DerefMut};
use std::ffi::{CStr, CString};
use std::collections::HashMap;
use llvm::analysis::LLVMVerifyFunction;
use llvm::analysis::LLVMVerifierFailureAction::{LLVMPrintMessageAction, LLVMAbortProcessAction};
use std::env::args;

#[derive(Parser)]
#[grammar = "ruda.pest"]
pub struct RudaParser;

type RuleList<'a> = Vec<Pair<'a, Rule>>;

#[derive(Debug)]
pub enum TyName {
    NameBind(String),
    MutBind(Box<Self>),
    Array(Box<Self>),
    Unit,
}

#[derive(Debug)]
pub enum BaseExpr {
    FuncDecl { ident: String, para_in: Vec<String>, is_par: bool, params: Vec<(String, TyName)>, ret: TyName, body: Vec<BaseExpr> },
    FuncCall(String, Vec<BaseExpr>),
    LetDecl(String, bool, Box<BaseExpr>),
    Assign(String, Box<BaseExpr>),
    Return(Box<BaseExpr>),
    RetNull,
    Ident(String),
    ConstantFloat(f64),
    ConstantInt(i64),
    Nope,
}

fn walk_pairs(pairs: pest::iterators::Pairs<Rule>) -> Vec<BaseExpr> {
    let vec = pairs.clone().collect::<RuleList>();
    let rules: RuleList = vec.into_iter().next().unwrap().into_inner().collect();
    rules.into_iter().map(walk_func).collect()
}

macro_rules! parse_param {
( $ x: expr) => {$x.map( | item| {
let pair: Vec < _ > = item.into_inner().collect();
return (pair[0].as_span().as_str().to_string(), walk_ty(pair[1].clone()));
}).collect()
}
}

fn walk_func(func: Pair<Rule>) -> BaseExpr {
    let vec: RuleList = func.into_inner().collect();
    let decl: RuleList = vec[0].clone().into_inner().collect();
    let body: RuleList = vec[1].clone().into_inner().collect();
    let mut para_in: Vec<String> = vec![];
    let ret_type = if decl.last().unwrap().as_rule() == Rule::ret_type {
        walk_ty(decl.last().unwrap().clone())
    } else {
        TyName::Unit
    };
    if decl[0].as_rule() == Rule::parfun_decl {
        para_in = decl[0].clone().into_inner().next().unwrap().clone().into_inner().map(|i| i.as_span().as_str().to_string()).collect();
        let name = decl[1].as_span().as_str().to_string();
        let params: Vec<_> = parse_param!(decl[2].clone().into_inner());
        return BaseExpr::FuncDecl {
            ident: name,
            para_in,
            params,
            ret: ret_type,
            is_par: true,
            body: walk_fun_body(body),
        };
    } else {
        let name = decl[0].as_span().as_str().to_string();
        let params: Vec<_> = parse_param!(decl[1].clone().into_inner());
        return BaseExpr::FuncDecl {
            ident: name,
            para_in,
            params,
            ret: ret_type,
            is_par: false,
            body: walk_fun_body(body),
        };
    }
}

fn walk_fun_body(body: RuleList) -> Vec<BaseExpr> {
    body.into_iter().map(|expr| {
        if expr.as_rule() == Rule::base_expr {
            let inner = expr.into_inner().take(1).collect::<RuleList>()[0].clone();
            return match inner.as_rule() {
                Rule::return_expr => {
                    let vec = inner.into_inner().collect::<RuleList>();
                    if vec.len() == 0 {
                        BaseExpr::RetNull
                    } else {
                        let value_expr = vec[0].clone();
                        BaseExpr::Return(Box::new(walk_value_expr(value_expr.into_inner().collect())))
                    }
                }
                Rule::let_expr => {
                    let composition = inner.into_inner().collect::<RuleList>();
                    let mut base = 0;
                    if composition[base].as_rule() == Rule::mut_let {
                        base = 1;
                    }
                    let id = composition[base].as_str().to_string();
                    let val = walk_value_expr(composition[base + 1].clone().into_inner().collect());
                    BaseExpr::LetDecl(id, base > 0, Box::new(val))
                }
                Rule::assignment => {
                    let composition = inner.into_inner().collect::<RuleList>();
                    let id = composition[0].as_str().to_string();
                    let val = walk_value_expr(composition[1].clone().into_inner().collect());
                    BaseExpr::Assign(id, Box::new(val))
                }
                Rule::value_expr => {
                    walk_value_expr(inner.into_inner().collect())
                }
                _ => {
                    BaseExpr::Nope
                }
            };
        } else {
            BaseExpr::Nope
        }
    }
    ).collect()
}

fn walk_value_expr(body: RuleList) -> BaseExpr {
    let primary = body[0].clone();
    match primary.as_rule() {
        Rule::value => {
            walk_value_node(primary.into_inner().collect())
        }
        Rule::func_call => {
            let composition = primary.into_inner().collect::<RuleList>();
            let id = composition[0].as_str().to_string();
            BaseExpr::FuncCall(id, composition.into_iter().skip(1)
                .map(|v| walk_value_expr(v.into_inner().collect())).collect())
        }
        _ => { BaseExpr::Nope }
    }
}

fn walk_value_node(body: RuleList) -> BaseExpr {
    let inner = body[0].clone();
    if inner.as_rule() == Rule::ident {
        return BaseExpr::Ident(inner.as_str().to_string());
    } else if inner.as_rule() == Rule::number {
        return BaseExpr::ConstantInt(inner.as_str().to_string().parse().unwrap());
    }
    BaseExpr::Nope
}

fn walk_ty(ty: Pair<Rule>) -> TyName {
    if ty.as_rule() == Rule::ident {
        return TyName::NameBind(ty.as_span().as_str().to_string());
    } else if ty.as_rule() == Rule::mut_type {
        return TyName::MutBind(Box::new(walk_ty(ty.into_inner().next().unwrap())));
    } else if ty.as_rule() == Rule::arr_type {
        return TyName::Array(Box::new(walk_ty(ty.into_inner().next().unwrap())));
    } else if ty.as_rule() == Rule::type_ident || ty.as_rule() == Rule::immut_type || ty.as_rule() == Rule::ret_type {
        return walk_ty(ty.into_inner().next().unwrap());
    } else {
        return TyName::Unit;
    }
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
        BaseExpr::LetDecl(id, mutate, value) => {
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

fn build_intrinsics(id: String, mut params: Vec<LLVMValueRef>, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef) -> LLVMValueRef {
    match &id[..] {
        "@add" => unsafe { LLVMBuildAdd(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@mul" => unsafe { LLVMBuildMul(builder, params[0], params[1], b"tmp\0".as_ptr() as *mut _) }
        "@load" => unsafe {
            let ptr = LLVMBuildGEP(builder, params[0], &mut (params[1].clone()) as *mut _, 1, b"loadtmp\0".as_ptr() as *mut _);
            LLVMBuildLoad(builder, ptr, b"tmp\0".as_ptr() as *mut _)
        }
        "@store" => unsafe {
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

fn llvm_declare_par_func(decl: BaseExpr, context: LLVMContextRef, module: LLVMModuleRef, builder: LLVMBuilderRef, nv: &NVIntrinsics) -> *mut LLVMType {
    if let BaseExpr::FuncDecl {
        ident, para_in, is_par, params, ret, body
    } = decl {
        let mut val_context = vec![HashMap::new()];
        let ret_type = map_type(&ret, context, false, is_par);
        let mut param_types: Vec<_> = params.iter().map(|p| map_type(&p.1, context, false, is_par)).collect();
        let func = unsafe { LLVMFunctionType(ret_type, param_types.deref_mut().as_mut_ptr(), param_types.len() as u32, 0) };
        let ident_str = CString::new(ident);
        let mut func_obj = unsafe { LLVMAddFunction(module, ident_str.unwrap().as_ptr(), func) };
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

struct NVIntrinsics {
    thread_x: LLVMValueRef,
    thread_y: LLVMValueRef,
    thread_z: LLVMValueRef,
    sync_thread: LLVMValueRef,
}

fn init_nvptx_intrinsics(context: LLVMContextRef, module: LLVMModuleRef) -> NVIntrinsics {
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

fn main() {
    if args().len() < 2 { panic!("ruda - no input file") };
    let mut str = String::new();
    std::fs::File::open(args().last().unwrap().as_str()).unwrap().read_to_string(&mut str);
    unsafe {
// Set up a context, module and builder in that context.
        let context = LLVMContextCreate();
        let module = LLVMModuleCreateWithNameInContext(b"canoe\0".as_ptr() as *const _, context);
        let builder = LLVMCreateBuilderInContext(context);
        let parser = RudaParser::parse(Rule::base, &str).unwrap_or_else(|e| panic!("{}", e));
        let intrinsics = init_nvptx_intrinsics(context, module);
        for func in walk_pairs(parser) {
            llvm_declare_par_func(func, context, module, builder, &intrinsics);
        }
        LLVMDisposeBuilder(builder);
        LLVMDumpModule(module);
    }
}
