extern crate llvm_sys as llvm;

mod parser;
mod llvm_gen;
mod llvm_context;
mod pool;

use llvm::core::*;

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use std::io::Read;
//use std::env::args;

pub use crate::parser::*;
pub use crate::llvm_gen::*;
pub use crate::llvm_context::*;
pub use crate::pool::StringPool;
use llvm::transforms::scalar::{LLVMAddGVNPass, LLVMAddCFGSimplificationPass, LLVMAddReassociatePass};
use std::collections::HashMap;
use llvm::prelude::{LLVMValueRef, LLVMModuleRef};
use std::ffi::{CStr, CString};
use std::ops::{AddAssign};
use llvm::link_time_optimizer::llvm_create_optimizer;
use std::ptr::null_mut;
use std::os::raw::c_char;
use std::collections::HashSet;
use llvm::linker::LLVMLinkModules2;
use llvm::analysis::LLVMVerifyModule;
use llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction;
use llvm::transforms::ipo::LLVMAddFunctionInliningPass;
use docopt::*;
use serde_derive::Deserialize;
use std::process::exit;

const USAGE: &'static str = "
Ruda compiler

Usage:
  ruda [options] <filename>...

Options:
  -h --help     Show this screen
  -o <file>     Place the output into <file>
";


#[derive(Debug, Deserialize)]
struct Args {
    arg_filename: Vec<String>,
    flag_o: Option<String>,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    do_compile(args.arg_filename, args.flag_o.unwrap_or("./a.ll".to_string()));
}

fn do_compile(files: Vec<String>, output: String) {
    //if args().len() < 2 { panic!("ruda - no input file") };
    //let obj = args().last().unwrap().as_str();
    let mut internal_module = String::from("; ModuleID = 'canoe_kernel'\n\n");
    let mut file_list = files.iter().map(|f| std::path::PathBuf::from(&f[..]));
    let mut dedup : HashSet<std::path::PathBuf> = HashSet::new();
    let mut queue : Vec<std::path::PathBuf> = file_list.collect();
    let mut parsed_files = vec![];
    // Lifetime lift hack - I can't think of a better way to resolve this
    let source_pool = StringPool::new();
    // file_list.map(|file| {
    //     let mut str = String::new();
    //     std::fs::File::open(file).unwrap_or_else(|_| {
    //         println!("Cannot open file : {}", file.display());
    //         exit(1)
    //     }).read_to_string(&mut str).unwrap();
    //     str
    // }).collect::<Vec<_>>();
    while queue.len() > 0 {
        let file = queue.pop().unwrap();
        let mut str = String::new();
        std::fs::File::open(&file).unwrap_or_else(|_| {
            println!("Cannot open file : {}", file.display());
            exit(1)
        }).read_to_string(&mut str).unwrap();
        dedup.insert(file);
        let syntax = RudaParser::parse(Rule::file
                , source_pool.insert(str))
                .unwrap_or_else(|e| panic!("{}", e)).collect::<RuleList>()[0].clone().into_inner();
        for header in syntax.clone().filter_map(|item| if item.as_rule() == Rule::import_module { 
            item.into_inner().last().map(|s| s.as_str().to_string()) 
        } else { None }) {
            // bad practice, but we just take this for now
            let path = std::path::PathBuf::from(&header[..]);
            if !dedup.contains(&path) {
                dedup.insert(path.clone());
                queue.push(path);
            }
        }
        parsed_files.push(syntax);
    }
    unsafe {
// Set up a context, module and builder in that context.
        let context = Context::new();

        let module = context.create_module("canoe".to_string());
        let manager = context.function_pass_manager(module);
        let global_manager = context.global_pass_manager();
        //LLVMAddGlobalOptimizerPass(manager);
        LLVMAddGVNPass(manager);
        LLVMAddCFGSimplificationPass(manager);
        LLVMAddReassociatePass(manager);
        LLVMInitializeFunctionPassManager(manager);
        LLVMAddFunctionInliningPass(global_manager);
//        dbg!(&parser);
        let mut module_vals = HashMap::<String, Vec<(LLVMValueRef, TyName)>>::new();
        let mut func_pairs: Vec<(TypedExpr, LLVMValueRef)> = vec![];
        let intrinsics = context.init_nvptx_intrinsics(module);
        for func in parsed_files.into_iter().map(|parser| walk_pairs(parser)).flatten()
                .filter(|f| if let BaseExpr::Nope = f.0 { false } else { true }) {
            //dbg!(&func);
            let func_ref = llvm_declare_func(func.0.clone(), context.context, module, context.builder);
            let defs = module_vals.entry(func_ref.1).or_insert(vec![]);
            defs.push((func_ref.0, func.1.clone()));
            func_pairs.push((func, func_ref.0));
        }
        for func_pair in func_pairs.into_iter() {
            let ident = Context::get_value_name(func_pair.1);
            let renamed_ident = ident.replace(".", "_"); // nvvm do not allow names with `.`, so we just replace it
            let string = CString::new(renamed_ident).unwrap();
            LLVMSetValueName(func_pair.1, string.as_ptr() as *const _);
            if let BaseExpr::IntrinsicsFuncDecl(_, params, ret, body) = (func_pair.0).0 {
                let ident = Context::get_value_name(func_pair.1);
                let func_def = llvm_embedded_ir(ident, params, ret, body, context.context);
                internal_module.add_assign(&func_def[..]);
                internal_module.add_assign("\n\n");
                continue;
            }
            if let BaseExpr::FuncVirtualDecl(_, params, ret) = (func_pair.0).0 {
                let ident = Context::get_value_name(func_pair.1);
                let func_def = llvm_embedded_kernel_decl(ident, params, ret, context.context);
                internal_module.add_assign(&func_def[..]);
                continue;
            }
            llvm_define_func(func_pair.0, func_pair.1, &module_vals, context.context, module, context.builder, &intrinsics);
            LLVMRunFunctionPassManager(manager, func_pair.1);
        }
        let mut kernel_module: LLVMModuleRef = null_mut();
        let str_len = internal_module.len();
        let string = CString::new(internal_module).unwrap();
        let code_buffer = LLVMCreateMemoryBufferWithMemoryRange(string.as_ptr() as *const _, str_len, b"kernel_code".as_ptr() as *const _, 0);
        let mut ptr: *mut c_char = std::mem::uninitialized();
        let rt = llvm_sys::ir_reader::LLVMParseIRInContext(context.context, code_buffer, &mut kernel_module as *mut _, &mut ptr);
        //println!("{}", internal_module);
        if kernel_module.is_null() || rt != 0 {
            let err = CStr::from_ptr(ptr).to_owned().into_string().unwrap();
            eprintln!("{}", err);
            panic!("Failed to compile kernel module, report for detailed reasons")
        }
        LLVMLinkModules2(module, kernel_module);
        LLVMRunPassManager(global_manager, module);
        LLVMDisposeBuilder(context.builder);
        LLVMVerifyModule(module, LLVMAbortProcessAction, null_mut());
        let output_handle = LLVMPrintModuleToString(module);
        let ptx = CStr::from_ptr(output_handle).to_owned().into_string().unwrap();
        std::fs::write(output, ptx).unwrap();
    }
}
