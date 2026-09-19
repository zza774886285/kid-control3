import { useEffect, useState, useCallback } from "react";
import {
  fetchPointsBalance,
  applyPoints,
  exchangePoints,
  fetchMyPoints,
  fetchPendingRequests,
  approveRequest,
  setPoints,
} from "../api";
import type {
  PointsBalance,
  PointRequest,
  PointTransaction,
  PointsConfig,
} from "../types";

// ── 常量 ──────────────────────────────────────────────────────
const REQUEST_TYPES: { key: string; label: string; icon: string; color: string }[] = [
  { key: "tutoring", label: "补课", icon: "📚", color: "#22c55e" },
  { key: "homework", label: "作业", icon: "📝", color: "#3b82f6" },
  { key: "other", label: "其他", icon: "🎯", color: "#f59e0b" },
];

const EXCHANGE_OPTIONS: { points: number; label: string; icon: string; color: string }[] = [
  { points: 30, label: "兑换 30min", icon: "🎁", color: "#f59e0b" },
  { points: 60, label: "兑换 60min", icon: "🏆", color: "#f97316" },
];

const STATUS_MAP: Record<string, { label: string; color: string }> = {
  pending:  { label: "待审批", color: "#eab308" },
  approved: { label: "已通过", color: "#22c55e" },
  rejected: { label: "已拒绝", color: "#ef4444" },
};

const REQUEST_TYPE_LABELS: Record<string, string> = { tutoring: "补课", homework: "作业", other: "其他" };
const REQUEST_TYPE_COLORS: Record<string, string> = { tutoring: "#8b5cf6", homework: "#3b82f6", other: "#14b8a6" };

const KID_COLORS: Record<string, string> = { lisa: "#8b5cf6", huawei: "#f97316" };

function getUserColor(username: string): string {
  for (const [key, color] of Object.entries(KID_COLORS)) {
    if (username.includes(key)) return color;
  }
  return "#8b5cf6";
}

