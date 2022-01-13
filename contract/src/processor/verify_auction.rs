use super::*;

pub fn process_verify_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;
    let auction_root_state_account = next_account_info(account_info_iter)?;

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   contract_admin_account
    if contract_bank_account.owner != program_id
        || auction_root_state_account.owner != program_id
    {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    let contract_bank_seeds = get_contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin_pubkey {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    let auction_root_state_seeds = get_auction_root_state_seeds(&auction_id);
    SignerPda::new_checked(
        &auction_root_state_seeds,
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    auction_root_state.is_verified = true;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
