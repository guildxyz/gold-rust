use super::*;
use solana_program::rent::Rent;

pub fn process_admin_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let withdraw_authority = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;

    if !withdraw_authority.is_signer {
        msg!("withdraw authority signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    if contract_bank_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    let contract_bank_seeds = get_contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.withdraw_authority != withdraw_authority.key {
        return Err(AuctionContractError::WithdrawAuthorityMismatch.into());
    }
    // rent check
    let mut available_lamports = **contract_bank_account.lamports.borrow();
    available_lamports = available_lamports
        .checked_sub(Rent::get()?.minimum_balance(ContractBankState::MAX_SERIALIZED_LEN))
        .ok_or(AuctionContractError::ArithmeticError)?;

    if amount > available_lamports {
        return Err(AuctionContractError::InvalidClaimAmount.into());
    }

    checked_credit_account(withdraw_authority, amount)?;
    checked_debit_account(contract_bank_account, amount)?;

    Ok(())
}

pub fn process_admin_withdraw_reassign(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_withdraw_authority: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let withdraw_authority = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;

    if !withdraw_authority.is_signer {
        msg!("withdraw authority signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }
    if contract_bank_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    let contract_bank_seeds = get_contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.withdraw_authority != withdraw_authority.key {
        return Err(AuctionContractError::WithdrawAuthorityMismatch.into());
    }

    contract_bank_state.withdraw_authority = new_withdraw_authority;
    contract_bank_state.write(contract_bank_account)?;

    Ok(())
}
