import {
  deriveConnectionStatus,
  parseWarehouseMap,
  parseEventStream,
  pendingDiagnosis,
  projectLsmartPose,
  selectRobotFrame,
  summarizeDiagnosis,
  type DiagnosisSummary,
  type LiveEvent,
  type SimSnapshot,
  type WarehouseMap,
  type WireEvent
} from "./model.js";

const SVG_NS = "http://www.w3.org/2000/svg";
const pollIntervalMs = 500;
let simulatorEvents: LiveEvent[] = [];
let wireEvents: LiveEvent[] = [];
let followLive = true;
let warehouseMap: WarehouseMap | undefined;
let officialLsmart = false;

const timeline = required<HTMLInputElement>("timeline");
const followButton = required<HTMLButtonElement>("follow-live");

timeline.addEventListener("input", () => {
  followLive = Number(timeline.value) === Number(timeline.max);
  renderFrame(Number(timeline.value));
});
followButton.addEventListener("click", () => {
  followLive = true;
  timeline.value = timeline.max;
  renderFrame(Number(timeline.value));
});

void refresh();
window.setInterval(() => void refresh(), pollIntervalMs);

async function refresh(): Promise<void> {
  const [simText, wireText, passive, evidence, plan, map] = await Promise.all([
    fetchOptionalText("/api/sim-events"),
    fetchOptionalText("/api/wire-events"),
    fetchOptionalJson("/api/report/passive"),
    fetchOptionalJson("/api/report/evidence"),
    fetchOptionalJson("/api/plan"),
    fetchOptionalJson("/api/map")
  ]);
  try {
    if (simText !== undefined) simulatorEvents = parseEventStream(simText);
    if (wireText !== undefined) wireEvents = parseEventStream(wireText);
    officialLsmart = isRecord(plan) && plan.schema === "vda5050-lab.lsmart-run/1";
    if (map !== undefined) warehouseMap = parseWarehouseMap(map);
  } catch (error) {
    setText("run-status", error instanceof Error ? "EVENT RETRY" : "EVENT ERROR");
    return;
  }
  const maxSimulationNs = simulatorEvents.reduce(
    (maximum, event) =>
      event.event_type === "SIM_SNAPSHOT"
        ? Math.max(maximum, event.snapshot.simulation_ns)
        : maximum,
    0
  );
  timeline.max = String(Math.max(maxSimulationNs, 1));
  if (followLive) timeline.value = timeline.max;
  renderFrame(Number(timeline.value));
  renderEvents();
  renderDiagnosis("passive", passive === undefined ? pendingDiagnosis() : summarizeDiagnosis(passive));
  renderDiagnosis("evidence", evidence === undefined ? pendingDiagnosis() : summarizeDiagnosis(evidence));
  setText("wire-count", `${wireEvents.filter(isWireEvent).length} MSG`);
  setText("run-status", evidence === undefined ? "RUNNING" : "DIAGNOSED");
  if (officialLsmart) {
    setText("simulator-title", "10 robots · RHCR warehouse traffic");
    setText("simulator-source", "OFFICIAL LSMART + ARGoS");
    setText("coordinate-label", `${warehouseMap?.name ?? "kiva_large_w_mode"} · ARGoS pose`);
  }
}

function renderFrame(simulationNs: number): void {
  const svg = required<SVGSVGElement>("warehouse");
  svg.replaceChildren();
  if (officialLsmart && warehouseMap !== undefined) {
    svg.setAttribute("viewBox", "0 0 820 740");
    drawLsmartWarehouse(svg, warehouseMap);
    drawLsmartOrders(svg, warehouseMap);
  } else {
    svg.setAttribute("viewBox", "0 0 820 390");
    drawWarehouse(svg);
  }
  for (const snapshot of selectRobotFrame(simulatorEvents, simulationNs)) {
    drawRobot(svg, snapshot, warehouseMap);
  }
  setText("sim-time", `T+${(simulationNs / 1_000_000_000).toFixed(2)}s`);
  setText("cursor-label", followLive ? "LIVE EDGE" : "REPLAY");
}

function drawWarehouse(svg: SVGSVGElement): void {
  for (const [lane, y] of [["LANE A", 120], ["LANE B", 270]] as const) {
    const route = element("line", { x1: "70", y1: String(y), x2: "750", y2: String(y), class: "stroke-line stroke-[8]" });
    const center = element("line", { x1: "70", y1: String(y), x2: "750", y2: String(y), class: "stroke-cyan/40 stroke-1 [stroke-dasharray:8_10]" });
    const label = element("text", { x: "70", y: String(y - 34), class: "fill-dim font-mono text-[12px]" });
    label.textContent = `${lane} · 0—6 m`;
    svg.append(route, center, label);
  }
  const fault = element("line", { x1: String(mapX(2.5)), y1: "70", x2: String(mapX(2.5)), y2: "315", class: "stroke-danger/50 stroke-2 [stroke-dasharray:4_6]" });
  const faultLabel = element("text", { x: String(mapX(2.5) + 8), y: "58", class: "fill-danger font-mono text-[11px]" });
  faultLabel.textContent = "FAULT @ 2.5 m";
  svg.append(fault, faultLabel);
}

