use super::*;

use solana_program::account_info::next_account_infos;

pub fn process_delete_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    num_of_cycles_to_delete: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let auction_owner_account = next_account_info(account_info_iter)?; // 1
    let top_bidder_account = next_account_info(account_info_iter)?; // 2
    let auction_root_state_account = next_account_info(account_info_iter)?; // 3
    let auction_bank_account = next_account_info(account_info_iter)?; // 4
    let contract_bank_account = next_account_info(account_info_iter)?; // 5
    let auction_pool_account = next_account_info(account_info_iter)?; // 6
    let secondary_pool_account = next_account_info(account_info_iter)?; // 7

    if !auction_owner_account.is_signer {
        msg!("Auction owner signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account owners and seeds
    // User accounts:
    //   auction_owner_account
    //   top_bidder_account
    SignerPda::check_owner(
        &auction_root_state_seeds(&auction_id),
        program_id,
        program_id,
        auction_root_state_account,
    )?;

    SignerPda::check_owner(
        &contract_bank_seeds(),
        program_id,
        program_id,
        contract_bank_account,
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

    if auction_bank_account.lamports() != 0 {
        SignerPda::check_owner(
            &auction_bank_seeds(&auction_id),
            program_id,
            program_id,
            auction_bank_account,
        )?;
    } else {
        let auction_bank_seeds = auction_bank_seeds(&auction_id);
        SignerPda::new_checked(&auction_bank_seeds, program_id, auction_bank_account)?;
    }

    // Check auction owner account
    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    if auction_owner_account.key != &auction_root_state.auction_owner {
        return Err(AuctionContractError::AuctionOwnerMismatch.into());
    }

    let removable_cycle_states_num = std::cmp::min(
        auction_root_state.status.current_auction_cycle,
        num_of_cycles_to_delete,
    ) as usize;

    // The auction cycle states to remove in reverse chronological order
    let auction_cycle_states = next_account_infos(account_info_iter, removable_cycle_states_num)?; // 8+

    // Iterate over auction cycle states
    let mut cycle_num = auction_root_state.status.current_auction_cycle;
    for auction_cycle_state_account in auction_cycle_states {
        // Check auction cycle state account address
        let cycle_num_bytes = cycle_num.to_le_bytes();

        SignerPda::check_owner(
            &auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num_bytes),
            program_id,
            program_id,
            auction_cycle_state_account,
        )?;

        // Refund top bidder of the last cycle
        if !auction_root_state.status.is_frozen && !auction_root_state.status.is_finished {
            let auction_cycle_state = AuctionCycleState::read(auction_cycle_state_account)?;
            refund_top_bidder(
                auction_bank_account,
                top_bidder_account,
                &auction_cycle_state,
            )?;
            auction_root_state.status.is_frozen = true;
        }

        // Deallocate cycle state
        deallocate_state(auction_cycle_state_account, contract_bank_account)?;

        cycle_num -= 1;
    }

    // Decrement cycle number
    // NOTE: This could be = cycle_num as well
    auction_root_state.status.current_auction_cycle -= removable_cycle_states_num as u64;

    // Return if there are still cycle states to remove (to not run out of compute units)
    if auction_root_state.status.current_auction_cycle > 0 {
        auction_root_state.write(auction_root_state_account)?;
        return Ok(());
    }

    // Deallocate remaining states if all cycle states are deallocated
    let bank_balance = **auction_bank_account.lamports.borrow();
    claim_lamports(
        bank_balance,
        auction_owner_account,
        auction_bank_account,
        contract_bank_account,
    )?;

    deallocate_state(auction_root_state_account, auction_owner_account)?;

    // Remove auction entry from auction pools
    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    auction_pool.remove(&auction_id);
    auction_pool.write(auction_pool_account)?;

    let mut secondary_auction_pool = AuctionPool::read(secondary_pool_account)?;
    secondary_auction_pool.remove(&auction_id);
    secondary_auction_pool.write(secondary_pool_account)?;

    Ok(())
}

#[inline(always)]
fn deallocate_state<'a>(
    from: &'a AccountInfo,
    to: &'a AccountInfo,
) -> Result<(), AuctionContractError> {
    let lamports_to_claim = **from.lamports.borrow();
    checked_debit_account(from, lamports_to_claim)?;
    checked_credit_account(to, lamports_to_claim)
}

fn refund_top_bidder<'a>(
    auction_bank_account: &'a AccountInfo,
    top_bidder_account: &'a AccountInfo,
    auction_cycle_state: &'a AuctionCycleState,
) -> Result<(), AuctionContractError> {
    let most_recent_bid_option = auction_cycle_state.bid_history.get_last_element();
    if let Some(most_recent_bid) = most_recent_bid_option {
        if top_bidder_account.key != &most_recent_bid.bidder_pubkey {
            return Err(AuctionContractError::TopBidderAccountMismatch);
        }

        checked_debit_account(auction_bank_account, most_recent_bid.bid_amount)?;
        checked_credit_account(top_bidder_account, most_recent_bid.bid_amount)?;
    }
    Ok(())
}
