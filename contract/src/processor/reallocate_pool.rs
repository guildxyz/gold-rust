use super::*;
use solana_program::sysvar::rent::Rent;

pub fn reallocate_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_max_auction_num: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;
    let auction_pool_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check cross-program invocation addresses
    assert_system_program(system_program.key)?;

    // check pda addresses
    SignerPda::check_owner(
        &contract_bank_seeds(),
        program_id,
        program_id,
        contract_bank_account,
    )?;

    let auction_pool_pda_check = SignerPda::check_owner(
        &auction_pool_seeds(),
        program_id,
        program_id,
        auction_pool_account,
    );

    let secondary_pool_pda_check = SignerPda::check_owner(
        &secondary_pool_seeds(),
        program_id,
        program_id,
        auction_pool_account,
    );

    if auction_pool_pda_check.is_err() && secondary_pool_pda_check.is_err() {
        auction_pool_pda_check?;
    }

    // check admin
    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.contract_admin != contract_admin_account.key {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    if new_max_auction_num < auction_pool.max_len {
        return Err(AuctionContractError::ShrinkingPoolIsNotAllowed.into());
    }
    let rent_program = Rent::get()?;

    let old_account_size = AuctionPool::max_serialized_len(auction_pool.max_len as usize)
        .ok_or(AuctionContractError::ArithmeticError)?;
    let new_account_size = AuctionPool::max_serialized_len(new_max_auction_num as usize)
        .ok_or(AuctionContractError::ArithmeticError)?;
    let old_rent = rent_program.minimum_balance(old_account_size);
    let new_rent = rent_program.minimum_balance(new_account_size);

    auction_pool.max_len = new_max_auction_num;
    auction_pool.write(auction_pool_account)?;

    // send rent difference for auction pool to be rent exempt
    let rent_difference = new_rent
        .checked_sub(old_rent)
        .ok_or(AuctionContractError::ArithmeticError)?;

    let transfer_ix = system_instruction::transfer(
        contract_admin_account.key,
        auction_pool_account.key,
        rent_difference,
    );

    invoke(
        &transfer_ix,
        &[
            contract_admin_account.to_owned(),
            auction_pool_account.to_owned(),
            system_program.to_owned(),
        ],
    )?;

    // reallocate auction pool
    auction_pool_account.realloc(new_account_size, false)
}
