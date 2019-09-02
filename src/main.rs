extern crate llvm_sys as llvm;

mod parser;
mod llvm_gen;

use llvm::core::*;

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use std::io::Read;
use std::env::args;

pub use crate::parser::*;
pub use crate::llvm_gen::*;

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
        let parser = RudaParser::parse(Rule::file, &str)
            .unwrap_or_else(|e| panic!("{}", e)).collect::<RuleList>()[0].clone().into_inner();
        dbg!(&parser);
        let intrinsics = init_nvptx_intrinsics(context, module);
        for func in walk_pairs(parser) {
            dbg!(&func);
            llvm_declare_par_func(func, context, module, builder, &intrinsics);
        }
        LLVMDisposeBuilder(builder);
        LLVMDumpModule(module);
    }
}
