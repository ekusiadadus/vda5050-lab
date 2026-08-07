import { describe, expect, it } from "vitest";

import {
  deriveConnectionStatus,
  parseWarehouseMap,
  parseEventStream,
  projectLsmartPose,
  selectRobotFrame,
  summarizeDiagnosis,
  type SimEvent
} from "../src/model.js";

const snapshot = (
  robotId: string,
  simulationNs: number,
  x: number,
  connected = true
): SimEvent => ({
  schema: "vda5050-lab.live-event/1",
  event_type: "SIM_SNAPSHOT" as const,
  source: "SIMULATOR_TRUTH",
  evidence: false,
  source_sequence: simulationNs,
  snapshot: {
    robot_id: robotId,
    simulation_ns: simulationNs,
    x,
    y: robotId === "demo-001" ? 0 : 2,
    theta: 0,
    linear_velocity: 0.5,
    angular_velocity: 0,
    motion_state: "DRIVING",
    transport_connected: connected,
    connection_epoch: connected ? "session-2" : "session-1"
  }
});

describe("parseEventStream", () => {
  it("parses bounded JSONL and rejects invalid or evidentiary UI events", () => {
    const events = parseEventStream(`${JSON.stringify(snapshot("demo-001", 10, 1))}\n\n`, 4);
    expect(events).toHaveLength(1);
    expect(() => parseEventStream("not-json", 4)).toThrow(/invalid JSONL/i);
    expect(() =>
      parseEventStream(
        JSON.stringify({ ...snapshot("demo-001", 10, 1), evidence: true }),
        4
      )
    ).toThrow(/non-evidentiary/i);
    expect(() =>
      parseEventStream(
        [snapshot("demo-001", 1, 1), snapshot("demo-001", 2, 2)]
          .map((event) => JSON.stringify(event))
          .join("\n"),
        1
      )
    ).toThrow(/event limit/i);
  });
});

describe("selectRobotFrame", () => {
  it("selects the newest snapshot per robot at a deterministic simulation time", () => {
    const events = [
      snapshot("demo-001", 10, 1),
      snapshot("demo-002", 10, 0.5),
      snapshot("demo-001", 20, 2, false),
      snapshot("demo-002", 20, 1.5)
    ];
    const frame = selectRobotFrame(events, 15);
    expect(frame.map(({ robot_id, x }) => [robot_id, x])).toEqual([
      ["demo-001", 1],
      ["demo-002", 0.5]
    ]);
    expect(selectRobotFrame(events, 20)[0]?.transport_connected).toBe(false);
  });
});

describe("diagnostic projections", () => {
  it("keeps simulator status separate from Doctor verdicts", () => {
    const offline = snapshot("demo-001", 20, 2, false).snapshot;
    expect(deriveConnectionStatus(offline)).toEqual({ label: "LINK LOST", tone: "danger" });
    expect(
      deriveConnectionStatus({
        ...offline,
        transport_connected: true,
        connection_epoch: "session-2"
      })
    ).toEqual({
      label: "MQTT SESSION 2",
      tone: "warning"
    });

    const report = {
      findings: [
        {
          rule_id: "LAB-D4-RECONNECT-STATE",
          evaluation: { verdict: "FAIL" },
          next_investigation_target: "MOBILE_ROBOT",
          summary: "ONLINE was not observed after reconnect"
        }
      ]
    };
    expect(summarizeDiagnosis(report)).toEqual({
      ruleId: "LAB-D4-RECONNECT-STATE",
      verdict: "FAIL",
      target: "MOBILE_ROBOT",
      summary: "ONLINE was not observed after reconnect"
    });
    expect(summarizeDiagnosis({ findings: [] }).verdict).toBe("NO D4 FINDING");
  });
});

describe("official LSMART warehouse projection", () => {
  it("validates the pinned grid map and projects ARGoS poses into cells", () => {
    const map = parseWarehouseMap({
      name: "tiny",
      n_row: 3,
      n_col: 4,
      layout: ["....", ".@@.", "w..e"]
    });
    expect(map.layout).toEqual(["....", ".@@.", "w..e"]);
    expect(projectLsmartPose({ x: -2, y: -3 }, map)).toEqual({ row: 2, column: 3 });
  });

  it("rejects malformed maps and clamps small ARGoS integration drift", () => {
    expect(() =>
      parseWarehouseMap({ n_row: 2, n_col: 2, layout: [".."] })
    ).toThrow(/dimensions/i);
    const map = parseWarehouseMap({ n_row: 1, n_col: 2, layout: [".."] });
    expect(projectLsmartPose({ x: 0.1, y: -2.2 }, map)).toEqual({ row: 0, column: 1 });
  });

  it("treats the first LSMART epoch as nominal", () => {
    const firstEpoch = {
      ...snapshot("0", 10, 0).snapshot,
      connection_epoch: "lsmart-session-1"
    };
    expect(deriveConnectionStatus(firstEpoch)).toEqual({ label: "ONLINE", tone: "nominal" });
  });
});
