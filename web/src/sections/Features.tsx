import { GlassCard, StatCounter } from "performative-ui";

const FEATURES = [
  {
    icon: "🖥️",
    title: "Per-monitor profiles",
    body: "Map a wallpaper to each monitor and apply the whole set with one click. Monitors get friendly aliases like “main”, “left”, “right” instead of device IDs.",
  },
  {
    icon: "🗂️",
    title: "Collections",
    body: "Group profiles into collections and step through them — next, previous, or random — from the app or the system tray.",
  },
  {
    icon: "▶️",
    title: "Slideshow",
    body: "Cycle a collection automatically on an interval you choose (1–120 minutes). Pause and resume from the tray, even with the window closed.",
  },
  {
    icon: "⏰",
    title: "Scheduler",
    body: "Switch to a given profile at a set time of day. A morning profile at 8:00, something darker for the evening.",
  },
  {
    icon: "🔁",
    title: "One cross-platform config",
    body: "Profiles store monitor aliases and relative paths, with per-platform base paths and device mappings. Keep config.json on a network drive and use it from both OSes.",
  },
  {
    icon: "🫥",
    title: "Lives in the tray",
    body: "Closing the window hides the app to the system tray. Slideshows and schedules keep running in the background.",
  },
];

const STATS = [
  { target: 2, label: "platforms supported" },
  { target: 1, label: "config file to share" },
  { target: 0, label: "telemetry, accounts, or ads" },
];

export default function Features() {
  return (
    <section className="features">
      <h2>What it actually does</h2>
      <div className="feature-grid">
        {FEATURES.map((f) => (
          <GlassCard key={f.title} glowOnHover>
            <GlassCard.Icon>{f.icon}</GlassCard.Icon>
            <GlassCard.Title>{f.title}</GlassCard.Title>
            <GlassCard.Body>{f.body}</GlassCard.Body>
          </GlassCard>
        ))}
      </div>
      <div className="stat-row">
        {STATS.map((s) => (
          <div className="stat" key={s.label}>
            <span className="stat-number">
              <StatCounter target={s.target} durationMs={1200} />
            </span>
            <span className="stat-label">{s.label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
