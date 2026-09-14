import { useEffect, useState } from "react";
import { fetchPointsBalance, earnPoints, exchangePoints, fetchMyPoints } from "../api";
import type { PointsBalance, PointTransaction } from "../types";

export default function PointsPage() {
  const [balances, setBalances] = useState<PointsBalance[]>([]);
  const [selectedUser, setSelectedUser] = useState<number | null>(null);
  const [history, setHistory] = useState<PointTransaction[]>([]);
  const [loading, setLoading] = useState(true);

  async function reload() {
    setLoading(true);
    try {
      const data = await fetchPointsBalance();
      setBalances(data);
      if (data.length > 0 && selectedUser === null) {
        setSelectedUser(data[0].user_id);
      }
    } catch (e) {
      console.error("加载积分失败:", e);
    }
    setLoading(false);
  }

  async function loadHistory(userId: number) {
    try {
      const data = await fetchMyPoints(userId);
      setHistory(data.history || []);
    } catch (e) {
      console.error("加载历史失败:", e);
    }
  }

  useEffect(() => { reload(); }, []);
  useEffect(() => { if (selectedUser) loadHistory(selectedUser); }, [selectedUser]);

  async function handleEarn(userId: number, type: string, points: number) {
    await earnPoints(userId, type, points);
    reload();
  }

  async function handleExchange(userId: number, points: number) {
    await exchangePoints(userId, points);
    reload();
    if (selectedUser) loadHistory(selectedUser);
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

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-bold tracking-tight">⭐ 积分系统</h2>

      {/* 积分余额卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {balances.map((b) => (
          <div
            key={b.user_id}
            className={`relative rounded-2xl p-5 backdrop-blur-xl border transition-all duration-300 cursor-pointer ${
              selectedUser === b.user_id
                ? "bg-white/[0.06] border-purple-500/30"
                : "bg-white/[0.04] border-white/[0.06] hover:bg-white/[0.06]"
            }`}
            onClick={() => setSelectedUser(b.user_id)}
          >
            {/* 顶部发光条 */}
            <div className="absolute top-0 left-6 right-6 h-[1px] rounded-full bg-gradient-to-r from-transparent via-purple-500/40 to-transparent" />

            <div className="flex justify-between items-center mb-1">
              <span className="font-semibold text-[color:var(--color-text-primary)]">{b.display_name}</span>
              <span className="text-2xl font-mono font-bold text-purple-400">{b.balance}</span>
            </div>
            <div className="text-xs text-[color:var(--color-text-muted)] mb-3">积分余额</div>

            {/* 快捷操作 */}
            <div className="flex flex-wrap gap-2">
              <button
                onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "homework", 30); }}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-blue-500/15 hover:border-blue-500/30 hover:text-blue-400 transition-all duration-200 cursor-pointer"
              >
                📝 作业 +30
              </button>
              <button
                onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "tutoring", 60); }}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-purple-500/15 hover:border-purple-500/30 hover:text-purple-400 transition-all duration-200 cursor-pointer"
              >
                📚 补课 +60
              </button>
              <button
                onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "other", 30); }}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-teal-500/15 hover:border-teal-500/30 hover:text-teal-400 transition-all duration-200 cursor-pointer"
              >
                🎯 其他 +30
              </button>
            </div>

            {/* 兑换按钮 */}
            <div className="flex gap-2 mt-2">
              <button
                onClick={(e) => { e.stopPropagation(); handleExchange(b.user_id, 30); }}
                disabled={b.balance < 30}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-orange-500/15 hover:border-orange-500/30 hover:text-orange-400 disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200 cursor-pointer"
              >
                ⏱️ 兑换 30min
              </button>
              <button
                onClick={(e) => { e.stopPropagation(); handleExchange(b.user_id, 60); }}
                disabled={b.balance < 60}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-[color:var(--color-text-secondary)] hover:bg-orange-500/15 hover:border-orange-500/30 hover:text-orange-400 disabled:opacity-30 disabled:cursor-not-allowed transition-all duration-200 cursor-pointer"
              >
                ⏱️ 兑换 60min
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* 积分记录 */}
      {selectedUser && (
        <div className="rounded-2xl backdrop-blur-xl bg-white/[0.04] border border-white/[0.06] p-5">
          <h3 className="font-semibold mb-3 text-[color:var(--color-text-primary)]">📋 积分记录</h3>
          {history.length === 0 ? (
            <div className="text-sm text-[color:var(--color-text-muted)] py-4 text-center">暂无记录</div>
          ) : (
            <div className="space-y-2">
              {history.map((h) => (
                <div
                  key={h.id}
                  className="flex justify-between items-center text-sm py-2 border-b border-white/[0.04] last:border-0"
                >
                  <div className="flex items-center gap-2">
                    <span className={h.tx_type === "earn" ? "text-emerald-400" : "text-rose-400"}>
                      {h.tx_type === "earn" ? "+" : "-"}{h.points}
                    </span>
                    <span className="text-[color:var(--color-text-secondary)]">{h.description}</span>
                  </div>
                  <div className="text-xs text-[color:var(--color-text-muted)]">
                    余额 {h.balance_after} · {h.created_at}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
