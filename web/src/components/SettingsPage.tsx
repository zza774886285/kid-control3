import { useEffect, useState } from "react";
import { fetchSettings, postSettings, setVacation } from "../api";

export default function SettingsPage() {
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState("");

  async function reload() {
    setLoading(true);
    try {
      const data = await fetchSettings();
      setSettings(data);
    } catch (e) {
      console.error("加载设置失败:", e);
    }
    setLoading(false);
  }

  useEffect(() => { reload(); }, []);

  async function handleSave() {
    setSaving(true);
    try {
      const result = await postSettings(settings);
      if (result.ok) {
        setMsg("✅ 保存成功");
        setTimeout(() => setMsg(""), 2000);
      } else {
        setMsg("❌ " + (result.error || "保存失败"));
      }
    } catch (_e) {
      setMsg("❌ 保存失败");
    }
    setSaving(false);
  }

  async function handleVacation(enabled: boolean) {
    await setVacation(enabled);
    reload();
  }

  function update(key: string, value: string) {
    setSettings((prev) => ({ ...prev, [key]: value }));
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
