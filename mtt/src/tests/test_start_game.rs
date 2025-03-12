use super::*;

#[test]
fn test_ready_event_in_init_stage_dispatch_wait_timeout() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    mtt.stage = MttStage::Init;
    mtt.start_time = 100;
    effect.timestamp = 50;

    mtt.handle_event(&mut effect, Event::Ready).unwrap();

    assert!(effect.wait_timeout.is_some());
}

#[test]
fn test_ready_event_in_init_stage_start_time() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    mtt.stage = MttStage::Init;
    mtt.start_time = 100;
    effect.timestamp = 150;
    mtt.handle_event(&mut effect, Event::Ready).unwrap();

    assert!(effect.start_game);
}
