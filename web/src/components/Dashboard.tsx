import { useState, useEffect, useCallback } from "react";
import type { UserData, RecentDeviceActivity } from "../types";
import { fetchData, fetchRecentActivity, kidAdjust, pauseDevice, switchDevice } from "../api";
import RealTimeActivity from "./RealTimeActivity";

function MiniBar({
  value,
  max,
  color,
}: {
  value: number;
  max: number;
  color: string;
}) {
  const pct = Math.min((value / max) * 100, 100);
  return (
    <div
      className="h-1.5 rounded-full overflow-hidden"
      style={{ background: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-full rounded-full transition-all duration-700"
        style={{
          width: `${pct}%`,
          background: `linear-gradient(90deg, ${color}88, ${color})`,
        }}
      />
    </div>
  );
}

/** 华为橙色系 or iPad 青色系 */
function deviceColor(name: string) {
  if (name.includes("华为")) {
    return {
      c: "var(--hw)",
      g: "var(--hw-g)",
      b: "var(--hw-b)",
      icon: "📱",
      css: "hw",
    };
  }
  return {
    c: "var(--ip)",
    g: "var(--ip-g)",
    b: "var(--ip-b)",
    icon: "📺",
    css: "ip",
  };
}

export default function Dashboard() {
  const [data, setData] = useState<UserData | null>(null);
  const [recentActivity, setRecentActivity] = useState<RecentDeviceActivity[]>([]);
  const [loading, setLoading] = useState(true);

  // 主数据加载
  const load = useCallback(async () => {
    try {
      const d = await fetchData();
      setData(d);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  // 最近20分钟活动数据（跟随主数据一起刷新）
  const loadRecent = useCallback(async () => {
    try {
      const r = await fetchRecentActivity();
      setRecentActivity(r);
    } catch {}
  }, []);

  useEffect(() => {
    loadRecent();
    const timer = setInterval(loadRecent, 30000);
    return () => clearInterval(timer);
  }, [loadRecent]);

  useEffect(() => {
    load();
    const timer = setInterval(load, 30000);
    return () => clearInterval(timer);
  }, [load]);

  if (loading && !data) {
    return (
      <div className="flex items-center justify-center py-20">
        <div
          className="w-8 h-8 border-2 rounded-full animate-spin"
          style={{
            borderColor: "var(--acc)",
            borderTopColor: "transparent",
          }}
        />
      </div>
    );
  }

  if (!data)
    return (
      <div className="text-center py-20" style={{ color: "var(--t3)" }}>
        加载失败
      </div>
    );

  const now = new Date();
  const HHMM = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;

  // 设备固定排序：周楷依 → 周芓翕
  const ORDER = ["周楷依", "周芓翕"];
  const sortedDevices = [...data.devices].sort(
    (a, b) => ORDER.indexOf(a.name) - ORDER.indexOf(b.name)
  );

  return (
    <div className="space-y-6">
      {/* 顶部状态栏 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold" style={{ color: "var(--t1)" }}>
            管控仪表盘
          </h1>
          <p className="text-sm mt-0.5" style={{ color: "var(--t3)" }}>
            {now.getFullYear()}年{now.getMonth() + 1}月{now.getDate()}日 ·{" "}
            {HHMM}
          </p>
        </div>
        <div className="flex items-center gap-2 text-xs" style={{ color: "var(--t3)" }}>
          <div
            className="w-1.5 h-1.5 rounded-full"
            style={{
              background: "var(--ok)",
              animation: "pulse-glow 2s ease-in-out infinite",
            }}
          />
          <span>系统运行中</span>
        </div>
      </div>

      {/* 设备卡片网格 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {[...data.devices].sort((a, b) => {
          const ORDER = ["周楷依", "周芓翕"];
          const ai = ORDER.indexOf(a.name);
          const bi = ORDER.indexOf(b.name);
          if (ai !== -1 && bi !== -1) return ai - bi;
          return a.name.localeCompare(b.name, "zh");
        }).map((dev) => {
          const dc = deviceColor(dev.name);
          const limitMin = Math.round((dev.limit_sec || 7200) / 60);
          const usageMin = Math.round((dev.usage_sec || 0) / 60);
          const remainMin = Math.max(limitMin - usageMin, 0);
          const over = usageMin >= limitMin;
          const isPaused = data?.config?.[`PAUSE_${dev.mac.toUpperCase()}`] === "true";
          const isBlocked = data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] === "false";

          const glowClass = isBlocked
            ? "glow-blocked"
            : over
              ? "glow-danger"
              : dev.online
                ? `glow-border glow-${dc.css} glow-ok`
                : "";
          const shimmerClass =
            dev.online && !over && !isBlocked ? `shimmer-${dc.css}` : "";

          // 状态徽标
          const statusBadge = isBlocked
            ? { text: "🚫 已禁止上网", color: "#ef4444", bg: "rgba(239,68,68,0.12)" }
            : isPaused
              ? { text: "👤 大人模式", color: "#f59e0b", bg: "rgba(245,158,11,0.12)" }
              : null;

          return (
            <div
              key={dev.mac}
              className={`relative rounded-2xl p-5 transition-all duration-300 overflow-hidden glass ${glowClass}`}
            >
              {/* 背景光晕 shimmer */}
              {shimmerClass && (
                <div
                  className={`absolute inset-0 rounded-2xl opacity-30 ${shimmerClass}`}
                  style={{ pointerEvents: "none" }}
                />
              )}

              <div className="relative z-10">
                {/* 设备头部 */}
                <div className="flex items-center justify-between mb-4">
                  <div className="flex items-center gap-3">
                    <div
                      className="w-10 h-10 rounded-xl flex items-center justify-center text-lg"
                      style={{
                        background: over ? "var(--err-g)" : `${dc.g}`,
                        border: `1px solid ${over ? "var(--err)" : dc.b}`,
                      }}
                    >
                      {dc.icon}
                    </div>
                    <div>
                      <div className="font-semibold" style={{ color: "var(--t1)" }}>
                        {dev.name}
                      </div>
                      <div
                        className="text-xs"
                        style={{
                          color: dev.online ? "var(--ok)" : "var(--t3)",
                        }}
                      >
                        {dev.online ? "● 在线" : "○ 离线"}
                      </div>
                    </div>
                  </div>

                  {/* 使用时间大数字 */}
                  <div className="text-right">
                    <div
                      className="text-3xl font-bold font-mono tracking-tighter"
                      style={{
                        color: over ? "var(--err)" : "var(--t1)",
                      }}
                    >
                      {remainMin}
                    </div>
                    <div className="text-xs" style={{ color: "var(--t3)" }}>
                      剩余分钟
                    </div>
                  </div>
                </div>

                {/* 状态徽标 */}
                {statusBadge && (
                  <div
                    className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium mb-3"
                    style={{
                      color: statusBadge.color,
                      background: statusBadge.bg,
                      border: `1px solid ${statusBadge.color}33`,
                    }}
                  >
                    {statusBadge.text}
                  </div>
                )}

                {/* 进度条 */}
                <div className="mb-4">
                  <div
                    className="flex items-center justify-between text-xs mb-1.5"
                    style={{ color: "var(--t3)" }}
                  >
                    <span>已用 {usageMin}min</span>
                    <span>限额 {limitMin}min</span>
                  </div>
                  <MiniBar
                    value={usageMin}
                    max={limitMin}
                    color={over ? "var(--err)" : dc.c}
                  />
                </div>

                {/* 今日活动 */}
                {dev.daily_video_min > 0 && (
                  <div className="flex items-center gap-2 text-xs" style={{ color: "var(--t3)" }}>
                    <span>🎬</span>
                    <span>视频 {dev.daily_video_min}min</span>
                  </div>
                )}
                {dev.daily_game_min > 0 && (
                  <div className="flex items-center gap-2 text-xs" style={{ color: "var(--t3)" }}>
                    <span>🎮</span>
                    <span>游戏 {dev.daily_game_min}min</span>
                  </div>
                )}

                {/* ─── 操作按钮 ─── */}
                <div className="flex flex-wrap gap-2 mt-3">
                  <button
                    onClick={async () => {
                      try { await kidAdjust(dev.mac, 1800); } catch (e) { console.error(e); }
                      setTimeout(load, 500);
                    }}
                    className="px-3 py-1.5 rounded-full text-xs font-medium transition-all cursor-pointer"
                    style={{
                      background: "rgba(34,197,94,0.12)",
                      border: "1px solid rgba(34,197,94,0.3)",
                      color: "#22c55e",
                    }}
                  >
                    +30min
                  </button>
                  <button
                    onClick={async () => {
                      try { await kidAdjust(dev.mac, -1800); } catch (e) { console.error(e); }
                      setTimeout(load, 500);
                    }}
                    className="px-3 py-1.5 rounded-full text-xs font-medium transition-all cursor-pointer"
                    style={{
                      background: "rgba(239,68,68,0.12)",
                      border: "1px solid rgba(239,68,68,0.3)",
                      color: "#ef4444",
                    }}
                  >
                    -30min
                  </button>
                  <button
                    onClick={async () => {
                      const paused = data?.config?.[`PAUSE_${dev.mac.toUpperCase()}`] === "true";
                      try { await pauseDevice(dev.mac, !paused); } catch (e) { console.error(e); }
                      setTimeout(load, 500);
                    }}
                    className="px-3 py-1.5 rounded-full text-xs font-medium transition-all cursor-pointer"
                    style={{
                      background: "rgba(245,158,11,0.12)",
                      border: "1px solid rgba(245,158,11,0.3)",
                      color: "#f59e0b",
                    }}
                  >
                    {data?.config?.[`PAUSE_${dev.mac.toUpperCase()}`] === "true" ? "恢复管控" : "大人模式"}
                  </button>
                  <button
                    onClick={async () => {
                      const enabled = data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] !== "false";
                      try { await switchDevice(dev.mac, !enabled); } catch (e) { console.error(e); }
                      setTimeout(load, 500);
                    }}
                    className="px-3 py-1.5 rounded-full text-xs font-medium transition-all cursor-pointer"
                    style={{
                      background: data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] !== "false"
                        ? "rgba(239,68,68,0.12)" : "rgba(34,197,94,0.12)",
                      border: `1px solid ${data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] !== "false"
                        ? "rgba(239,68,68,0.3)" : "rgba(34,197,94,0.3)"}`,
                      color: data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] !== "false"
                        ? "#ef4444" : "#22c55e",
                    }}
                  >
                    {data?.config?.[`SWITCH_${dev.mac.toUpperCase()}`] !== "false" ? "禁止上网" : "恢复上网"}
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* 实时活动卡片 */}
      <RealTimeActivity
        windows={data.video_windows}
        devices={data.devices.map((d) => ({
          mac: d.mac,
          name: d.name,
          online: d.online,
        }))}
      />

      {/* 最近20分钟活动 */}
      <div className="rounded-2xl p-5 glass">
        <h3
          className="text-sm font-semibold mb-3 tracking-wide uppercase"
          style={{ color: "var(--t1)" }}
        >
          最近20分钟活动
        </h3>
        <div className="space-y-4">
          {sortedDevices.map((dev) => {
            const dc = deviceColor(dev.name);
            const devActivity = recentActivity.find((r) => r.mac === dev.mac);
            const minutes = devActivity?.minutes || [];

            // 生成20个时间槽（当前分钟往前推19分钟）
            const slots: { time: string; status: string; type: string }[] = [];
            for (let i = 19; i >= 0; i--) {
              const d = new Date(now.getTime() - i * 60000);
              const time = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
              const found = minutes.find((m) => m.time === time);
              slots.push(found || { time, status: "NODATA", type: "none" });
            }

            const activeCount = slots.filter((s) => s.status === "ACTIVE").length;

            return (
              <div key={dev.mac}>
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-sm">{dc.icon}</span>
                  <span className="text-sm font-medium" style={{ color: "var(--t1)" }}>
                    {dev.name}
                  </span>
                  <span className="text-xs" style={{ color: "var(--t3)" }}>
                    {activeCount}/20min 活跃
                  </span>
                </div>
                {/* 时间轴 */}
                <div className="flex items-center gap-1">
                  <span className="text-[9px] w-8 text-right" style={{ color: "var(--t3)" }}>
                    -20m
                  </span>
                  <div className="flex gap-0.5 flex-1">
                    {slots.map((slot, i) => {
                      const isActive = slot.status === "ACTIVE";
                      const isNoData = slot.status === "NODATA";
                      const bg = isActive
                        ? dc.c
                        : isNoData
                          ? "rgba(255,255,255,0.02)"
                          : "rgba(255,255,255,0.08)";
                      return (
                        <div
                          key={i}
                          className="flex-1 h-5 rounded-sm transition-all duration-300"
                          style={{
                            background: bg,
                            border: isActive
                              ? `1px solid ${dc.c}`
                              : "1px solid rgba(255,255,255,0.04)",
                          }}
                          title={`${slot.time} — ${slot.status === "ACTIVE" ? slot.type : slot.status === "IDLE" ? "空闲" : "无数据"}`}
                        />
                      );
                    })}
                  </div>
                  <span className="text-[9px] w-8" style={{ color: "var(--t3)" }}>
                    now
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
