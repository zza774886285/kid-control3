import { useEffect, useState, useCallback } from "react";
import {
  fetchPointsBalance,
  applyPoints,
  exchangePoints,
  fetchMyPoints,
  fetchPendingRequests,
  approveRequest,
  fetchPointsConfig,
} from "../api";
import type {
  PointsBalance,
  PointRequest,
  PointTransaction,
  PointsConfig,
} from "../types";

// ── 常量 ──────────────────────────────────────────────────────
const REQUEST_TYPES: { key: string; label: string; icon: string; color: string }[] = [
  { key: "tutoring", label: "补课", icon: "📚", color: "#8b5cf6" },
  { key: "homework", label: "作业", icon: "📝", color: "#3b82f6" },
  { key: "other", label: "其他", icon: "🎯", color: "#14b8a6" },
];

const EXCHANGE_OPTIONS: { points: number; label: string; color: string }[] = [
  { points: 30, label: "兑换 30min", color: "#f59e0b" },
  { points: 60, label: "兑换 60min", color: "#f97316" },
];

const STATUS_MAP: Record<string, { label: string; color: string; bg: string }> = {
  pending: { label: "待审批", color: "#eab308", bg: "bg-amber-500/20" },
  approved: { label: "已通过", color: "#22c55e", bg: "bg-emerald-500/20" },
  rejected: { label: "已拒绝", color: "#ef4444", bg: "bg-red-500/20" },
};

const REQUEST_TYPE_LABELS: Record<string, string> = {
  tutoring: "补课",
  homework: "作业",
  other: "其他",
};

const REQUEST_TYPE_COLORS: Record<string, string> = {
  tutoring: "#8b5cf6",
  homework: "#3b82f6",
  other: "#14b8a6",
};

const KID_COLORS: Record<string, string> = {
  lisa: "#8b5cf6",
  huawei: "#f97316",
};

// ── 工具函数 ──────────────────────────────────────────────────
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

