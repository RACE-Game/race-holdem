#[macro_export]
macro_rules! custom_err {
    ($fn:ident) => {
        pub fn $fn() -> race_api::prelude::HandleError {
            race_api::prelude::HandleError::Custom(String::from(stringify!($fn)))
        }
    }
}

custom_err!(internal_invalid_bridge_event);
custom_err!(invalid_bridge_event);
custom_err!(duplicated_player_in_relocate);
custom_err!(duplicated_position_in_relocate);
custom_err!(invalid_player_in_start_game);