function formatTimeAgo(dateStr: string): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  const now = new Date();
  const diff = Math.floor((now.getTime() - d.getTime()) / 1000);
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)}分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}小时前`;
  return `${Math.floor(diff / 86400)}天前`;
}

// ── 主组件 ──────────────────────────────────────────────────────
export default function PointsPage({ hideAdmin = false }: { hideAdmin?: boolean } = {}) {
  const [balances, setBalances] = useState<PointsBalance[]>([]);
  const [config, setConfig] = useState<PointsConfig>({ tutoring: 30, homework: 30, other: 30 });
  const [pendingRequests, setPendingRequests] = useState<PointRequest[]>([]);
  const [history, setHistory] = useState<PointTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [appliedType, setAppliedType] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);

  const [currentUserId, setCurrentUserId] = useState<number>(() => {
    const saved = localStorage.getItem("kc_current_user");
    return saved ? Number(saved) : 0;
  });
  const [currentRole, setCurrentRole] = useState<string>(() => {
    return localStorage.getItem("kc_current_role") || "admin";
  });

  // ── 数据加载 ───────────────────────────────────────────────
  const loadBalances = useCallback(async () => {
    try {
      const data = await fetchPointsBalance();
      setBalances(data);
      // 仅在无 localStorage 记录时初始化，不覆盖已选角色
      if (currentUserId === 0 && data.length > 0) {
        const saved = localStorage.getItem("kc_current_user");
        if (saved) {
          setCurrentUserId(Number(saved));
        }
        // 不自动选孩子，保持默认 admin 角色
      }
    } catch (e) {
      console.error("加载积分余额失败:", e);
    }
  }, [currentUserId]);

  const loadConfig = useCallback(async (userId?: number) => {
    try {
      const url = userId ? `/api/points/config?user_id=${userId}` : `/api/points/config`;
      const resp = await fetch(url).then(r => r.json());
      setConfig(resp.config || resp);
    } catch (e) {
      console.error("加载积分配置失败:", e);
    }
  }, []);

  const loadHistory = useCallback(async (userId: number) => {
    try {
      const data = await fetchMyPoints(userId);
      setHistory(data.history || []);
    } catch (e) {
      console.error("加载积分历史失败:", e);
    }
  }, []);

  const loadPending = useCallback(async () => {
    try {
      const data = await fetchPendingRequests();
      setPendingRequests(data);
    } catch (e) {
      console.error("加载待审批失败:", e);
    }
  }, []);

  async function reload() {
    setLoading(true);
    await loadBalances();
    await loadConfig(currentUserId);
    await loadPending();
    if (currentUserId) await loadHistory(currentUserId);
    setLoading(false);
  }

  useEffect(() => { reload(); }, []);

  useEffect(() => {
    const timer = setInterval(() => {
      loadBalances();
      loadPending();
      if (currentUserId && currentUserId > 0) loadHistory(currentUserId);
    }, 10000);
    return () => clearInterval(timer);
  }, [currentUserId, loadBalances, loadPending, loadHistory]);

  useEffect(() => {
    if (currentUserId && currentUserId > 0) loadHistory(currentUserId);
  }, [currentUserId, loadHistory]);

  function switchRole(userId: number, role: string) {
    setCurrentUserId(userId);
    setCurrentRole(role);
    localStorage.setItem("kc_current_user", String(userId));
    localStorage.setItem("kc_current_role", role);
    setAppliedType(null);
    loadConfig(userId);
    loadHistory(userId);
  }

  async function handleApply(requestType: string) {
    try {
      const res = await applyPoints(currentUserId, requestType);
      if (res.ok === false) {
        setApplyError(res.error || "申请失败");
        setTimeout(() => setApplyError(null), 5000);
        return;
      }
      setAppliedType(requestType);
      setTimeout(() => setAppliedType(null), 3000);
      loadPending();
    } catch (e: any) {
      setApplyError(e.message || "申请失败");
      setTimeout(() => setApplyError(null), 5000);
    }
  }

  async function handleExchange(points: number) {
    try {
      await exchangePoints(currentUserId, points);
      await reload();
    } catch (e) {
      console.error("兑换失败:", e);
    }
  }

  async function handleApprove(requestId: number, action: "approve" | "reject") {
    try {
      await approveRequest(requestId, action);
      loadPending();
      if (action === "approve") loadBalances();
    } catch (e) {
      console.error("审批失败:", e);
    }
  }

  const isAdmin = !hideAdmin && currentRole === "admin";
  const myBalance = balances.find((b) => b.user_id === currentUserId);

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
    <div className="space-y-6">
      {/* ─── 标题栏 + 角色切换 ─── */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold tracking-tight" style={{ color: hideAdmin ? "var(--t1)" : "var(--t1)" }}>
          {hideAdmin ? "⭐ 我的积分" : "⭐ 积分系统"}
        </h2>
        {balances.filter((b) => b.username !== "admin").length > 1 && (
          <div className="flex gap-3 p-2 rounded-2xl" style={{ background: "rgba(255,255,255,0.05)" }}>
            {balances.filter((b) => b.username !== "admin").map((b) => (
              <button
                key={b.user_id}
                onClick={() => switchRole(b.user_id, b.username)}
                className="flex-1 px-6 py-3 rounded-xl text-base font-bold transition-all duration-300 cursor-pointer"
                style={
                  currentUserId === b.user_id
                    ? {
                        background: "linear-gradient(135deg, #8b5cf6, #6366f1)",
                        color: "#fff",
                        boxShadow: "0 4px 16px rgba(99,102,241,0.4)",
                        transform: "scale(1.05)",
                      }
                    : {
                        background: "rgba(255,255,255,0.06)",
                        border: "1px solid rgba(255,255,255,0.1)",
                        color: "var(--t2)",
                      }
                }
              >
                {b.display_name}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* ═══════════════ 孩子视图 ═══════════════ */}
      {!isAdmin && myBalance && (
        <ChildView
          balance={myBalance}
          config={config}
          history={history}
          pendingRequests={pendingRequests.filter((r) => r.user_id === currentUserId)}
          appliedType={appliedType}
          applyError={applyError}
          onApply={handleApply}
          onExchange={handleExchange}
        />
      )}

      {/* ═══════════════ 管理员视图 ═══════════════ */}
      {isAdmin && (
        <AdminView
          balances={balances}
          pendingRequests={pendingRequests}
          balancesMap={Object.fromEntries(balances.map((b) => [b.user_id, b]))}
          onApprove={handleApprove}
          onSwitchUser={switchRole}
        />
      )}
    </div>
  );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 孩子视图 — 儿童友好风格 🌈
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
interface ChildViewProps {
  balance: PointsBalance;
  config: PointsConfig;
  history: PointTransaction[];
  pendingRequests: PointRequest[];
  appliedType: string | null;
  applyError: string | null;
  onApply: (requestType: string) => void;
  onExchange: (points: number) => void;
}

function ChildView({ balance, config, history, pendingRequests, appliedType, applyError, onApply, onExchange }: ChildViewProps) {
  const pointsMap: Record<string, number> = { tutoring: config.tutoring, homework: config.homework, other: config.other };

  // 儿童风申请按钮颜色
  const kidBtnColors: Record<string, { bg: string; border: string; text: string }> = {
    tutoring: { bg: "linear-gradient(135deg, #86efac, #22c55e)", border: "#16a34a", text: "#fff" },
    homework: { bg: "linear-gradient(135deg, #93c5fd, #3b82f6)", border: "#2563eb", text: "#fff" },
    other:    { bg: "linear-gradient(135deg, #fcd34d, #f59e0b)", border: "#d97706", text: "#fff" },
  };

  return (
    <div className="space-y-6">
      {/* ─── 积分余额大数字（可爱风） ─── */}
      <div
        className="relative rounded-3xl p-8 text-center overflow-hidden"
        style={{
          background: "linear-gradient(135deg, #fef3c7, #fde68a, #fcd34d)",
          boxShadow: "0 8px 32px rgba(245,158,11,0.3)",
        }}
      >
        {/* 装饰星星 */}
        <div className="absolute top-3 left-4 text-xl" style={{ animation: "float 3s ease-in-out infinite", opacity: 0.7 }}>⭐</div>
        <div className="absolute top-5 right-6 text-lg" style={{ animation: "float 3s ease-in-out infinite 0.5s", opacity: 0.6 }}>✨</div>
        <div className="absolute bottom-4 left-8 text-sm" style={{ animation: "float 3s ease-in-out infinite 1s", opacity: 0.5 }}>🌟</div>

        <div className="relative z-10">
          <div className="text-sm mb-2" style={{ color: "#92400e" }}>👑 我的积分</div>
          <div
            className="text-6xl font-mono font-bold tracking-tight"
            style={{ color: "#92400e", textShadow: "0 2px 8px rgba(245,158,11,0.4)" }}
          >
            {balance.balance}
          </div>
          <div className="text-xs mt-2" style={{ color: "#a16207" }}>
            ⭐ 1分 = 1分钟屏幕时间
          </div>
        </div>
      </div>

      {/* ─── 申请积分（彩色大按钮） ─── */}
      <div
        className="rounded-3xl p-6"
        style={{ background: "rgba(255,255,255,0.7)", boxShadow: "0 4px 20px rgba(0,0,0,0.05)" }}
      >
        <h3 className="font-bold text-lg mb-4 text-center" style={{ color: "#1e293b" }}>📋 申请积分</h3>
        <div className="grid grid-cols-3 gap-4">
          {REQUEST_TYPES.map((rt) => {
            const pts = pointsMap[rt.key] || 0;
            const justApplied = appliedType === rt.key;
            const colors = kidBtnColors[rt.key];
            return (
              <button
                key={rt.key}
                onClick={() => onApply(rt.key)}
                disabled={justApplied}
                className="relative flex flex-col items-center gap-2 p-5 rounded-2xl border-2 transition-all duration-300 cursor-pointer hover:scale-105 active:scale-95 disabled:opacity-60"
                style={{
                  background: justApplied ? `${colors.border}20` : "rgba(255,255,255,0.9)",
                  borderColor: justApplied ? colors.border : `${colors.border}40`,
                  boxShadow: justApplied ? `0 4px 20px ${colors.border}30` : "0 2px 8px rgba(0,0,0,0.06)",
                }}
              >
                <span className="text-3xl">{rt.icon}</span>
                <span className="text-base font-bold" style={{ color: colors.border }}>{rt.label}</span>
                <span
                  className="text-sm font-bold px-3 py-1 rounded-full"
                  style={{ background: colors.bg, color: colors.text }}
                >
                  +{pts}分
                </span>
                {justApplied && (
                  <div
                    className="absolute inset-0 flex items-center justify-center rounded-2xl"
                    style={{ background: "rgba(0,0,0,0.3)" }}
                  >
                    <span className="text-sm font-bold text-white animate-bounce">✅ 已提交！</span>
                  </div>
                )}
              </button>
            );
          })}
        </div>
        {applyError && (
          <div className="mt-3 text-sm text-center font-medium" style={{ color: "#ef4444" }}>
            ❌ {applyError}
          </div>
        )}
      </div>

      {/* ─── 兑换时间（金色渐变） ─── */}
      <div
        className="rounded-3xl p-6"
        style={{ background: "rgba(255,255,255,0.7)", boxShadow: "0 4px 20px rgba(0,0,0,0.05)" }}
      >
        <h3 className="font-bold text-lg mb-4 text-center" style={{ color: "#1e293b" }}>🎁 兑换时间</h3>
        <div className="grid grid-cols-2 gap-4">
          {EXCHANGE_OPTIONS.map((ex) => {
            const canAfford = balance.balance >= ex.points;
            return (
              <button
                key={ex.points}
                onClick={() => canAfford && onExchange(ex.points)}
                disabled={!canAfford}
                className="flex flex-col items-center gap-2 p-5 rounded-2xl border-2 transition-all duration-300 cursor-pointer hover:scale-105 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:scale-100"
                style={{
                  background: canAfford
                    ? "linear-gradient(135deg, #fef3c7, #fde68a)"
                    : "rgba(255,255,255,0.5)",
                  borderColor: canAfford ? "#d97706" : "#e5e7eb",
                  boxShadow: canAfford ? "0 4px 20px rgba(217,119,6,0.2)" : "none",
                }}
              >
                <span className="text-3xl">{ex.icon}</span>
                <span className="text-base font-bold" style={{ color: canAfford ? "#92400e" : "#9ca3af" }}>
                  {ex.label}
                </span>
                <span className="text-sm font-medium" style={{ color: canAfford ? "#b45309" : "#d1d5db" }}>
                  -{ex.points}分
                </span>
              </button>
            );
          })}
        </div>
      </div>

      {/* ─── 待审批（可爱动画） ─── */}
      {pendingRequests.length > 0 && (
        <div
          className="rounded-3xl p-6"
          style={{ background: "rgba(255,255,255,0.7)", boxShadow: "0 4px 20px rgba(0,0,0,0.05)" }}
        >
          <h3 className="font-bold text-lg mb-4 text-center" style={{ color: "#1e293b" }}>⏳ 等待审批</h3>
          <div className="space-y-3">
            {pendingRequests.map((r) => {
              const st = STATUS_MAP[r.status] || STATUS_MAP.pending;
              return (
                <div
                  key={r.id}
                  className="flex items-center justify-between py-3 px-4 rounded-xl"
                  style={{ background: "rgba(255,255,255,0.8)", border: "1px solid #e5e7eb" }}
                >
                  <div className="flex items-center gap-3">
                    <span className="text-lg">
                      {REQUEST_TYPES.find((t) => t.key === r.request_type)?.icon || "📋"}
                    </span>
                    <div>
                      <span className="text-sm font-medium" style={{ color: "#1e293b" }}>
                        {REQUEST_TYPE_LABELS[r.request_type] || r.request_type}
                      </span>
                      <span className="text-sm font-bold ml-2" style={{ color: "#8b5cf6" }}>+{r.points}分</span>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs" style={{ color: "#9ca3af" }}>{formatTimeAgo(r.created_at)}</span>
                    {r.status === "pending" ? (
                      <span
                        className="text-xs px-2 py-1 rounded-full font-medium"
                        style={{
                          background: "#fef3c7",
                          color: "#d97706",
                          animation: "pulse-glow 2s ease-in-out infinite",
                        }}
                      >
                        ⏳ 等待中...
                      </span>
                    ) : (
                      <span
                        className="text-xs px-2 py-1 rounded-full font-medium"
                        style={{
                          background: st.color === "#22c55e" ? "#dcfce7" : "#fee2e2",
                          color: st.color,
                        }}
                      >
                        {st.label}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ─── 积分记录 ─── */}
      <div
        className="rounded-3xl p-6"
        style={{ background: "rgba(255,255,255,0.7)", boxShadow: "0 4px 20px rgba(0,0,0,0.05)" }}
      >
        <h3 className="font-bold text-lg mb-4 text-center" style={{ color: "#1e293b" }}>📋 积分记录</h3>
        {history.length === 0 ? (
          <div className="text-sm py-4 text-center" style={{ color: "#9ca3af" }}>暂无记录</div>
        ) : (
          <div className="space-y-2">
            {history.map((h) => (
              <div
                key={h.id}
                className="flex justify-between items-center text-sm py-2"
                style={{ borderBottom: "1px solid #f3f4f6" }}
              >
                <div className="flex items-center gap-2">
                  <span style={{ color: h.tx_type === "earn" ? "#16a34a" : "#ef4444" }}>
                    {h.tx_type === "earn" ? "+" : "-"}{h.points}
                  </span>
                  <span style={{ color: "#4b5563" }}>{h.description}</span>
                </div>
                <div className="text-xs" style={{ color: "#9ca3af" }}>
                  余额 {h.balance_after} · {formatTimeAgo(h.created_at)}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 管理员视图 — 液态玻璃风格
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
interface AdminViewProps {
  balances: PointsBalance[];
  pendingRequests: PointRequest[];
  balancesMap: Record<number, PointsBalance>;
  onApprove: (requestId: number, action: "approve" | "reject") => void;
  onSwitchUser: (userId: number, role: string) => void;
}

function AdminView({ balances, pendingRequests, balancesMap, onApprove, onSwitchUser }: AdminViewProps) {
  return (
    <div className="space-y-6">
      {/* ─── 所有孩子积分卡片 ─── */}
      <div className="rounded-2xl glass p-5">
        <h3 className="font-semibold mb-4" style={{ color: "var(--t1)" }}>👧 孩子积分</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {balances.filter((b) => b.username !== "admin").map((b) => {
            const accent = getUserColor(b.username);
            return (
              <div
                key={b.user_id}
                className="relative rounded-xl p-4 cursor-pointer transition-all duration-200 hover:scale-[1.02]"
                style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }}
                onClick={() => onSwitchUser(b.user_id, b.username)}
              >
                <div
                  className="absolute top-0 left-4 right-4 h-[1px] rounded-full"
                  style={{ background: `linear-gradient(90deg, transparent, ${accent}40, transparent)` }}
                />
                <div className="flex justify-between items-center">
                  <div className="flex items-center gap-2.5">
                    <div
                      className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold"
                      style={{ background: `${accent}20`, color: accent }}
                    >
                      {b.display_name[0]}
                    </div>
                    <div>
                      <div className="font-semibold" style={{ color: "var(--t1)" }}>{b.display_name}</div>
                      <div className="text-[10px]" style={{ color: "var(--t3)" }}>点击查看 →</div>
                    </div>
                  </div>
                  <div className="text-2xl font-mono font-bold" style={{ color: accent }}>{b.balance}</div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ─── 直接调整积分 ─── */}
      <AdminAdjustPanel balances={balances} />

      {/* ─── 积分配置 ─── */}
      <AdminPointsConfigEditor balances={balances} />

      {/* ─── 待审批列表 ─── */}
      <div className="rounded-2xl glass p-5">
        <h3 className="font-semibold mb-4" style={{ color: "var(--t1)" }}>
          ⏳ 待审批
          {pendingRequests.length > 0 && (
            <span
              className="ml-2 text-xs px-2 py-0.5 rounded-full"
              style={{ background: "rgba(234,179,8,0.2)", color: "#eab308" }}
            >
              {pendingRequests.length}
            </span>
          )}
        </h3>
        {pendingRequests.length === 0 ? (
          <div className="text-sm py-4 text-center" style={{ color: "var(--t3)" }}>无待审批请求</div>
        ) : (
          <div className="space-y-3">
            {pendingRequests.map((r) => {
              const user = balancesMap[r.user_id];
              const typeName = REQUEST_TYPE_LABELS[r.request_type] || r.request_type;
              const typeColor = REQUEST_TYPE_COLORS[r.request_type] || "#8b5cf6";
              return (
                <div
                  key={r.id}
                  className="flex items-center justify-between p-3 rounded-xl"
                  style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.04)" }}
                >
                  <div className="flex items-center gap-3">
                    <div
                      className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold"
                      style={{ background: `${typeColor}20`, color: typeColor }}
                    >
                      {typeName[0]}
                    </div>
                    <div>
                      <div className="text-sm font-medium" style={{ color: "var(--t1)" }}>
                        {user?.display_name || "未知"} · {typeName}
                      </div>
                      <div className="text-xs" style={{ color: "var(--t3)" }}>
                        +{r.points}分 · {formatTimeAgo(r.created_at)}
                      </div>
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => onApprove(r.id, "approve")}
                      className="px-3 py-1.5 rounded-full text-xs font-medium cursor-pointer transition-all duration-200"
                      style={{ background: "rgba(16,185,129,0.2)", border: "1px solid rgba(16,185,129,0.3)", color: "#34d399" }}
                    >
                      ✅ 同意
                    </button>
                    <button
                      onClick={() => onApprove(r.id, "reject")}
                      className="px-3 py-1.5 rounded-full text-xs font-medium cursor-pointer transition-all duration-200"
                      style={{ background: "rgba(239,68,68,0.2)", border: "1px solid rgba(239,68,68,0.3)", color: "#f87171" }}
                    >
                      ❌ 拒绝
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* ─── 最近交易 ─── */}
      <AdminRecentTransactions balancesMap={balancesMap} />
    </div>
  );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 管理员 - 每用户积分配置
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
function AdminPointsConfigEditor({ balances }: { balances: PointsBalance[] }) {
  const kids = balances.filter((b) => b.username !== "admin");
  const [selected, setSelected] = useState<number>(0);
  const [cfg, setCfg] = useState({ tutoring: 30, homework: 30, other: 30 });
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState("");

  useEffect(() => {
    if (kids.length > 0 && selected === 0) setSelected(kids[0].user_id);
  }, [kids, selected]);

  useEffect(() => {
    if (selected > 0) {
      fetch(`/api/points/config?user_id=${selected}`)
        .then((r) => r.json())
        .then((d) => setCfg(d.config || d))
        .catch(console.error);
    }
  }, [selected]);

  async function save() {
    setSaving(true);
    setMsg("");
    try {
      const resp = await fetch("/api/points/config", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ user_id: selected, ...cfg }),
      }).then((r) => r.json());
      if (resp.ok) setMsg("✅ 已保存");
      else setMsg("❌ " + (resp.error || "保存失败"));
    } catch {
      setMsg("❌ 网络错误");
    }
    setSaving(false);
  }

  const LABELS: Record<string, string> = { tutoring: "📚 补课", homework: "📝 作业", other: "🎯 其他" };

  return (
    <div className="rounded-2xl glass p-5">
      <h3 className="font-semibold mb-4" style={{ color: "var(--t1)" }}>⚙️ 积分配置（按孩子）</h3>
      <div className="flex gap-2 mb-4">
        {kids.map((k) => (
          <button
            key={k.user_id}
            onClick={() => setSelected(k.user_id)}
            className="px-3 py-1.5 rounded-full text-sm font-medium cursor-pointer transition-all"
            style={{
              background: selected === k.user_id ? "rgba(99,102,241,0.2)" : "rgba(255,255,255,0.03)",
              border: `1px solid ${selected === k.user_id ? "rgba(99,102,241,0.4)" : "rgba(255,255,255,0.06)"}`,
              color: selected === k.user_id ? "#818cf8" : "var(--t2)",
            }}
          >
            {k.display_name}
          </button>
        ))}
      </div>
      {selected > 0 && (
        <div className="space-y-3">
          {(["tutoring", "homework", "other"] as const).map((key) => (
            <div key={key} className="flex items-center gap-3">
              <span className="w-20 text-sm" style={{ color: "var(--t2)" }}>{LABELS[key]}</span>
              <input
                type="number"
                min={0}
                max={999}
                value={cfg[key]}
                onChange={(e) => setCfg((p) => ({ ...p, [key]: Number(e.target.value) || 0 }))}
                className="w-20 px-3 py-1.5 rounded-lg text-sm font-mono"
                style={{ background: "rgba(255,255,255,0.05)", border: "1px solid rgba(255,255,255,0.1)", color: "var(--t1)" }}
              />
              <span className="text-xs" style={{ color: "var(--t3)" }}>分/次</span>
            </div>
          ))}
          <div className="flex items-center gap-3 pt-2">
            <button
              onClick={save}
              disabled={saving}
              className="px-4 py-1.5 rounded-full text-sm font-medium cursor-pointer transition-all"
              style={{ background: "rgba(99,102,241,0.2)", border: "1px solid rgba(99,102,241,0.4)", color: "#818cf8" }}
            >
              {saving ? "保存中..." : "保存配置"}
            </button>
            {msg && <span className="text-sm" style={{ color: msg.startsWith("✅") ? "#16a34a" : "#ef4444" }}>{msg}</span>}
          </div>
        </div>
      )}
    </div>
  );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 管理员 - 最近交易
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
function AdminRecentTransactions({ balancesMap }: { balancesMap: Record<number, PointsBalance> }) {
  const [transactions, setTransactions] = useState<PointTransaction[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    async function load() {
      try {
        const users = Object.values(balancesMap).filter((b) => b.username !== "admin");
        const allHistory: PointTransaction[] = [];
        for (const u of users) {
          const data = await fetchMyPoints(u.user_id);
          if (data.history) allHistory.push(...data.history);
        }
        allHistory.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
        setTransactions(allHistory.slice(0, 20));
      } catch (e) {
        console.error("加载交易记录失败:", e);
      }
      setLoaded(true);
    }
    load();
  }, [balancesMap]);

  return (
    <div className="rounded-2xl glass p-5">
      <h3 className="font-semibold mb-4" style={{ color: "var(--t1)" }}>📊 最近交易</h3>
      {!loaded ? (
        <div className="flex items-center justify-center py-4">
          <div
            className="w-4 h-4 border-2 rounded-full animate-spin"
            style={{ borderColor: "rgba(255,255,255,0.2)", borderTopColor: "rgba(255,255,255,0.6)" }}
          />
        </div>
      ) : transactions.length === 0 ? (
        <div className="text-sm py-4 text-center" style={{ color: "var(--t3)" }}>暂无交易</div>
      ) : (
        <div className="space-y-2">
          {transactions.map((t) => {
            const user = balancesMap[t.user_id];
            return (
              <div
                key={t.id}
                className="flex justify-between items-center text-sm py-2"
                style={{ borderBottom: "1px solid rgba(255,255,255,0.04)" }}
              >
                <div className="flex items-center gap-2">
                  <span style={{ color: t.tx_type === "earn" ? "#34d399" : "#f87171" }}>
                    {t.tx_type === "earn" ? "+" : "-"}{t.points}
                  </span>
                  <span style={{ color: "var(--t2)" }}>{t.description}</span>
                </div>
                <div className="flex items-center gap-2 text-xs" style={{ color: "var(--t3)" }}>
                  <span>{user?.display_name || "?"}</span>
                  <span>·</span>
                  <span>{formatTimeAgo(t.created_at)}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 管理员 - 直接调整积分
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
function AdminAdjustPanel({ balances }: { balances: PointsBalance[] }) {
  const [userId, setUserId] = useState<number>(0);
  const [points, setPointsVal] = useState<string>("");
  const [desc, setDesc] = useState("");
  const [msg, setMsg] = useState<{ type: "ok" | "err"; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const kids = balances.filter((b) => b.username !== "admin");

  const handleSubmit = async () => {
    if (!userId || !points) return;
    const p = parseInt(points, 10);
    if (isNaN(p) || p === 0) {
      setMsg({ type: "err", text: "请输入非零整数" });
      return;
    }
    setSaving(true);
    setMsg(null);
    try {
      const res = await setPoints(userId, p, desc || undefined);
      if (res.ok) {
        setMsg({ type: "ok", text: `成功：当前 ${res.new_balance} 分` });
        setPointsVal("");
        setDesc("");
      } else {
        setMsg({ type: "err", text: res.error || "操作失败" });
      }
    } catch (e: any) {
      setMsg({ type: "err", text: e.message || "请求失败" });
    }
    setSaving(false);
  };

  const glassInput = {
    background: "rgba(255,255,255,0.06)",
    border: "1px solid rgba(255,255,255,0.08)",
    color: "var(--t1)",
  };

  return (
    <div className="rounded-2xl glass p-5">
      <h3 className="font-semibold mb-4" style={{ color: "var(--t1)" }}>✏️ 直接调整积分</h3>
      <div className="flex flex-wrap gap-3 items-end">
        <div className="flex flex-col gap-1">
          <label className="text-xs" style={{ color: "var(--t3)" }}>用户</label>
          <select
            className="px-3 py-1.5 rounded-lg text-sm"
            style={glassInput}
            value={userId}
            onChange={(e) => setUserId(Number(e.target.value))}
          >
            <option value={0}>选择用户</option>
            {kids.map((k) => (
              <option key={k.user_id} value={k.user_id}>{k.display_name} ({k.balance}分)</option>
            ))}
          </select>
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs" style={{ color: "var(--t3)" }}>积分（正加负减）</label>
          <input
            type="number"
            className="w-28 px-3 py-1.5 rounded-lg text-sm"
            style={glassInput}
            placeholder="+10 / -5"
            value={points}
            onChange={(e) => setPointsVal(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs" style={{ color: "var(--t3)" }}>备注</label>
          <input
            className="w-40 px-3 py-1.5 rounded-lg text-sm"
            style={glassInput}
            placeholder="可选"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
        </div>
        <button
          onClick={handleSubmit}
          disabled={saving || !userId || !points}
          className="px-4 py-1.5 rounded-lg text-sm font-medium cursor-pointer transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed"
          style={{ background: "var(--acc-g2)", border: "1px solid var(--acc-g)", color: "var(--acc)" }}
        >
          {saving ? "处理中..." : "确认"}
        </button>
      </div>
      {msg && (
        <div className="mt-3 text-sm" style={{ color: msg.type === "ok" ? "var(--ok)" : "var(--err)" }}>
          {msg.text}
        </div>
      )}
    </div>
  );
}
