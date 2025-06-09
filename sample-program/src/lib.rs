use serde::{Deserialize, Serialize};
use derivative::Derivative;
use thiserror::Error;
use std::fmt::Display;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[allow(dead_code)]
// Base trait for state management
trait StateManagement {
    fn reset(&mut self);
    fn is_valid(&self) -> bool;
}

// Define the inner validation macro that generates the actual code
#[macro_export]
macro_rules! generate_validation_check {
    ($field:expr, $condition:expr, $error_msg:expr) => {
        if !($condition) {
            msg!($error_msg);
            return Err(ProgramError::InvalidAccountData);
        }
    };
}

// Define the outer validation macro that expands to use the inner macro
#[macro_export]
macro_rules! validate_state_field {
    ($state:expr, $field:ident, $condition:expr) => {
        generate_validation_check!(
            $state.$field,
            $condition,
            concat!("Validation failed for field: ", stringify!($field))
        );
    };
}

#[allow(dead_code)]
// Supertrait combining Display and StateManagement
trait AdvancedState: Display + StateManagement {
    fn describe(&self) -> String;
}

#[derive(Debug, Serialize, Deserialize, Derivative, Clone)]
#[derivative(Default)]
struct MetaData {
    created_at: u64,
    version: String,
}

#[derive(Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct TestState {
    #[derivative(Default(value = "42"))]
    count: u64,
    #[derivative(Default(value = "String::from(\"test\")"))]
    name: String,
    #[derivative(Default(value = "MetaData::default()"))]
    metadata: MetaData,
    #[derivative(Default(value = "Vec::new()"))]
    history: Vec<String>,
}

#[derive(Error, Debug)]
pub enum TestError {
    #[error("Invalid count: {0}")]
    InvalidCount(u64),
    #[error("Invalid name: {0}")]
    InvalidName(String),
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),
}

// Implement Display for TestState
impl Display for TestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TestState(count: {}, name: {})", self.count, self.name)
    }
}

// Implement base trait
impl StateManagement for TestState {
    fn reset(&mut self) {
        *self = TestState::default();
    }

    fn is_valid(&self) -> bool {
        !self.name.is_empty() && self.count > 0
    }
}

// Implement supertrait
impl AdvancedState for TestState {
    fn describe(&self) -> String {
        format!("State with {} entries in history", self.history.len())
    }
}

// Regular functions replacing macros
fn validate_account(
    account: &AccountInfo,
    owner: &Pubkey,
    is_signer: bool,
    is_writable: bool
) -> ProgramResult {
    if account.owner != owner {
        msg!("Invalid account owner");
        return Err(ProgramError::InvalidAccountData);
    }
    if is_signer && !account.is_signer {
        msg!("Account must be a signer");
        return Err(ProgramError::MissingRequiredSignature);
    }
    if is_writable && !account.is_writable {
        msg!("Account must be writable");
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

fn deserialize_account_data<T: serde::de::DeserializeOwned>(account_data: &[u8]) -> Result<T, ProgramError> {
    bincode::deserialize_from(account_data)
        .map_err(|e| {
            msg!("Error deserializing account data: {}", e);
            ProgramError::InvalidAccountData
        })
}

fn handle_program_error(error: &str) -> ProgramError {
    msg!("Error: {}", error);
    ProgramError::Custom(1)
}

// Solana program entrypoint
entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let account = next_account_info(accounts_iter)?;
    
    // Validate account
    validate_account(account, program_id, true, true)?;
    
    // Process instruction
    if instruction_data.is_empty() {
        return Err(handle_program_error("Invalid instruction data"));
    }
    
    // Deserialize and process state
    let state: TestState = deserialize_account_data(&account.data.borrow())?;
    
    // Use our nested macros for validation
    validate_state_field!(state, count, state.count > 0);
    validate_state_field!(state, name, !state.name.is_empty());
    
    msg!("State description: {}", state.describe());
    
    Ok(())
} 