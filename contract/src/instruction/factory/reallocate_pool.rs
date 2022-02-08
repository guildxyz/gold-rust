use super::*;

pub fn reallocate_pool<'a, F>(
    contract_admin_pubkey: &Pubkey,
    new_max_auction_num: u32,
    pool_seeds: F,
) -> Instruction
where
    F: Fn() -> [&'a [u8]; 1],
{
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);
    let (pool_pubkey, _) = Pubkey::find_program_address(&pool_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(*contract_admin_pubkey, true),
        AccountMeta::new_readonly(contract_bank_pubkey, false),
        AccountMeta::new(pool_pubkey, false),
        AccountMeta::new_readonly(SYS_ID, false),
    ];

    let instruction = AuctionInstruction::ReallocatePool {
        new_max_auction_num,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