function drawRobot(
  svg: SVGSVGElement,
  snapshot: SimSnapshot,
  map: WarehouseMap | undefined
): void {
  const status = deriveConnectionStatus(snapshot);
  const lsmart = officialLsmart && map !== undefined;
  const point = lsmart
    ? lsmartPoint(projectLsmartPose(snapshot, map), map)
    : { x: mapX(snapshot.x), y: snapshot.robot_id === "demo-001" ? 120 : 270 };
  const group = element("g", { transform: `translate(${point.x} ${point.y})` });
  const haloClass = status.tone === "danger" ? "fill-danger/15 stroke-danger" : status.tone === "warning" ? "fill-amber/15 stroke-amber" : "fill-cyan/15 stroke-cyan";
  const radius = lsmart ? 9 : 28;
  group.append(element("circle", { r: String(radius + (lsmart ? 3 : 0)), class: `${haloClass} stroke-2` }));
  group.append(element("circle", { r: String(radius), class: "fill-panel stroke-cloud/70" }));
  const headingLength = lsmart ? 9 : 17;
  group.append(element("line", {
    x1: "0",
    y1: "0",
    x2: String(Math.cos(snapshot.theta) * headingLength),
    y2: String(-Math.sin(snapshot.theta) * headingLength),
    class: "stroke-cloud stroke-2"
  }));
  const id = element("text", { x: "0", y: lsmart ? "-15" : "48", "text-anchor": "middle", class: `fill-cloud font-mono ${lsmart ? "text-[9px]" : "text-[12px]"} font-bold` });
  id.textContent = snapshot.robot_id;
  const state = element("text", { x: "0", y: lsmart ? "26" : "66", "text-anchor": "middle", class: status.tone === "danger" ? "fill-danger font-mono text-[8px]" : status.tone === "warning" ? "fill-amber font-mono text-[8px]" : "fill-cyan font-mono text-[8px]" });
  state.textContent = status.label;
  group.append(id, state);
  svg.append(group);
}

function drawLsmartWarehouse(svg: SVGSVGElement, map: WarehouseMap): void {
  const geometry = lsmartGeometry(map);
  svg.append(element("rect", {
    x: String(geometry.left),
    y: String(geometry.top),
    width: String(geometry.width),
    height: String(geometry.height),
    rx: "8",
    class: "fill-ink stroke-line stroke-2"
  }));
  map.layout.forEach((row, rowIndex) => {
    [...row].forEach((cell, columnIndex) => {
      const point = lsmartPoint({ row: rowIndex, column: columnIndex }, map);
      if (cell === "@" || cell === "T") {
        svg.append(element("rect", {
          x: String(point.x - geometry.cell * 0.42),
          y: String(point.y - geometry.cell * 0.42),
          width: String(geometry.cell * 0.84),
          height: String(geometry.cell * 0.84),
          rx: "2",
          class: "fill-line stroke-cloud/15 stroke-1"
        }));
      } else if (cell === "w") {
        svg.append(element("circle", {
          cx: String(point.x), cy: String(point.y), r: String(geometry.cell * 0.26),
          class: "fill-amber/35 stroke-amber/70 stroke-1"
        }));
      } else if (cell === "e") {
        svg.append(element("circle", {
          cx: String(point.x), cy: String(point.y), r: String(geometry.cell * 0.08),
          class: "fill-cyan/40"
        }));
      }
    });
  });
  const legend = element("text", { x: "40", y: "716", class: "fill-dim font-mono text-[11px]" });
  legend.textContent = "■ shelf   ● workstation   · endpoint   path = MQTT-returned VDA order";
  svg.append(legend);
}

function drawLsmartOrders(svg: SVGSVGElement, map: WarehouseMap): void {
  const latest = new Map<string, WireEvent>();
  for (const event of wireEvents.filter(isWireEvent)) {
    if (!event.topic.endsWith("/order")) continue;
    const serial = serialFrom(event);
    latest.set(serial, event);
  }
  for (const event of latest.values()) {
    const nodes = Array.isArray(event.payload.nodes) ? event.payload.nodes : [];
    const points = nodes.flatMap((node) => {
      if (!isRecord(node) || !isRecord(node.nodePosition)) return [];
      const x = node.nodePosition.x;
      const y = node.nodePosition.y;
      if (typeof x !== "number" || typeof y !== "number") return [];
      const point = lsmartPoint(projectLsmartPose({ x, y }, map), map);
      return [`${point.x},${point.y}`];
    });
    if (points.length > 1) {
      svg.append(element("polyline", {
        points: points.join(" "),
        class: "fill-none stroke-cyan/35 stroke-2 [stroke-dasharray:5_4]"
      }));
    }
  }
}

