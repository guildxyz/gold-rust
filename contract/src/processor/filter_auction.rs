use super::*;

pub fn filter_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?; // 1
    let auction_root_state_account = next_account_info(account_info_iter)?; // 2
    let contract_bank_account = next_account_info(account_info_iter)?; // 3

    if !contract_admin_account.is_signer {
        msg!("Contract admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   contract_admin_account
    if contract_bank_account.owner != program_id || auction_root_state_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    SignerPda::new_checked(
        &auction_root_state_seeds(&auction_id),
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    SignerPda::new_checked(
        &contract_bank_seeds(),
        contract_bank_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    // filter logic
    auction_root_state.status.is_filtered = true;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}

pub fn unfilter_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?; // 1
    let auction_root_state_account = next_account_info(account_info_iter)?; // 2
    let contract_bank_account = next_account_info(account_info_iter)?; // 3

    if !contract_admin_account.is_signer {
        msg!("Contract admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   contract_admin_account
    if contract_bank_account.owner != program_id || auction_root_state_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    SignerPda::new_checked(
        &auction_root_state_seeds(&auction_id),
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;
    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;

    SignerPda::new_checked(
        &contract_bank_seeds(),
        contract_bank_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Initial checks
    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    // Thaw logic
    auction_root_state.status.is_filtered = false;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
