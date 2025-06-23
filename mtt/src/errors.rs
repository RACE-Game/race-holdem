#[macro_export]
macro_rules! custom_err {
    ($fn:ident) => {
        pub fn $fn() -> race_api::prelude::HandleError {
            race_api::prelude::HandleError::Custom(String::from(stringify!($fn)))
        }
    }
}

custom_err!(error_player_not_found);
custom_err!(error_invalid_entry_close_time);
custom_err!(error_invalid_prize_rules);
custom_err!(error_table_not_fonud);
custom_err!(error_empty_blind_rules);
custom_err!(error_player_id_not_found);
custom_err!(error_invalid_bridge_event);
custom_err!(error_invalid_index_usage);
custom_err!(error_custom_event_not_allowed);
custom_err!(error_invalid_table_id);
custom_err!(error_leave_not_allowed);
custom_err!(error_invalid_rake_and_bounty);
custom_err!(error_start_chips_too_low);
