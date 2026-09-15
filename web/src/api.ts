import axios from "axios";
import type { UserData, ControlStatus, PointsBalance, PointRequest, PointTransaction, PointsConfig, User } from "./types";

const api = axios.create({ baseURL: "/api" });

export async function fetchData(): Promise<UserData> {
  const { data } = await api.get<UserData>("/data");
  return data;
}

export async function fetchControlStatus(): Promise<ControlStatus> {
  const { data } = await api.get<ControlStatus>("/control-status");
  return data;
}

export async function fetchVideoWindows(mac: string, date: string) {
  const { data } = await api.get("/video-windows", { params: { mac, date } });
  return data.windows || [];
}

export async function kidAdjust(mac: string, delta: number) {
  const { data } = await api.post("/kid-adjust", { mac, delta });
  return data;
}

export async function switchDevice(mac: string, enabled: boolean) {
  const { data } = await api.post("/switch", { mac, enabled });
  return data;
}

export async function pauseDevice(mac: string, paused: boolean) {
  const { data } = await api.post("/pause", { mac, paused });
  return data;
}

export async function fetchSettings() {
  const { data } = await api.get("/settings");
  return data.settings || {};
}

export async function postSettings(settings: Record<string, string>) {
  const { data } = await api.post("/settings", settings);
  return data;
}

export async function setVacation(enabled: boolean) {
  const { data } = await api.post("/vacation", { enabled });
  return data;
}

// Points API
export async function fetchPointsBalance(): Promise<PointsBalance[]> {
  const { data } = await api.get("/points/balance");
  return data.balances || [];
}

export async function fetchPendingRequests(): Promise<PointRequest[]> {
  const { data } = await api.get("/points/pending");
  return data.requests || [];
}

export async function approveRequest(requestId: number, action: string, note?: string) {
  const { data } = await api.post("/points/approve", { request_id: requestId, action, note });
  return data;
}

export async function exchangePoints(userId: number, points: number) {
  const { data } = await api.post("/points/exchange", { user_id: userId, points });
  return data;
}

export async function fetchMyPoints(userId: number): Promise<{ balance: number; history: PointTransaction[] }> {
  const { data } = await api.get("/points/my", { params: { user_id: userId } });
  return data;
}

export async function applyPoints(userId: number, requestType: string) {
  const { data } = await api.post("/points/apply", { user_id: userId, request_type: requestType });
  return data;
}

export async function fetchPointsConfig(tabletKey?: string): Promise<PointsConfig> {
  const { data } = await api.get("/points/config", { params: tabletKey ? { tablet_key: tabletKey } : {} });
  return data.config || data;
}

export async function fetchUsers(): Promise<User[]> {
  const { data } = await api.get("/points/balance");
  // Extract user info from balance endpoint
  return (data.balances || []).map((b: any) => ({
    id: b.user_id,
    username: b.username,
    display_name: b.display_name,
    role: b.role || b.username, // fallback
  }));
}

export async function fetchRecentTransactions(): Promise<PointTransaction[]> {
  const { data } = await api.get("/points/recent");
  return data.transactions || [];
}
