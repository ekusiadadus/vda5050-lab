use std::collections::BTreeSet;

use vda5050_demo::{DEMO_ROBOT_COUNTS, FleetLayout, Position, message_budget};

#[test]
fn published_demo_matrix_covers_the_requested_scales() {
    assert_eq!(DEMO_ROBOT_COUNTS, [1, 2, 5, 10, 50, 100]);
}

#[test]
fn layout_rejects_empty_and_oversized_fleets() {
    assert!(FleetLayout::new(0).is_err());
    assert!(FleetLayout::new(101).is_err());
}

#[test]
fn one_hundred_robots_have_unique_vda_identities_and_routes() {
    let layout = FleetLayout::new(100).expect("100 robots is the public maximum");
    let serials = layout
        .robots()
        .iter()
        .map(|robot| robot.serial_number.as_str())
        .collect::<BTreeSet<_>>();
    let topics = layout
        .robots()
        .iter()
        .map(|robot| robot.topic_prefix.as_str())
        .collect::<BTreeSet<_>>();
    let starts = layout
        .robots()
        .iter()
        .map(|robot| robot.start)
        .collect::<BTreeSet<_>>();

    assert_eq!(serials.len(), 100);
    assert_eq!(topics.len(), 100);
    assert_eq!(starts.len(), 100);
}

#[test]
fn xy_layout_is_deterministic_and_uses_both_axes() {
    let layout = FleetLayout::new(100).expect("layout is valid");
    let robots = layout.robots();

    assert_eq!(robots[0].start, Position { x: 0, y: 0 });
    assert_eq!(robots[9].start, Position { x: 0, y: 18 });
    assert_eq!(robots[10].start, Position { x: 6, y: 0 });
    assert_eq!(robots[99].start, Position { x: 54, y: 18 });
    assert_eq!(robots[99].released_end, Position { x: 58, y: 18 });
    assert_eq!(robots[99].horizon_end, Position { x: 58, y: 19 });
}

#[test]
fn hard_message_budget_scales_but_remains_bounded() {
    assert_eq!(message_budget(1).expect("one robot"), 26);
    assert_eq!(message_budget(100).expect("one hundred robots"), 1_016);
    assert!(message_budget(0).is_err());
    assert!(message_budget(101).is_err());
}
