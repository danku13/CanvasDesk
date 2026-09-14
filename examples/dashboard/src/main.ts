// Repo Dashboard (T22-C): виджет с fetch наружу — демонстрация permission
// "network". Хост блокирует внешние запросы без него (403) и сам CSP
// пускает только на api.github.com; задача виджета — деградировать с
// понятной ошибкой, а не падать (AGENTS: отказы — нормальный путь).
import { CanvasDesk, type ThemeInfo } from "./canvasdesk";

const REPO = "danku13/CanvasDesk";
const API = `https://api.github.com/repos/${REPO}`;

const elRepo = document.getElementById("repo") as HTMLElement;
const elStars = document.getElementById("stars") as HTMLElement;
const elForks = document.getElementById("forks") as HTMLElement;
const elIssues = document.getElementById("issues") as HTMLElement;
const elUpdated = document.getElementById("updated") as HTMLElement;
const elError = document.getElementById("error") as HTMLDivElement;
const elErrorText = document.getElementById("error-text") as HTMLElement;

function applyTheme(theme: ThemeInfo): void {
  const root = document.documentElement.style;
  root.setProperty("--bg", theme.dark ? "#1e2430" : "#f5f7fb");
  root.setProperty("--fg", theme.dark ? "#e8ecf4" : "#1a2233");
  root.setProperty("--muted", theme.dark ? "#8a93a6" : "#5b6575");
  root.setProperty("--cell", theme.dark ? "#262e3d" : "#e8ecf4");
  root.setProperty("--cell-hover", theme.dark ? "#313b4e" : "#dce4f0");
  root.setProperty("--accent", theme.accent || "#3B82F6");
}

interface RepoInfo {
  stargazers_count?: number;
  forks_count?: number;
  open_issues_count?: number;
  pushed_at?: string;
}

function showError(message: string): void {
  elError.style.display = "block";
  elErrorText.textContent = message;
}

async function load(): Promise<void> {
  elError.style.display = "none";
  elStars.textContent = elForks.textContent = elIssues.textContent = "…";
  try {
    const response = await fetch(API, { headers: { Accept: "application/vnd.github+json" } });
    if (!response.ok) {
      // 403 от хоста = нет permission "network"; 403 от GitHub = rate limit
      showError(response.status === 403
        ? "403: либо у пакета нет permission \"network\" (верните его в " +
          "widget.json и обновите пакет drag-ом), либо исчерпан лимит " +
          "GitHub API (60 запросов/час без токена) — подождите сброса."
        : `GitHub API: HTTP ${response.status}`);
      return;
    }
    const info = (await response.json()) as RepoInfo;
    elStars.textContent = String(info.stargazers_count ?? "—");
    elForks.textContent = String(info.forks_count ?? "—");
    elIssues.textContent = String(info.open_issues_count ?? "—");
    elUpdated.textContent = info.pushed_at
      ? `обновлён: ${new Date(info.pushed_at).toLocaleString("ru-RU")}`
      : "";
  } catch (error) {
    // fetch кидает TypeError при сетевом отказе (в т.ч. CSP-блоке connect-src)
    showError(`Сеть недоступна: ${error instanceof Error ? error.message : String(error)}` +
      " Проверьте permission \"network\" в widget.json и connect-src в CSP.");
  }
}

document.getElementById("refresh")!.addEventListener("click", () => {
  void load();
});

CanvasDesk.onTheme(applyTheme);
CanvasDesk.init((data) => {
  applyTheme(data.theme);
  void load();
});

elRepo.textContent = REPO;

// Standalone (npm run dev): fetch работает напрямую, без хоста и permissions.
if (CanvasDesk.standalone) {
  elUpdated.textContent = "standalone: сеть без посредника хоста";
  void load();
}
