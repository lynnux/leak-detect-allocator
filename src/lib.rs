#![feature(new_uninit)]

pub use heapless::consts;
use heapless::consts::*;
use heapless::FnvIndexMap;

use once_cell::sync::OnceCell;
use spin::Mutex;
use std::alloc::{GlobalAlloc, Layout, System};

use std::sync::atomic::{AtomicBool, Ordering};

type FixedMap = FnvIndexMap<usize, FixedVec, U32768>;
type FixedVec = heapless::Vec<usize, U10>;

static TRACE_ACTIVATE: AtomicBool = AtomicBool::new(false);

pub struct LeakTracer;

impl LeakTracer {
    fn new_map() -> Box<FixedMap> {
        let alloc_on_heap = Box::<FixedMap>::new_zeroed();
        unsafe { alloc_on_heap.assume_init() }
    }
    pub fn init() {
        use std::mem::size_of;
        println!("size: {}", size_of::<FixedMap>());
        ALLOC_MAP.set(Mutex::new(Self::new_map())).unwrap();
        TRACE_ACTIVATE.store(true, Ordering::Relaxed);
    }

    pub fn alived() {
        let mut cloned = Self::new_map();
        {
            let x = ALLOC_MAP.get().unwrap().lock();
            //cloned = .map(|a, s|(*a, s.clone())).collect();
            for (addr, symbol_address) in (*x).iter() {
                cloned.insert(*addr, symbol_address.clone()).ok();
            }
        }
        let mut out = String::new();
        let mut count = 0;
        for (addr, symbol_address) in cloned.iter() {
            let mut it = symbol_address.into_iter();
            count+=1;
            out += &format!(
                "leak memory address: {:#x}, size: {}\r\n",
                addr,
                it.next().unwrap_or(&0)
            );
            for s in it {
                let ss: usize = *s;
                // Resolve this instruction pointer to a symbol name
                unsafe {
                    backtrace::resolve_unsynchronized(ss as *mut _, |symbol| {
                        if let Some(name) = symbol.name() {
                            out += &format!("\t{}\r\n", name);
                        }
                    });
                }
            }
        }
        out += &format!("total count:{}\r\n", count);

        std::fs::write("foo.txt", out.as_str().as_bytes()).ok();
    }
    pub fn uninit() {
        TRACE_ACTIVATE.store(false, Ordering::Relaxed);
    }
}

// 计算器双字，二进制开头为1的数，不能太大，会把rustc整崩！可以用rsh移位来减小 U32768
static ALLOC_MAP: OnceCell<Mutex<Box<FixedMap>>> = OnceCell::new(); // = ;

unsafe impl GlobalAlloc for LeakTracer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);

        if TRACE_ACTIVATE.load(Ordering::Relaxed) {
            let mut x = ALLOC_MAP.get().unwrap().lock();

            // we only save 10 symbol addresses.
            let mut v = FixedVec::new();
            v.push(size).ok(); // first is size
            let mut count = 0;
            backtrace::trace_unsynchronized(|frame| {
                count += 1;

                //let ip = frame.ip();
                // skip 2 frame
                if count < 3 {
                    return true;
                }
                let symbol_address = frame.symbol_address();
                v.push(symbol_address as usize).ok();

                if count < 11 {
                    true // going to the next frame
                } else {
                    false
                }
            });
            x.insert(ptr as usize, v).ok();
        }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACE_ACTIVATE.load(Ordering::Relaxed) {
            let mut x = ALLOC_MAP.get().unwrap().lock();
            if !x.contains_key(&(ptr as usize)) {
                println!("got missed {}", ptr as usize);
            //println!("{:?}", x);
            } else {
                x.remove(&(ptr as usize));
            }
        }

        System.dealloc(ptr, layout);
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
