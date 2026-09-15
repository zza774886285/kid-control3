import { useEffect, useState } from "react";
import { fetchData, fetchControlStatus, kidAdjust, switchDevice, pauseDevice } from "../api";
import type { Device, ControlDevice } from "../types";

const DEVICE_COLORS: Record<string, string> = {
  "华为": "#FF6B35",
  "iPad": "#4ECDC4",
};

function getDeviceColor(name: string): string {
  for (const [key, color] of Object.entries(DEVICE_COLORS)) {
    if (name.includes(key)) return color;
  }
  return "#8b5cf6";
}

function formatTime(sec: number): string {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}min`;
}

interface PillButtonProps {
  onClick: () => void;
  color?: string;
  active?: boolean;
  children: React.ReactNode;
}

function PillButton({ onClick, color, active, children }: PillButtonProps) {
  const baseStyle = "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-all duration-200 cursor-pointer border";
  const activeStyle = active
    ? "bg-white/10 border-white/20 text-white"
    : "bg-white/[0.03] border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-white/[0.08] hover:border-white/10";
  const accentStyle = color
    ? `bg-[${color}]/15 border-[${color}]/30 text-[${color}] hover:bg-[${color}]/25`
    : "";

  return (
    <button
      onClick={onClick}
      className={`${baseStyle} ${color ? accentStyle : activeStyle}`}
      style={color ? { backgroundColor: `${color}20`, borderColor: `${color}50`, color } : undefined}
    >
      {children}
    </button>
  );
}

export default function Dashboard() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [controls, setControls] = useState<ControlDevice[]>([]);
  const [dayType, setDayType] = useState("");
  const [loading, setLoading] = useState(true);

  async function reload() {
    setLoading(true);
    try {
      const [data, ctrl] = await Promise.all([fetchData(), fetchControlStatus()]);
      setDevices(data.devices || []);
      setControls(ctrl.devices || []);
      setDayType(ctrl.day_type || "");
    } catch (e) {
      console.error("加载失败:", e);
    }
    setLoading(false);
  }

  useEffect(() => { reload(); }, []);

  // 每30秒自动刷新首页
  useEffect(() => {
    const timer = setInterval(() => {
      setLoading(true);
      Promise.all([fetchData(), fetchControlStatus()])
        .then(([data, ctrl]) => {
          setDevices(data.devices || []);
          setControls(ctrl.devices || []);
          setDayType(ctrl.day_type || "");
        })
        .catch(() => {})
        .finally(() => setLoading(false));
    }, 30000);
    return () => clearInterval(timer);
  }, []);

  function getCtrl(mac: string): ControlDevice | undefined {
    return controls.find((c) => c.mac === mac);
  }

  async function handleAdjust(mac: string, delta: number) {
    await kidAdjust(mac, delta);
    reload();
  }

  async function handleSwitch(mac: string, enabled: boolean) {
    await switchDevice(mac, enabled);
    reload();
  }

  async function handlePause(mac: string, paused: boolean) {
    await pauseDevice(mac, paused);
    reload();
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="flex items-center gap-3 text-[color:var(--color-text-muted)]">
          <div className="w-5 h-5 border-2 border-white/20 border-t-white/60 rounded-full animate-spin" />
          <span>加载中...</span>
        </div>
      </div>
    );
  }

  const dayLabel = dayType === "workday" ? "工作日" : dayType === "weekend" ? "周末" : "假期";

  return (
    <div className="space-y-6">
      {/* 标题栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-xl font-bold tracking-tight">仪表盘</h2>
          <span className="text-xs px-2.5 py-0.5 rounded-full bg-white/[0.06] text-[color:var(--color-text-muted)] border border-white/[0.06]">
            {dayLabel}
          </span>
        </div>
        <button
          onClick={reload}
          className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-full bg-white/[0.05] border border-white/[0.06] text-[color:var(--color-text-muted)] hover:text-[color:var(--color-text-secondary)] hover:bg-white/[0.08] transition-all duration-200 cursor-pointer"
        >
          🔄 刷新
        </button>
      </div>

      {/* 设备卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {devices.map((d) => {
          const ctrl = getCtrl(d.mac);
          const usageSec = ctrl?.usage_sec ?? d.usage_sec ?? 0;
          const limitSec = ctrl?.limit_sec ?? d.limit_sec ?? 0;
          const usageMin = Math.round(usageSec / 60);
          const limitMin = Math.round(limitSec / 60);
          const pct = limitMin > 0 ? Math.min(100, Math.round((usageMin / limitMin) * 100)) : 0;
          const accent = getDeviceColor(d.name);

          const progressColor = pct > 80 ? "#ef4444" : pct > 50 ? "#eab308" : accent;

          return (
            <div
              key={d.mac}
              className="relative rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] hover:bg-white/[0.06] transition-all duration-300"
            >
              {/* 顶部发光条 */}
              <div
                className="absolute top-0 left-6 right-6 h-[1px] rounded-full"
                style={{ background: `linear-gradient(90deg, transparent, ${accent}40, transparent)` }}
              />

              {/* 设备名 + 在线状态 */}
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-2.5">
                  <div
                    className="w-2.5 h-2.5 rounded-full"
                    style={{
                      backgroundColor: d.online ? "#22c55e" : "#6b7280",
                      boxShadow: d.online ? "0 0 8px #22c55e60" : "none",
                    }}
                  />
                  <span className="font-semibold text-[color:var(--color-text-primary)]">{d.name}</span>
                  {ctrl?.blocked && (
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-red-500/20 text-red-400 border border-red-500/30">
                      已封禁
                    </span>
                  )}
                  {ctrl?.paused && (
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-400 border border-amber-500/30">
                      暂停中
                    </span>
                  )}
                </div>
                <span className="text-[11px] text-[color:var(--color-text-muted)] font-mono">{d.mac}</span>
              </div>

              {/* 使用量进度条 */}
              <div className="mb-4">
                <div className="flex items-baseline justify-between mb-2">
                  <span className="text-sm font-medium text-[color:var(--color-text-primary)]">
                    {formatTime(usageSec)}
                  </span>
                  <span className="text-xs text-[color:var(--color-text-muted)]">
                    限额 {formatTime(limitSec)} · {pct}%
                  </span>
                </div>
                <div className="h-1.5 bg-white/[0.06] rounded-full overflow-hidden">
                  <div
                    className="h-full rounded-full transition-all duration-500"
                    style={{
                      width: `${pct}%`,
                      background: `linear-gradient(90deg, ${progressColor}90, ${progressColor})`,
                      boxShadow: `0 0 8px ${progressColor}40`,
                    }}
                  />
                </div>
              </div>

              {/* 今日活动 */}
              <div className="flex gap-3 text-xs mb-4">
                {d.daily_video_min > 0 && (
                  <span className="text-blue-400">📺 视频 {d.daily_video_min}min</span>
                )}
                {d.daily_game_min > 0 && (
                  <span className="text-orange-400">🎮 游戏 {d.daily_game_min}min</span>
                )}
                {d.daily_video_min === 0 && d.daily_game_min === 0 && (
                  <span className="text-[color:var(--color-text-muted)]">暂无活动</span>
                )}
              </div>

              {/* 操作按钮 */}
              <div className="flex flex-wrap gap-2">
                <PillButton onClick={() => handleAdjust(d.mac, 1800)} color="#22c55e">
                  ➕ 30分钟
                </PillButton>
                <PillButton onClick={() => handleAdjust(d.mac, -1800)} color="#f97316">
                  ➖ 30分钟
                </PillButton>
                <PillButton
                  onClick={() => handleSwitch(d.mac, !ctrl?.switch_enabled)}
                  color={ctrl?.switch_enabled ? "#ef4444" : "#3b82f6"}
                  active={!ctrl?.switch_enabled}
                >
                  🔴 {ctrl?.blocked ? "恢复" : "断网"}
                </PillButton>
                <PillButton
                  onClick={() => handlePause(d.mac, !ctrl?.paused)}
                  color={ctrl?.paused ? "#22c55e" : "#eab308"}
                >
                  {ctrl?.paused ? "▶️ 恢复" : "⏸️ 暂停"}
                </PillButton>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
