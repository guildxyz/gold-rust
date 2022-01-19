use super::claim_funds::claim_lamports;
use super::*;

pub fn freeze_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let auction_owner_account = next_account_info(account_info_iter)?; // 1
    let auction_root_state_account = next_account_info(account_info_iter)?; // 2
    let auction_cycle_state_account = next_account_info(account_info_iter)?; // 2
    let auction_bank_account = next_account_info(account_info_iter)?; // 3
    let top_bidder_account = next_account_info(account_info_iter)?; // 4
    let contract_bank_account = next_account_info(account_info_iter)?; // 5

    if !auction_owner_account.is_signer {
        msg!("Auction owner signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check root and cycle state owners and pdas
    if auction_root_state_account.owner != program_id
        || auction_cycle_state_account.owner != program_id
    {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    SignerPda::new_checked(
        &auction_root_state_seeds(&auction_id),
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    let cycle_num_bytes = auction_root_state
        .status
        .current_auction_cycle
        .to_le_bytes();
    let auction_cycle_state_seeds =
        auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num_bytes);
    SignerPda::new_checked(
        &auction_cycle_state_seeds,
        auction_cycle_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Initial state checks
    if auction_owner_account.key != &auction_root_state.auction_owner {
        return Err(AuctionContractError::AuctionOwnerMismatch.into());
    }
    if auction_root_state.status.is_frozen {
        return Err(AuctionContractError::AuctionFrozen.into());
    }
    if auction_root_state.status.is_finished {
        return Err(AuctionContractError::AuctionEnded.into());
    }
    // check auction and contract bank accounts
    if auction_bank_account.owner != program_id || contract_bank_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }
    let auction_bank_seeds = auction_bank_seeds(&auction_id);
    SignerPda::new_checked(&auction_bank_seeds, auction_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    SignerPda::new_checked(
        &contract_bank_seeds(),
        contract_bank_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Freeze logic
    let auction_cycle_state = AuctionCycleState::read(auction_cycle_state_account)?;

    let mut bank_balance = **auction_bank_account.lamports.borrow();
    // refund latest top bidder
    let most_recent_bid_option = auction_cycle_state.bid_history.get_last_element();
    if let Some(most_recent_bid) = most_recent_bid_option {
        if top_bidder_account.key != &most_recent_bid.bidder_pubkey {
            return Err(AuctionContractError::TopBidderAccountMismatch.into());
        }

        checked_debit_account(auction_bank_account, most_recent_bid.bid_amount)?;
        checked_credit_account(top_bidder_account, most_recent_bid.bid_amount)?;

        auction_root_state.all_time_treasury = auction_root_state
            .all_time_treasury
            .checked_sub(most_recent_bid.bid_amount)
            .ok_or(AuctionContractError::ArithmeticError)?;

        bank_balance = bank_balance
            .checked_sub(most_recent_bid.bid_amount)
            .ok_or(AuctionContractError::ArithmeticError)?;
    }

    // claim all funds from the auction bank
    claim_lamports(
        bank_balance,
        auction_owner_account,
        auction_bank_account,
        contract_bank_account,
    )?;

    auction_root_state.available_funds = 0;
    auction_root_state.status.is_frozen = true;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
