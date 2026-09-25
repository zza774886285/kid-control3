import { useEffect, useState } from "react";
import { fetchSettings, postSettings, setVacation, fetchGameIps, addGameIp, removeGameIp, addGameDomain, removeGameDomain } from "../api";
import type { GameIpEntry } from "../types";

interface TabletInfo {
  name: string;
  mac: string;
  ip: string;
  ipv6_comment: string;
}

const glassInput =
  "w-full rounded-xl px-3 py-1.5 text-sm outline-none transition-all duration-200";

function GameIpManager() {
  const [domains, setDomains] = useState<string[]>([]);
  const [ips, setIps] = useState<GameIpEntry[]>([]);
  const [newIp, setNewIp] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [newDomain, setNewDomain] = useState("");
  const [msg, setMsg] = useState("");

  async function reload() {
    try {
      const data = await fetchGameIps();
      setDomains(data.domains);
      setIps(data.ips);
    } catch (e) {
      console.error("加载游戏IP表失败:", e);
    }
  }
  useEffect(() => { reload(); }, []);

  function flash(text: string) {
    setMsg(text);
    setTimeout(() => setMsg(""), 2000);
  }

  async function handleAddIp() {
    if (!newIp.trim()) return;
    const res = await addGameIp(newIp.trim(), newLabel.trim() || undefined);
    flash(res.message);
    if (res.ok) { setNewIp(""); setNewLabel(""); reload(); }
  }

  async function handleRemoveIp(addr: string) {
    const res = await removeGameIp(addr);
    flash(res.message);
    if (res.ok) reload();
  }

  async function handleAddDomain() {
    if (!newDomain.trim()) return;
    const res = await addGameDomain(newDomain.trim());
    flash(res.message);
    if (res.ok) { setNewDomain(""); reload(); }
  }

  async function handleRemoveDomain(domain: string) {
    const res = await removeGameDomain(domain);
    flash(res.message);
    if (res.ok) reload();
  }

  const manualIps = ips.filter(i => i.source === "manual");
  const autoIps = ips.filter(i => i.source === "auto");

  return (
    <div className="rounded-2xl glass p-5">
      <div className="flex items-center justify-between mb-3">
        <h3 className="font-semibold" style={{ color: "var(--t1)" }}>🎮 游戏识别</h3>
        {msg && <span className="text-xs" style={{ color: "var(--t2)" }}>{msg}</span>}
      </div>

      {/* 域名模式 */}
      <div className="mb-4">
        <label className="text-xs block mb-2" style={{ color: "var(--t3)" }}>域名模式（匹配则判定为游戏）</label>
        <div className="flex flex-wrap gap-2 mb-2">
          {domains.map(d => (
            <span key={d} className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-full"
              style={{ background: "rgba(139,92,246,0.15)", border: "1px solid rgba(139,92,246,0.3)", color: "#a78bfa" }}>
              {d}
              <button onClick={() => handleRemoveDomain(d)} className="ml-1 cursor-pointer hover:opacity-70">×</button>
            </span>
          ))}
        </div>
        <div className="flex gap-2">
          <input value={newDomain} onChange={e => setNewDomain(e.target.value)}
            placeholder="*.example.com"
            className={`flex-1 ${glassInput}`}
            style={{ background: "rgba(255,255,255,0.06)", border: "1px solid rgba(255,255,255,0.08)", color: "var(--t1)" }}
            onKeyDown={e => e.key === "Enter" && handleAddDomain()} />
          <button onClick={handleAddDomain}
            className="text-xs px-3 py-1 rounded-full cursor-pointer"
            style={{ background: "var(--acc-g2)", border: "1px solid var(--acc-g)", color: "var(--acc)" }}>
            + 域名
          </button>
        </div>
      </div>

      {/* 手动添加的 IP */}
      <div className="mb-4">
        <label className="text-xs block mb-2" style={{ color: "var(--t3)" }}>手动添加的 IP/CIDR</label>
        <div className="space-y-1.5 mb-2">
          {manualIps.map(ip => (
            <div key={ip.addr} className="flex items-center justify-between text-xs px-3 py-1.5 rounded-lg"
              style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div className="flex items-center gap-2">
                <span className="font-mono" style={{ color: "var(--t1)" }}>{ip.addr}</span>
                {ip.label && <span style={{ color: "var(--t3)" }}>({ip.label})</span>}
              </div>
              <button onClick={() => handleRemoveIp(ip.addr)}
                className="cursor-pointer transition-colors" style={{ color: "rgba(239,68,68,0.6)" }}>删除</button>
            </div>
          ))}
          {manualIps.length === 0 && (
            <div className="text-xs text-center py-2" style={{ color: "var(--t3)" }}>暂无手动 IP</div>
          )}
        </div>
        <div className="flex gap-2">
          <input value={newIp} onChange={e => setNewIp(e.target.value)}
            placeholder="IP 或 CIDR（如 1.2.3.4 或 10.0.0.0/24）"
            className={`flex-1 ${glassInput}`}
            style={{ background: "rgba(255,255,255,0.06)", border: "1px solid rgba(255,255,255,0.08)", color: "var(--t1)" }}
            onKeyDown={e => e.key === "Enter" && handleAddIp()} />
          <input value={newLabel} onChange={e => setNewLabel(e.target.value)}
            placeholder="备注（可选）"
            className={`w-28 ${glassInput}`}
            style={{ background: "rgba(255,255,255,0.06)", border: "1px solid rgba(255,255,255,0.08)", color: "var(--t1)" }} />
          <button onClick={handleAddIp}
            className="text-xs px-3 py-1 rounded-full cursor-pointer"
            style={{ background: "var(--acc-g2)", border: "1px solid var(--acc-g)", color: "var(--acc)" }}>
            + IP
          </button>
        </div>
      </div>

      {/* 自动发现的 IP */}
      {autoIps.length > 0 && (
        <div>
          <label className="text-xs block mb-2" style={{ color: "var(--t3)" }}>
            自动发现（{autoIps.length} 条，来自 DNS 和连接表）
          </label>
          <div className="space-y-1.5">
            {autoIps.map(ip => (
              <div key={ip.addr} className="flex items-center justify-between text-xs px-3 py-1.5 rounded-lg"
                style={{ background: "rgba(16,185,129,0.05)", border: "1px solid rgba(16,185,129,0.1)" }}>
                <div className="flex items-center gap-2">
                  <span className="font-mono" style={{ color: "var(--t1)" }}>{ip.addr}</span>
                  {ip.first_seen && <span style={{ color: "var(--t3)" }}>{ip.first_seen}</span>}
                </div>
                <button onClick={() => handleRemoveIp(ip.addr)}
                  className="cursor-pointer transition-colors" style={{ color: "rgba(239,68,68,0.6)" }}>删除</button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
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
      try {
        const raw = data.TABLETS || "{}";
        const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
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
      const result = await postSettings(settings);
      if (result.ok) {
        const tabletsResult = await postSettings({ TABLETS: JSON.stringify(tablets) });
        if (tabletsResult.ok) {
          setMsg("✅ 保存成功");
        } else {
          setMsg("❌ " + (tabletsResult.error || "设备保存失败"));
        }
      } else {
        setMsg("❌ " + (result.error || "保存失败"));
      }
    } catch {
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

  const inputStyle = {
    background: "rgba(255,255,255,0.06)",
    border: "1px solid rgba(255,255,255,0.08)",
    color: "var(--t1)",
  };
  const inputFocus = "focus:border-[var(--acc)]/50 focus:bg-white/[0.08]";

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="flex items-center gap-3" style={{ color: "var(--t3)" }}>
          <div
            className="w-5 h-5 border-2 rounded-full animate-spin"
            style={{ borderColor: "rgba(255,255,255,0.2)", borderTopColor: "rgba(255,255,255,0.6)" }}
          />
          <span>加载中...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-2xl mx-auto">
      <h2 className="text-xl font-bold tracking-tight" style={{ color: "var(--t1)" }}>
        ⚙️ 设置
      </h2>

      {/* 设备管理 */}
      <div className="rounded-2xl glass p-5">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-semibold" style={{ color: "var(--t1)" }}>📱 设备管理</h3>
          <button
            onClick={addTablet}
            className="text-xs px-3 py-1 rounded-full cursor-pointer transition-all duration-200"
            style={{ background: "var(--acc-g2)", border: "1px solid var(--acc-g)", color: "var(--acc)" }}
          >
            + 添加设备
          </button>
        </div>
        <div className="space-y-4">
          {Object.entries(tablets).map(([mac, tablet]) => (
            <div key={mac} className="rounded-xl p-4 space-y-3" style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div className="flex items-center justify-between">
                <span className="text-xs font-mono" style={{ color: "var(--t3)" }}>{mac}</span>
                <button
                  onClick={() => removeTablet(mac)}
                  className="text-xs cursor-pointer transition-colors"
                  style={{ color: "rgba(239,68,68,0.6)" }}
                >
                  删除
                </button>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs block mb-1" style={{ color: "var(--t3)" }}>设备名称</label>
                  <input
                    value={tablet.name}
                    onChange={(e) => updateTablet(mac, "name", e.target.value)}
                    className={`${glassInput} ${inputFocus}`}
                    style={inputStyle}
                    placeholder="如 周芓翕"
                  />
                </div>
                <div>
                  <label className="text-xs block mb-1" style={{ color: "var(--t3)" }}>MAC 地址</label>
                  <input
                    value={tablet.mac}
                    onChange={(e) => updateTablet(mac, "mac", e.target.value)}
                    className={`${glassInput} ${inputFocus}`}
                    style={inputStyle}
                    placeholder="A0:DE:0F:45:2D:39"
                  />
                </div>
                <div>
                  <label className="text-xs block mb-1" style={{ color: "var(--t3)" }}>IP（可选）</label>
                  <input
                    value={tablet.ip}
                    onChange={(e) => updateTablet(mac, "ip", e.target.value)}
                    className={`${glassInput} ${inputFocus}`}
                    style={inputStyle}
                    placeholder="10.1.1.97"
                  />
                </div>
                <div>
                  <label className="text-xs block mb-1" style={{ color: "var(--t3)" }}>IPv6 规则 comment</label>
                  <input
                    value={tablet.ipv6_comment}
                    onChange={(e) => updateTablet(mac, "ipv6_comment", e.target.value)}
                    className={`${glassInput} ${inputFocus}`}
                    style={inputStyle}
                    placeholder="华为平板"
                  />
                </div>
              </div>
            </div>
          ))}
          {Object.keys(tablets).length === 0 && (
            <div className="text-sm text-center py-4" style={{ color: "var(--t3)" }}>
              暂无设备，点击"添加设备"开始
            </div>
          )}
        </div>
      </div>

      {/* 假期模式 */}
      <div className="rounded-2xl glass p-5">
        <h3 className="font-semibold mb-3" style={{ color: "var(--t1)" }}>🏖️ 假期模式</h3>
        <div className="flex gap-2">
          <button
            onClick={() => handleVacation(false)}
            className="flex-1 px-4 py-2 rounded-full text-sm font-medium transition-all duration-200 border cursor-pointer"
            style={
              settings.VACATION_MODE !== "true"
                ? { background: "rgba(59,130,246,0.2)", borderColor: "rgba(59,130,246,0.3)", color: "#60a5fa" }
                : { background: "rgba(255,255,255,0.03)", borderColor: "rgba(255,255,255,0.06)", color: "var(--t3)" }
            }
          >
            关闭（工作日/周末）
          </button>
          <button
            onClick={() => handleVacation(true)}
            className="flex-1 px-4 py-2 rounded-full text-sm font-medium transition-all duration-200 border cursor-pointer"
            style={
              settings.VACATION_MODE === "true"
                ? { background: "rgba(16,185,129,0.2)", borderColor: "rgba(16,185,129,0.3)", color: "#34d399" }
                : { background: "rgba(255,255,255,0.03)", borderColor: "rgba(255,255,255,0.06)", color: "var(--t3)" }
            }
          >
            开启（假期规则）
          </button>
        </div>
      </div>

      {/* 时间规则 */}
      <div className="rounded-2xl glass p-5">
        <h3 className="font-semibold mb-3" style={{ color: "var(--t1)" }}>⏰ 时间规则</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm" style={{ color: "var(--t2)" }}>每日额度（分钟）</label>
            <input
              type="number"
              value={settings.DAILY_LIMIT || "60"}
              onChange={(e) => update("DAILY_LIMIT", e.target.value)}
              className={glassInput}
              style={inputStyle}
            />
          </div>
        </div>
      </div>

      {/* DNS 采集间隔 */}
      <div className="rounded-2xl glass p-5">
        <h3 className="font-semibold mb-3" style={{ color: "var(--t1)" }}>📡 采集设置</h3>
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-3 items-center">
            <label className="text-sm" style={{ color: "var(--t2)" }}>DNS 采集间隔（秒）</label>
            <input
              type="number"
              value={settings.DNS_INTERVAL || "60"}
              onChange={(e) => update("DNS_INTERVAL", e.target.value)}
              className={glassInput}
              style={inputStyle}
            />
          </div>
        </div>
      </div>

      {/* 游戏 IP 管理 */}
      <GameIpManager />

      {/* 保存按钮 */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-6 py-2 rounded-full font-semibold text-sm transition-all duration-200 cursor-pointer disabled:opacity-50"
          style={{
            background: "linear-gradient(135deg, #8b5cf6, #6366f1)",
            boxShadow: "0 4px 15px rgba(99,102,241,0.3)",
          }}
        >
          {saving ? "保存中..." : "💾 保存设置"}
        </button>
        {msg && <span className="text-sm" style={{ color: "var(--t2)" }}>{msg}</span>}
      </div>
    </div>
  );
}