// ── 组件 ──────────────────────────────────────────────────────
export default function PointsPage() {
  // State
  const [balances, setBalances] = useState<PointsBalance[]>([]);
  const [config, setConfig] = useState<PointsConfig>({ tutoring: 60, homework: 30, other: 30 });
  const [pendingRequests, setPendingRequests] = useState<PointRequest[]>([]);
  const [history, setHistory] = useState<PointTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [appliedType, setAppliedType] = useState<string | null>(null);

  // Current user: read from localStorage, default to first kid
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
      // Auto-select first kid if no user selected
      if (currentUserId === 0 && data.length > 0) {
        const firstKid = data.find((b) => b.username !== "admin");
        if (firstKid) {
          setCurrentUserId(firstKid.user_id);
          setCurrentRole(firstKid.username);
          localStorage.setItem("kc_current_user", String(firstKid.user_id));
          localStorage.setItem("kc_current_role", firstKid.username);
        }
      }
    } catch (e) {
      console.error("加载积分余额失败:", e);
    }
  }, [currentUserId]);

  const loadConfig = useCallback(async () => {
    try {
      const c = await fetchPointsConfig();
      setConfig(c);
    } catch (_) {
      // config endpoint may not exist yet, use defaults
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
    await Promise.all([loadBalances(), loadConfig(), loadPending()]);
    if (currentUserId) {
      await loadHistory(currentUserId);
    }
    setLoading(false);
  }

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (currentUserId && currentUserId > 0) {
      loadHistory(currentUserId);
    }
  }, [currentUserId, loadHistory]);

  // ── 切换角色 ─────────────────────────────────────────────
  function switchRole(userId: number, role: string) {
    setCurrentUserId(userId);
    setCurrentRole(role);
    localStorage.setItem("kc_current_user", String(userId));
    localStorage.setItem("kc_current_role", role);
    setAppliedType(null);
    loadHistory(userId);
  }

  // ── 操作 ─────────────────────────────────────────────────
  async function handleApply(requestType: string) {
    try {
      await applyPoints(currentUserId, requestType);
      setAppliedType(requestType);
      setTimeout(() => setAppliedType(null), 3000);
      loadPending();
    } catch (e) {
      console.error("申请失败:", e);
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
      if (action === "approve") {
        loadBalances();
      }
    } catch (e) {
      console.error("审批失败:", e);
    }
  }

  // ── 子组件 ─────────────────────────────────────────────
  const isAdmin = currentRole === "admin";
  const myBalance = balances.find((b) => b.user_id === currentUserId);

  // ── Loading ──────────────────────────────────────────────
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

  return (
    <div className="space-y-6">
      {/* ─── 标题栏 + 角色切换 ─── */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold tracking-tight">⭐ 积分系统</h2>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1 bg-white/[0.05] rounded-full p-1">
            {balances.map((b) => (
              <button
                key={b.user_id}
                onClick={() => switchRole(b.user_id, b.username)}
                className={`px-3 py-1 rounded-full text-xs font-medium transition-all duration-200 cursor-pointer ${
                  currentUserId === b.user_id
                    ? "bg-white/10 text-white"
                    : "text-[color:var(--color-text-muted)] hover:text-[color:var(--color-text-secondary)]"
                }`}
              >
                {b.display_name}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* ═══════════════ 孩子视图 ═══════════════ */}
      {!isAdmin && myBalance && (
        <ChildView
          balance={myBalance}
          config={config}
          history={history}
          pendingRequests={pendingRequests.filter((r) => r.user_id === currentUserId)}
          appliedType={appliedType}
          onApply={handleApply}
          onExchange={handleExchange}
        />
      )}

      {/* ═══════════════ 管理员视图 ═══════════════ */}
      {isAdmin && (
        <AdminView
          balances={balances}
          config={config}
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
// 孩子视图
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
interface ChildViewProps {
  balance: PointsBalance;
  config: PointsConfig;
  history: PointTransaction[];
  pendingRequests: PointRequest[];
  appliedType: string | null;
  onApply: (type: string) => void;
  onExchange: (points: number) => void;
}

function ChildView({ balance, config, history, pendingRequests, appliedType, onApply, onExchange }: ChildViewProps) {
  const pointsMap: Record<string, number> = { tutoring: config.tutoring, homework: config.homework, other: config.other };

  return (
    <div className="space-y-6">
      {/* ─── 积分余额大数字 ─── */}
      <div className="relative rounded-2xl p-6 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] text-center overflow-hidden">
        <div className="absolute top-0 left-6 right-6 h-[1px] rounded-full bg-gradient-to-r from-transparent via-purple-500/40 to-transparent" />
        <div className="text-sm text-[color:var(--color-text-muted)] mb-2">我的积分</div>
        <div className="text-5xl font-mono font-bold text-purple-400 tracking-tight" style={{ textShadow: "0 0 30px #8b5cf640" }}>
          {balance.balance}
        </div>
        <div className="text-xs text-[color:var(--color-text-muted)] mt-2">
          1分 = 1分钟屏幕时间
        </div>
      </div>

      {/* ─── 申请积分 ─── */}
      <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
        <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">📋 申请积分</h3>
        <div className="grid grid-cols-3 gap-3">
          {REQUEST_TYPES.map((rt) => {
            const pts = pointsMap[rt.key] || 0;
            const justApplied = appliedType === rt.key;
            return (
              <button
                key={rt.key}
                onClick={() => onApply(rt.key)}
                disabled={justApplied}
                className="relative flex flex-col items-center gap-2 p-4 rounded-xl border transition-all duration-200 cursor-pointer hover:scale-[1.02]"
                style={{
                  backgroundColor: justApplied ? `${rt.color}25` : `${rt.color}10`,
                  borderColor: justApplied ? `${rt.color}60` : `${rt.color}30`,
                }}
              >
                <span className="text-2xl">{rt.icon}</span>
                <span className="text-sm font-medium" style={{ color: rt.color }}>{rt.label}</span>
                <span className="text-xs px-2 py-0.5 rounded-full" style={{ backgroundColor: `${rt.color}20`, color: rt.color }}>
                  +{pts}分
                </span>
                {justApplied && (
                  <span className="absolute inset-0 flex items-center justify-center bg-black/40 rounded-xl text-xs text-emerald-400 font-medium">
                    ✅ 已申请，等待审批
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* ─── 兑换时间 ─── */}
      <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
        <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">⏱️ 兑换时间</h3>
        <div className="grid grid-cols-2 gap-3">
          {EXCHANGE_OPTIONS.map((ex) => {
            const canAfford = balance.balance >= ex.points;
            return (
              <button
                key={ex.points}
                onClick={() => canAfford && onExchange(ex.points)}
                disabled={!canAfford}
                className="flex flex-col items-center gap-2 p-4 rounded-xl border transition-all duration-200 cursor-pointer hover:scale-[1.02] disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:scale-100"
                style={{
                  backgroundColor: canAfford ? `${ex.color}15` : "rgba(255,255,255,0.03)",
                  borderColor: canAfford ? `${ex.color}30` : "rgba(255,255,255,0.06)",
                }}
              >
                <span className="text-2xl">⏱️</span>
                <span className="text-sm font-medium" style={{ color: canAfford ? ex.color : undefined }}>
                  {ex.label}
                </span>
                <span className="text-xs text-[color:var(--color-text-muted)]">-{ex.points}分</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* ─── 待审批 ─── */}
      {pendingRequests.length > 0 && (
        <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
          <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">⏳ 待审批</h3>
          <div className="space-y-2">
            {pendingRequests.map((r) => {
              const st = STATUS_MAP[r.status] || STATUS_MAP.pending;
              return (
                <div key={r.id} className="flex items-center justify-between py-2.5 border-b border-white/[0.04] last:border-0">
                  <div className="flex items-center gap-3">
                    <span className="text-sm" style={{ color: REQUEST_TYPE_COLORS[r.request_type] }}>
                      {REQUEST_TYPE_LABELS[r.request_type] || r.request_type}
                    </span>
                    <span className="text-sm font-mono text-purple-400">+{r.points}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-[color:var(--color-text-muted)]">{formatTimeAgo(r.created_at)}</span>
                    <span className={`text-[10px] px-2 py-0.5 rounded-full ${st.bg}`} style={{ color: st.color }}>
                      {st.label}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ─── 积分历史 ─── */}
      <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
        <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">📋 积分记录</h3>
        {history.length === 0 ? (
          <div className="text-sm text-[color:var(--color-text-muted)] py-4 text-center">暂无记录</div>
        ) : (
          <div className="space-y-2">
            {history.map((h) => (
              <div key={h.id} className="flex justify-between items-center text-sm py-2 border-b border-white/[0.04] last:border-0">
                <div className="flex items-center gap-2">
                  <span className={h.tx_type === "earn" ? "text-emerald-400" : "text-rose-400"}>
                    {h.tx_type === "earn" ? "+" : "-"}{h.points}
                  </span>
                  <span className="text-[color:var(--color-text-secondary)]">{h.description}</span>
                </div>
                <div className="text-xs text-[color:var(--color-text-muted)]">
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
// 管理员视图
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
interface AdminViewProps {
  balances: PointsBalance[];
  config: PointsConfig;
  pendingRequests: PointRequest[];
  balancesMap: Record<number, PointsBalance>;
  onApprove: (requestId: number, action: "approve" | "reject") => void;
  onSwitchUser: (userId: number, role: string) => void;
}

function AdminView({ balances, pendingRequests, balancesMap, onApprove, onSwitchUser }: AdminViewProps) {
  return (
    <div className="space-y-6">
      {/* ─── 所有孩子积分卡片 ─── */}
      <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
        <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">👧 孩子积分</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {balances.filter((b) => b.username !== "admin").map((b) => {
            const accent = getUserColor(b.username);
            return (
              <div
                key={b.user_id}
                className="relative rounded-xl p-4 border border-white/[0.06] cursor-pointer hover:bg-white/[0.04] transition-all duration-200"
                onClick={() => onSwitchUser(b.user_id, b.username)}
              >
                <div className="absolute top-0 left-4 right-4 h-[1px] rounded-full" style={{ background: `linear-gradient(90deg, transparent, ${accent}40, transparent)` }} />
                <div className="flex justify-between items-center">
                  <div className="flex items-center gap-2.5">
                    <div className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold" style={{ backgroundColor: `${accent}20`, color: accent }}>
                      {b.display_name[0]}
                    </div>
                    <div>
                      <div className="font-semibold text-[color:var(--color-text-primary)]">{b.display_name}</div>
                      <div className="text-[10px] text-[color:var(--color-text-muted)]">点击查看 →</div>
                    </div>
                  </div>
                  <div className="text-2xl font-mono font-bold" style={{ color: accent }}>{b.balance}</div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ─── 待审批列表 ─── */}
      <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
        <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">
          ⏳ 待审批
          {pendingRequests.length > 0 && (
            <span className="ml-2 text-xs px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-400 border border-amber-500/30">
              {pendingRequests.length}
            </span>
          )}
        </h3>
        {pendingRequests.length === 0 ? (
          <div className="text-sm text-[color:var(--color-text-muted)] py-4 text-center">无待审批请求</div>
        ) : (
          <div className="space-y-3">
            {pendingRequests.map((r) => {
              const user = balancesMap[r.user_id];
              const typeName = REQUEST_TYPE_LABELS[r.request_type] || r.request_type;
              const typeColor = REQUEST_TYPE_COLORS[r.request_type] || "#8b5cf6";
              return (
                <div key={r.id} className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.04]">
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold" style={{ backgroundColor: `${typeColor}20`, color: typeColor }}>
                      {typeName[0]}
                    </div>
                    <div>
                      <div className="text-sm font-medium text-[color:var(--color-text-primary)]">
                        {user?.display_name || "未知"} · {typeName}
                      </div>
                      <div className="text-xs text-[color:var(--color-text-muted)]">
                        +{r.points}分 · {formatTimeAgo(r.created_at)}
                      </div>
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => onApprove(r.id, "approve")}
                      className="px-3 py-1.5 rounded-full text-xs font-medium bg-emerald-500/20 border border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/30 transition-all duration-200 cursor-pointer"
                    >
                      ✅ 同意
                    </button>
                    <button
                      onClick={() => onApprove(r.id, "reject")}
                      className="px-3 py-1.5 rounded-full text-xs font-medium bg-red-500/20 border border-red-500/30 text-red-400 hover:bg-red-500/30 transition-all duration-200 cursor-pointer"
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

      {/* ─── 最近交易（管理员看全局） ─── */}
      <AdminRecentTransactions balancesMap={balancesMap} />
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
        // Fetch history for all kids
        const users = Object.values(balancesMap).filter((b) => b.username !== "admin");
        const allHistory: PointTransaction[] = [];
        for (const u of users) {
          const data = await fetchMyPoints(u.user_id);
          if (data.history) allHistory.push(...data.history);
        }
        // Sort by created_at desc, take latest 20
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
    <div className="rounded-2xl p-5 backdrop-blur-xl bg-white/[0.04] border border-white/[0.06]">
      <h3 className="font-semibold mb-4 text-[color:var(--color-text-primary)]">📊 最近交易</h3>
      {!loaded ? (
        <div className="flex items-center justify-center py-4">
          <div className="w-4 h-4 border-2 border-white/20 border-t-white/60 rounded-full animate-spin" />
        </div>
      ) : transactions.length === 0 ? (
        <div className="text-sm text-[color:var(--color-text-muted)] py-4 text-center">暂无交易</div>
      ) : (
        <div className="space-y-2">
          {transactions.map((t) => {
            const user = balancesMap[t.user_id];
            return (
              <div key={t.id} className="flex justify-between items-center text-sm py-2 border-b border-white/[0.04] last:border-0">
                <div className="flex items-center gap-2">
                  <span className={t.tx_type === "earn" ? "text-emerald-400" : "text-rose-400"}>
                    {t.tx_type === "earn" ? "+" : "-"}{t.points}
                  </span>
                  <span className="text-[color:var(--color-text-secondary)]">{t.description}</span>
                </div>
                <div className="flex items-center gap-2 text-xs text-[color:var(--color-text-muted)]">
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
