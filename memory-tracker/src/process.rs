use std::{thread, time};

use crate::allocator;
use futures::executor::block_on;
use heim::units::information::byte;
use jemalloc_ctl::{epoch, stats};
use log::{error, info};

macro_rules! je_mib {
    ($key:ty) => {
        if let Ok(value) = <$key>::mib() {
            value
        } else {
            error!("failed to lookup jemalloc mib for {}", stringify!($key));
            return;
        }
    };
}

macro_rules! mib_read {
    ($mib:ident) => {
        if let Ok(value) = $mib.read() {
            value as i64
        } else {
            error!("failed to read jemalloc stats for {}", stringify!($mib));
            return;
        }
    };
}

pub fn track_current_process(interval: u64) {
    if interval == 0 {
        info!("track current process: disable");
        return;
    }
    info!("track current process: enable");
    allocator::enable_tracking("memory_tracker");

    let wait_secs = time::Duration::from_secs(interval);
    let je_epoch = je_mib!(epoch);
    // Bytes allocated by the application.
    let allocated = je_mib!(stats::allocated);
    // Bytes in physically resident data pages mapped by the allocator.
    let resident = je_mib!(stats::resident);
    // Bytes in active pages allocated by the application.
    let active = je_mib!(stats::active);
    // Bytes in active extents mapped by the allocator.
    let mapped = je_mib!(stats::mapped);
    // Bytes in virtual memory mappings that were retained
    // rather than being returned to the operating system
    let retained = je_mib!(stats::retained);
    // Bytes dedicated to jemalloc metadata.
    let metadata = je_mib!(stats::metadata);

    if let Err(err) = thread::Builder::new().name("MemoryTracker".to_string()).spawn(move || {
        if let Ok(process) = block_on(heim::process::current()) {
            loop {
                if je_epoch.advance().is_err() {
                    error!("failed to refresh the jemalloc stats");
                    return;
                }
                if let Ok(memory) = block_on(process.memory()) {
                    info!(
                        "memory: {{ rss: {} vms: {} }} jemalloc {{ \
                                    allocated: {}, resident: {}, active: {}, \
                                    mapped: {}, retained: {}, \
                                    metadata: {} \
                                }}",
                        // Resident set size, amount of non-swapped physical memory.
                        memory.rss().get::<byte>() as i64,
                        // Virtual memory size, total amount of memory.
                        memory.vms().get::<byte>() as i64,
                        mib_read!(allocated),
                        mib_read!(resident),
                        mib_read!(active),
                        mib_read!(mapped),
                        mib_read!(retained),
                        mib_read!(metadata)
                    );
                    allocator::print_counters_ary();
                } else {
                    error!("failed to fetch the memory information about current process");
                }
                thread::sleep(wait_secs);
            }
        } else {
            error!("failed to track the currently running program");
        }
    }) {
        error!("failed to spawn the thread to track current process: {}", err);
    }
}
