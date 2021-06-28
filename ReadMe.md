## Idea
It's hard to detect memory leak, with a global allocator, we can trace the `alloc` add `dealloc`, if we record the call stacks of `alloc` operation, then we can see where the code lead memory leak. This tool do NOT record ALL allocation, but delete the record when `dealloc`.

Powerd by `global allocator` + `heapless` + `backtrace`, it's only support nightly toolchain, caused by `new_uninit` features.

## Usage
Add this to your cargo.toml:
```toml
leak-detect-allocator = {git = "https://github.com/lynnux/leak-detect-allocator.git"}
```
Example:
```rust
use leak_detect_allocator::LeakTracerDefault;

#[global_allocator]
static LEAK_TRACER: LeakTracerDefault = LeakTracerDefault::new();

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    LEAK_TRACER.init();
    tokio::spawn(async move {
        loop {
            tokio::signal::ctrl_c().await.ok();
            let mut out = String::new();
            let mut count = 0;
            let mut count_size = 0;
            LEAK_TRACER.now_leaks(|address: usize, size: usize, stack: &[usize]| {
                count += 1;
                count_size += size;
                out += &format!("leak memory address: {:#x}, size: {}\r\n", address, size);
                for f in it {
                    // Resolve this instruction pointer to a symbol name
					out += &format!(
					"\t{:#x}, {}\r\n",
					*f,
					LEAK_TRACER.get_symbol_name(*f).unwrap_or("".to_owned())
				);
                }
                true // continue until end
            });
            out += &format!("\r\ntotal address:{}, bytes:{}\r\n", count, count_size);
            std::fs::write("foo.txt", out.as_str().as_bytes()).ok();
        }
    });
}
```
When CTRL+C, it will get a "foo.txt", and the output is like:
```
leak memory address: 0x44e440, size: 10
	0x13fe09443, backtrace::backtrace::trace_unsynchronized<closure-0>
	0x13fe09443, leak_detect_allocator::{{impl}}::alloc<leak_detect_allocator::LeakData<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UTerm, typenum::bit::B1>, typenum::bit::B0>, typenum::bit::B1>, typenum::bit::B0>>,typenu
	0x13fe09443, flashcore::_::__rg_alloc
	0x13fe09443, alloc::alloc::alloc
	0x13fe09443, alloc::alloc::{{impl}}::alloc
	0x13fe09443, alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::allocate_in<u8,alloc::alloc::Global>
	0x13fe09443, alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::with_capacity<u8>
	0x13fe09443, alloc::vec::Vec<u8>::with_capacity<u8>
	0x13fe09443, alloc::slice::hack::to_vec<u8>
leak memory address: 0x4508c0, size: 30
	0x13fe09443, backtrace::backtrace::trace_unsynchronized<closure-0>
	0x13fe09443, leak_detect_allocator::{{impl}}::alloc<leak_detect_allocator::LeakData<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UInt<typenum::uint::UTerm, typenum::bit::B1>, typenum::bit::B0>, typenum::bit::B1>, typenum::bit::B0>>,typenu
	0x13fe09443, flashcore::_::__rg_alloc
	0x13fe09443, alloc::alloc::alloc
	0x13fe09443, alloc::alloc::{{impl}}::alloc
	0x13fe09443, alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::allocate_in<u8,alloc::alloc::Global>
	0x13fe09443, alloc::raw_vec::RawVec<u8, alloc::alloc::Global>::with_capacity<u8>
	0x13fe09443, alloc::vec::Vec<u8>::with_capacity<u8>
	0x13fe09443, alloc::slice::hack::to_vec<u8>
	...
	
total address:38, bytes:6373
```
Stack calls seems better in debug version.
## Customize
More stack by this:
```rust
use leak_detect_allocator::LeakTracer;

#[global_allocator]
static LEAK_TRACER: LeakTracer<20> = LeakTracer::<20>::new();
```

## Known Issues
On Win7 64, if you encounter deadlock, you can try place a newer version of dbghelp.dll to your bin directory.

## Changelog
2021/28/6, use cpp(c++11) to speedup for rust debug version.
