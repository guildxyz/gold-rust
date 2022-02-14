use super::*;

use crate::{MAX_CYCLE_PERIOD, MIN_CYCLE_PERIOD, UNIVERSAL_BID_FLOOR};
use solana_program::clock::UnixTimestamp;

// In case of token auction creation there are two possibilities:
// - Create new mint
// - Use an existing mint

// If using an existing mint account, the mint authority must be
// transferred to the contract pda.

#[allow(clippy::too_many_arguments)]
pub fn initialize_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    auction_name: AuctionName,
    auction_description: AuctionDescription,
    mut auction_config: AuctionConfig,
    create_token_args: CreateTokenArgs,
    auction_start_timestamp: Option<UnixTimestamp>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // User accounts
    let auction_owner_account = next_account_info(account_info_iter)?;
    // Contract state accounts
    let auction_pool_account = next_account_info(account_info_iter)?;
    let auction_root_state_account = next_account_info(account_info_iter)?;
    let auction_cycle_state_account = next_account_info(account_info_iter)?;
    let auction_bank_account = next_account_info(account_info_iter)?;
    // Contract PDA account
    let contract_pda = next_account_info(account_info_iter)?;
    // Solana accounts
    let rent_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    if !auction_owner_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check account ownership
    // User accounts:
    //   auction_owner_account
    // Pda accounts:
    //   contract_pda
    // Accounts created in this instruction:
    //   auction_root_state_account
    //   auction_cycle_state_account
    //   auction_bank_account
    // Check cross-program invocation addresses
    assert_rent_program(rent_program.key)?;
    assert_system_program(system_program.key)?;
    assert_token_program(token_program.key)?;

    // Check pda addresses
    SignerPda::check_owner(
        &auction_pool_seeds(),
        program_id,
        program_id,
        auction_pool_account,
    )?;

    let auction_root_state_seeds = auction_root_state_seeds(&auction_id);
    let auction_root_state_pda = SignerPda::new_checked(
        &auction_root_state_seeds,
        program_id,
        auction_root_state_account,
    )?;

    let cycle_num_bytes = 1_u64.to_le_bytes();
    let auction_cycle_state_seeds =
        auction_cycle_state_seeds(auction_root_state_account.key, &cycle_num_bytes);
    let auction_cycle_state_pda = SignerPda::new_checked(
        &auction_cycle_state_seeds,
        program_id,
        auction_cycle_state_account,
    )?;

    let auction_bank_seeds = auction_bank_seeds(&auction_id);
    let auction_bank_pda =
        SignerPda::new_checked(&auction_bank_seeds, program_id, auction_bank_account)?;

    let contract_pda_seeds = contract_pda_seeds();
    let contract_signer_pda =
        SignerPda::new_checked(&contract_pda_seeds, program_id, contract_pda)?;

    // Register new auction into the auction pool
    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    auction_pool.try_insert_sorted(auction_id)?;

    auction_pool.write(auction_pool_account)?;

    if !auction_root_state_account.data_is_empty() {
        return Err(AuctionContractError::AuctionAlreadyInitialized.into());
    }

    // Create auction root and first cycle state accounts
    create_state_account(
        auction_owner_account,
        auction_root_state_account,
        auction_root_state_pda.signer_seeds(),
        program_id,
        system_program,
        AuctionRootState::MAX_SERIALIZED_LEN
            .checked_add(crate::EXTRA_ROOT_STATE_BYTES)
            .unwrap(),
    )?;
    create_state_account(
        auction_owner_account,
        auction_cycle_state_account,
        auction_cycle_state_pda.signer_seeds(),
        program_id,
        system_program,
        AuctionCycleState::MAX_SERIALIZED_LEN,
    )?;

    // Create auction bank account
    create_state_account(
        auction_owner_account,
        auction_bank_account,
        auction_bank_pda.signer_seeds(),
        program_id,
        system_program,
        0,
    )?;

    // Check if provided minimum_bid_amount is higher than the universal bid floor
    if auction_config.minimum_bid_amount < UNIVERSAL_BID_FLOOR {
        return Err(AuctionContractError::InvalidMinimumBidAmount.into());
    }

    // Check if provided auction cycle period is valid
    if auction_config.cycle_period < MIN_CYCLE_PERIOD
        || auction_config.cycle_period > MAX_CYCLE_PERIOD
    {
        return Err(AuctionContractError::InvalidCyclePeriod.into());
    }

    // Check validity of number of cycles
    if let Some(0) = auction_config.number_of_cycles {
        auction_config.number_of_cycles = None;
    }

    // Check that the auction id contains only ascii characters
    if !auction_id.is_ascii() {
        return Err(AuctionContractError::AuctionIdNotAscii.into());
    }

    // Check auction start time (if provided)
    let clock = Clock::get()?;
    if let Some(start_time) = auction_start_timestamp {
        if start_time < clock.unix_timestamp {
            return Err(AuctionContractError::InvalidStartTime.into());
        }
    }
    let start_time = auction_start_timestamp.unwrap_or(clock.unix_timestamp);
    let end_time = start_time
        .checked_add(auction_config.cycle_period)
        .ok_or(AuctionContractError::ArithmeticError)?;

    // Create default initialization state objects
    let bid_history = BidHistory::new();

    let cycle_state = AuctionCycleState {
        end_time,
        bid_history,
    };
    cycle_state.write(auction_cycle_state_account)?;

    let token_config = match create_token_args {
        CreateTokenArgs::Nft {
            mut metadata_args,
            is_repeating,
        } => {
            // Nft accounts
            let master_edition_account = next_account_info(account_info_iter)?;
            let master_holding_account = next_account_info(account_info_iter)?;
            let master_metadata_account = next_account_info(account_info_iter)?;
            let master_mint_account = next_account_info(account_info_iter)?;
            // Metaplex account
            let metadata_program = next_account_info(account_info_iter)?;

            // Check account ownership
            // Accounts created in this instruction:
            //   master_edition_account
            //   master_holding_account
            //   master_metadata_account
            //   master_mint_account

            // Check cross-program invocation addresses
            assert_metaplex_program(metadata_program.key)?;

            // Check pda addresses

            // Not checking the following pdas since these are checked (and owned) by metaplex
            // master_edition_account
            // master_metadata_account

            let master_mint_seeds = master_mint_seeds(&auction_id);
            let master_mint_pda =
                SignerPda::new_checked(&master_mint_seeds, program_id, master_mint_account)?;

            let master_holding_seeds = master_holding_seeds(&auction_id);
            let master_holding_pda =
                SignerPda::new_checked(&master_holding_seeds, program_id, master_holding_account)?;

            if !master_metadata_account.data_is_empty() {
                return Err(AuctionContractError::AuctionAlreadyInitialized.into());
            }

            // Create mint and respective holding account
            // and mint a single NFT to the holding account

            initialize_create_metadata_args(&mut metadata_args, is_repeating);

            // create mint account
            create_mint_account(
                auction_owner_account,
                master_mint_account,
                contract_pda,
                master_mint_pda.signer_seeds(),
                rent_program,
                system_program,
                token_program,
                0,
            )?;

            // create master holding account
            create_token_holding_account(
                auction_owner_account,
                contract_pda,
                master_holding_account,
                master_mint_account,
                master_holding_pda.signer_seeds(),
                system_program,
                token_program,
                rent_program,
            )?;

            // mint a single token to the holding account
            let mint_ix = token_instruction::mint_to(
                token_program.key,
                master_mint_account.key,
                master_holding_account.key,
                contract_pda.key,
                &[contract_pda.key],
                1,
            )?;

            invoke_signed(
                &mint_ix,
                &[
                    contract_pda.clone(),
                    token_program.clone(),
                    master_holding_account.clone(),
                    master_mint_account.clone(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            msg!("metaplex id: {:?}", *metadata_program.key);
            // create metadata on this nft account
            let metadata_ix = meta_instruction::create_metadata_accounts(
                *metadata_program.key,
                *master_metadata_account.key,
                *master_mint_account.key,
                *contract_pda.key,
                *auction_owner_account.key,
                *contract_pda.key,
                metadata_args.data.name,
                metadata_args.data.symbol,
                metadata_args.data.uri,
                metadata_args.data.creators,
                metadata_args.data.seller_fee_basis_points,
                true, // update authority is signer (NOTE contract pda will sign, so could be true)
                true, // master edition metadata must be mutable regardless of the input
            );

            invoke_signed(
                &metadata_ix,
                &[
                    metadata_program.clone(),
                    master_metadata_account.clone(),
                    master_mint_account.clone(),
                    auction_owner_account.clone(),
                    contract_pda.clone(),
                    system_program.clone(),
                    rent_program.clone(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            // turn nft into master edition
            let master_edition_ix = meta_instruction::create_master_edition(
                *metadata_program.key,
                *master_edition_account.key,
                *master_mint_account.key,
                *contract_pda.key,
                *contract_pda.key,
                *master_metadata_account.key,
                *auction_owner_account.key,
                auction_config.number_of_cycles,
            );

            invoke_signed(
                &master_edition_ix,
                &[
                    metadata_program.clone(),
                    master_edition_account.clone(),
                    master_mint_account.clone(),
                    contract_pda.clone(),
                    auction_owner_account.clone(),
                    master_metadata_account.clone(),
                    rent_program.clone(),
                    system_program.clone(),
                    token_program.clone(),
                ],
                &[&contract_signer_pda.signer_seeds()],
            )?;

            TokenConfig::Nft(NftData {
                master_edition: *master_edition_account.key,
                is_repeating,
            })
        }
        CreateTokenArgs::Token {
            decimals,
            per_cycle_amount,
            existing_account,
        } => {
            if per_cycle_amount == 0 {
                return Err(AuctionContractError::InvalidPerCycleAmount.into());
            }
            // Parse mint account
            let token_mint_account = next_account_info(account_info_iter)?;

            // Accounts (potentially) created in this instruction:
            //   token_mint_account

            assert_token_mint_arg_consistency(token_mint_account, &existing_account)?;

            if token_mint_account.data_is_empty() {
                // New mint account
                // Check pda address
                let token_mint_seeds = token_mint_seeds(&auction_id);
                let token_mint_pda =
                    SignerPda::new_checked(&token_mint_seeds, program_id, token_mint_account)?;

                // Create ERC20 mint
                create_mint_account(
                    auction_owner_account,
                    token_mint_account,
                    contract_pda,
                    token_mint_pda.signer_seeds(),
                    rent_program,
                    system_program,
                    token_program,
                    decimals,
                )?;
            } else {
                // Existing mint account
                // Check if auction owner is the mint authority of provided mint
                assert_mint_authority(token_mint_account, auction_owner_account.key)?;

                let transfer_authority_ix = spl_token::instruction::set_authority(
                    &TOKEN_ID,
                    token_mint_account.key,
                    Some(contract_pda.key),
                    spl_token::instruction::AuthorityType::MintTokens,
                    auction_owner_account.key,
                    &[auction_owner_account.key],
                )?;

                invoke(
                    &transfer_authority_ix,
                    &[
                        token_program.to_owned(),
                        contract_pda.to_owned(),
                        auction_owner_account.to_owned(),
                        token_mint_account.to_owned(),
                    ],
                )?;
            }

            TokenConfig::Token(TokenData {
                per_cycle_amount,
                mint: *token_mint_account.key,
            })
        }
    };

    // Initialize root state account
    let root_state = AuctionRootState {
        auction_name,
        auction_owner: *auction_owner_account.key,
        description: auction_description,
        auction_config,
        token_config,
        status: AuctionStatus {
            current_auction_cycle: 1,
            current_idle_cycle_streak: 0,
            is_finished: false,
            is_frozen: false,
            is_filtered: false,
            is_verified: false,
        },
        all_time_treasury: 0,
        available_funds: 0,
        start_time,
    };
    root_state.write(auction_root_state_account)?;

    Ok(())
}
