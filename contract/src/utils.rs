use solana_program::account_info::AccountInfo;
use solana_program::clock::UnixTimestamp;
use solana_program::program_error::ProgramError;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::system_program::ID as SYS_ID;
use solana_program::sysvar::rent::ID as RENT_ID;

use spl_token::state::{Account, Mint};
use spl_token::ID as TOKEN_ID;

use metaplex_token_metadata::instruction::CreateMetadataAccountArgs;
use metaplex_token_metadata::ID as META_ID;

use crate::state::{AuctionCycleState, AuctionRootState};
use crate::AuctionContractError;

// ************************ Accounts assertions ************************ //

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

// ************************ Arithmetic checks ************************ //

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

// ************************ Contract business logic checks ************************ //

#[repr(C)]
pub enum AuctionInteraction {
    Bid,
    CloseCycle,
}

pub fn check_status(
    root_state: &AuctionRootState,
    cycle_state: &AuctionCycleState,
    current_timestamp: UnixTimestamp,
    interaction_type: AuctionInteraction,
) -> Result<(), AuctionContractError> {
    if root_state.status.is_frozen {
        return Err(AuctionContractError::AuctionFrozen);
    }
    if !root_state.status.is_active {
        return Err(AuctionContractError::AuctionEnded);
    }
    match interaction_type {
        AuctionInteraction::Bid => {
            if current_timestamp >= cycle_state.end_time {
                return Err(AuctionContractError::AuctionCycleEnded);
            }
        }
        AuctionInteraction::CloseCycle => {
            if current_timestamp < cycle_state.end_time {
                return Err(AuctionContractError::AuctionIsInProgress);
            }
        }
    }

    Ok(())
}

pub fn check_bid_amount(
    root_state: &AuctionRootState,
    cycle_state: &AuctionCycleState,
    bid_amount: u64,
) -> Result<(), AuctionContractError> {
    if bid_amount < root_state.auction_config.minimum_bid_amount {
        return Err(AuctionContractError::InvalidBidAmount);
    }
    if let Some(most_recent_bid) = cycle_state.bid_history.get_last_element() {
        if bid_amount <= most_recent_bid.bid_amount {
            return Err(AuctionContractError::InvalidBidAmount);
        }
    }
    Ok(())
}

pub fn is_last_auction_cycle(root_state: &AuctionRootState) -> bool {
    if let Some(number_of_cycles) = root_state.auction_config.number_of_cycles {
        return root_state.status.current_auction_cycle >= number_of_cycles;
    }
    false
}

pub fn initialize_create_metadata_args(
    metadata_args: &mut CreateMetadataAccountArgs,
    is_repeating: bool,
) -> Result<(), AuctionContractError> {
    if is_repeating {
        metadata_args.data.uri.push_str("/0.json");
    } else {
        metadata_args.data.uri.push_str("/1.json");
    }
    Ok(())
}

#[cfg(test)]
mod initialize_auction_tests {
    use super::*;

    fn get_test_args() -> CreateMetadataAccountArgs {
        CreateMetadataAccountArgs {
            data: metaplex_token_metadata::state::Data {
                name: "random auction".to_owned(),
                symbol: "RAND".to_owned(),
                uri: "uri".to_owned(),
                seller_fee_basis_points: 10,
                creators: None,
            },
            is_mutable: true,
        }
    }

    #[test]
    fn test_initialize_metadata_args_valid() {
        let mut test_args_not_repeating = get_test_args();
        initialize_create_metadata_args(&mut test_args_not_repeating, false).unwrap();
        assert_eq!("uri/1.json", test_args_not_repeating.data.uri);

        let mut test_args_repeating = get_test_args();
        initialize_create_metadata_args(&mut test_args_repeating, true).unwrap();
        assert_eq!("uri/0.json", test_args_repeating.data.uri);

        let mut longer_uri_args = get_test_args();
        longer_uri_args.data.uri = "something/with/long/path".to_owned();
        initialize_create_metadata_args(&mut longer_uri_args, true).unwrap();
        assert_eq!("something/with/long/path/0.json", longer_uri_args.data.uri);
    }
}
