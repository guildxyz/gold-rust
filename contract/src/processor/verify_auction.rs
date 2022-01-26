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

    // Check pda addresses
    SignerPda::check_owner(
        &contract_bank_seeds(),
        program_id,
        program_id,
        contract_bank_account,
    )?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    SignerPda::check_owner(
        &auction_root_state_seeds(&auction_id),
        program_id,
        program_id,
        auction_root_state_account,
    )?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    auction_root_state.status.is_verified = true;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
