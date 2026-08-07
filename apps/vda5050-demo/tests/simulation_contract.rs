use vda5050_demo::{Position, simulate_robot_route};
use vda5050_sim_core::RobotMode;

#[test]
fn demo_route_is_generated_by_a_continuous_fixed_step_controller() {
    let snapshots =
        simulate_robot_route(Position { x: 0, y: 0 }, Position { x: 4, y: 0 }).expect("simulation");

    assert!(snapshots.len() > 80);
    assert_eq!(snapshots.first().expect("first").tick, 0);
    assert_eq!(snapshots.last().expect("last").mode, RobotMode::Arrived);
    assert!((snapshots.last().expect("last").pose.x - 4.0).abs() <= f64::EPSILON);
    assert!(snapshots.last().expect("last").pose.y.abs() <= f64::EPSILON);
    assert!(
        snapshots
            .windows(2)
            .all(|window| window[1].tick > window[0].tick)
    );
    assert!(snapshots.iter().any(|snapshot| {
        snapshot.pose.x > 0.0 && snapshot.pose.x < 1.0 && snapshot.pose.x.fract() != 0.0
    }));
}

#[test]
fn demo_route_sampling_returns_four_ordered_wire_positions() {
    let snapshots = simulate_robot_route(Position { x: 6, y: 2 }, Position { x: 10, y: 2 })
        .expect("simulation");
    let samples = vda5050_demo::sample_wire_positions(&snapshots).expect("samples");

    assert_eq!(samples.len(), 4);
    assert!(samples.windows(2).all(|window| window[0].x < window[1].x));
    assert_eq!(samples.last(), Some(&Position { x: 10, y: 2 }));
}
