use super::*;

// TODO: Maybe add option to let the historical data on chain
//      If the owner pays the fees of the state accounts
pub fn process_delete_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    num_of_cycles_to_delete: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?; // 1
    let contract_bank_account = next_account_info(account_info_iter)?; // 2
    let auction_pool_account = next_account_info(account_info_iter)?; // 3
    let auction_owner_account = next_account_info(account_info_iter)?; // 4
    let auction_bank_account = next_account_info(account_info_iter)?; // 5

    let auction_root_state_account = next_account_info(account_info_iter)?; // 6

    // Check account ownership
    // User accounts:
    //   contract_admin_account
    //   auction_owner_account
    if auction_bank_account.owner != program_id
        || auction_root_state_account.owner != program_id
        || auction_pool_account.owner != program_id
        || contract_bank_account.owner != program_id
    {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let auction_root_state_seeds = get_auction_root_state_seeds(&auction_id);
    SignerPda::new_checked(
        &auction_root_state_seeds,
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let auction_pool_seeds = get_auction_pool_seeds();
    SignerPda::new_checked(&auction_pool_seeds, auction_pool_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;

    if auction_owner_account.key != &auction_root_state.auction_owner {
        return Err(AuctionContractError::AuctionOwnerMismatch.into());
    }

    if auction_root_state.status.is_active && !auction_root_state.status.is_frozen {
        return Err(AuctionContractError::AuctionIsActive.into());
    }

    let removable_cycle_states_num = std::cmp::min(
        auction_root_state.status.current_auction_cycle,
        num_of_cycles_to_delete,
    ) as usize;

    // The auction cycle states to remove in reverse chronological order
    let auction_cycle_states = next_account_infos(account_info_iter, removable_cycle_states_num)?; // 7+

    let contract_bank_seeds = get_contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin_pubkey {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    let auction_bank_seeds = get_auction_bank_seeds(&auction_id);
    SignerPda::new_checked(&auction_bank_seeds, auction_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Iterate over auction cycle states
    let mut cycle_num = auction_root_state.status.current_auction_cycle;
    for auction_cycle_state_account in auction_cycle_states {
        if auction_cycle_state_account.owner != program_id {
            return Err(AuctionContractError::InvalidAccountOwner.into());
        }

        // Check auction cycle state account address
        let cycle_num_bytes = cycle_num.to_le_bytes();
        let auction_cycle_state_seeds =
            get_auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num_bytes);
        SignerPda::new_checked(
            &auction_cycle_state_seeds,
            auction_cycle_state_account.key,
            program_id,
        )
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

        // Deallocate cycle state
        deallocate_state(auction_cycle_state_account, contract_admin_account)?;

        cycle_num = cycle_num
            .checked_sub(1)
            .ok_or(AuctionContractError::ArithmeticError)?;
    }

    // Decrement cycle number
    auction_root_state.status.current_auction_cycle = auction_root_state
        .status
        .current_auction_cycle
        .checked_sub(removable_cycle_states_num as u64)
        .ok_or(AuctionContractError::ArithmeticError)?;

    // Return if there are still cycle states to remove (to not run out of compute units)
    if auction_root_state.status.current_auction_cycle > 0 {
        auction_root_state.write(auction_root_state_account)?;
        return Ok(());
    }

    // Deallocate remaining states if all cycle states are deallocated
    deallocate_state(auction_bank_account, auction_owner_account)?;
    deallocate_state(auction_root_state_account, auction_owner_account)?;

    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    auction_pool.pool.remove(&auction_id);
    auction_pool.write(auction_pool_account)?;

    Ok(())
}

#[inline(always)]
fn deallocate_state<'a>(from: &'a AccountInfo, to: &'a AccountInfo) -> Result<(), ProgramError> {
    let lamports_to_claim = **from.lamports.borrow();
    checked_debit_account(from, lamports_to_claim)?;
    checked_credit_account(to, lamports_to_claim)?;
    Ok(())
}
