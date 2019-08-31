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
use std::ptr::null;
use llvm::prelude::{LLVMContextRef, LLVMModuleRef, LLVMValueRef};
use std::ops::{Deref, DerefMut};
use std::ffi::{CStr, CString};

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
    FuncCall(String, Box<BaseExpr>),
    LetDecl(String, bool, Box<BaseExpr>),
    Assign(Box<BaseExpr>, Box<BaseExpr>),
    Return(Box<BaseExpr>),
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
    vec![]
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
                    _ => LLVMVoidType()
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
        _ => unsafe { LLVMVoidType() }
    }
}

fn llvm_declare_par_func(decl: BaseExpr, context: LLVMContextRef, module: LLVMModuleRef) -> *mut LLVMType {
    if let BaseExpr::FuncDecl {
        ident, para_in, is_par, params, ret, body
    } = decl {
        let ret_type = map_type(&ret, context, false, is_par);
        let mut param_types: Vec<_> = params.iter().map(|p| map_type(&p.1, context, false, is_par)).collect();
        let func = unsafe { LLVMFunctionType(ret_type, param_types.deref_mut().as_mut_ptr(), param_types.len() as u32, 0) };
        let ident_str = CString::new(ident);
        let mut func_obj = unsafe { LLVMAddFunction(module, ident_str.unwrap().as_ptr(), func) };
        for i in 0..params.len() {
            let param_str = CString::new(params[i].0.clone());
            unsafe { LLVMSetValueName(LLVMGetParam(func_obj, i as u32), param_str.unwrap().as_ptr()); }
        }
        if is_par {
            unsafe {
                let node = LLVMMDNodeInContext(context,
                                               [func_obj, LLVMMDString(b"kernel\0".as_ptr() as *mut _, 6),
                                                   LLVMConstInt(LLVMInt32Type(), 1, 1)].as_mut_ptr(), 3);
                LLVMAddNamedMetadataOperand(module, b"nvvm.annotations\0".as_ptr() as *mut i8, node)
            }
        }
        return func;
    } else {
        panic!("Unable to resolve function")
    }
}

fn main() {
    let mut str = String::new();
    std::fs::File::open("./src/example.ru").unwrap().read_to_string(&mut str);
    unsafe {
// Set up a context, module and builder in that context.
        let context = LLVMContextCreate();
        let module = LLVMModuleCreateWithNameInContext(b"canoe\0".as_ptr() as *const _, context);
        let builder = LLVMCreateBuilderInContext(context);
        let parser = RudaParser::parse(Rule::base, &str).unwrap_or_else(|e| panic!("{}", e));
        for func in walk_pairs(parser) {
            llvm_declare_par_func(func, context, module);
        }
        LLVMDisposeBuilder(builder);
        LLVMDumpModule(module);
    }
}
