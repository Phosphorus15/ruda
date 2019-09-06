extern crate llvm_sys as llvm;

mod parser;
mod llvm_gen;

use llvm::core::*;

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use std::io::Read;
//use std::env::args;

pub use crate::parser::*;
pub use crate::llvm_gen::*;
use llvm::transforms::scalar::{LLVMAddGVNPass, LLVMAddCFGSimplificationPass, LLVMAddReassociatePass};
use std::collections::HashMap;
use llvm::prelude::{LLVMValueRef, LLVMModuleRef};
use std::ffi::{CStr, CString};
use std::ops::{Add, AddAssign};
use llvm::link_time_optimizer::llvm_create_optimizer;
use std::ptr::null_mut;
use std::os::raw::c_char;
use llvm::linker::LLVMLinkModules2;
use llvm::analysis::LLVMVerifyModule;
use llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction;
use llvm::transforms::ipo::LLVMAddFunctionInliningPass;

fn main() {
    //if args().len() < 2 { panic!("ruda - no input file") };
    //let obj = args().last().unwrap().as_str();
    let mut internal_module = String::from("; ModuleID = 'canoe_kernel'\n\n");
    let mut file_list = vec!["src/arith.ru", "example.ru"];
    let file_strs = file_list.iter().map(|file| {
        let mut str = String::new();
        std::fs::File::open(*file).unwrap().read_to_string(&mut str).unwrap();
        str
    }).collect::<Vec<_>>();
    unsafe {
// Set up a context, module and builder in that context.
        let context = LLVMContextCreate();
        let module = LLVMModuleCreateWithNameInContext(b"canoe\0".as_ptr() as *const _, context);
        let builder = LLVMCreateBuilderInContext(context);

        let manager = LLVMCreateFunctionPassManagerForModule(module);
        let global_manager = LLVMCreatePassManager();
        //LLVMAddGlobalOptimizerPass(manager);
        LLVMAddGVNPass(manager);
        LLVMAddCFGSimplificationPass(manager);
        LLVMAddReassociatePass(manager);
        LLVMInitializeFunctionPassManager(manager);
        LLVMAddFunctionInliningPass(global_manager);
        let parsed_files = file_strs.iter().map(|str| {
            RudaParser::parse(Rule::file, str)
                .unwrap_or_else(|e| panic!("{}", e)).collect::<RuleList>()[0].clone().into_inner()
        });
//        dbg!(&parser);
        let mut module_vals = HashMap::<String, Vec<(LLVMValueRef, TyName)>>::new();
        let mut func_pairs: Vec<(TypedExpr, LLVMValueRef)> = vec![];
        let intrinsics = init_nvptx_intrinsics(context, module);
        for func in parsed_files.map(|parser| walk_pairs(parser)).flatten() {
            //dbg!(&func);
            let func_ref = llvm_declare_func(func.0.clone(), context, module, builder);
            let defs = module_vals.entry(func_ref.1).or_insert(vec![]);
            defs.push((func_ref.0, func.1.clone()));
            func_pairs.push((func, func_ref.0));
        }
        for func_pair in func_pairs.into_iter() {
            let ident = CStr::from_ptr(LLVMGetValueName(func_pair.1))
                .to_owned().into_string().unwrap();
            let renamed_ident = ident.replace(".", "_"); // nvvm do not allow names with `.`, so we just replace it
            let string = CString::new(renamed_ident).unwrap();
            LLVMSetValueName(func_pair.1, string.as_ptr() as *const _);
            if let BaseExpr::IntrinsicsFuncDecl(_, params, ret, body) = (func_pair.0).0 {
                let ident = CStr::from_ptr(LLVMGetValueName(func_pair.1))
                    .to_owned().into_string().unwrap();
                let func_def = llvm_embedded_ir(ident, params, ret, body, context);
                internal_module.add_assign(&func_def[..]);
                internal_module.add_assign("\n\n");
                continue;
            }
            if let BaseExpr::FuncVirtualDecl(_, params, ret) = (func_pair.0).0 {
                let ident = CStr::from_ptr(LLVMGetValueName(func_pair.1))
                    .to_owned().into_string().unwrap();
                let func_def = llvm_embedded_kernel_decl(ident, params, ret, context);
                internal_module.add_assign(&func_def[..]);
                continue;
            }
            llvm_define_func(func_pair.0, func_pair.1, &module_vals, context, module, builder, &intrinsics);
            LLVMRunFunctionPassManager(manager, func_pair.1);
        }
        let mut kernel_module: LLVMModuleRef = null_mut();
        let str_len = internal_module.len();
        let string = CString::new(internal_module).unwrap();
        let code_buffer = LLVMCreateMemoryBufferWithMemoryRange(string.as_ptr() as *const _, str_len, b"kernel_code".as_ptr() as *const _, 0);
        let mut ptr: *mut c_char = std::mem::uninitialized();
        let rt = llvm_sys::ir_reader::LLVMParseIRInContext(context, code_buffer, &mut kernel_module as *mut _, &mut ptr);
        //println!("{}", internal_module);
        if kernel_module.is_null() || rt != 0 {
            let err = CStr::from_ptr(ptr).to_owned().into_string().unwrap();
            eprintln!("{}", err);
            panic!("Failed to compile kernel module, report for detailed reasons")
        }
        LLVMLinkModules2(module, kernel_module);
        LLVMRunPassManager(global_manager, module);
        LLVMDisposeBuilder(builder);
        LLVMVerifyModule(module, LLVMAbortProcessAction, null_mut());
        LLVMDumpModule(module);
    }
}
