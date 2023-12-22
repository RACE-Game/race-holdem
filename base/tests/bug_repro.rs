use borsh::BorshDeserialize;
use race_api::prelude::{Effect, Event, GameHandler};
use race_holdem_base::game::Holdem;

#[test]
fn repro_0() -> anyhow::Result<()> {
    let effect_bs: Vec<u8> = vec![
        0, 0, 0, 0, 0, 231, 112, 20, 146, 140, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
        0, 0, 9, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
        1, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 107, 5, 0, 0,
        0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 106, 6, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 115, 106, 7, 0, 0,
        0, 0, 0, 0, 0, 2, 0, 0, 0, 100, 116, 18, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 99, 55, 19, 0, 0,
        0, 0, 0, 0, 0, 2, 0, 0, 0, 100, 53, 20, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 116, 21, 0,
        0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 56, 22, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 104, 57, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 1, 12, 14, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 168, 97, 0, 0, 0, 0, 0, 0,
        80, 195, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 5, 0,
        0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 2, 0, 0, 0, 99, 55, 2, 0, 0, 0, 100, 53, 2, 0, 0, 0, 104,
        116, 2, 0, 0, 0, 104, 56, 2, 0, 0, 0, 104, 57, 8, 0, 0, 0, 44, 0, 0, 0, 50, 76, 118, 67,
        97, 111, 119, 50, 71, 89, 119, 80, 54, 70, 104, 106, 49, 117, 101, 78, 87, 119, 69, 84, 72,
        71, 119, 88, 122, 121, 74, 90, 55, 107, 82, 100, 112, 55, 83, 72, 98, 88, 68, 113, 2, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 51, 111, 54, 88, 84, 87,
        52, 111, 89, 84, 66, 70, 101, 70, 90, 117, 122, 119, 56, 51, 51, 122, 105, 106, 70, 56, 89,
        114, 107, 75, 74, 114, 118, 122, 68, 97, 82, 105, 109, 87, 99, 76, 57, 120, 2, 0, 0, 0, 2,
        0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53,
        112, 115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74,
        98, 109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 2, 0, 0, 0, 4, 0, 0,
        0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54,
        56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98,
        106, 101, 52, 89, 86, 66, 120, 74, 121, 110, 71, 65, 110, 85, 2, 0, 0, 0, 6, 0, 0, 0, 0, 0,
        0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121,
        87, 84, 106, 89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111,
        80, 82, 51, 53, 115, 49, 53, 117, 87, 67, 65, 102, 2, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 9,
        0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 68, 102, 103, 116, 65, 67, 86, 57, 86, 122, 82, 85, 75,
        85, 113, 82, 106, 117, 122, 120, 82, 72, 109, 101, 121, 99, 121, 111, 109, 99, 67, 122,
        102, 82, 72, 103, 86, 121, 68, 80, 104, 97, 57, 70, 2, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0,
        11, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 71, 77, 70, 50, 74, 115, 97, 99, 81, 112, 102, 102,
        66, 90, 66, 115, 57, 113, 76, 117, 55, 104, 82, 90, 55, 89, 119, 121, 112, 100, 71, 68,
        106, 65, 109, 86, 77, 77, 89, 75, 103, 98, 72, 118, 2, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0,
        13, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 72, 113, 74, 120, 105, 114, 98, 116, 103, 117, 88,
        111, 119, 109, 115, 82, 122, 75, 71, 83, 78, 57, 112, 72, 104, 88, 99, 65, 117, 119, 83,
        50, 84, 110, 82, 120, 90, 82, 117, 68, 114, 76, 107, 67, 2, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0,
        0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49,
        75, 53, 112, 115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85,
        102, 74, 98, 109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 240, 73, 2,
        0, 0, 0, 0, 0, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77,
        72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66,
        120, 74, 121, 110, 71, 65, 110, 85, 240, 73, 2, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 55, 77,
        121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107, 104, 55,
        74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67, 65, 102,
        80, 195, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 71, 77, 70, 50, 74, 115, 97, 99, 81, 112, 102, 102,
        66, 90, 66, 115, 57, 113, 76, 117, 55, 104, 82, 90, 55, 89, 119, 121, 112, 100, 71, 68,
        106, 65, 109, 86, 77, 77, 89, 75, 103, 98, 72, 118, 168, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        9, 0, 0, 0, 44, 0, 0, 0, 50, 76, 118, 67, 97, 111, 119, 50, 71, 89, 119, 80, 54, 70, 104,
        106, 49, 117, 101, 78, 87, 119, 69, 84, 72, 71, 119, 88, 122, 121, 74, 90, 55, 107, 82,
        100, 112, 55, 83, 72, 98, 88, 68, 113, 44, 0, 0, 0, 50, 76, 118, 67, 97, 111, 119, 50, 71,
        89, 119, 80, 54, 70, 104, 106, 49, 117, 101, 78, 87, 119, 69, 84, 72, 71, 119, 88, 122,
        121, 74, 90, 55, 107, 82, 100, 112, 55, 83, 72, 98, 88, 68, 113, 160, 2, 148, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 1, 44, 0, 0, 0, 51, 111, 54, 88, 84, 87, 52, 111, 89, 84, 66,
        70, 101, 70, 90, 117, 122, 119, 56, 51, 51, 122, 105, 106, 70, 56, 89, 114, 107, 75, 74,
        114, 118, 122, 68, 97, 82, 105, 109, 87, 99, 76, 57, 120, 44, 0, 0, 0, 51, 111, 54, 88, 84,
        87, 52, 111, 89, 84, 66, 70, 101, 70, 90, 117, 122, 119, 56, 51, 51, 122, 105, 106, 70, 56,
        89, 114, 107, 75, 74, 114, 118, 122, 68, 97, 82, 105, 109, 87, 99, 76, 57, 120, 192, 137,
        70, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 4, 0, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75,
        53, 112, 115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102,
        74, 98, 109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 44, 0, 0, 0, 53,
        56, 87, 90, 84, 49, 75, 53, 112, 115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65,
        105, 77, 69, 85, 102, 74, 98, 109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69,
        105, 240, 197, 148, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 44, 0, 0, 0, 53, 82, 99,
        102, 89, 68, 51, 118, 102, 117, 112, 121, 82, 54, 80, 119, 118, 107, 80, 54, 112, 82, 55,
        51, 67, 109, 74, 103, 89, 83, 118, 77, 78, 100, 66, 85, 117, 52, 117, 74, 116, 84, 65, 119,
        44, 0, 0, 0, 53, 82, 99, 102, 89, 68, 51, 118, 102, 117, 112, 121, 82, 54, 80, 119, 118,
        107, 80, 54, 112, 82, 55, 51, 67, 109, 74, 103, 89, 83, 118, 77, 78, 100, 66, 85, 117, 52,
        117, 74, 116, 84, 65, 119, 128, 150, 152, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 6, 0, 44,
        0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56, 104, 122,
        76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120, 74, 121,
        110, 71, 65, 110, 85, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68,
        76, 77, 72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89,
        86, 66, 120, 74, 121, 110, 71, 65, 110, 85, 240, 197, 148, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0,
        0, 0, 1, 0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89,
        105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53,
        115, 49, 53, 117, 87, 67, 65, 102, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87,
        121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70,
        111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67, 65, 102, 48, 211, 151, 0, 0, 0, 0, 0, 6, 0,
        0, 0, 0, 0, 0, 0, 4, 0, 44, 0, 0, 0, 68, 102, 103, 116, 65, 67, 86, 57, 86, 122, 82, 85,
        75, 85, 113, 82, 106, 117, 122, 120, 82, 72, 109, 101, 121, 99, 121, 111, 109, 99, 67, 122,
        102, 82, 72, 103, 86, 121, 68, 80, 104, 97, 57, 70, 44, 0, 0, 0, 68, 102, 103, 116, 65, 67,
        86, 57, 86, 122, 82, 85, 75, 85, 113, 82, 106, 117, 122, 120, 82, 72, 109, 101, 121, 99,
        121, 111, 109, 99, 67, 122, 102, 82, 72, 103, 86, 121, 68, 80, 104, 97, 57, 70, 64, 137,
        149, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 4, 0, 44, 0, 0, 0, 71, 77, 70, 50, 74, 115, 97,
        99, 81, 112, 102, 102, 66, 90, 66, 115, 57, 113, 76, 117, 55, 104, 82, 90, 55, 89, 119,
        121, 112, 100, 71, 68, 106, 65, 109, 86, 77, 77, 89, 75, 103, 98, 72, 118, 44, 0, 0, 0, 71,
        77, 70, 50, 74, 115, 97, 99, 81, 112, 102, 102, 66, 90, 66, 115, 57, 113, 76, 117, 55, 104,
        82, 90, 55, 89, 119, 121, 112, 100, 71, 68, 106, 65, 109, 86, 77, 77, 89, 75, 103, 98, 72,
        118, 136, 113, 151, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 44, 0, 0, 0, 72, 113, 74,
        120, 105, 114, 98, 116, 103, 117, 88, 111, 119, 109, 115, 82, 122, 75, 71, 83, 78, 57, 112,
        72, 104, 88, 99, 65, 117, 119, 83, 50, 84, 110, 82, 120, 90, 82, 117, 68, 114, 76, 107, 67,
        44, 0, 0, 0, 72, 113, 74, 120, 105, 114, 98, 116, 103, 117, 88, 111, 119, 109, 115, 82,
        122, 75, 71, 83, 78, 57, 112, 72, 104, 88, 99, 65, 117, 119, 83, 50, 84, 110, 82, 120, 90,
        82, 117, 68, 114, 76, 107, 67, 112, 171, 142, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 4, 0,
        9, 0, 0, 0, 44, 0, 0, 0, 71, 77, 70, 50, 74, 115, 97, 99, 81, 112, 102, 102, 66, 90, 66,
        115, 57, 113, 76, 117, 55, 104, 82, 90, 55, 89, 119, 121, 112, 100, 71, 68, 106, 65, 109,
        86, 77, 77, 89, 75, 103, 98, 72, 118, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54,
        56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98,
        106, 101, 52, 89, 86, 66, 120, 74, 121, 110, 71, 65, 110, 85, 44, 0, 0, 0, 72, 113, 74,
        120, 105, 114, 98, 116, 103, 117, 88, 111, 119, 109, 115, 82, 122, 75, 71, 83, 78, 57, 112,
        72, 104, 88, 99, 65, 117, 119, 83, 50, 84, 110, 82, 120, 90, 82, 117, 68, 114, 76, 107, 67,
        44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69,
        120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53,
        117, 87, 67, 65, 102, 44, 0, 0, 0, 68, 102, 103, 116, 65, 67, 86, 57, 86, 122, 82, 85, 75,
        85, 113, 82, 106, 117, 122, 120, 82, 72, 109, 101, 121, 99, 121, 111, 109, 99, 67, 122,
        102, 82, 72, 103, 86, 121, 68, 80, 104, 97, 57, 70, 44, 0, 0, 0, 53, 82, 99, 102, 89, 68,
        51, 118, 102, 117, 112, 121, 82, 54, 80, 119, 118, 107, 80, 54, 112, 82, 55, 51, 67, 109,
        74, 103, 89, 83, 118, 77, 78, 100, 66, 85, 117, 52, 117, 74, 116, 84, 65, 119, 44, 0, 0, 0,
        50, 76, 118, 67, 97, 111, 119, 50, 71, 89, 119, 80, 54, 70, 104, 106, 49, 117, 101, 78, 87,
        119, 69, 84, 72, 71, 119, 88, 122, 121, 74, 90, 55, 107, 82, 100, 112, 55, 83, 72, 98, 88,
        68, 113, 44, 0, 0, 0, 51, 111, 54, 88, 84, 87, 52, 111, 89, 84, 66, 70, 101, 70, 90, 117,
        122, 119, 56, 51, 51, 122, 105, 106, 70, 56, 89, 114, 107, 75, 74, 114, 118, 122, 68, 97,
        82, 105, 109, 87, 99, 76, 57, 120, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112, 115,
        109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98, 109,
        81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 1, 0, 0, 0, 2, 0, 0, 0, 44, 0,
        0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112, 115, 109, 69, 105, 72, 104, 116, 122, 120, 88,
        83, 83, 65, 105, 77, 69, 85, 102, 74, 98, 109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78,
        71, 117, 69, 105, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76,
        77, 72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86,
        66, 120, 74, 121, 110, 71, 65, 110, 85, 0, 0, 0, 0, 216, 184, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 2, 2, 0, 0, 0, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112, 115, 109,
        69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98, 109, 81,
        122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 80, 195, 0, 0, 0, 0, 0, 0, 44, 0,
        0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76,
        85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120, 74, 121, 110,
        71, 65, 110, 85, 80, 195, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 5, 0, 0, 0, 2, 0,
        0, 0, 99, 55, 2, 0, 0, 0, 100, 53, 2, 0, 0, 0, 104, 116, 2, 0, 0, 0, 104, 56, 2, 0, 0, 0,
        104, 57, 2, 0, 0, 0, 44, 0, 0, 0, 71, 77, 70, 50, 74, 115, 97, 99, 81, 112, 102, 102, 66,
        90, 66, 115, 57, 113, 76, 117, 55, 104, 82, 90, 55, 89, 119, 121, 112, 100, 71, 68, 106,
        65, 109, 86, 77, 77, 89, 75, 103, 98, 72, 118, 0, 168, 97, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0,
        57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76, 85,
        77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120, 74, 121, 110, 71,
        65, 110, 85, 1, 80, 195, 0, 0, 0, 0, 0, 0, 248, 36, 1, 0, 0, 0, 0, 0, 8, 0, 0, 0, 44, 0, 0,
        0, 72, 113, 74, 120, 105, 114, 98, 116, 103, 117, 88, 111, 119, 109, 115, 82, 122, 75, 71,
        83, 78, 57, 112, 72, 104, 88, 99, 65, 117, 119, 83, 50, 84, 110, 82, 120, 90, 82, 117, 68,
        114, 76, 107, 67, 3, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84,
        106, 89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82,
        51, 53, 115, 49, 53, 117, 87, 67, 65, 102, 2, 44, 0, 0, 0, 68, 102, 103, 116, 65, 67, 86,
        57, 86, 122, 82, 85, 75, 85, 113, 82, 106, 117, 122, 120, 82, 72, 109, 101, 121, 99, 121,
        111, 109, 99, 67, 122, 102, 82, 72, 103, 86, 121, 68, 80, 104, 97, 57, 70, 3, 44, 0, 0, 0,
        50, 76, 118, 67, 97, 111, 119, 50, 71, 89, 119, 80, 54, 70, 104, 106, 49, 117, 101, 78, 87,
        119, 69, 84, 72, 71, 119, 88, 122, 121, 74, 90, 55, 107, 82, 100, 112, 55, 83, 72, 98, 88,
        68, 113, 3, 44, 0, 0, 0, 51, 111, 54, 88, 84, 87, 52, 111, 89, 84, 66, 70, 101, 70, 90,
        117, 122, 119, 56, 51, 51, 122, 105, 106, 70, 56, 89, 114, 107, 75, 74, 114, 118, 122, 68,
        97, 82, 105, 109, 87, 99, 76, 57, 120, 3, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112,
        115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98,
        109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 2, 44, 0, 0, 0, 71, 77,
        70, 50, 74, 115, 97, 99, 81, 112, 102, 102, 66, 90, 66, 115, 57, 113, 76, 117, 55, 104, 82,
        90, 55, 89, 119, 121, 112, 100, 71, 68, 106, 65, 109, 86, 77, 77, 89, 75, 103, 98, 72, 118,
        3, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56,
        104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120,
        74, 121, 110, 71, 65, 110, 85, 1, 152, 171, 2, 0, 0, 0, 0, 0, 3, 0, 0, 0, 44, 0, 0, 0, 57,
        71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76, 85, 77,
        81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120, 74, 121, 110, 71, 65,
        110, 85, 1, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89,
        105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53,
        115, 49, 53, 117, 87, 67, 65, 102, 1, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112,
        115, 109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98,
        109, 81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 1, 152, 171, 2, 0, 0, 0,
        0, 0, 3, 0, 0, 0, 44, 0, 0, 0, 57, 71, 104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76,
        77, 72, 56, 104, 122, 76, 85, 77, 81, 84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86,
        66, 120, 74, 121, 110, 71, 65, 110, 85, 0, 80, 195, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 55,
        77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107, 104,
        55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67, 65,
        102, 3, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112, 115, 109, 69, 105, 72, 104, 116,
        122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98, 109, 81, 122, 82, 51, 80, 104, 88,
        81, 70, 78, 71, 117, 69, 105, 2, 56, 50, 4, 0, 0, 0, 0, 0, 2, 0, 0, 0, 44, 0, 0, 0, 57, 71,
        104, 65, 54, 72, 98, 113, 54, 56, 116, 103, 68, 76, 77, 72, 56, 104, 122, 76, 85, 77, 81,
        84, 106, 109, 90, 115, 117, 98, 106, 101, 52, 89, 86, 66, 120, 74, 121, 110, 71, 65, 110,
        85, 0, 80, 195, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 53, 56, 87, 90, 84, 49, 75, 53, 112, 115,
        109, 69, 105, 72, 104, 116, 122, 120, 88, 83, 83, 65, 105, 77, 69, 85, 102, 74, 98, 109,
        81, 122, 82, 51, 80, 104, 88, 81, 70, 78, 71, 117, 69, 105, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        1, 0, 0, 0, 0,
    ];
    let mut effect = Effect::try_from_slice(&effect_bs)?;
    let event_bs: Vec<u8> = vec![16, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    let event = Event::try_from_slice(&event_bs)?;

    println!("Effect: {:?}", effect);
    println!("Event: {:?}", event);

    let mut handler: Holdem = effect.__handler_state::<Holdem>();
    let r = handler.handle_event(&mut effect, event);

    println!("{:?}", r);

    Ok(())
}