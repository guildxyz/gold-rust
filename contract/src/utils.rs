use agsol_token_metadata::instruction::CreateMetadataAccountArgs;
use agsol_token_metadata::state::Data as MetadataStateData;

// NOTE special characters are chopped off to fit an u8, so it won't be
// correct, however, we may assume in this case that the input is valid.
// Else, we will throw an error when the auction with this id is not found
pub fn pad_to_32_bytes(input: &str) -> Result<[u8; 32], &'static str> {
    if input.len() > 32 {
        return Err("input is longer than 32 bytes");
    }
    let mut array = [0_u8; 32];
    for (i, c) in input.chars().enumerate() {
        array[i] = c as u8;
    }
    Ok(array)
}

/// We are assuming that there are no valid auction names with random `\u{0}`
/// characters in them.
pub fn unpad_id(id: &[u8; 32]) -> String {
    let mut unpadded = String::from_utf8_lossy(id).to_string();
    unpadded.retain(|c| c != '\u{0}');
    unpadded
}

pub fn initialize_create_metadata_args(
    metadata_args: &mut CreateMetadataAccountArgs,
    is_repeating: bool,
) {
    if is_repeating {
        metadata_args.data.uri.push_str("/0.json");
    } else {
        metadata_args.data.uri.push_str("/1.json");
    }
}

pub fn unpuff_metadata(metadata_state_data: &mut MetadataStateData) {
    metadata_state_data.name.retain(|c| c != '\u{0}');
    metadata_state_data.uri.retain(|c| c != '\u{0}');
    metadata_state_data.symbol.retain(|c| c != '\u{0}');
}

#[cfg(test)]
mod initialize_auction_tests {
    use super::*;

    fn get_test_args() -> CreateMetadataAccountArgs {
        CreateMetadataAccountArgs {
            data: agsol_token_metadata::state::Data {
                name: "random auction".to_owned(),
                symbol: "RAND".to_owned(),
                uri: "uri".to_owned(),
                seller_fee_basis_points: 10,
                creators: None,
            },
            is_mutable: true,
        }
    }

    #[test]
    fn test_initialize_metadata_args_valid() {
        let mut test_args_not_repeating = get_test_args();
        initialize_create_metadata_args(&mut test_args_not_repeating, false);
        assert_eq!("uri/1.json", test_args_not_repeating.data.uri);

        let mut test_args_repeating = get_test_args();
        initialize_create_metadata_args(&mut test_args_repeating, true);
        assert_eq!("uri/0.json", test_args_repeating.data.uri);

        let mut longer_uri_args = get_test_args();
        longer_uri_args.data.uri = "something/with/long/path".to_owned();
        initialize_create_metadata_args(&mut longer_uri_args, true);
        assert_eq!("something/with/long/path/0.json", longer_uri_args.data.uri);
    }

    #[test]
    fn str_padding() {
        assert_eq!(
            pad_to_32_bytes("this is definitely longer than 32 bytes")
                .err()
                .unwrap()
                .to_string(),
            "input is longer than 32 bytes"
        );
        assert_eq!(
            pad_to_32_bytes("this-is-fine").unwrap(),
            [
                0x74, 0x68, 0x69, 0x73, 0x2d, 0x69, 0x73, 0x2d, 0x66, 0x69, 0x6e, 0x65, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
        assert_eq!(
            pad_to_32_bytes("hélló").unwrap(),
            [
                0x68, 0xe9, 0x6c, 0x6c, 0xf3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn unpadding() {
        let mut auction_id = [0_u8; 32];
        let unpadded = unpad_id(&auction_id);
        assert!(unpadded.is_empty());
        auction_id[0] = 0x68;
        let unpadded = unpad_id(&auction_id);
        assert_eq!(unpadded, "h");

        let slugified = "this-is-fine";
        let auction_id = pad_to_32_bytes(slugified).unwrap();
        let unpadded = unpad_id(&auction_id);
        assert_eq!(unpadded, slugified);

        let slugified = "hi-this-is-exactly-32-bytes-long";
        let auction_id = pad_to_32_bytes(slugified).unwrap();
        let unpadded = unpad_id(&auction_id);
        assert_eq!(unpadded, slugified);

        let slugified = "Höh";
        let auction_id = pad_to_32_bytes(slugified).unwrap();
        let unpadded = unpad_id(&auction_id);
        assert_eq!(unpadded, "H�h");
    }
}
