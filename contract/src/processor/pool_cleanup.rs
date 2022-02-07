use super::*;
use slugify::slugify;

pub fn process_pool_cleanup(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let payer_account = next_account_info(account_info_iter)?;
    let auction_pool_account = next_account_info(account_info_iter)?;
    let secondary_pool_account = next_account_info(account_info_iter)?;

    if !payer_account.is_signer {
        msg!("withdraw authority signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

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

    let mut auction_pool = AuctionPool::read(auction_pool_account)?;
    let mut secondary_pool = AuctionPool::read(secondary_pool_account)?;

    for account in account_info_iter {
        let root_state = AuctionRootState::read(account)?;
        if !root_state.status.is_filtered || !root_state.status.is_finished {
            return Err(AuctionContractError::AuctionIsInProgress.into());
        }
        let auction_name = String::from_utf8_lossy(&root_state.auction_name);
        let auction_id_bytes = slugify!(&auction_name).into_bytes();
        let mut auction_id = [0_u8; 32];
        auction_id.copy_from_slice(&auction_id_bytes);
        secondary_pool.try_insert_sorted(auction_id)?;
        auction_pool.remove(&auction_id);
    }

    auction_pool.write(auction_pool_account)?;
    secondary_pool.write(secondary_pool_account)?;

    Ok(())
}
