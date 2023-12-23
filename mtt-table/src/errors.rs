#[macro_export]
macro_rules! custom_err {
    ($fn:ident) => {
        pub fn $fn() -> race_api::prelude::HandleError {
            race_api::prelude::HandleError::Custom(String::from(stringify!($fn)))
        }
    }
}

custom_err!(internal_player_position_missing);
custom_err!(internal_player_addr_missing);
custom_err!(internal_invalid_bridge_event);
