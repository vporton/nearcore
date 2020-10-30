use std::convert::TryInto;
use std::io::{Read, Write};

use borsh::{BorshDeserialize, BorshSerialize};
use ethereum_types::{Address, U256};

use near_vm_errors::{EvmError, InconsistentStateError, VMLogicError};
use near_vm_logic::types::AccountId;

pub type RawAddress = [u8; 20];
pub type RawHash = [u8; 32];
pub type RawU256 = [u8; 32];
pub type DataKey = [u8; 52];

pub type Result<T> = std::result::Result<T, VMLogicError>;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AddressArg {
    pub address: RawAddress,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct GetStorageAtArgs {
    pub address: RawAddress,
    pub key: RawHash,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawArgs {
    pub account_id: AccountId,
    pub amount: RawU256,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransferArgs {
    pub address: RawAddress,
    pub amount: RawU256,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ViewCallArgs {
    pub sender: RawAddress,
    pub address: RawAddress,
    pub amount: RawU256,
    pub args: Vec<u8>,
}

#[derive(Debug)]
pub struct MetaCallArgs {
    pub sender: Address,
    pub nonce: U256,
    pub fee_amount: U256,
    pub fee_address: Address,
    pub contract_address: Address,
    pub input: Vec<u8>,
}

impl BorshSerialize for ViewCallArgs {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write(&self.sender)?;
        writer.write(&self.address)?;
        writer.write(&self.amount)?;
        writer.write(&self.args)?;
        Ok(())
    }
}

impl BorshDeserialize for ViewCallArgs {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        if buf.len() < 72 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unexpected length of input",
            ));
        }
        let sender = RawAddress::deserialize(buf)?;
        let address = RawAddress::deserialize(buf)?;
        let amount = RawU256::deserialize(buf)?;
        let mut args = Vec::with_capacity(buf.len());
        buf.read_to_end(&mut args)?;
        Ok(Self { sender, address, amount, args })
    }
}

pub fn convert_vm_error(err: vm::Error) -> VMLogicError {
    match err {
        vm::Error::OutOfGas => VMLogicError::EvmError(EvmError::OutOfGas),
        vm::Error::BadJumpDestination { destination } => {
            VMLogicError::EvmError(EvmError::BadJumpDestination {
                destination: destination.try_into().unwrap_or(0),
            })
        }
        vm::Error::BadInstruction { instruction } => {
            VMLogicError::EvmError(EvmError::BadInstruction { instruction })
        }
        vm::Error::StackUnderflow { instruction, wanted, on_stack } => {
            VMLogicError::EvmError(EvmError::StackUnderflow {
                instruction: instruction.to_string(),
                wanted: wanted.try_into().unwrap_or(0),
                on_stack: on_stack.try_into().unwrap_or(0),
            })
        }
        vm::Error::OutOfStack { instruction, wanted, limit } => {
            VMLogicError::EvmError(EvmError::OutOfStack {
                instruction: instruction.to_string(),
                wanted: wanted.try_into().unwrap_or(0),
                limit: limit.try_into().unwrap_or(0),
            })
        }
        vm::Error::BuiltIn(msg) => VMLogicError::EvmError(EvmError::BuiltIn(msg.to_string())),
        vm::Error::MutableCallInStaticContext => VMLogicError::EvmError(EvmError::OutOfBounds),
        vm::Error::Internal(err) => {
            VMLogicError::InconsistentStateError(InconsistentStateError::StorageError(err))
        }
        // This should not happen ever, because NEAR EVM is not using WASM.
        vm::Error::Wasm(_) => unreachable!(),
        vm::Error::OutOfBounds => VMLogicError::EvmError(EvmError::OutOfBounds),
        vm::Error::Reverted => VMLogicError::EvmError(EvmError::Reverted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_view_call() {
        let x = ViewCallArgs {
            sender: [1; 20],
            address: [2; 20],
            amount: [3; 32],
            args: vec![1, 2, 3],
        };
        let bytes = x.try_to_vec().unwrap();
        let res = ViewCallArgs::try_from_slice(&bytes).unwrap();
        assert_eq!(x, res);
        let res = ViewCallArgs::try_from_slice(&[0; 72]).unwrap();
        assert_eq!(res.args.len(), 0);
    }

    #[test]
    fn test_view_call_fail() {
        let bytes = [0; 71];
        let _ = ViewCallArgs::try_from_slice(&bytes).unwrap_err();
    }
}