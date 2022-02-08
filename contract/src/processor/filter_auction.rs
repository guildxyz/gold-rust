use super::*;

pub fn filter_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    filter: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?; // 1
    let auction_root_state_account = next_account_info(account_info_iter)?; // 2
    let contract_bank_account = next_account_info(account_info_iter)?; // 3
    let auction_pool_account = next_account_info(account_info_iter)?; // 4
    let secondary_pool_account = next_account_info(account_info_iter)?; // 5

    if !contract_admin_account.is_signer {
        msg!("Contract admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   contract_admin_account

    // Check pda addresses
    SignerPda::check_owner(
        &auction_root_state_seeds(&auction_id),
        program_id,
        program_id,
        auction_root_state_account,
    )?;

    SignerPda::check_owner(
        &auction_pool_seeds(),
        program_id,
        program_id,
        auction_pool_account,
    )?;

    SignerPda::check_owner(
        &secondary_pool_seeds(),
        program_id,
        program_id,
        secondary_pool_account,
    )?;

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

    // filter logic
    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    let mut secondary_pool = AuctionPool::read(secondary_pool_account)?;
    if filter {
        auction_pool.remove(&auction_id);
        secondary_pool.try_insert_sorted(auction_id)?;
    } else {
        secondary_pool.remove(&auction_id);
        auction_pool.try_insert_sorted(auction_id)?;
    }
    auction_pool.write(auction_pool_account)?;
    secondary_pool.write(secondary_pool_account)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    auction_root_state.status.is_filtered = filter;
    auction_root_state.write(auction_root_state_account)
}
