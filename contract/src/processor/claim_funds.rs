use super::*;
use solana_program::rent::Rent;

pub fn process_claim_funds(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let auction_owner_account = next_account_info(account_info_iter)?;
    let auction_bank_account = next_account_info(account_info_iter)?;
    let auction_root_state_account = next_account_info(account_info_iter)?;
    let auction_cycle_state_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;

    if !auction_owner_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   auction_owner_account
    //   contract_admin_account
    if auction_bank_account.owner != program_id
        || auction_root_state_account.owner != program_id
        || auction_cycle_state_account.owner != program_id
        || contract_bank_account.owner != program_id
    {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    let contract_bank_seeds = contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let auction_root_state_seeds = auction_root_state_seeds(&auction_id);
    SignerPda::new_checked(
        &auction_root_state_seeds,
        auction_root_state_account.key,
        program_id,
    )
    .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;

    if auction_owner_account.key != &auction_root_state.auction_owner {
        return Err(AuctionContractError::AuctionOwnerMismatch.into());
    }

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

    let auction_cycle_state = AuctionCycleState::read(auction_cycle_state_account)?;

    let auction_bank_seeds = auction_bank_seeds(&auction_id);
    SignerPda::new_checked(&auction_bank_seeds, auction_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let mut lamports_to_claim = **auction_bank_account.lamports.borrow();

    // If the auction is not active, the bank account does not need to persist
    // anymore. Otherwise (i.e. !is_finished), leave the rent
    let rent = Rent::get()?.minimum_balance(0);
    if !auction_root_state.status.is_finished {
        lamports_to_claim = lamports_to_claim
            .checked_sub(rent)
            .ok_or(AuctionContractError::ArithmeticError)?;
        // Current bid cannot be claimed until the end of the auction cycle, unless
        // it's the last one
        if let Some(most_recent_bid) = auction_cycle_state.bid_history.get_last_element() {
            lamports_to_claim = lamports_to_claim
                .checked_sub(most_recent_bid.bid_amount)
                .ok_or(AuctionContractError::ArithmeticError)?;
        }
    }

    if amount > lamports_to_claim {
        return Err(AuctionContractError::InvalidClaimAmount.into());
    }

    // This may not be precise because of integer rounding but it is more simple
    // Error is at most 19 lamports which is negligible
    let lamport_divided = amount / 20;
    let auction_owner_share = lamport_divided * 19;
    let contract_bank_share = amount
        .checked_sub(auction_owner_share)
        .ok_or(AuctionContractError::ArithmeticError)?;

    checked_credit_account(contract_bank_account, contract_bank_share)?;
    checked_credit_account(auction_owner_account, auction_owner_share)?;

    checked_debit_account(
        auction_bank_account,
        auction_owner_share + contract_bank_share,
    )?;

    // Update available funds in the root state
    auction_root_state.available_funds = auction_root_state
        .available_funds
        .checked_sub(amount)
        .ok_or(AuctionContractError::ArithmeticError)?;

    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
