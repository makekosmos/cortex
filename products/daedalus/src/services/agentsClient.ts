import { createArkAgentsApi, type AgentsEvent } from "@kosmos/ark/agents";

const bridge = window.kepler;
if (!bridge) throw new Error("Daedalus должен быть запущен внутри Kosmos");

export const agentsClient = createArkAgentsApi(
  <T>(operation: string, params?: Record<string, unknown>) =>
    bridge.ark.request<T>(operation, params),
  (listener) => bridge.ark.subscribe("agents_event", (payload) => listener(payload as AgentsEvent)),
);
