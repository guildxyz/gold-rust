use super::*;

pub fn initialize_contract(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    withdraw_authority: Pubkey,
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

    // Check account ownership
    // User accounts:
    //   contract_admin_account
    // Accounts created in this instruction:
    //   auction_pool_account
    //   contract_bank_account

    // Check cross-program invocation addresses
    assert_system_program(system_program.key)?;

    // Check pda addresses
    let contract_bank_seeds = get_contract_bank_seeds();
    let contract_bank_pda =
        SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
            .map_err(|_| AuctionContractError::InvalidSeeds)?;
    let auction_pool_seeds = get_auction_pool_seeds();
    let auction_pool_pda =
        SignerPda::new_checked(&auction_pool_seeds, auction_pool_account.key, program_id)
            .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Create auction pool account
    if auction_pool_account.data_is_empty() {
        create_state_account(
            contract_admin_account,
            auction_pool_account,
            auction_pool_pda.signer_seeds(),
            program_id,
            system_program,
            AuctionPool::MAX_SERIALIZED_LEN,
        )?;
    } else {
        return Err(AuctionContractError::ContractAlreadyInitialized.into());
    }

    // Create contract bank account
    if contract_bank_account.lamports() == 0 {
        create_state_account(
            contract_admin_account,
            contract_bank_account,
            contract_bank_pda.signer_seeds(),
            program_id,
            system_program,
            ContractBankState::MAX_SERIALIZED_LEN,
        )?;
        let contract_bank_state = ContractBankState {
            contract_admin: *contract_admin_account.key,
            withdraw_authority,
        };
        contract_bank_state.write(contract_bank_account)?;
    }
    Ok(())
}
