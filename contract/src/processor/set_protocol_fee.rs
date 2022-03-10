use super::*;

pub fn process_set_protocol_fee(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_fee: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let contract_admin_account = next_account_info(account_info_iter)?;
    let contract_bank_account = next_account_info(account_info_iter)?;
    let protocol_fee_state_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    if !contract_admin_account.is_signer {
        msg!("admin signature is missing");
        return Err(ProgramError::MissingRequiredSignature);
    }

    assert_system_program(system_program.key)?;

    // Check pda addresses
    SignerPda::check_owner(
        &contract_bank_seeds(),
        program_id,
        program_id,
        contract_bank_account,
    )?;

    let fee_state_seeds = protocol_fee_state_seeds();
    let fee_account_pda =
        SignerPda::new_checked(&fee_state_seeds, program_id, protocol_fee_state_account)?;

    // Check contract admin authority
    let contract_bank_state = ContractBankState::read(contract_bank_account)?;
    if contract_admin_account.key != &contract_bank_state.contract_admin {
        return Err(AuctionContractError::ContractAdminMismatch.into());
    }

    // Check fee account owner or create it if necessary
    let mut fee_state = if protocol_fee_state_account.data_is_empty() {
        create_state_account(
            contract_admin_account,
            protocol_fee_state_account,
            fee_account_pda.signer_seeds(),
            program_id,
            system_program,
            ProtocolFeeState::MAX_SERIALIZED_LEN,
        )?;
        ProtocolFeeState { fee: 50 }
    } else {
        assert_owner(protocol_fee_state_account, program_id)?;
        ProtocolFeeState::read(protocol_fee_state_account)?
    };

    if new_fee > 50 {
        return Err(AuctionContractError::InvalidProtocolFee.into());
    }

    fee_state.fee = new_fee;
    fee_state.write(protocol_fee_state_account)?;

    Ok(())
}
