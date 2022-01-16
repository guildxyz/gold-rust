use super::*;

pub fn deallocate_pool(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;
    let auction_pool_account = next_account_info(account_info_iter)?;
    let temporary_pool_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check cross-program invocation addresses
    assert_system_program(system_program.key)?;

    // check pda addresses
    SignerPda::new_checked(
        &contract_bank_seeds(),
        contract_bank_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    SignerPda::new_checked(&auction_pool_seeds(), auction_pool_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let temporary_pool_seeds = temporary_pool_seeds();
    let temporary_pool_pda = SignerPda::new_checked(
        &temporary_pool_seeds,
        temporary_pool_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // check admin
    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.contract_admin != contract_admin_account.key {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    // create temporary auction pool account
    let auction_pool = AuctionPool::read(auction_pool_account)?;
    let account_size = AuctionPool::max_serialized_len(auction_pool.max_len as usize)
        .ok_or(AuctionContractError::ArithmeticError)?;

    create_state_account(
        contract_admin_account,
        temporary_pool_account,
        temporary_pool_pda.signer_seeds(),
        program_id,
        system_program,
        account_size,
    )?;

    auction_pool.write(temporary_pool_account)?;
    deallocate_state(auction_pool_account, contract_admin_account)
}

pub fn reallocate_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_max_auction_num: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;
    let auction_pool_account = next_account_info(account_info_iter)?;
    let temporary_pool_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check cross-program invocation addresses
    assert_system_program(system_program.key)?;

    // check pda addresses
    SignerPda::new_checked(
        &contract_bank_seeds(),
        contract_bank_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let auction_pool_seeds = auction_pool_seeds();
    let auction_pool_pda =
        SignerPda::new_checked(&auction_pool_seeds, auction_pool_account.key, program_id)
            .map_err(|_| AuctionContractError::InvalidSeeds)?;

    SignerPda::new_checked(
        &temporary_pool_seeds(),
        temporary_pool_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // check admin
    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.contract_admin != contract_admin_account.key {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    // reallocate old auction pool account
    let mut auction_pool = AuctionPool::read(temporary_pool_account)?;
    if new_max_auction_num < auction_pool.max_len {
        return Err(AuctionContractError::ShrinkingPoolIsNotAllowed.into());
    }
    let account_size = AuctionPool::max_serialized_len(new_max_auction_num as usize)
        .ok_or(AuctionContractError::ArithmeticError)?;

    create_state_account(
        contract_admin_account,
        auction_pool_account,
        auction_pool_pda.signer_seeds(),
        program_id,
        system_program,
        account_size,
    )?;

    auction_pool.max_len = new_max_auction_num;
    auction_pool.write(auction_pool_account)?;
    deallocate_state(temporary_pool_account, contract_admin_account)
}