function lsmartGeometry(map: WarehouseMap): {
  left: number; top: number; width: number; height: number; cell: number;
} {
  const cell = Math.min(740 / map.n_col, 650 / map.n_row);
  const width = cell * map.n_col;
  const height = cell * map.n_row;
  return { left: (820 - width) / 2, top: 28, width, height, cell };
}

function lsmartPoint(position: { row: number; column: number }, map: WarehouseMap): { x: number; y: number } {
  const geometry = lsmartGeometry(map);
  return {
    x: geometry.left + (position.column + 0.5) * geometry.cell,
    y: geometry.top + (position.row + 0.5) * geometry.cell
  };
}

function renderEvents(): void {
  const list = required<HTMLOListElement>("event-list");
  const phases = simulatorEvents.filter((event) => event.event_type === "SCENARIO_PHASE");
  const recentWire = wireEvents.filter(isWireEvent).slice(-10);
  const combined = [...phases, ...recentWire].slice(-18);
  list.replaceChildren(...combined.map(eventRow));
}

function eventRow(event: LiveEvent): HTMLLIElement {
  const item = document.createElement("li");
  item.className = "event-row";
  const dot = document.createElement("span");
  const title = document.createElement("p");
  const detail = document.createElement("p");
  if (event.event_type === "SCENARIO_PHASE") {
    const danger = event.phase === "CONNECTION_BROKEN";
    const warning = event.phase === "RECONNECTED_WITHOUT_ONLINE";
    dot.className = `event-dot ${danger ? "event-dot-danger" : warning ? "event-dot-warn" : "event-dot-ok"}`;
    title.textContent = event.phase.replaceAll("_", " ");
    detail.textContent = "scenario projection · non-evidentiary";
  } else if (event.event_type === "WIRE_OBSERVED") {
    dot.className = `event-dot ${event.topic.endsWith("/connection") ? "event-dot-warn" : ""}`;
    title.textContent = terminalTopic(event.topic);
    detail.textContent = `${serialFrom(event)} · ${event.topic}`;
  } else {
    dot.className = "event-dot";
    title.textContent = event.snapshot.motion_state;
    detail.textContent = event.snapshot.robot_id;
  }
  title.className = "event-title";
  detail.className = "event-detail";
  const copy = document.createElement("div");
  copy.append(title, detail);
  item.append(dot, copy);
  return item;
}

function renderDiagnosis(prefix: "passive" | "evidence", summary: DiagnosisSummary): void {
  setText(`${prefix}-verdict`, summary.verdict);
  setText(`${prefix}-summary`, summary.summary);
  setText(`${prefix}-rule`, summary.ruleId);
  setText(`${prefix}-target`, summary.target);
  const verdict = required<HTMLElement>(`${prefix}-verdict`);
  verdict.className = `verdict ${verdictClass(summary.verdict)}`;
}

function verdictClass(verdict: string): string {
  if (verdict === "FAIL") return "verdict-fail";
  if (verdict === "INCONCLUSIVE") return "verdict-inconclusive";
  if (verdict === "PENDING") return "verdict-pending";
  return "verdict-clear";
}

function element(name: string, attributes: Record<string, string>): SVGElement {
  const node = document.createElementNS(SVG_NS, name);
  for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value);
  return node;
}

function mapX(x: number): number {
  return 70 + (Math.max(0, Math.min(6, x)) / 6) * 680;
}

function terminalTopic(topic: string): string {
  return topic.split("/").at(-1)?.toUpperCase() ?? "MQTT";
}

function serialFrom(event: WireEvent): string {
  return typeof event.payload.serialNumber === "string" ? event.payload.serialNumber : "lab control";
}

function isWireEvent(event: LiveEvent): event is WireEvent {
  return event.event_type === "WIRE_OBSERVED";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function fetchOptionalText(url: string): Promise<string | undefined> {
  const response = await fetch(url, { cache: "no-store" });
  return response.ok ? response.text() : undefined;
}

async function fetchOptionalJson(url: string): Promise<unknown | undefined> {
  const response = await fetch(url, { cache: "no-store" });
  return response.ok ? response.json() : undefined;
}

function required<T extends Element>(id: string): T {
  const node = document.getElementById(id);
  if (node === null) throw new Error(`missing required element: ${id}`);
  return node as unknown as T;
}

function setText(id: string, value: string): void {
  required<HTMLElement>(id).textContent = value;
}
