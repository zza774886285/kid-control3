import { useTheme } from "../ThemeContext";

export default function ThemeToggle() {
  const { theme, toggle } = useTheme();

  return (
    <button
      onClick={toggle}
      className="relative w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-300 cursor-pointer
        bg-[var(--bg-card)] border border-[var(--border)] hover:border-[var(--accent)]
        hover:shadow-[0_0_15px_var(--accent-glow)]"
      title={theme === "dark" ? "切换浅色" : "切换深色"}
    >
      <span className="text-lg transition-transform duration-300" style={{ transform: theme === "light" ? "rotate(180deg)" : "rotate(0)" }}>
        {theme === "dark" ? "☀️" : "🌙"}
      </span>
    </button>
  );
}
