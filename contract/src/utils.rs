use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::system_program::ID as SYS_ID;
use solana_program::sysvar::rent::ID as RENT_ID;

use spl_token::state::{Account, Mint};
use spl_token::ID as TOKEN_ID;

use metaplex_token_metadata::ID as META_ID;

use crate::AuctionContractError;

pub fn assert_token_program(account_pubkey: &Pubkey) -> Result<(), ProgramError> {
    if *account_pubkey != TOKEN_ID {
        return Err(AuctionContractError::InvalidProgramAddress.into());
    }
    Ok(())
}

pub fn assert_metaplex_program(account_pubkey: &Pubkey) -> Result<(), ProgramError> {
    if *account_pubkey != META_ID {
        return Err(AuctionContractError::InvalidProgramAddress.into());
    }
    Ok(())
}

pub fn assert_system_program(account_pubkey: &Pubkey) -> Result<(), ProgramError> {
    if *account_pubkey != SYS_ID {
        return Err(AuctionContractError::InvalidProgramAddress.into());
    }
    Ok(())
}

pub fn assert_rent_program(account_pubkey: &Pubkey) -> Result<(), ProgramError> {
    if *account_pubkey != RENT_ID {
        return Err(AuctionContractError::InvalidProgramAddress.into());
    }
    Ok(())
}

pub fn checked_credit_account(
    account: &AccountInfo,
    amount: u64,
) -> Result<(), AuctionContractError> {
    // The lamports need to be cloned otherwise the transaction fails with ProgramFailedToComplete
    let account_current_lamports = **account.lamports.borrow();
    if let Some(lamports) = account_current_lamports.checked_add(amount) {
        **account.lamports.borrow_mut() = lamports;
        Ok(())
    } else {
        Err(AuctionContractError::ArithmeticError)
    }
}

pub fn checked_debit_account(
    account: &AccountInfo,
    amount: u64,
) -> Result<(), AuctionContractError> {
    // The lamports need to be cloned otherwise the transaction fails with ProgramFailedToComplete
    let account_current_lamports = **account.lamports.borrow();
    if let Some(lamports) = account_current_lamports.checked_sub(amount) {
        **account.lamports.borrow_mut() = lamports;
        Ok(())
    } else {
        Err(AuctionContractError::ArithmeticError)
    }
}

pub fn assert_token_account_owner<'a>(
    token_account_info: &AccountInfo<'a>,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    let token_account = Account::unpack_from_slice(&token_account_info.data.borrow())?;

    if token_account.owner != *expected_owner {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    Ok(())
}

pub fn assert_mint_authority<'a>(
    mint_info: &AccountInfo<'a>,
    expected_authority: &Pubkey,
) -> Result<(), ProgramError> {
    let mint = Mint::unpack_from_slice(&mint_info.data.borrow())?;

    if let COption::Some(mint_authority) = mint.mint_authority {
        if mint_authority != *expected_authority {
            return Err(AuctionContractError::InvalidAccountOwner.into());
        }
    }

    Ok(())
}
