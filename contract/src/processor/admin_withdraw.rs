use super::*;
use solana_program::rent::Rent;

pub fn process_admin_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let withdraw_authority = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;

    if !withdraw_authority.is_signer {
        msg!("withdraw authority signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    if contract_bank_account.owner != program_id {
        return Err(AuctionContractError::InvalidAccountOwner.into());
    }

    // Check pda addresses
    let contract_bank_seeds = get_contract_bank_seeds();
    SignerPda::new_checked(&contract_bank_seeds, contract_bank_account.key, program_id)
        .map_err(|_| AuctionContractError::InvalidSeeds)?;

    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if &contract_bank_state.withdraw_authority != withdraw_authority.key {
        return Err(AuctionContractError::WithdrawAuthorityMismatch.into());
    }
    // rent check
    todo!();
}
