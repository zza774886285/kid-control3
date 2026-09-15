import { Outlet, Link, useLocation } from "react-router-dom";
import ThemeToggle from "./ThemeToggle";

const NAV = [
  { path: "/", label: "首页", icon: "🏠" },
  { path: "/points", label: "积分", icon: "⭐" },
  { path: "/settings", label: "设置", icon: "⚙️" },
];

export default function Layout() {
  const location = useLocation();

  return (
    <div className="min-h-screen flex flex-col">
      <header
        className="sticky top-0 z-50 glass transition-colors duration-300"
        style={{ borderBottom: "1px solid var(--glass-b)" }}
      >
        <div className="max-w-5xl mx-auto px-4 flex items-center justify-between h-14">
          <div className="flex items-center gap-2">
            <span className="text-lg">📱</span>
            <span className="font-semibold" style={{ color: "var(--t1)" }}>
              平板管控
            </span>
          </div>

          <nav
            className="flex items-center gap-1 rounded-full p-1"
            style={{
              background: "var(--glass)",
              border: "1px solid var(--glass-b)",
            }}
          >
            {NAV.map((item) => {
              const active =
                item.path === "/"
                  ? location.pathname === "/"
                  : location.pathname.startsWith(item.path);
              return (
                <Link
                  key={item.path}
                  to={item.path}
                  className="flex items-center gap-1.5 px-4 py-1.5 rounded-full text-sm font-medium transition-all duration-200"
                  style={
                    active
                      ? {
                          background: "var(--acc-g2)",
                          color: "var(--acc)",
                          boxShadow: "0 0 10px var(--acc-g)",
                        }
                      : { color: "var(--t3)" }
                  }
                >
                  <span className="text-base">{item.icon}</span>
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>

          <ThemeToggle />
        </div>
      </header>

      <main className="flex-1 max-w-5xl mx-auto w-full px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}
