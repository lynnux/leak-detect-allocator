#![feature(new_uninit)]
use heapless::String as HeaplessString;
use once_cell::sync::OnceCell;
use spin::Mutex;
use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::c_void;

type EnumProc = unsafe extern "C" fn(
    usr_data: *const c_void,
    address: usize,
    size: usize,
    stack: *const usize,
) -> i32;

extern "C" {
    fn alloc_internal_init(stack_size: usize);
    fn alloc_internal_alloc(address: usize, size: usize, stack: *const usize);
    fn alloc_internal_dealloc(address: usize);
    fn alloc_enum(usr_data: *const c_void, cb: EnumProc);
}

pub struct LeakTracer<const STACK_SIZE: usize> {
    backtrace_lock: OnceCell<Mutex<()>>, // we can't use backtrace internal locker, cause it need "std" feature which are not allowed
}

pub type LeakTracerDefault = LeakTracer<10>;

impl<const STACK_SIZE: usize> LeakTracer<STACK_SIZE> {
    pub const fn new() -> Self {
        LeakTracer {
            backtrace_lock: OnceCell::new(),
        }
    }
    pub fn init(&self) {
        assert!(STACK_SIZE > 0);
        self.backtrace_lock.set(Mutex::new(())).ok();
        unsafe {
            alloc_internal_init(STACK_SIZE);
        }
    }

    extern "C" fn alloc_enum_cb(
        usr_data: *const c_void,
        address: usize,
        size: usize,
        stack: *const usize,
    ) -> i32 {
        let closure: &mut &mut dyn FnMut(usize, usize, &[usize]) -> bool =
            unsafe { std::mem::transmute(usr_data) };

        let s = unsafe { std::slice::from_raw_parts(stack, STACK_SIZE) };
        if closure(address, size, s) {
            1
        } else {
            0
        }
    }

    pub fn now_leaks<F>(&self, mut callback: F)
    where
        F: FnMut(usize, usize, &[usize]) -> bool,
    {
        // https://stackoverflow.com/questions/32270030/how-do-i-convert-a-rust-closure-to-a-c-style-callback
        let mut cb: &mut dyn FnMut(usize, usize, &[usize]) -> bool = &mut callback;
        let cb = &mut cb;
        unsafe {
            alloc_enum(cb as *const _ as *const c_void, Self::alloc_enum_cb);
        }
    }

    pub fn get_symbol_name(&self, addr: usize) -> Option<String> {
        let mut ret: Option<String> = None;
        if let Some(locker) = self.backtrace_lock.get() {
            let mut symbol_buf = {
                // some symbol name really long, so alloc 500 bytes to hold
                let alloc_on_heap = Box::<HeaplessString<500>>::new_zeroed();
                unsafe { alloc_on_heap.assume_init() }
            };
            let mut got = false;
            let l = locker.lock();
            unsafe {
                backtrace::resolve_unsynchronized(addr as *mut _, |symbol| {
                    // do NOT trigger alloc in closure scope!
                    match symbol.name() {
                        Some(x) => {
                            use std::fmt::Write;
                            write!(&mut symbol_buf, "{}", x).ok();
                            got = true;
                        }
                        _ => {}
                    }
                });
            }
            drop(l);
            if got {
                ret = Some(symbol_buf.as_str().to_owned());
            }
        }
        ret
    }

    fn alloc_accounting(&self, size: usize, ptr: *mut u8) -> *mut u8 {
        let locker = if let Some(l) = self.backtrace_lock.get() {
            l
        } else {
            return ptr;
        };
        let mut vs = [0usize; STACK_SIZE];
        let l = if cfg!(os = "windows") {
            Some(locker.lock())
        } else {
            None
        };
        let mut count = 0;
        // On win7 64, it's may cause deadlock, solution is to palce a newer version of dbghelp.dll combined with exe
        unsafe {
            backtrace::trace_unsynchronized(|frame| {
                let symbol_address = frame.ip();
                vs[count] = symbol_address as usize;
                count += 1;
                if count >= STACK_SIZE {
                    false
                } else {
                    true
                }
            });
        }
        drop(l);
        unsafe {
            alloc_internal_alloc(ptr as usize, size, vs.as_ptr());
        }
        ptr
    }

    fn dealloc_accounting(&self, ptr: *mut u8) {
        unsafe {
            alloc_internal_dealloc(ptr as usize);
        }
    }
}

unsafe impl<const STACK_SIZE: usize> GlobalAlloc for LeakTracer<STACK_SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_accounting(layout.size(), System.alloc(layout))
    }

    unsafe fn realloc(&self, ptr0: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = System.realloc(ptr0, layout, new_size);
        if ptr != ptr0 {
            self.dealloc_accounting(ptr0);
            self.alloc_accounting(new_size, ptr);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_accounting(ptr);
        System.dealloc(ptr, layout);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let aa = crate::LeakTracer::<15>::new();
    }
}
