const REPO = "https://github.com/tgotsch/wallpaper-helper";

export default function Footer() {
  return (
    <footer className="footer">
      <p>
        Wallpaper Helper is open source —{" "}
        <a href={REPO}>source code on GitHub</a>. Issues and pull requests
        welcome.
      </p>
    </footer>
  );
}
