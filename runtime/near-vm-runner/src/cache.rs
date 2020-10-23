use crate::errors::IntoVMError;
use crate::prepare;
use borsh::{BorshDeserialize, BorshSerialize};
use near_primitives::hash::CryptoHash;
use near_vm_errors::CacheError::{DeserializationError, SerializationError, WriteError};
use near_vm_errors::VMError;
use near_vm_logic::{VMConfig, VMKind};
use std::convert::TryFrom;
use std::fmt;
use wasmer_runtime::{compiler_for_backend, Backend};
use wasmer_runtime_core::cache::Artifact;
use wasmer_runtime_core::load_cache_with;

pub(crate) fn compile_module(
    code: &[u8],
    config: &VMConfig,
) -> Result<wasmer_runtime::Module, VMError> {
    let prepared_code = prepare::prepare_contract(code, config)?;
    wasmer_runtime::compile(&prepared_code).map_err(|err| err.into_vm_error())
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
enum ContractCacheKey {
    Version1 { code_hash: CryptoHash, vm_config_non_crypto_hash: u64, vm_kind: u32 },
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
enum CacheRecord {
    Error(VMError),
    Code(Vec<u8>),
}

fn get_key(code_hash: &[u8], code: &[u8], vm_kind: VMKind, config: &VMConfig) -> CryptoHash {
    let hash = match CryptoHash::try_from(code_hash) {
        Ok(hash) => hash,
        // Sometimes caller doesn't compute code_hash, so hash the code ourselves.
        Err(_e) => near_primitives::hash::hash(code),
    };
    let key = ContractCacheKey::Version1 {
        code_hash: hash,
        vm_config_non_crypto_hash: config.non_crypto_hash(),
        vm_kind: vm_kind as u32,
    };
    near_primitives::hash::hash(&key.try_to_vec().unwrap())
}

fn cache_error(error: VMError, key: &CryptoHash, cache: &dyn CompiledContractCache) -> VMError {
    let record = CacheRecord::Error(error.clone());
    if cache.put(&(key.0).0, &record.try_to_vec().unwrap()).is_err() {
        // That's fine, just cannot cache compilation error.
        println!("Cannot cache an error");
    }
    error
}

fn compile_and_serialize_wasmer(
    wasm_code: &[u8],
    config: &VMConfig,
    key: &CryptoHash,
    cache: &dyn CompiledContractCache,
) -> Result<wasmer_runtime::Module, VMError> {
    let module = compile_module(wasm_code, config).map_err(|e| cache_error(e, &key, cache))?;
    let artifact = module.cache().map_err(|_e| {
        cache_error(
            VMError::CacheError(SerializationError { hash: (key.0).0 }),
            &key,
            cache,
        )
    })?;
    let code = artifact.serialize().map_err(|_e| {
        VMError::CacheError(SerializationError { hash: (key.0).0 })
    })?;
    // If errors comes from serialization we shall not cache it.
    let serialized = CacheRecord::Code(code).try_to_vec().map_err(|_e| {
        VMError::CacheError(SerializationError { hash: (key.0).0 })
    })?;
    cache.put(key.as_ref(), &serialized).map_err(|_e| VMError::CacheError(WriteError))?;
    Ok(module)
}

fn deserialize_wasmer(serialized: &[u8]) -> Result<wasmer_runtime::Module, VMError> {
    let record = CacheRecord::try_from_slice(serialized)
        .map_err(|_e| VMError::CacheError(DeserializationError))?;
    let code = match record {
        CacheRecord::Error(err) => return Err(err),
        CacheRecord::Code(code) => code,
    };
    let artifact = Artifact::deserialize(code.as_slice())
        .map_err(|_e| VMError::CacheError(DeserializationError))?;
    unsafe {
        let compiler = compiler_for_backend(Backend::Singlepass).unwrap();
        load_cache_with(artifact, compiler.as_ref())
            .map_err(|_e| VMError::CacheError(DeserializationError))
    }
}

pub(crate) fn compile_module_cached_wasmer(
    wasm_code_hash: &[u8],
    wasm_code: &[u8],
    config: &VMConfig,
    cache: Option<&dyn CompiledContractCache>,
) -> Result<wasmer_runtime::Module, VMError> {
    /* Consider adding `|| cfg!(feature = "no_cache")` */
    if cache.is_none() {
        return compile_module(wasm_code, config);
    }
    let key = get_key(wasm_code_hash, wasm_code, VMKind::Wasmer, config);
    let cache = cache.unwrap();
    match cache.get(&(key.0).0) {
        Ok(serialized) => match serialized {
            Some(serialized) => {
                match deserialize_wasmer(serialized.as_slice()) {
                    Ok(module) => Ok(module),
                    // We trying to be extra careful, and if cannot deserialize code,
                    // try to serialize again.
                    Err(_e) => compile_and_serialize_wasmer(wasm_code, config, &key, cache),
                }
            }
            None => compile_and_serialize_wasmer(wasm_code, config, &key, cache),
        },
        Err(_) => {
            // Cache access error happened, avoid attempts to cache.
            println!("Cannot use cache");
            compile_module(wasm_code, config)
        }
    }
}

/// Cache for compiled modules
pub trait CompiledContractCache: Send + Sync {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), std::io::Error>;
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, std::io::Error>;
}

impl CompiledContractCache for () {
    fn put(&self, _: &[u8], _: &[u8]) -> Result<(), std::io::Error> {
        Ok(())
    }
    fn get(&self, _: &[u8]) -> Result<Option<Vec<u8>>, std::io::Error> {
        Ok(None)
    }
}

impl fmt::Debug for dyn CompiledContractCache {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Compiled contracts cache")
    }
}
