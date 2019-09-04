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
use llvm::prelude::LLVMValueRef;

fn main() {
    //if args().len() < 2 { panic!("ruda - no input file") };
    //let obj = args().last().unwrap().as_str();
    let mut str = String::new();
    std::fs::File::open("src/arith.ru").unwrap().read_to_string(&mut str).unwrap();
    unsafe {
// Set up a context, module and builder in that context.
        let context = LLVMContextCreate();
        let module = LLVMModuleCreateWithNameInContext(b"canoe\0".as_ptr() as *const _, context);
        let builder = LLVMCreateBuilderInContext(context);

        let manager = LLVMCreateFunctionPassManagerForModule(module);
        //LLVMAddGlobalOptimizerPass(manager);
        LLVMAddGVNPass(manager);
        LLVMAddCFGSimplificationPass(manager);
        LLVMAddReassociatePass(manager);
        LLVMInitializeFunctionPassManager(manager);
        let parser = RudaParser::parse(Rule::file, &str)
            .unwrap_or_else(|e| panic!("{}", e)).collect::<RuleList>()[0].clone().into_inner();
        dbg!(&parser);
        let mut module_vals = HashMap::<String, Vec<(LLVMValueRef, TyName)>>::new();
        let mut func_pairs: Vec<(TypedExpr, LLVMValueRef)> = vec![];
        let intrinsics = init_nvptx_intrinsics(context, module);
        for func in walk_pairs(parser) {
            dbg!(&func);
            let func_ref = llvm_declare_func(func.0.clone(), context, module, builder);
            let defs = module_vals.entry(func_ref.1).or_insert(vec![]);
            defs.push((func_ref.0, func.1.clone()));
            func_pairs.push((func, func_ref.0));
        }
        for func_pair in func_pairs.into_iter() {
            llvm_define_func(func_pair.0, func_pair.1, &module_vals, context, module, builder, &intrinsics);
            LLVMRunFunctionPassManager(manager, func_pair.1);
        }
        LLVMDisposeBuilder(builder);
        LLVMDumpModule(module);
    }
}
