use super::*;

use agsol_token_metadata::state::Data as MetadataStateData;
use std::str::FromStr;

const METADATA_DATA_START_POS: usize = 65;

// NOTE: The user can be made to pay for this account's creation by locking its fee besides their bid at the time of bidding
//   and using this locked fee now.
// NOTE: With the current calculation method we may scam the auction owner with at most 19 lamports due to rounding.
//   This may be improved.

// NOTE: We might introduce a "grace period" in which the user can not bid before initiating a new auction
//   in case they wanted to bid in the last second, so that they do not bid on the next auctioned asset accidentally
/// Closes auction cycle
///
/// Creates holding account for the won asset for the user with the highest bid.
/// The cost of this account's creation is deducted from the highest bid.
///
/// Then, distributes the deducted highest bid in the following fashion:
///
/// - 95% to the auction owner
///
/// - 5% to the contract admin
pub fn process_claim_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    cycle_number: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // misc
    let payer_account = next_account_info(account_info_iter)?;

    // user accounts
    let top_bidder_account = next_account_info(account_info_iter)?;

    // contract state accounts
    let auction_root_state_account = next_account_info(account_info_iter)?;
    let auction_cycle_state_account = next_account_info(account_info_iter)?;

    // contract signer pda
    let contract_pda = next_account_info(account_info_iter)?;

    // external programs
    let rent_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    if !payer_account.is_signer {
        msg!("payer signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check cross-program invocation addresses
    assert_rent_program(rent_program.key)?;
    assert_system_program(system_program.key)?;
    assert_token_program(token_program.key)?;

    // Check account ownership
    // User accounts:
    //   payer_account
    //   top_bidder_account
    // Pda accounts:
    //   contract_pda
    // Accounts created in this instruction:
    //   next_auction_cycle_state_account

    // check root and cycle states
    SignerPda::check_owner(
        &auction_root_state_seeds(&auction_id),
        program_id,
        program_id,
        auction_root_state_account,
    )?;

    let cycle_num_bytes = cycle_number.to_le_bytes();

    SignerPda::check_owner(
        &auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num_bytes),
        program_id,
        program_id,
        auction_cycle_state_account,
    )?;

    let contract_pda_seeds = contract_pda_seeds();
    let contract_signer_pda =
        SignerPda::new_checked(&contract_pda_seeds, program_id, contract_pda)?;

    let mut auction_cycle_state = AuctionCycleState::read(auction_cycle_state_account)?;

    // Check auction status (frozen, active, able to end cycle)
    let clock = Clock::get()?;
    let current_timestamp = clock.unix_timestamp;

    if current_timestamp < auction_cycle_state.end_time {
        return Err(AuctionContractError::AuctionIsInProgress.into());
    }
    if auction_cycle_state.end_time == 0 {
        return Err(AuctionContractError::RewardAlreadyClaimed.into());
    }

    // Check top bidder account
    let most_recent_bid_option = auction_cycle_state.bid_history.get_last_element();
    if let Some(most_recent_bid) = most_recent_bid_option {
        if top_bidder_account.key != &most_recent_bid.bidder_pubkey {
            return Err(AuctionContractError::TopBidderAccountMismatch.into());
        }
    } else {
        return Err(AuctionContractError::AuctionIsInProgress.into());
    }

    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    match auction_root_state.token_config {
        TokenConfig::Nft(_) => {
            let metadata_program = next_account_info(account_info_iter)?;
            // nft child accounts
            let child_edition_account = next_account_info(account_info_iter)?;
            let child_edition_marker_account = next_account_info(account_info_iter)?;
            let child_metadata_account = next_account_info(account_info_iter)?;
            let child_mint_account = next_account_info(account_info_iter)?;
            let child_holding_account = next_account_info(account_info_iter)?;
            // master accounts
            let master_edition_account = next_account_info(account_info_iter)?;
            let master_metadata_account = next_account_info(account_info_iter)?;
            let master_mint_account = next_account_info(account_info_iter)?;
            let master_holding_account = next_account_info(account_info_iter)?;

            // Check account ownership
            // Accounts created in this instruction:
            //   child_edition_account
            //   child_metadata_account
            //   child_mint_account
            //   child_holding_account

            if !child_edition_marker_account.data_is_empty()
                && *child_edition_marker_account.owner != META_ID
            {
                return Err(AuctionContractError::InvalidAccountOwner.into());
            }

            // Check cross-program invocation addresses
            assert_metaplex_program(metadata_program.key)?;

            // Check pda addresses
            // Not checking the following pdas since these are checked (and owned) by metaplex
            // child_edition_account
            // child_metadata_account
            // child_edition_marker_account
            // master_edition_account
            // master_metadata_account
            let child_mint_seeds = child_mint_seeds(&cycle_num_bytes, &auction_id);
            let child_mint_pda =
                SignerPda::new_checked(&child_mint_seeds, program_id, child_mint_account)?;

            let child_holding_seeds = child_holding_seeds(&cycle_num_bytes, &auction_id);
            let child_holding_pda =
                SignerPda::new_checked(&child_holding_seeds, program_id, child_holding_account)?;

            // check nft validity
            if !child_metadata_account.data_is_empty() {
                return Err(AuctionContractError::NftAlreadyExists.into());
            }

            SignerPda::check_owner(
                &master_mint_seeds(&auction_id),
                program_id,
                &TOKEN_ID,
                master_mint_account,
            )?;

            // Mint child nft to highest bidder
            // create child nft mint account
            //msg!("Mint account creation");
            create_mint_account(
                payer_account,
                child_mint_account,
                contract_pda,
                child_mint_pda.signer_seeds(),
                rent_program,
                system_program,
                token_program,
                0,
            )?;

            //msg!("Holding account creation");
            // create child nft holding account
            create_token_holding_account(
                payer_account,
                top_bidder_account,
                child_holding_account,
                child_mint_account,
                child_holding_pda.signer_seeds(),
                system_program,
                token_program,
                rent_program,
            )?;

            //msg!("Minting nft");
            let mint_ix = spl_token::instruction::mint_to(
                token_program.key,
                child_mint_account.key,
                child_holding_account.key,
                contract_pda.key,
                &[contract_pda.key],
                1,
            )?;

            invoke_signed(
                &mint_ix,
                &[
                    contract_pda.to_owned(),
                    token_program.to_owned(),
                    child_holding_account.to_owned(),
                    child_mint_account.to_owned(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            // change master metadata so that child can inherit it
            //msg!("Updating metadata account");
            let mut new_master_metadata = try_from_slice_unchecked::<MetadataStateData>(
                &master_metadata_account.data.borrow_mut()[METADATA_DATA_START_POS..],
            )
            .unwrap();

            let edition_number_range =
                find_edition_number_range_in_uri(&mut new_master_metadata.uri)?;
            let current_edition_number =
                u64::from_str(&new_master_metadata.uri[edition_number_range.clone()]).unwrap();

            new_master_metadata
                .uri
                .replace_range(edition_number_range, &(cycle_number).to_string());

            let change_master_metadata_ix = meta_instruction::update_metadata_accounts(
                *metadata_program.key,
                *master_metadata_account.key,
                *contract_pda.key,
                None,
                Some(new_master_metadata.clone()),
                None,
            );

            invoke_signed(
                &change_master_metadata_ix,
                &[master_metadata_account.clone(), contract_pda.clone()],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            // turn single child token into nft
            //msg!("Creating child nft");
            let mint_child_ix = meta_instruction::mint_new_edition_from_master_edition_via_token(
                *metadata_program.key,
                *child_metadata_account.key,
                *child_edition_account.key,
                *master_edition_account.key,
                *child_mint_account.key,
                *contract_pda.key,
                *payer_account.key,
                *contract_pda.key,
                *master_holding_account.key,
                *contract_pda.key,
                *master_metadata_account.key,
                *master_mint_account.key,
                cycle_number,
            );

            invoke_signed(
                &mint_child_ix,
                &[
                    master_edition_account.clone(),
                    master_holding_account.clone(),
                    master_metadata_account.clone(),
                    child_edition_account.clone(),
                    child_edition_marker_account.clone(),
                    child_holding_account.clone(),
                    child_metadata_account.clone(),
                    child_mint_account.clone(),
                    payer_account.clone(),
                    contract_pda.clone(),
                    rent_program.clone(),
                    system_program.clone(),
                    token_program.clone(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            // Change metadata back to live edition number
            let edition_number_range =
                find_edition_number_range_in_uri(&mut new_master_metadata.uri)?;
            new_master_metadata
                .uri
                .replace_range(edition_number_range, &(current_edition_number).to_string());

            let change_master_metadata_ix = meta_instruction::update_metadata_accounts(
                *metadata_program.key,
                *master_metadata_account.key,
                *contract_pda.key,
                None,
                Some(new_master_metadata),
                None,
            );

            invoke_signed(
                &change_master_metadata_ix,
                &[master_metadata_account.clone(), contract_pda.clone()],
                &[&contract_signer_pda.signer_seeds()],
            )?;
        }
        TokenConfig::Token(ref token_data) => {
            // Token mint account
            let token_mint_account = next_account_info(account_info_iter)?;
            // User's token holding account
            let token_holding_account = next_account_info(account_info_iter)?;

            // Check account ownership
            // Accounts created in this instruction:
            //   token_holding_account
            assert_token_mint(&token_data.mint, token_mint_account)?;
            assert_mint_authority(token_mint_account, contract_pda.key)?;
            assert_owner(token_mint_account, &TOKEN_ID)?;

            // SignerPda check is not required due to the previous checks
            if token_mint_account.key != &token_data.mint {
                return Err(AuctionContractError::InvalidSeeds.into());
            }

            let token_holding_seeds =
                token_holding_seeds(token_mint_account.key, top_bidder_account.key);
            let token_holding_pda =
                SignerPda::new_checked(&token_holding_seeds, program_id, token_holding_account)?;

            // create token holding account (if needed)
            if token_holding_account.data_is_empty() {
                create_token_holding_account(
                    payer_account,
                    top_bidder_account,
                    token_holding_account,
                    token_mint_account,
                    token_holding_pda.signer_seeds(),
                    system_program,
                    token_program,
                    rent_program,
                )?;
            }

            // mint tokens to the highest bidder
            let mint_ix = spl_token::instruction::mint_to(
                token_program.key,
                token_mint_account.key,
                token_holding_account.key,
                contract_pda.key,
                &[contract_pda.key],
                token_data.per_cycle_amount,
            )?;

            invoke_signed(
                &mint_ix,
                &[
                    contract_pda.to_owned(),
                    token_program.to_owned(),
                    token_holding_account.to_owned(),
                    token_mint_account.to_owned(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;
        }
    }

    auction_cycle_state.end_time = 0;
    auction_cycle_state.write(auction_cycle_state_account)?;

    auction_root_state.unclaimed_rewards = auction_root_state
        .unclaimed_rewards
        .checked_sub(1)
        .ok_or(AuctionContractError::ArithmeticError)?;
    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}

pub fn find_edition_number_range_in_uri(
    uri: &mut String,
) -> Result<std::ops::Range<usize>, AuctionContractError> {
    let uri_len = uri.len();
    let mut last_pos = uri_len;
    let mut dot_pos = uri_len;
    let mut slash_pos = uri_len;

    let str_bytes = uri.as_bytes();
    for i in (0..uri_len).rev() {
        if str_bytes[i] == 0 {
            last_pos = i;
        }

        // ".".as_bytes() == [46]
        if str_bytes[i] == 46 {
            dot_pos = i;
        }

        // "/".as_bytes() == [47]
        if str_bytes[i] == 47 {
            slash_pos = i + 1;
            break;
        }
    }

    if last_pos == 0 || dot_pos == 0 || slash_pos == 0 || dot_pos < slash_pos {
        return Err(AuctionContractError::MetadataManipulationError);
    }

    uri.truncate(last_pos);

    Ok(slash_pos..dot_pos)
}
