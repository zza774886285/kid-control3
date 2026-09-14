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
    } catch (e) {
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

  if (loading) return <div className="text-center py-10 text-gray-400">加载中...</div>;

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-xl font-bold">⚙️ 设置</h2>

      {/* 假期模式 */}
      <div className="rounded-xl border border-gray-700 bg-gray-800/50 p-4">
        <h3 className="font-bold mb-3">🏖️ 假期模式</h3>
        <div className="flex gap-3">
          <button onClick={() => handleVacation(false)} className={`px-4 py-2 rounded ${settings.VACATION_MODE !== "true" ? "bg-blue-600" : "bg-gray-700 hover:bg-gray-600"}`}>
            关闭（使用工作日/周末规则）
          </button>
          <button onClick={() => handleVacation(true)} className={`px-4 py-2 rounded ${settings.VACATION_MODE === "true" ? "bg-green-600" : "bg-gray-700 hover:bg-gray-600"}`}>
            开启（使用假期规则）
          </button>
        </div>
      </div>

      {/* 时间规则 */}
      <div className="rounded-xl border border-gray-700 bg-gray-800/50 p-4">
        <h3 className="font-bold mb-3">⏰ 时间规则</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm">工作日管控</label>
            <select value={settings.WORKDAY_TIME_MODE || "block"} onChange={(e) => update("WORKDAY_TIME_MODE", e.target.value)}
              className="col-span-2 bg-gray-700 rounded px-3 py-1 text-sm">
              <option value="block">限制模式</option>
              <option value="allow">允许模式</option>
              <option value="all">全天模式</option>
            </select>
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm">免费时段开始</label>
            <input type="time" value={settings.WORKDAY_FREE_START || "08:00"} onChange={(e) => update("WORKDAY_FREE_START", e.target.value)}
              className="col-span-2 bg-gray-700 rounded px-3 py-1 text-sm" />
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm">免费时段结束</label>
            <input type="time" value={settings.WORKDAY_FREE_END || "21:00"} onChange={(e) => update("WORKDAY_FREE_END", e.target.value)}
              className="col-span-2 bg-gray-700 rounded px-3 py-1 text-sm" />
          </div>
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm">每日限额（分钟）</label>
            <input type="number" value={settings.DEFAULT_LIMIT || "60"} onChange={(e) => update("DEFAULT_LIMIT", e.target.value)}
              className="col-span-2 bg-gray-700 rounded px-3 py-1 text-sm" />
          </div>
        </div>
      </div>

      {/* DNS 采集间隔 */}
      <div className="rounded-xl border border-gray-700 bg-gray-800/50 p-4">
        <h3 className="font-bold mb-3">📡 采集设置</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm">DNS 采集间隔（秒）</label>
            <input type="number" value={settings.DNS_INTERVAL || "60"} onChange={(e) => update("DNS_INTERVAL", e.target.value)}
              className="col-span-2 bg-gray-700 rounded px-3 py-1 text-sm" />
          </div>
        </div>
      </div>

      {/* 保存按钮 */}
      <div className="flex items-center gap-3">
        <button onClick={handleSave} disabled={saving}
          className="px-6 py-2 rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-50 font-bold">
          {saving ? "保存中..." : "💾 保存设置"}
        </button>
        {msg && <span className="text-sm">{msg}</span>}
      </div>
    </div>
  );
}
