export interface Device {
  mac: string;
  name: string;
  ip: string;
  online: boolean;
  daily_active_min: number;
  daily_video_min: number;
  daily_game_min: number;
  limit_sec: number;
  usage_sec: number;
}

export interface VideoWindow {
  id: number;
  ts: string;
  date: string;
  mac: string;
  activity_type: string;
}

export interface UserData {
  devices: Device[];
  config: Record<string, string>;
  video_windows: VideoWindow[];
}

export interface ControlStatus {
  devices: ControlDevice[];
}

export interface ControlDevice {
  mac: string;
  name: string;
  ip: string;
  online: boolean;
  blocked: boolean;
  paused: boolean;
  switch_enabled: boolean | null;
  usage_sec: number;
  limit_sec: number;
}

export interface PointsBalance {
  user_id: number;
  username: string;
  display_name: string;
  balance: number;
}

export interface PointRequest {
  id: number;
  user_id: number;
  request_type: string;
  points: number;
  status: string;
  admin_note: string | null;
  created_at: string;
}

export interface PointTransaction {
  id: number;
  user_id: number;
  tx_type: string;
  points: number;
  balance_after: number;
  description: string;
  created_at: string;
}

export interface PointsConfig {
  tutoring: number;
  homework: number;
  other: number;
}

export interface User {
  id: number;
  username: string;
  display_name: string;
  role: string;
}

export interface GameIpEntry {
  addr: string;
  source: string;
  label: string | null;
  first_seen: string | null;
}

export interface GameIpListResponse {
  domains: string[];
  ips: GameIpEntry[];
  total: number;
}

export interface RecentMinute {
  time: string;
  status: string;
  type: string;
}

export interface RecentDeviceActivity {
  mac: string;
  name: string;
  minutes: RecentMinute[];
}

export interface OpResponse {
  ok: boolean;
  message: string;
}
