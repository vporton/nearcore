use arr_macro::arr;
use backtrace;
use log::info;
use std::alloc::{GlobalAlloc, Layout};
use std::cell::RefCell;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

const COUNTERS_SIZE: usize = 16384;
static JEMALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
static MEM_SIZE: [AtomicUsize; COUNTERS_SIZE as usize] = arr![AtomicUsize::new(0); 16384];
static MEM_CNT: [AtomicUsize; COUNTERS_SIZE as usize] = arr![AtomicUsize::new(0); 16384];
static ENABLED: AtomicUsize = AtomicUsize::new(0);
static GTID: AtomicUsize = AtomicUsize::new(0);
static SANITY: AtomicUsize = AtomicUsize::new(0);
static mut FILENAME: [u8; 32] = [0; 32];

static mut ADDR: Option<*mut c_void> = None;

pub struct MyAllocator;

const EXTRA: usize = 32;
const MAGIC: usize = 0x12345678991124;

thread_local! {
    pub static TID: RefCell<usize> = RefCell::new(usize::max_value());
    pub static TID2: RefCell<usize> = RefCell::new(usize::max_value());
    pub static IN_TRACE: RefCell<usize> = RefCell::new(0);
}

pub fn get_tid() -> usize {
    SANITY.fetch_add(1, Ordering::SeqCst);
    let res = TID.with(|t| {
        if *t.borrow() == usize::max_value() {
            *t.borrow_mut() = GTID.fetch_add(1, Ordering::SeqCst);
        }
        *t.borrow()
    });
    SANITY.fetch_sub(1, Ordering::SeqCst);
    res
}

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        SANITY.fetch_add(1, Ordering::SeqCst);

        let new_layout = Layout::from_size_align(layout.size() + EXTRA, layout.align()).unwrap();

        let tid = get_tid();

        let res = JEMALLOC.alloc(new_layout);
        MEM_SIZE[tid % COUNTERS_SIZE].fetch_add(layout.size(), Ordering::SeqCst);
        MEM_CNT[tid % COUNTERS_SIZE].fetch_add(1, Ordering::SeqCst);

        let mut addr: Option<*mut c_void> = None;
        // let mut cnt = 0;

        IN_TRACE.with(|in_trace| {
            if *in_trace.borrow() != 0 {
                return;
            }
            *in_trace.borrow_mut() = 1;

            backtrace::trace(|frame| {
                let ip = frame.ip();
                // let symbol_address = frame.symbol_address();
                let mut skip = false;

                // backtrace::resolve_frame(frame, |symbol| {
                backtrace::resolve(ip, |symbol| {
                    if let Some(name) = symbol.name() {
                        //_ZN103_$LT$tracing_subscribe
                        //_ZN9backtrace9backtrace5trace17h
                        if !name.as_str().unwrap_or("").contains("near")
                            && !name.as_str().unwrap_or("").contains("actix")
                            && !name.as_str().unwrap_or("").contains("wasm")
                            && !name.as_str().unwrap_or("").contains("rocksdb")
                            && !name.as_str().unwrap_or("").contains("main")
                            && !name.as_str().unwrap_or("").contains("tokio")
                            && !name.as_str().unwrap_or("").contains("serde")
                        {
                            addr = symbol.addr();
                            /*
                            if name.as_str().unwrap_or("").contains("_ZN9backtrace9")
                                || name.as_str().unwrap_or("").contains("_ZN3std")
                                || name.as_str().unwrap_or("").contains("ZN91_$LT$memory_tracker..")
                                || name.as_str().unwrap_or("").contains("_ZN5bytes")
                                || name.as_str().unwrap_or("").contains("__rg_")
                                || name.as_str().unwrap_or("").contains("_ZN5alloc")
                                || name.as_str().unwrap_or("").contains("_ZN6alloc")
                            {
                            } else {
                                info!("NAME2 {}", name.as_str().unwrap_or("<NONE>"));
                                return;
                            }   */

                            skip = true;
                            return;
                        }

                        if name.as_str().unwrap_or("").contains("main") {
                            return;
                        }
                        addr = symbol.addr();

                    /*
                        if !name.as_str().unwrap_or("").contains("actix") {
                            //    info!("NAME {}", name.as_str().unwrap_or("<NONE>"));
                        }
                    if name.as_bytes()[0] == '_' as u8
                        && name.as_bytes()[1] == 'Z' as u8
                        && name.as_bytes()[2] == 'N' as u8
                        && (name.as_bytes()[3] == '3' as u8
                            || name.as_bytes()[3] == '9' as u8)
                    {
                        skip = true;
                    } else {
                        let mut idx = 0;
                        for b in name.as_bytes() {
                            if idx < 32 {
                                FILENAME[idx] = *b;
                            }
                            idx += 1;
                        }
                    }   */
                    } else {
                        skip = true;
                    }
                });
                // });
                skip
            });
            *in_trace.borrow_mut() = 0;
        });

        ADDR = addr;

        *(res.offset(layout.size() as isize) as *mut u64) = MAGIC as u64;
        *(res.offset(layout.size() as isize) as *mut u64).offset(1) = layout.size() as u64;
        *(res.offset(layout.size() as isize) as *mut u64).offset(2) = tid as u64;
        *(res.offset(layout.size() as isize) as *mut *mut c_void).offset(3) =
            addr.unwrap_or(0 as *mut c_void);
        SANITY.fetch_sub(1, Ordering::SeqCst);
        res
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        SANITY.fetch_add(1, Ordering::SeqCst);
        let new_layout = Layout::from_size_align(layout.size() + EXTRA, layout.align()).unwrap();

        *(ptr.offset(layout.size() as isize) as *mut u64) = 0;
        let tid: usize = *(ptr.offset(layout.size() as isize) as *mut u64).offset(2) as usize;

        MEM_SIZE[tid % COUNTERS_SIZE].fetch_sub(layout.size(), Ordering::SeqCst);
        MEM_CNT[tid % COUNTERS_SIZE].fetch_sub(1, Ordering::SeqCst);

        JEMALLOC.dealloc(ptr, new_layout);
        SANITY.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn enable_tracking(name: &str) {
    ENABLED.store(1, Ordering::SeqCst);

    TID2.with(|t| {
        if *t.borrow() == usize::max_value() {
            let tid = get_tid();
            info!("enabling tracking for {}: {}", name, tid);
            *t.borrow_mut() = tid;
        }
    });
}

pub fn print_counters_ary() {
    info!(
        "HMM {} {:?}",
        unsafe { ADDR.unwrap_or(0 as *mut c_void) as u64 },
        std::str::from_utf8(unsafe { FILENAME.as_ref() })
    );
    info!("tid {}", get_tid());
    let mut total_cnt: usize = 0;
    let mut total_size: usize = 0;
    for idx in 0..COUNTERS_SIZE {
        let val: usize = MEM_SIZE.get(idx).unwrap().load(Ordering::SeqCst);
        if val != 0 {
            let cnt = MEM_CNT.get(idx).unwrap().load(Ordering::SeqCst);
            total_cnt += cnt;
            info!("COUNTERS {}: {} {}", idx, cnt, val);
            total_size += val;
        }
    }
    info!("COUNTERS TOTAL {} {}", total_cnt, total_size);
}

pub fn get_sanity_val() -> usize {
    SANITY.load(Ordering::SeqCst)
}
