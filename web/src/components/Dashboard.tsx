import { useEffect, useState } from "react";
import { fetchData, fetchControlStatus, kidAdjust, switchDevice, pauseDevice } from "../api";
import type { Device, ControlDevice } from "../types";

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

  function getCtrl(mac: string) {
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

  if (loading) return <div className="text-center py-10 text-gray-400">加载中...</div>;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <h2 className="text-xl font-bold">仪表盘</h2>
        <span className="text-xs px-2 py-1 rounded bg-gray-700">{dayType === "workday" ? "工作日" : dayType === "weekend" ? "周末" : "假期"}</span>
        <button onClick={reload} className="text-xs px-2 py-1 rounded bg-gray-700 hover:bg-gray-600">🔄 刷新</button>
      </div>

      {/* 设备卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {devices.map((d) => {
          const ctrl = getCtrl(d.mac);
          const usageMin = Math.round((ctrl?.usage_sec || d.usage_sec || 0) / 60);
          const limitMin = Math.round((ctrl?.limit_sec || d.limit_sec || 0) / 60);
          const pct = limitMin > 0 ? Math.min(100, Math.round((usageMin / limitMin) * 100)) : 0;

          return (
            <div key={d.mac} className={`rounded-xl p-4 border ${d.online ? "border-green-600 bg-gray-800" : "border-gray-700 bg-gray-800/50"}`}>
              <div className="flex items-center justify-between mb-3">
                <div>
                  <span className="font-bold text-lg">{d.name}</span>
                  <span className={`ml-2 text-xs px-2 py-0.5 rounded ${d.online ? "bg-green-600" : "bg-gray-600"}`}>
                    {d.online ? "在线" : "离线"}
                  </span>
                  {ctrl?.blocked && <span className="ml-2 text-xs px-2 py-0.5 rounded bg-red-600">已封禁</span>}
                  {ctrl?.paused && <span className="ml-2 text-xs px-2 py-0.5 rounded bg-yellow-600">暂停中</span>}
                </div>
                <span className="text-sm text-gray-400">{d.mac}</span>
              </div>

              {/* 使用进度条 */}
              <div className="mb-3">
                <div className="flex justify-between text-sm mb-1">
                  <span>今日 {usageMin} 分钟</span>
                  <span className="text-gray-400">限额 {limitMin} 分钟 ({pct}%)</span>
                </div>
                <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
                  <div className={`h-full rounded-full ${pct > 80 ? "bg-red-500" : pct > 50 ? "bg-yellow-500" : "bg-green-500"}`}
                    style={{ width: `${pct}%` }} />
                </div>
              </div>

              {/* 今日活动 */}
              <div className="flex gap-4 text-sm mb-3">
                {d.daily_video_min > 0 && <span className="text-blue-400">📺 视频 {d.daily_video_min}min</span>}
                {d.daily_game_min > 0 && <span className="text-orange-400">🎮 游戏 {d.daily_game_min}min</span>}
                {d.daily_video_min === 0 && d.daily_game_min === 0 && <span className="text-gray-500">暂无活动记录</span>}
              </div>

              {/* 操作按钮 */}
              <div className="flex flex-wrap gap-2">
                <button onClick={() => handleAdjust(d.mac, 1800)} className="text-xs px-3 py-1 rounded bg-green-600 hover:bg-green-500">+30分钟</button>
                <button onClick={() => handleAdjust(d.mac, -1800)} className="text-xs px-3 py-1 rounded bg-orange-600 hover:bg-orange-500">-30分钟</button>
                <button onClick={() => handleSwitch(d.mac, !ctrl?.switch_enabled)} className={`text-xs px-3 py-1 rounded ${ctrl?.switch_enabled ? "bg-red-600 hover:bg-red-500" : "bg-blue-600 hover:bg-blue-500"}`}>
                  {ctrl?.switch_enabled ? "🔌 关闭管控" : "🔌 开启管控"}
                </button>
                <button onClick={() => handlePause(d.mac, !ctrl?.paused)} className={`text-xs px-3 py-1 rounded ${ctrl?.paused ? "bg-green-600 hover:bg-green-500" : "bg-yellow-600 hover:bg-yellow-500"}`}>
                  {ctrl?.paused ? "▶️ 恢复" : "⏸️ 暂停"}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
