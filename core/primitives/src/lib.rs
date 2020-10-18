//#[cfg(jemallocator)]
extern crate jemallocator;

// #[cfg(jemallocator)]
//#[global_allocator]
//static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

pub use borsh;

pub mod account;
pub mod block;
pub mod block_header;
pub mod challenge;
pub mod contract;
pub mod epoch_manager;
pub mod errors;
pub mod hash;
pub mod logging;
pub mod merkle;
pub mod network;
pub mod receipt;
pub mod rpc;
pub mod serialize;
pub mod sharding;
pub mod state_record;
pub mod syncing;
pub mod telemetry;
pub mod test_utils;
pub mod transaction;
pub mod trie_key;
pub mod types;
pub mod utils;
pub mod validator_signer;
pub mod version;
pub mod views;

/*
pub trait MemorySize {
    fn memory_size(&self) -> u64;
}

impl<A, B> MemorySize for HashMap<A, B>
where
    B: MemorySize,
{
    fn memory_size(&self) -> u64 {
        let mut res = 0;
        for value in self.values() {
            res += value.memory_size()
        }
        res
    }
}

#[macro_export]
macro_rules! my_macro {
    (pub struct $name:ident {
        $($field_name:ident: $field_type:ty,)*
    }) => {
        impl $name {
            // This is purely an example—not a good one.
            fn memory_size(&mut self) -> u64 {
                $(self.$field_name.memory_size() + )*
                0
            }
        }
    }
}

impl MemorySize for CryptoHash {
    fn memory_size(&self) -> u64 {
        mem::size_of::<CryptoHash>().try_into().unwrap()
    }
}

impl MemorySize for Version {
    fn memory_size(&self) -> u64 {
        mem::size_of::<Version>().try_into().unwrap()
    }
}
        */
