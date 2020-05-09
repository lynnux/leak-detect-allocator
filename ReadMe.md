## Idea
It's hard to detect memory leak, with a global allocator, we can trace the `alloc` add `dealloc`, if we record the call stacks of `alloc` operation, then we can see where the code lead memory leak. This tool do NOT record ALL allocation, but delete the record when `dealloc`.

Powerd by `global allocator` + `heapless` + `backtrace`, it's only support nightly toolchain, caused by `new_uninit` and `const_fn` features.

## Usage
Add this to your cargo.toml:
```toml
leak-detect-allocator = {git = "https://github.com/lynnux/leak-detect-allocator.git"}
```
Example:
```rust
#[global_allocator]
static LEAK_TRACER: LeakTracerDefault = LeakTracerDefault::new();

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let lda_size = LEAK_TRACER.init();
    tokio::spawn(async move {
        loop {
            tokio::signal::ctrl_c().await.ok();
            let mut out = String::new();
            let mut count = 0;
            let mut count_size = 0;
            LEAK_TRACER.now_leaks(|addr, frames| {
                count += 1;
                let mut it = frames.iter();
                // first is the alloc size
                let size = it.next().unwrap_or(&0);
                if *size == lda_size {
                    return true;
                }
                count_size += size;
                out += &format!("leak memory address: {:#x}, size: {}\r\n", addr, size);
                for f in it {
                    // Resolve this instruction pointer to a symbol name
                    unsafe {
                        out += &format!(
                            "\t{}\r\n",
                            LEAK_TRACER.get_symbol_name(*f).unwrap_or("".to_owned())
                        );
                    }
                }
                true // continue until end
            });
            out += &format!("\r\ntotal address:{}, bytes:{}, internal use for leak-detect-allacator:{} bytes\r\n", count, count_size, lda_size*2);
            std::fs::write("foo.txt", out.as_str().as_bytes()).ok();
        }
    });
}
```
When CTRL+C, it will get a "foo.txt", and the output is like:
```
leak memory address: 0x44e440, size: 10
	backtrace::backtrace::trace_unsynchronized<closure-0>
	leak_detect_allocator::{{impl}}::alloc<leak_detect_allocator::LeakData<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UTerm, typenum::bit::B1>, typenum::bit::B0>, typenum::bit::B1>, typenum::bit::B0>>,typenu
	flashcore::_::__rg_alloc
	alloc::alloc::alloc
	alloc::alloc::{{impl}}::alloc
	alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::allocate_in<u8,alloc::alloc::Global>
	alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::with_capacity<u8>
	alloc::vec::Vec<u8>::with_capacity<u8>
	alloc::slice::hack::to_vec<u8>
leak memory address: 0x4508c0, size: 30
	backtrace::backtrace::trace_unsynchronized<closure-0>
	leak_detect_allocator::{{impl}}::alloc<leak_detect_allocator::LeakData<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UTerm, typenum::bit::B1>, typenum::bit::B0>, typenum::bit::B1>, typenum::bit::B0>>,typenu
	flashcore::_::__rg_alloc
	alloc::alloc::alloc
	alloc::alloc::{{impl}}::alloc
	alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::allocate_in<u8,alloc::alloc::Global>
	alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::with_capacity<u8>
	alloc::vec::Vec<u8>::with_capacity<u8>
	alloc::slice::hack::to_vec<u8>
	...
total address:38, bytes:6373, internal use for leak-detect-allacator:7077904 bytes	
```
Stack calls seems better in debug version.
## Customize
```rust
	// change the vec size to bigger, so we can save more call stack
        use crate::{
            consts, ArrayLength, FnvIndexMap, HeaplessVec, LeakData, LeakDataTrait, LeakTracer,
        };
        let aa = LeakTracer::<LeakData<consts::U20>, _>::new();
        println!("size: {}", aa.init());
		
		
	// change the whole indexmap size, so we get more space to save
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
```

## Known Issues
On Win7 64, if you encounter deadlock, you can try place a newer version of dbghelp.dll to your bin directory.
