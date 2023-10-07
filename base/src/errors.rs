#[macro_export]
macro_rules! custom_err {
    ($fn:ident) => {
        pub fn $fn() -> race_api::prelude::HandleError {
            race_api::prelude::HandleError::Custom(String::from(stringify!($fn)))
        }
    }
}

custom_err!(heads_up_missing_sb);
custom_err!(heads_up_missing_bb);
custom_err!(mplayers_missing_sb);
custom_err!(mplayers_missing_bb);
custom_err!(internal_player_not_found);
custom_err!(internal_pot_has_no_owner);
custom_err!(internal_malformed_total_bet);
