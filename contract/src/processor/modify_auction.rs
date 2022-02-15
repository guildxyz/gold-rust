use super::*;

pub fn process_modify_auction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    auction_id: AuctionId,
    modify_data: ModifyAuctionData,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let auction_owner_account = next_account_info(account_info_iter)?;
    let auction_root_state_account = next_account_info(account_info_iter)?;

    if !auction_owner_account.is_signer {
        msg!("owner signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check pda addresses
    SignerPda::check_owner(
        &auction_root_state_seeds(&auction_id),
        program_id,
        program_id,
        auction_root_state_account,
    )?;

    // Check auction owner account
    let mut auction_root_state = AuctionRootState::read(auction_root_state_account)?;
    if auction_owner_account.key != &auction_root_state.auction_owner {
        return Err(AuctionContractError::AuctionOwnerMismatch.into());
    }

    if let Some(new_description) = modify_data.new_description {
        auction_root_state.description.description = new_description;
    }

    if let Some(new_socials) = modify_data.new_socials {
        auction_root_state.description.socials = new_socials;
    }

    if let Some(new_encore_period) = modify_data.new_encore_period {
        if new_encore_period < 0 {
            return Err(AuctionContractError::InvalidEncorePeriod.into());
        }
        auction_root_state.auction_config.encore_period = new_encore_period;
    }

    auction_root_state.write(auction_root_state_account)?;

    Ok(())
}
