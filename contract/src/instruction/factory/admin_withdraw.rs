use super::*;

pub struct AdminWithdrawArgs {
    withdraw_authority: Pubkey,
    amount: u64,
}

pub fn admin_withdraw(args: &AdminWithdrawArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&get_contract_bank_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.withdraw_authority, true),
        AccountMeta::new(contract_bank_pubkey, false),
    ];

    let instruction = AuctionInstruction::AdminWithdraw {
        amount: args.amount,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}

pub struct AdminWithdrawReassignArgs {
    withdraw_authority: Pubkey,
    new_withdraw_authority: Pubkey,
}

pub fn admin_withdraw_reassign(args: &AdminWithdrawReassignArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&get_contract_bank_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new_readonly(args.withdraw_authority, true),
        AccountMeta::new(contract_bank_pubkey, false),
    ];

    let instruction = AuctionInstruction::AdminWithdrawReassign {
        new_withdraw_authority: args.new_withdraw_authority,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
