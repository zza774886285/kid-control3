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

  if (loading) return <div className="text-center py-10 text-gray-400">加载中...</div>;

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-bold">⭐ 积分系统</h2>

      {/* 积分余额卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {balances.map((b) => (
          <div key={b.user_id}
            className={`rounded-xl p-4 border cursor-pointer transition-all ${
              selectedUser === b.user_id ? "border-blue-500 bg-gray-800" : "border-gray-700 bg-gray-800/50 hover:border-gray-500"
            }`}
            onClick={() => setSelectedUser(b.user_id)}
          >
            <div className="flex justify-between items-center mb-2">
              <span className="font-bold">{b.display_name}</span>
              <span className="text-2xl font-mono">{b.balance}</span>
            </div>
            <div className="text-sm text-gray-400">积分余额</div>

            {/* 快捷操作 */}
            <div className="flex gap-2 mt-3">
              <button onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "homework", 30); }}
                className="text-xs px-2 py-1 rounded bg-blue-600 hover:bg-blue-500">📝 作业 +30</button>
              <button onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "tutoring", 60); }}
                className="text-xs px-2 py-1 rounded bg-purple-600 hover:bg-purple-500">📚 补课 +60</button>
              <button onClick={(e) => { e.stopPropagation(); handleEarn(b.user_id, "other", 30); }}
                className="text-xs px-2 py-1 rounded bg-teal-600 hover:bg-teal-500">🎯 其他 +30</button>
            </div>

            {/* 兑换按钮 */}
            <div className="flex gap-2 mt-2">
              <button onClick={(e) => { e.stopPropagation(); handleExchange(b.user_id, 30); }}
                disabled={b.balance < 30}
                className="text-xs px-2 py-1 rounded bg-orange-600 hover:bg-orange-500 disabled:opacity-40">⏱️ 兑换 30分钟</button>
              <button onClick={(e) => { e.stopPropagation(); handleExchange(b.user_id, 60); }}
                disabled={b.balance < 60}
                className="text-xs px-2 py-1 rounded bg-orange-600 hover:bg-orange-500 disabled:opacity-40">⏱️ 兑换 60分钟</button>
            </div>
          </div>
        ))}
      </div>

      {/* 积分记录 */}
      {selectedUser && (
        <div className="rounded-xl border border-gray-700 bg-gray-800/50 p-4">
          <h3 className="font-bold mb-3">📋 积分记录</h3>
          {history.length === 0 ? (
            <div className="text-gray-500 text-sm">暂无记录</div>
          ) : (
            <div className="space-y-2">
              {history.map((h) => (
                <div key={h.id} className="flex justify-between items-center text-sm py-1 border-b border-gray-700/50">
                  <div>
                    <span className={h.tx_type === "earn" ? "text-green-400" : "text-red-400"}>
                      {h.tx_type === "earn" ? "+" : "-"}{h.points}
                    </span>
                    <span className="ml-2 text-gray-400">{h.description}</span>
                  </div>
                  <div className="text-gray-500 text-xs">
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
