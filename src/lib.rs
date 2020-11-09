#![feature(new_uninit)]
#![feature(const_fn)]

use heapless::String as HeaplessString;
pub use heapless::{consts, ArrayLength, FnvIndexMap, Vec as HeaplessVec};
use once_cell::sync::OnceCell;
use spin::Mutex;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    marker::PhantomData,
};

pub struct LeakTracer<LDT, VN>
where
    VN: ArrayLength<usize>,
    LDT: LeakDataTrait<VN>,
{
    leak_data: OnceCell<Mutex<Box<LDT>>>,
    backtrace_lock: OnceCell<Mutex<()>>, // we can't use backtrace internal locker, cause it need "std" feature which are not allowed
    phantom: PhantomData<VN>,
}

pub type LeakTracerDefault = LeakTracer<LeakData<consts::U10>, consts::U10>;

pub trait LeakDataTrait<VN: ArrayLength<usize>> {
    fn insert(&mut self, key: usize, value: HeaplessVec<usize, VN>);
    fn contains_key(&self, key: usize) -> bool;
    fn remove(&mut self, key: usize);
    fn iter_all<F>(&self, f: F)
    where
        F: FnMut(usize, &HeaplessVec<usize, VN>) -> bool;
}

pub struct LeakData<VN: ArrayLength<usize>> {
    inner: FnvIndexMap<usize, HeaplessVec<usize, VN>, consts::U32768>,
}
impl<VN: ArrayLength<usize>> LeakDataTrait<VN> for LeakData<VN> {
    fn insert(&mut self, key: usize, value: HeaplessVec<usize, VN>) {
        self.inner.insert(key, value).ok();
    }
    fn contains_key(&self, key: usize) -> bool {
        self.inner.contains_key(&key)
    }
    fn remove(&mut self, key: usize) {
        self.inner.remove(&key);
    }
    fn iter_all<F>(&self, mut f: F)
    where
        F: FnMut(usize, &HeaplessVec<usize, VN>) -> bool,
    {
        for (addr, symbol_address) in self.inner.iter() {
            if !f(*addr, symbol_address) {
                break;
            }
        }
    }
}

impl<VN: ArrayLength<usize>> LeakData<VN> {}
impl<LDT, VN> LeakTracer<LDT, VN>
where
    VN: ArrayLength<usize>,
    LDT: LeakDataTrait<VN>,
{
    pub const fn new() -> Self {
        LeakTracer {
            leak_data: OnceCell::new(),
            backtrace_lock: OnceCell::new(),
            phantom: PhantomData,
        }
    }

    fn new_data() -> Box<LDT> {
        let alloc_on_heap = Box::<LDT>::new_zeroed();
        unsafe { alloc_on_heap.assume_init() }
    }
    pub fn init(&self) -> usize {
        self.leak_data.set(Mutex::new(Self::new_data())).ok();
        self.backtrace_lock.set(Mutex::new(())).ok();
        std::mem::size_of::<LDT>()
    }

    pub fn now_leaks<F>(&self, f: F)
    where
        F: FnMut(usize, &HeaplessVec<usize, VN>) -> bool,
    {
        let all = self.alive_now();
        all.iter_all(f);
    }

    pub unsafe fn get_symbol_name(&self, addr: usize) -> Option<String> {
        let mut ret: Option<String> = None;
        if let Some(locker) = self.backtrace_lock.get() {
            let mut symbol_buf = {
                // some symbol name really long, so alloc 500 bytes to hold
                let alloc_on_heap = Box::<HeaplessString<consts::U500>>::new_zeroed();
                alloc_on_heap.assume_init()
            };
            let mut got = false;
            let l = locker.lock();
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
            drop(l);
            if got {
                ret = Some(symbol_buf.as_str().to_owned());
            }
        }
        ret
    }

    fn alive_now(&self) -> Box<LDT> {
        let mut cloned = Self::new_data();
        if let Some(data) = self.leak_data.get() {
            let x = data.lock();
            (*x).iter_all(|addr, vec| {
                cloned.insert(addr, vec.clone());
                true
            });
        }
        cloned
    }
}

unsafe impl<LDT, VN> GlobalAlloc for LeakTracer<LDT, VN>
where
    VN: ArrayLength<usize>,
    LDT: LeakDataTrait<VN>,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);
        let locker = if let Some(locker) = self.backtrace_lock.get() {
            locker
        } else {
            return ptr;
        };
        let mut v = HeaplessVec::new();
        v.push(size).ok(); // first is size
        let l = if cfg!(os = "windows") {
            Some(locker.lock())
        } else {
            None
        };
        // On win7 64, it's may cause deadlock, solution is to palce a newer version of dbghelp.dll combined with exe
        backtrace::trace_unsynchronized(|frame| {
            let symbol_address = frame.ip();
            v.push(symbol_address as usize).is_ok()
        });
        drop(l);
        if let Some(data) = self.leak_data.get() {
            data.lock().insert(ptr as usize, v);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(data) = self.leak_data.get() {
            let mut x = data.lock();
            if !x.contains_key(ptr as usize) {
                //println!("got missed {}", ptr as usize);
            } else {
                x.remove(ptr as usize);
            }
        }
        System.dealloc(ptr, layout);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use crate::{
            consts, ArrayLength, FnvIndexMap, HeaplessVec, LeakData, LeakDataTrait, LeakTracer,
        };
        let aa = LeakTracer::<LeakData<consts::U10>, _>::new();
        println!("size: {}", aa.init());

        struct CustomData<VN: ArrayLength<usize>> {
            inner: FnvIndexMap<usize, HeaplessVec<usize, VN>, consts::U16384>, // --> U16384 is customized
        }
        impl<VN: ArrayLength<usize>> LeakDataTrait<VN> for CustomData<VN> {
            fn insert(&mut self, key: usize, value: HeaplessVec<usize, VN>) {
                self.inner.insert(key, value).ok();
            }
            fn contains_key(&self, key: usize) -> bool {
                self.inner.contains_key(&key)
            }
            fn remove(&mut self, key: usize) {
                self.inner.remove(&key);
            }
            fn iter_all<F>(&self, mut f: F)
            where
                F: FnMut(usize, &HeaplessVec<usize, VN>) -> bool,
            {
                for (addr, symbol_address) in self.inner.iter() {
                    if !f(*addr, symbol_address) {
                        break;
                    }
                }
            }
        }
        let bb = LeakTracer::<CustomData<consts::U12>, _>::new();
        println!("size: {}", bb.init());
    }
}
