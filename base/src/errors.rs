#[macro_export]
macro_rules! custom_err {
    ($fn:ident) => {
        pub fn $fn() -> race_api::prelude::HandleError {
            race_api::prelude::HandleError::Custom(String::from(stringify!($fn)))
        }
    }
}

custom_err!(internal_player_not_found);
custom_err!(internal_pot_has_no_owner);
custom_err!(internal_malformed_total_bet);
custom_err!(internal_cannot_find_action_player);
custom_err!(internal_player_not_in_game_but_assigned_cards);
custom_err!(internal_failed_to_reveal_board);
custom_err!(heads_up_missing_sb);
custom_err!(heads_up_missing_bb);
custom_err!(mplayers_missing_sb);
custom_err!(mplayers_missing_bb);
custom_err!(not_the_acting_player_to_bet);
custom_err!(not_the_acting_player_to_raise);
custom_err!(not_the_acting_player_to_check);
custom_err!(not_the_acting_player_to_fold);
custom_err!(not_the_acting_player_to_call);
