use super::*;

pub struct SetProtocolFeeArgs {
    pub contract_admin_pubkey: Pubkey,
    pub new_fee: u8,
}

pub fn set_protocol_fee(args: &SetProtocolFeeArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (protocol_fee_state_pubkey, _) =
        Pubkey::find_program_address(&protocol_fee_state_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.contract_admin_pubkey, true),
        AccountMeta::new_readonly(contract_bank_pubkey, false),
        AccountMeta::new(protocol_fee_state_pubkey, false),
        AccountMeta::new_readonly(SYS_ID, false),
    ];

    let instruction = AuctionInstruction::SetProtocolFee {
        new_fee: args.new_fee,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
