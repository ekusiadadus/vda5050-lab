use vda5050_sim_core::{
    MotionLimits, Obstacle, Pose2, Robot, RobotGeometry, RobotMode, Route, SimError, Waypoint,
    World,
};

const DT: f64 = 0.05;

fn route(points: &[(f64, f64)]) -> Route {
    Route::new(
        points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Waypoint::new(format!("n{index}"), x, y))
            .collect(),
    )
    .expect("valid route")
}

fn robot(route: Route) -> Robot {
    robot_named("demo-001", route)
}

fn robot_named(id: &str, route: Route) -> Robot {
    Robot::new(
        id,
        Pose2::new(0.0, 0.0, std::f64::consts::FRAC_PI_2).expect("pose"),
        RobotGeometry::new(0.8, 0.5).expect("geometry"),
        MotionLimits::new(1.0, 1.5, 0.8, 2.0, 0.01, 0.01).expect("limits"),
        route,
    )
    .expect("robot")
}

#[test]
fn world_steps_robots_in_identity_order_and_rejects_duplicates_without_replacement() {
    let mut world = World::new();
    world
        .add_robot(robot_named("robot-b", route(&[(0.0, 0.0), (2.0, 0.0)])))
        .expect("robot b");
    world
        .add_robot(robot_named("robot-a", route(&[(0.0, 0.0), (2.0, 0.0)])))
        .expect("robot a");
    assert!(matches!(
        world.add_robot(robot_named("robot-a", route(&[(0.0, 0.0), (0.0, 2.0)]))),
        Err(SimError::DuplicateRobot(id)) if id == "robot-a"
    ));
    world.add_obstacle(Obstacle::new(10.0, 10.0, 11.0, 11.0).expect("obstacle"));

    let results = world.step(DT).expect("world step");
    assert_eq!(
        results
            .iter()
            .map(|result| result.snapshot.robot_id.as_str())
            .collect::<Vec<_>>(),
        ["robot-a", "robot-b"]
    );
    assert!(results[0].snapshot.pose.y.abs() <= f64::EPSILON);
}

#[test]
fn invalid_numeric_and_physical_inputs_fail_closed() {
    assert!(matches!(
        Pose2::new(f64::NAN, 0.0, 0.0),
        Err(SimError::NonFinite(_))
    ));
    assert!(RobotGeometry::new(0.0, 0.5).is_err());
    assert!(MotionLimits::new(1.0, 1.0, -0.1, 1.0, 0.01, 0.01).is_err());
    assert!(Route::new(vec![Waypoint::new("only", 0.0, 0.0)]).is_err());
}

#[test]
fn robot_rotates_before_translating_and_respects_acceleration_limits() {
    let mut robot = robot(route(&[(0.0, 0.0), (2.0, 0.0)]));
    let first = robot.step(DT, &[]).expect("first step");

    assert!(first.snapshot.pose.x.abs() <= f64::EPSILON);
    assert!(first.snapshot.pose.y.abs() <= f64::EPSILON);
    assert!(first.snapshot.twist.linear.abs() <= f64::EPSILON);
    assert!(first.snapshot.twist.angular.abs() <= 2.0 * DT);

    let mut previous_linear = first.snapshot.twist.linear;
    let mut observed_translation = false;
    for _ in 0..100 {
        let step = robot.step(DT, &[]).expect("step");
        assert!(step.snapshot.twist.linear - previous_linear <= 0.8 * DT + 1e-12);
        if step.snapshot.pose.x > 0.0 {
            observed_translation = true;
            assert!(step.snapshot.pose.theta.abs() <= 0.01);
            break;
        }
        previous_linear = step.snapshot.twist.linear;
    }
    assert!(observed_translation);
}

#[test]
fn fixed_step_run_is_deterministic_and_reaches_each_node_once() {
    let route = route(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
    let mut first = robot(route.clone());
    let mut second = robot(route);
    let mut first_snapshots = Vec::new();
    let mut second_snapshots = Vec::new();
    let mut reached = Vec::new();

    for _ in 0..1_000 {
        let a = first.step(DT, &[]).expect("first run");
        let b = second.step(DT, &[]).expect("second run");
        reached.extend(a.reached_nodes);
        first_snapshots.push(a.snapshot.clone());
        second_snapshots.push(b.snapshot);
        if first.mode() == RobotMode::Arrived {
            break;
        }
    }

    assert_eq!(first_snapshots, second_snapshots);
    assert_eq!(reached, ["n1", "n2"]);
    assert_eq!(first.mode(), RobotMode::Arrived);
    assert!((first.snapshot().pose.x - 1.0).abs() <= f64::EPSILON);
    assert!((first.snapshot().pose.y - 1.0).abs() <= f64::EPSILON);
    assert!(first.snapshot().twist.linear.abs() <= f64::EPSILON);
}

#[test]
fn footprint_collision_stops_without_crossing_the_obstacle() {
    let mut robot = robot(route(&[(0.0, 0.0), (3.0, 0.0)]));
    let obstacle = Obstacle::new(1.0, -0.5, 1.5, 0.5).expect("obstacle");

    for _ in 0..500 {
        robot.step(DT, &[obstacle]).expect("step");
        if robot.mode() == RobotMode::Blocked {
            break;
        }
    }

    assert_eq!(robot.mode(), RobotMode::Blocked);
    assert!(robot.snapshot().pose.x < obstacle.min_x());
    assert!(robot.snapshot().twist.linear.abs() <= f64::EPSILON);
    assert!(robot.snapshot().twist.angular.abs() <= f64::EPSILON);
}
