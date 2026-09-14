import { Outlet, Link, useLocation } from "react-router-dom";

const NAV = [
  { path: "/", label: "首页", icon: "🏠" },
  { path: "/points", label: "积分", icon: "⭐" },
  { path: "/settings", label: "设置", icon: "⚙️" },
];

export default function Layout() {
  const location = useLocation();

  return (
    <div className="min-h-screen flex flex-col">
      {/* 顶部导航栏 */}
      <header className="sticky top-0 z-50 backdrop-blur-xl bg-black/30 border-b border-white/[0.06]">
        <div className="max-w-4xl mx-auto px-4 flex items-center justify-between h-14">
          {/* Logo */}
          <div className="flex items-center gap-2">
            <span className="text-lg">📱</span>
            <span className="font-semibold text-[color:var(--color-text-primary)] tracking-tight">
              平板管控
            </span>
          </div>

          {/* Tab 导航 */}
          <nav className="flex items-center gap-1 bg-white/[0.05] rounded-full p-1">
            {NAV.map((item) => {
              const active =
                item.path === "/"
                  ? location.pathname === "/"
                  : location.pathname.startsWith(item.path);
              return (
                <Link
                  key={item.path}
                  to={item.path}
                  className={`flex items-center gap-1.5 px-4 py-1.5 rounded-full text-sm font-medium transition-all duration-200 ${
                    active
                      ? "bg-white/10 text-white shadow-sm"
                      : "text-[color:var(--color-text-muted)] hover:text-[color:var(--color-text-secondary)]"
                  }`}
                >
                  <span className="text-base">{item.icon}</span>
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
        </div>
      </header>

      {/* 主内容 */}
      <main className="flex-1 max-w-4xl mx-auto w-full px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}
