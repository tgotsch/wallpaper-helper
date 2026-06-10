const REPO = "https://github.com/tgotsch/wallpaper-helper";

export default function Install() {
  return (
    <section className="install">
      <h2>Install</h2>
      <p>
        Build from source with{" "}
        <a href="https://rustup.rs/">a Rust toolchain</a>, or grab a build from{" "}
        <a href={`${REPO}/releases`}>the releases page</a>.
      </p>
      <div className="install-grid">
        <div className="install-card">
          <h3>Windows</h3>
          <pre>
            <code>{`git clone ${REPO}.git
cd wallpaper-helper
cargo build --release
.\\target\\release\\WallpaperHelper_rs.exe`}</code>
          </pre>
          <p className="install-note">
            Optionally pass a config path:{" "}
            <code>WallpaperHelper_rs.exe Y:\wallpapers\config.json</code>
          </p>
        </div>
        <div className="install-card">
          <h3>Linux (KDE Plasma 6)</h3>
          <pre>
            <code>{`# Runtime/build dependencies (names vary by distro):
#   webkit2gtk-4.1, kscreen (kscreen-doctor), qdbus6
git clone ${REPO}.git
cd wallpaper-helper
cargo build --release
./target/release/WallpaperHelper_rs`}</code>
          </pre>
          <p className="install-note">
            Wallpapers are set through Plasma&apos;s scripting interface;
            monitor detection uses kscreen-doctor with a sysfs fallback.
          </p>
        </div>
      </div>
      <p className="install-note">
        Closing the window keeps the app running in the system tray — use the
        tray menu&apos;s “Quit” to exit fully.
      </p>
    </section>
  );
}
