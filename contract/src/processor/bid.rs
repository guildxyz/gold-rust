use super::*;

pub fn process_bid(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let user_main_account = next_account_info(account_info_iter)?; // 1
    let auction_bank_account = next_account_info(account_info_iter)?; // 2
    let auction_root_state_account = next_account_info(account_info_iter)?; // 3
    let auction_cycle_state_account = next_account_info(account_info_iter)?; // 4
    let top_bidder_account = next_account_info(account_info_iter)?; // 5
    let system_program = next_account_info(account_info_iter)?; // 6

    // Check cross-program invocation addresses
    assert_system_program(system_program.key)?;

    // Check account ownership
    // User accounts:
    //   user_main_account
    //   top_bidder_account
    if auction_bank_account.owner != program_id
        || auction_root_state_account.owner != program_id
        || auction_cycle_state_account.owner != program_id
    {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    let auction_root_state_seeds = get_auction_root_state_seeds(&auction_id);
    SignerPda::new_checked(
        &auction_root_state_seeds,
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;

    let cycle_num = auction_root_state
        .status
        .current_auction_cycle
        .to_le_bytes();
    let auction_cycle_state_seeds =
        get_auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num);
    SignerPda::new_checked(
        &auction_cycle_state_seeds,
        auction_cycle_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_cycle_state = AuctionCycleState::read(auction_cycle_state_account)?;

    let auction_bank_seeds = get_auction_bank_seeds(&auction_id);
    SignerPda::new_checked(&auction_bank_seeds, auction_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    // Check if user is signer
    if !user_main_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check and update auction status
    let clock = Clock::get()?;
    let current_timestamp = clock.unix_timestamp;
    check_status(
        &auction_root_state,
        &auction_cycle_state,
        current_timestamp,
        AuctionInteraction::Bid,
    )?;
    check_bid_amount(&auction_root_state, &auction_cycle_state, amount)?;

    let most_recent_bid_option = auction_cycle_state.bid_history.get_last_element();
    let previous_bid_amount = if let Some(ref most_recent_bid) = most_recent_bid_option {
        if top_bidder_account.key != &most_recent_bid.bidder_pubkey {
            return Err(AuctionContractError::TopBidderAccountMismatch.into());
        }
        most_recent_bid.bid_amount
    } else {
        0
    };

    auction_root_state.all_time_treasury = auction_root_state
        .all_time_treasury
        .checked_add(amount)
        .ok_or(AuctionContractError::ArithmeticError)?
        .checked_sub(previous_bid_amount)
        .ok_or(AuctionContractError::ArithmeticError)?;
    auction_root_state.write(auction_root_state_account)?;

    // Transfer SOL to fund
    let lamport_transfer_ix =
        system_instruction::transfer(user_main_account.key, auction_bank_account.key, amount);

    invoke(
        &lamport_transfer_ix,
        &[
            user_main_account.to_owned(),
            auction_bank_account.to_owned(),
            system_program.to_owned(),
        ],
    )?;

    // Transfer SOL to previous top bidder
    if previous_bid_amount > 0 {
        checked_debit_account(auction_bank_account, previous_bid_amount)?;
        checked_credit_account(top_bidder_account, previous_bid_amount)?;
    }

    let bid_data = BidData {
        bid_amount: amount,
        bidder_pubkey: *user_main_account.key,
    };

    auction_cycle_state.bid_history.cyclic_push(bid_data);

    // Check if auction end time needs to be updated
    let current_timestamp = clock.unix_timestamp;
    let min_time_for_encore_trigger = auction_cycle_state
        .end_time
        .checked_sub(auction_root_state.auction_config.encore_period)
        .ok_or(AuctionContractError::ArithmeticError)?;
    if current_timestamp > min_time_for_encore_trigger {
        auction_cycle_state.end_time = current_timestamp
            .checked_add(auction_root_state.auction_config.encore_period)
            .ok_or(AuctionContractError::ArithmeticError)?;
    }

    auction_cycle_state.write(auction_cycle_state_account)?;

    Ok(())
}
