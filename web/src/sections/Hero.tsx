import { Aurora, Button, EyebrowPill, GradientText } from "performative-ui";

const REPO = "https://github.com/tgotsch/wallpaper-helper";

export default function Hero() {
  return (
    <header className="hero">
      <Aurora />
      <div className="hero-content">
        <EyebrowPill statusColor="#34d399">Free &amp; open source</EyebrowPill>
        <h1>
          One config. Every monitor.{" "}
          <GradientText as="span">Both desktops.</GradientText>
        </h1>
        <p className="hero-sub">
          Wallpaper Helper is a desktop app that manages per-monitor wallpaper
          profiles on Windows and KDE Plasma 6. Name your monitors once, build
          profiles and collections, schedule switches — and share a single
          config file between machines.
        </p>
        <div className="hero-actions">
          <Button as="a" variant="glow" size="lg" href={`${REPO}/releases`}>
            Download
          </Button>
          <Button as="a" variant="ghost" size="lg" href={REPO}>
            View source
          </Button>
        </div>
      </div>
    </header>
  );
}
