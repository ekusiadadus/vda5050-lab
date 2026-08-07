export interface SimSnapshot {
  robot_id: string;
  simulation_ns: number;
  x: number;
  y: number;
  theta: number;
  linear_velocity: number;
  angular_velocity: number;
  motion_state: string;
  transport_connected: boolean;
  connection_epoch: string;
}

export interface SimEvent {
  schema: string;
  event_type: "SIM_SNAPSHOT";
  source: string;
  evidence: false;
  source_sequence: number;
  snapshot: SimSnapshot;
}

export interface PhaseEvent {
  schema: string;
  event_type: "SCENARIO_PHASE";
  source: string;
  evidence: false;
  source_sequence: number;
  phase: string;
}

export interface WireEvent {
  schema: string;
  event_type: "WIRE_OBSERVED";
  source: string;
  evidence: false;
  source_sequence: number;
  monotonic_ns: number;
  topic: string;
  payload: Record<string, unknown>;
}

export type LiveEvent = SimEvent | PhaseEvent | WireEvent;

export interface ConnectionStatus {
  label: string;
  tone: "nominal" | "warning" | "danger";
}

export interface DiagnosisSummary {
  ruleId: string;
  verdict: string;
  target: string;
  summary: string;
}

export interface WarehouseMap {
  name?: string;
  n_row: number;
  n_col: number;
  layout: string[];
}

export interface GridPosition {
  row: number;
  column: number;
}

export function parseEventStream(input: string, eventLimit = 8_192): LiveEvent[] {
  const lines = input.split(/\r?\n/u).filter((line) => line.trim().length > 0);
  if (lines.length > eventLimit) {
    throw new Error(`event limit exceeded: ${lines.length} > ${eventLimit}`);
  }
  return lines.map((line, index) => {
    let candidate: unknown;
    try {
      candidate = JSON.parse(line);
    } catch {
      throw new Error(`invalid JSONL at line ${index + 1}`);
    }
    if (!isObject(candidate) || candidate.evidence !== false) {
      throw new Error(`line ${index + 1} is not an explicitly non-evidentiary UI event`);
    }
    if (
      candidate.schema !== "vda5050-lab.live-event/1" ||
      !["SIM_SNAPSHOT", "SCENARIO_PHASE", "WIRE_OBSERVED"].includes(
        String(candidate.event_type)
      )
    ) {
      throw new Error(`line ${index + 1} has an unsupported event schema`);
    }
    return candidate as unknown as LiveEvent;
  });
}

export function selectRobotFrame(
  events: readonly LiveEvent[],
  simulationNs: number
): SimSnapshot[] {
  const byRobot = new Map<string, SimSnapshot>();
  for (const event of events) {
    if (event.event_type !== "SIM_SNAPSHOT" || event.snapshot.simulation_ns > simulationNs) {
      continue;
    }
    const current = byRobot.get(event.snapshot.robot_id);
    if (current === undefined || current.simulation_ns <= event.snapshot.simulation_ns) {
      byRobot.set(event.snapshot.robot_id, event.snapshot);
    }
  }
  return [...byRobot.values()].sort((left, right) => left.robot_id.localeCompare(right.robot_id));
}

export function deriveConnectionStatus(snapshot: SimSnapshot): ConnectionStatus {
  if (!snapshot.transport_connected) {
    return { label: "LINK LOST", tone: "danger" };
  }
  if (!snapshot.connection_epoch.endsWith("session-1")) {
    return { label: "MQTT SESSION 2", tone: "warning" };
  }
  return { label: "ONLINE", tone: "nominal" };
}

export function parseWarehouseMap(candidate: unknown): WarehouseMap {
  if (
    !isObject(candidate) ||
    !Number.isInteger(candidate.n_row) ||
    !Number.isInteger(candidate.n_col) ||
    Number(candidate.n_row) <= 0 ||
    Number(candidate.n_col) <= 0 ||
    Number(candidate.n_row) > 256 ||
    Number(candidate.n_col) > 256 ||
    !Array.isArray(candidate.layout)
  ) {
    throw new Error("warehouse map dimensions are invalid");
  }
  const rows = Number(candidate.n_row);
  const columns = Number(candidate.n_col);
  if (
    candidate.layout.length !== rows ||
    !candidate.layout.every(
      (row) => typeof row === "string" && row.length === columns && /^[.@Tew]+$/u.test(row)
    )
  ) {
    throw new Error("warehouse map layout does not match its dimensions");
  }
  return {
    ...(typeof candidate.name === "string" ? { name: candidate.name } : {}),
    n_row: rows,
    n_col: columns,
    layout: [...candidate.layout] as string[]
  };
}

export function projectLsmartPose(
  pose: Pick<SimSnapshot, "x" | "y">,
  map: WarehouseMap
): GridPosition {
  const row = Math.max(0, Math.min(map.n_row - 1, Math.round(-pose.x)));
  const column = Math.max(0, Math.min(map.n_col - 1, Math.round(-pose.y)));
  return { row, column };
}

export function summarizeDiagnosis(report: unknown): DiagnosisSummary {
  if (!isObject(report) || !Array.isArray(report.findings)) {
    return pendingDiagnosis();
  }
  const finding = report.findings.find(
    (entry) => isObject(entry) && entry.rule_id === "LAB-D4-RECONNECT-STATE"
  );
  if (!isObject(finding)) {
    return {
      ruleId: "LAB-D4-RECONNECT-STATE",
      verdict: "NO D4 FINDING",
      target: "—",
      summary: "No reconnect finding was emitted. This is not a conformance PASS."
    };
  }
  const evaluation = isObject(finding.evaluation) ? finding.evaluation : {};
  return {
    ruleId: stringOr(finding.rule_id, "LAB-D4-RECONNECT-STATE"),
    verdict: stringOr(evaluation.verdict, "UNKNOWN"),
    target: stringOr(
      finding.investigation_target ?? finding.next_investigation_target,
      "UNRESOLVED"
    ),
    summary: stringOr(finding.summary, "Diagnosis has no summary.")
  };
}

export function pendingDiagnosis(): DiagnosisSummary {
  return {
    ruleId: "LAB-D4-RECONNECT-STATE",
    verdict: "PENDING",
    target: "—",
    summary: "Waiting for the recorder to seal the trace and Doctor to analyze it."
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringOr(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}
