use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Debug)]
pub struct ModifyAuctionArgs {
    pub auction_owner_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub modify_data: ModifyAuctionData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FrontendModifyAuctionArgs {
    pub auction_owner_pubkey: String,
    pub auction_id: String,
    pub description: Option<String>,
    pub socials: Option<Vec<String>>,
    pub encore_period: Option<UnixTimestamp>,
}

impl TryFrom<FrontendModifyAuctionArgs> for ModifyAuctionArgs {
    type Error = String;
    fn try_from(args: FrontendModifyAuctionArgs) -> Result<Self, Self::Error> {
        let new_description = if let Some(desc) = args.description {
            Some(DescriptionString::try_from(desc)?)
        } else {
            None
        };

        let new_socials: Option<SocialsVec> = if let Some(socials) = args.socials {
            let mut vec = Vec::<SocialsString>::with_capacity(socials.len());
            for social in socials.into_iter() {
                vec.push(social.try_into()?);
            }
            Some(vec.try_into()?)
        } else {
            None
        };

        Ok(ModifyAuctionArgs {
            auction_owner_pubkey: Pubkey::from_str(&args.auction_owner_pubkey)
                .map_err(|e| e.to_string())?,
            auction_id: pad_to_32_bytes(&args.auction_id).map_err(|e| e.to_string())?,
            modify_data: ModifyAuctionData {
                new_description,
                new_socials,
                new_encore_period: args.encore_period,
            },
        })
    }
}

pub fn modify_auction(args: &ModifyAuctionArgs) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.auction_owner_pubkey, true),
        AccountMeta::new(auction_root_state_pubkey, false),
    ];

    let instruction = AuctionInstruction::ModifyAuction {
        id: args.auction_id,
        modify_data: args.modify_data.clone(),
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}

#[test]
fn frontend_conversion() {
    let frontend_args = FrontendModifyAuctionArgs {
        auction_owner_pubkey: Pubkey::default().to_string(),
        auction_id: "hello-auction".to_owned(),
        description: Some("This is a description".to_owned()),
        socials: Some(vec![
            "hello.com".to_owned(),
            "bello.dc".to_owned(),
            "yello.tg".to_owned(),
        ]),
        encore_period: None,
    };

    let args = ModifyAuctionArgs::try_from(frontend_args).unwrap();
    assert_eq!(args.auction_owner_pubkey, Pubkey::default());
    assert_eq!(args.auction_id, pad_to_32_bytes("hello-auction").unwrap());
    assert_eq!(
        args.modify_data.new_description.unwrap().contents(),
        "This is a description"
    );
    let new_socials = args.modify_data.new_socials.unwrap();
    assert_eq!(new_socials.contents()[0].contents(), "hello.com");
    assert_eq!(new_socials.contents()[1].contents(), "bello.dc");
    assert_eq!(new_socials.contents()[2].contents(), "yello.tg");
    assert!(args.modify_data.new_encore_period.is_none());
}
