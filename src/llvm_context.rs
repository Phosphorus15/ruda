use llvm::prelude::*;
use llvm::core::*;
use std::ffi::{CString, CStr};

pub(crate) struct Context {
    pub context: LLVMContextRef,
    pub builder: LLVMBuilderRef
}

#[allow(dead_code)]
pub struct NVIntrinsics {
    pub thread_x: LLVMValueRef,
    pub thread_y: LLVMValueRef,
    pub thread_z: LLVMValueRef,
    pub sync_thread: LLVMValueRef,
}

impl Context {
    pub fn new() -> Self {
        unsafe{
            let context = LLVMContextCreate();
            Context {
                context: context,
                builder: LLVMCreateBuilderInContext(context),
            }
        }
    }

    pub fn get_value_name(value: LLVMValueRef) -> String {
        unsafe { CStr::from_ptr(LLVMGetValueName(value))
                .to_owned().into_string().unwrap() }
    }

    pub fn create_module(&self, name: String) -> LLVMModuleRef {
        let module_name = std::mem::ManuallyDrop::new(CString::new(name).unwrap());
        unsafe{
            LLVMModuleCreateWithNameInContext(module_name.as_ptr() as *const _, self.context)
        }
    }

    pub fn global_pass_manager(&self) -> LLVMPassManagerRef {
        unsafe { LLVMCreatePassManager() }
    }

    pub fn function_pass_manager(&self, module: LLVMModuleRef) -> LLVMPassManagerRef {
        unsafe { LLVMCreateFunctionPassManagerForModule(module) }
    }

    pub fn init_nvptx_intrinsics(&self, module: LLVMModuleRef) -> NVIntrinsics {
        unsafe {
            let type_cu_index = LLVMFunctionType(LLVMInt32TypeInContext(self.context), [].as_mut_ptr(), 0, 0);
            let type_barrier = LLVMFunctionType(LLVMVoidTypeInContext(self.context), [].as_mut_ptr(), 0, 0);
            NVIntrinsics {
                thread_x: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.x\0".as_ptr() as *mut _, type_cu_index),
                thread_y: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.y\0".as_ptr() as *mut _, type_cu_index),
                thread_z: LLVMAddFunction(module, b"llvm.nvvm.read.ptx.sreg.tid.z\0".as_ptr() as *mut _, type_cu_index),
                sync_thread: LLVMAddFunction(module, b"llvm.nvvm.barrier0\0".as_ptr() as *mut _, type_barrier),
            }
        }
    }
}