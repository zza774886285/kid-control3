import type { VideoWindow } from "../types";

interface Props {
  windows: VideoWindow[];
  devices: { mac: string; name: string; online: boolean }[];
}

const ACTIVITY_MAP: Record<string, { label: string; icon: string; color: string }> = {
  // 具体应用
  xiaohongshu:       { label: "小红书",   icon: "📕", color: "#ff2442" },
  game_egg:          { label: "蛋仔派对", icon: "🥚", color: "#f59e0b" },
  game_preternatural:{ label: "超自然",   icon: "👻", color: "#8b5cf6" },
  tencent_video:     { label: "腾讯视频", icon: "🎬", color: "#3b82f6" },
  hongguo:           { label: "红果短剧", icon: "🍊", color: "#f97316" },
  douyin:            { label: "抖音",     icon: "🎵", color: "#111" },
  bilibili:          { label: "B站",      icon: "📺", color: "#00aeec" },
  weixin:            { label: "微信",     icon: "💬", color: "#07c160" },
  qq:                { label: "QQ",       icon: "🐧", color: "#12b7f5" },
  // 通用分类
  game:              { label: "游戏",     icon: "🎮", color: "#a855f7" },
  video:             { label: "视频",     icon: "🎬", color: "#3b82f6" },
  other:             { label: "其他",     icon: "📱", color: "#94a3b8" },
  // 空闲
  idle:              { label: "空闲",     icon: "😴", color: "#64748b" },
  none:              { label: "空闲",     icon: "😴", color: "#64748b" },
};

function classify(activityType: string): { label: string; icon: string; color: string } {
  const lower = activityType.toLowerCase();
  if (ACTIVITY_MAP[lower]) return ACTIVITY_MAP[lower];
  if (lower.startsWith("game")) return ACTIVITY_MAP.game;
  if (lower.startsWith("video")) return ACTIVITY_MAP.video;
  return ACTIVITY_MAP.other;
}

export default function RealTimeActivity({ windows, devices }: Props) {
  const now = new Date();
  const HHMM = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;

  // 构建 HHMM -> activity_type 映射，用于快速查找
  const timeMap = new Map<string, string>();
  for (const w of windows) {
    // ts 格式: "2026-09-15T21:07:00" -> 提取 "21:07"
    const match = w.ts.match(/T(\d{2}:\d{2})/);
    if (match) {
      const key = `${w.mac}|${match[1]}`;
      timeMap.set(key, w.activity_type);
    }
  }

  const deviceActivities = devices.map((dev) => {
    // 找当前分钟的活动
    let activityType = timeMap.get(`${dev.mac}|${HHMM}`);

    // 如果当前分钟没数据，往前找最近3分钟
    if (!activityType) {
      const [h, m] = HHMM.split(":").map(Number);
      for (let i = 1; i <= 3; i++) {
        let mins = m - i;
        let hours = h;
        if (mins < 0) { mins += 60; hours -= 1; }
        if (hours < 0) hours += 24;
        const prev = `${String(hours).padStart(2, "0")}:${String(mins).padStart(2, "0")}`;
        const found = timeMap.get(`${dev.mac}|${prev}`);
        if (found && found !== "none") {
          activityType = found;
          break;
        }
      }
    }

    const act = classify(activityType || "none");
    const isActive = activityType && activityType !== "none" && activityType !== "idle";

    return { ...dev, activity: act, isActive, activityType: activityType || "none" };
  });

  const anyActive = deviceActivities.some((d) => d.isActive);

  return (
    <div className="rounded-2xl p-5 transition-all duration-300 glass glow-border">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <div className="relative">
            <div className="w-2 h-2 rounded-full" style={{ background: "var(--acc)" }} />
            {anyActive && (
              <div
                className="absolute inset-0 w-2 h-2 rounded-full animate-ping opacity-75"
                style={{ background: "var(--acc)" }}
              />
            )}
          </div>
          <h3 className="text-sm font-semibold tracking-wide uppercase" style={{ color: "var(--t1)" }}>
            实时活动
          </h3>
        </div>
        <span className="text-xs font-mono" style={{ color: "var(--t3)" }}>{HHMM}</span>
      </div>

      <div className="space-y-3">
        {deviceActivities.map((dev) => {
          const isHw = dev.name.includes("华为");
          const devColor = isHw ? "var(--hw)" : "var(--ip)";

          return (
            <div key={dev.mac} className="flex items-center gap-3">
              <div className="relative flex-shrink-0">
                <div
                  className="w-10 h-10 rounded-xl flex items-center justify-center text-lg transition-all duration-300"
                  style={
                    dev.isActive
                      ? { background: `${devColor}20`, border: `1px solid ${devColor}40` }
                      : { background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }
                  }
                >
                  {dev.activity.icon}
                </div>
                {dev.isActive && (
                  <>
                    <div
                      className="absolute inset-0 rounded-xl"
                      style={{
                        boxShadow: `0 0 12px ${devColor}60`,
                        animation: "pulse-glow 2s ease-in-out infinite",
                      }}
                    />
                    <div
                      className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full border-2"
                      style={{
                        background: "var(--ok)",
                        borderColor: "var(--bg-solid)",
                        boxShadow: "0 0 6px var(--ok)",
                        animation: "pulse-glow 2s ease-in-out infinite",
                      }}
                    />
                  </>
                )}
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium" style={{ color: "var(--t1)" }}>
                    {dev.name}
                  </span>
                  <span
                    className="text-xs px-1.5 py-0.5 rounded-full font-medium"
                    style={
                      dev.isActive
                        ? { background: `${devColor}20`, color: devColor }
                        : { background: "rgba(255,255,255,0.05)", color: "var(--t3)" }
                    }
                  >
                    {dev.activity.label}
                  </span>
                </div>
                {dev.isActive && (
                  <div className="text-xs mt-0.5 truncate" style={{ color: "var(--t3)" }}>
                    正在使用 {dev.activity.label}...
                  </div>
                )}
              </div>

              <div
                className="text-xs font-mono"
                style={{ color: dev.online ? "var(--ok)" : "var(--t3)" }}
              >
                {dev.online ? "在线" : "离线"}
              </div>
            </div>
          );
        })}
      </div>

      {deviceActivities.length === 0 && (
        <div className="text-center text-sm py-4" style={{ color: "var(--t3)" }}>
          暂无设备
        </div>
      )}
    </div>
  );
}
