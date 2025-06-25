use super::*;
use borsh;

#[test]
fn test_unregister_success() -> anyhow::Result<()> {
    let mut mtt = Mtt {
        stage: MttStage::Init,
        ticket: 1000,
        ranks: vec![
            PlayerRank::new(1, 10000, PlayerRankStatus::Play, 0, vec![], 0),
            PlayerRank::new(2, 10000, PlayerRankStatus::Play, 1, vec![], 0),
        ],
        rake: 200,
        bounty: 300,
        total_prize: 1000,
        total_bounty: 600,
        total_rake: 400,
        ..Default::default()
    };

    let mut e = Effect::default();

    mtt.handle_event(&mut e, Event::Custom { sender: 1, raw: borsh::to_vec(&MttEvent::Unregister)? })?;

    assert_eq!(mtt.ranks.len(), 1);
    assert_eq!(mtt.total_prize, 500);
    assert_eq!(mtt.total_bounty, 300);
    assert_eq!(mtt.total_rake, 200);

    Ok(())
}

#[test]
fn test_unregister_given_invalid_sender_expect_error() -> anyhow::Result<()> {
    let mut mtt = Mtt {
        stage: MttStage::Init,
        ticket: 1000,
        ranks: vec![
            PlayerRank::new(1, 10000, PlayerRankStatus::Play, 0, vec![], 0),
            PlayerRank::new(2, 10000, PlayerRankStatus::Play, 1, vec![], 0),
        ],
        rake: 200,
        bounty: 300,
        total_prize: 1000,
        total_bounty: 600,
        total_rake: 400,
        ..Default::default()
    };

    let mut e = Effect::default();

    let r = mtt.handle_event(&mut e, Event::Custom { sender: 3, raw: borsh::to_vec(&MttEvent::Unregister)? });

    assert!(r.is_err());

    Ok(())
}
