import { useEffect, useState } from "react";
import { fetchSettings, postSettings, setVacation } from "../api";

interface TabletInfo {
  name: string;
  mac: string;
  ip: string;
  ipv6_comment: string;
}

export default function SettingsPage() {
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [tablets, setTablets] = useState<Record<string, TabletInfo>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState("");

  async function reload() {
    setLoading(true);
    try {
      const data = await fetchSettings();
      setSettings(data);
      // 解析 TABLETS JSON
      try {
        const raw = data.TABLETS || "{}";
        const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
        // 标准化：确保每个设备有完整的字段
        const normalized: Record<string, TabletInfo> = {};
        for (const [mac, info] of Object.entries(parsed) as [string, any][]) {
          normalized[mac.toUpperCase()] = {
            name: info.name || "",
            mac: info.mac || mac.toUpperCase(),
            ip: info.ip || "",
            ipv6_comment: info.ipv6_comment || "",
          };
        }
        setTablets(normalized);
      } catch {
        setTablets({});
      }
    } catch (e) {
      console.error("加载设置失败:", e);
    }
    setLoading(false);
  }
  useEffect(() => { reload(); }, []);

  async function handleSave() {
    setSaving(true);
    try {
      // 先保存基础设置
      const result = await postSettings(settings);
      if (result.ok) {
        // 再保存 TABLETS
        const tabletsResult = await postSettings({ TABLETS: JSON.stringify(tablets) });
        if (tabletsResult.ok) {
          setMsg("✅ 保存成功");
        } else {
          setMsg("❌ " + (tabletsResult.error || "设备保存失败"));
        }
      } else {
        setMsg("❌ " + (result.error || "保存失败"));
      }
    } catch (_e) {
      setMsg("❌ 保存失败");
    }
    setTimeout(() => setMsg(""), 2000);
    setSaving(false);
  }

  async function handleVacation(enabled: boolean) {
    await setVacation(enabled);
    reload();
  }

  function update(key: string, value: string) {
    setSettings((prev) => ({ ...prev, [key]: value }));
  }

  function updateTablet(mac: string, field: keyof TabletInfo, value: string) {
    setTablets((prev) => ({
      ...prev,
      [mac.toUpperCase()]: { ...prev[mac], [field]: value },
    }));
  }

  function addTablet() {
    const newMac = "XX:XX:XX:XX:XX:XX";
    setTablets((prev) => ({
      ...prev,
      [newMac]: { name: "", mac: newMac, ip: "", ipv6_comment: "" },
    }));
  }

  function removeTablet(mac: string) {
    setTablets((prev) => {
      const next = { ...prev };
      delete next[mac];
      return next;
    });
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

  const inputClass = "bg-white/[0.06] border border-white/[0.08] rounded-xl px-3 py-1.5 text-sm text-[color:var(--color-text-primary)] outline-none focus:border-purple-500/50 focus:bg-white/[0.08] transition-all duration-200 w-full";
  const selectClass = "bg-white/[0.06] border border-white/[0.08] rounded-xl px-3 py-1.5 text-sm text-[color:var(--color-text-primary)] outline-none focus:border-purple-500/50 transition-all duration-200 w-full appearance-none";

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-xl font-bold tracking-tight">⚙️ 设置</h2>

      {/* 设备管理 */}
      <div className="rounded-2xl backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] p-5">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-semibold text-[color:var(--color-text-primary)]">📱 设备管理</h3>
          <button onClick={addTablet}
            className="text-xs px-3 py-1 rounded-full bg-purple-500/20 border border-purple-500/30 text-purple-400 hover:bg-purple-500/30 transition-all duration-200 cursor-pointer"
          >+ 添加设备</button>
        </div>
        <div className="space-y-4">
          {Object.entries(tablets).map(([mac, tablet]) => (
            <div key={mac} className="bg-white/[0.03] border border-white/[0.06] rounded-xl p-4 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-mono text-[color:var(--color-text-muted)]">{mac}</span>
                <button onClick={() => removeTablet(mac)}
                  className="text-xs text-red-400/60 hover:text-red-400 transition-colors cursor-pointer"
                >删除</button>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs text-[color:var(--color-text-muted)] block mb-1">设备名称</label>
                  <input value={tablet.name} onChange={(e) => updateTablet(mac, "name", e.target.value)}
                    className={inputClass} placeholder="如 周芓翕" />
                </div>
                <div>
                  <label className="text-xs text-[color:var(--color-text-muted)] block mb-1">MAC 地址</label>
                  <input value={tablet.mac} onChange={(e) => updateTablet(mac, "mac", e.target.value)}
                    className={inputClass} placeholder="A0:DE:0F:45:2D:39" />
                </div>
                <div>
                  <label className="text-xs text-[color:var(--color-text-muted)] block mb-1">IP（可选，离线时用静态IP）</label>
                  <input value={tablet.ip} onChange={(e) => updateTablet(mac, "ip", e.target.value)}
                    className={inputClass} placeholder="10.1.1.97" />
                </div>
                <div>
                  <label className="text-xs text-[color:var(--color-text-muted)] block mb-1">IPv6 规则 comment</label>
                  <input value={tablet.ipv6_comment} onChange={(e) => updateTablet(mac, "ipv6_comment", e.target.value)}
                    className={inputClass} placeholder="华为平板" />
                </div>
              </div>
            </div>
          ))}
          {Object.keys(tablets).length === 0 && (
            <div className="text-sm text-[color:var(--color-text-muted)] text-center py-4">
              暂无设备，点击"添加设备"开始
            </div>
          )}
        </div>
      </div>

      {/* 假期模式 */}
      <div className="rounded-2xl backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] p-5">
        <h3 className="font-semibold mb-3 text-[color:var(--color-text-primary)]">🏖️ 假期模式</h3>
        <div className="flex gap-2">
          <button
            onClick={() => handleVacation(false)}
            className={`flex-1 px-4 py-2 rounded-full text-sm font-medium transition-all duration-200 border cursor-pointer ${
              settings.VACATION_MODE !== "true"
                ? "bg-blue-500/20 border-blue-500/30 text-blue-400"
                : "bg-white/[0.03] border-white/[0.06] text-[color:var(--color-text-muted)] hover:bg-white/[0.06]"
            }`}
          >
            关闭（工作日/周末）
          </button>
          <button
            onClick={() => handleVacation(true)}
            className={`flex-1 px-4 py-2 rounded-full text-sm font-medium transition-all duration-200 border cursor-pointer ${
              settings.VACATION_MODE === "true"
                ? "bg-emerald-500/20 border-emerald-500/30 text-emerald-400"
                : "bg-white/[0.03] border-white/[0.06] text-[color:var(--color-text-muted)] hover:bg-white/[0.06]"
            }`}
          >
            开启（假期规则）
          </button>
        </div>
      </div>

      {/* 时间规则 */}
      <div className="rounded-2xl backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] p-5">
        <h3 className="font-semibold mb-3 text-[color:var(--color-text-primary)]">⏰ 时间规则</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm text-[color:var(--color-text-secondary)]">工作日管控</label>
            <select
              value={settings.WORKDAY_TIME_MODE || "block"}
              onChange={(e) => update("WORKDAY_TIME_MODE", e.target.value)}
              className={selectClass}
            >
              <option value="block">限制模式</option>
              <option value="allow">允许模式</option>
              <option value="all">全天模式</option>
            </select>
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm text-[color:var(--color-text-secondary)]">免费时段开始</label>
            <input
              type="time"
              value={settings.WORKDAY_FREE_START || "08:00"}
              onChange={(e) => update("WORKDAY_FREE_START", e.target.value)}
              className={inputClass}
            />
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm text-[color:var(--color-text-secondary)]">免费时段结束</label>
            <input
              type="time"
              value={settings.WORKDAY_FREE_END || "21:00"}
              onChange={(e) => update("WORKDAY_FREE_END", e.target.value)}
              className={inputClass}
            />
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm text-[color:var(--color-text-secondary)]">每日限额（分钟）</label>
            <input
              type="number"
              value={settings.DEFAULT_LIMIT || "60"}
              onChange={(e) => update("DEFAULT_LIMIT", e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </div>

      {/* DNS 采集间隔 */}
      <div className="rounded-2xl backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] p-5">
        <h3 className="font-semibold mb-3 text-[color:var(--color-text-primary)]">📡 采集设置</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm text-[color:var(--color-text-secondary)]">DNS 采集间隔（秒）</label>
            <input
              type="number"
              value={settings.DNS_INTERVAL || "60"}
              onChange={(e) => update("DNS_INTERVAL", e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </div>

      {/* 保存按钮 */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-6 py-2 rounded-full bg-gradient-to-r from-purple-500 to-blue-500 hover:from-purple-400 hover:to-blue-400 disabled:opacity-50 font-semibold text-sm transition-all duration-200 cursor-pointer shadow-lg shadow-purple-500/20"
        >
          {saving ? "保存中..." : "💾 保存设置"}
        </button>
        {msg && <span className="text-sm text-[color:var(--color-text-secondary)]">{msg}</span>}
      </div>
    </div>
  );
}
