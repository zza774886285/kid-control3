import { Outlet, Link, useLocation } from "react-router-dom";
import { useState } from "react";

const NAV = [
  { path: "/", label: "首页", icon: "🏠" },
  { path: "/points", label: "积分", icon: "⭐" },
  { path: "/settings", label: "设置", icon: "⚙️" },
];

export default function Layout() {
  const location = useLocation();
  const [dark, setDark] = useState(true);

  return (
    <div className={`min-h-screen ${dark ? "bg-gray-900 text-gray-100" : "bg-gray-50 text-gray-900"}`}>
      {/* 顶栏 */}
      <header className="flex items-center justify-between px-6 py-3 border-b border-gray-700">
        <h1 className="text-lg font-bold">📱 平板管控</h1>
        <button onClick={() => setDark(!dark)} className="text-sm px-3 py-1 rounded bg-gray-700 hover:bg-gray-600">
          {dark ? "☀️ 亮色" : "🌙 暗色"}
        </button>
      </header>

      <div className="flex">
        {/* 侧栏 */}
        <nav className="w-48 min-h-[calc(100vh-52px)] border-r border-gray-700 p-4 space-y-1">
          {NAV.map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className={`block px-3 py-2 rounded text-sm ${
                location.pathname === item.path
                  ? "bg-blue-600 text-white"
                  : "text-gray-400 hover:bg-gray-800"
              }`}
            >
              {item.icon} {item.label}
            </Link>
          ))}
        </nav>

        {/* 主内容 */}
        <main className="flex-1 p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
