import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  FluentProvider,
  Input,
  Switch,
  Tooltip,
  webDarkTheme,
  webLightTheme,
} from "@fluentui/react-components";
import {
  ClipboardRegular,
  CodeRegular,
  CopyRegular,
  DeleteRegular,
  DismissRegular,
  ImageRegular,
  LinkRegular,
  PinFilled,
  PinRegular,
  SearchRegular,
  SettingsRegular,
} from "@fluentui/react-icons";
import packageInfo from "../package.json";
import "./App.css";

type ClipType = "text" | "link" | "code" | "image";
type Filter = "all" | "pinned" | ClipType;
type SettingsTab = "general" | "updates";
type ThemePreference = "system" | "light" | "dark";

type Clip = {
  id: number;
  clipType: ClipType;
  content: string;
  createdAt: number;
  pinned: boolean;
  imagePath?: string;
};

type UpdateState = "idle" | "checking" | "available" | "downloading" | "installed";

const appVersion = packageInfo.version;

const filters: { value: Filter; label: string }[] = [
  { value: "all", label: "Tất cả" },
  { value: "pinned", label: "Đã ghim" },
  { value: "text", label: "Văn bản" },
  { value: "link", label: "Liên kết" },
  { value: "code", label: "Mã" },
  { value: "image", label: "Hình ảnh" },
];

const typeLabels: Record<ClipType, string> = {
  text: "Văn bản",
  link: "Liên kết",
  code: "Mã",
  image: "Hình ảnh",
};

const themeOptions: { value: ThemePreference; label: string }[] = [
  { value: "system", label: "Theo hệ thống" },
  { value: "light", label: "Sáng" },
  { value: "dark", label: "Tối" },
];

function TypeIcon({ type }: { type: ClipType }) {
  if (type === "link") return <LinkRegular />;
  if (type === "code") return <CodeRegular />;
  if (type === "image") return <ImageRegular />;
  return <ClipboardRegular />;
}

function App() {
  const [systemDark, setSystemDark] = useState(() =>
    window.matchMedia("(prefers-color-scheme: dark)").matches,
  );
  const [themePreference, setThemePreference] = useState<ThemePreference>("system");
  const darkMode =
    themePreference === "system" ? systemDark : themePreference === "dark";
  const [clips, setClips] = useState<Clip[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const [reviewingClip, setReviewingClip] = useState<Clip | null>(null);
  const reviewingClipRef = useRef<Clip | null>(null);
  const settingsOpenRef = useRef(false);
  const [shortcut, setShortcut] = useState("Ctrl+Shift+V");
  const [shortcutError, setShortcutError] = useState("");
  const [settingsError, setSettingsError] = useState("");
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState>("idle");
  const [updateMessage, setUpdateMessage] = useState("");
  const [updateVersion, setUpdateVersion] = useState("");
  const [updateProgress, setUpdateProgress] = useState(0);
  const [newVersionAvailable, setNewVersionAvailable] = useState("");

  const refreshSettings = async () => {
    setShortcut(await invoke<string>("get_shortcut"));
    setAutostartEnabled(await invoke<boolean>("get_autostart_enabled"));
    setThemePreference(await invoke<ThemePreference>("get_theme"));
  };

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const updateTheme = (event: MediaQueryListEvent) => {
      setSystemDark(event.matches);
    };
    setSystemDark(media.matches);
    media.addEventListener("change", updateTheme);

    invoke<Clip[]>("get_history").then(setClips).catch(console.error);
    refreshSettings().catch(console.error);

    const unlistenHistory = listen<Clip[]>("clipboard-history-updated", (event) => {
      setClips(event.payload);
    });
    const unlistenShown = listen("manager-shown", () => {
      setSettingsOpen(false);
      setFilter("all");
      setQuery("");
      setSelectedIndex(0);
      invoke<Clip[]>("get_history").then(setClips).catch(console.error);
    });
    const unlistenSettings = listen("settings-opened", () => {
      setSettingsOpen(true);
      setSettingsTab("general");
      setShortcutError("");
      setSettingsError("");
      resetUpdateStatus();
      refreshSettings().catch(console.error);
      invoke<Clip[]>("get_history").then(setClips).catch(console.error);
    });

    return () => {
      void unlistenHistory.then((dispose) => dispose());
      void unlistenShown.then((dispose) => dispose());
      void unlistenSettings.then((dispose) => dispose());
      media.removeEventListener("change", updateTheme);
    };
  }, []);

  // Tự kiểm tra cập nhật một lần khi mở app (im lặng, không cản trở người dùng).
  // Nếu có bản mới sẽ hiện banner; người dùng tự bấm cài trong tab "Cập nhật".
  useEffect(() => {
    check()
      .then((update) => {
        if (update) {
          setNewVersionAvailable(update.version);
        }
      })
      .catch(console.error);
  }, []);

  const filteredClips = useMemo(
    () =>
      clips.filter((clip) => {
        const matchesFilter =
          filter === "all" ||
          (filter === "pinned" ? clip.pinned : clip.clipType === filter);
        return (
          matchesFilter &&
          `${clip.content} ${typeLabels[clip.clipType]}`
            .toLocaleLowerCase()
            .includes(query.toLocaleLowerCase())
        );
      }),
    [clips, filter, query],
  );

  const copyClip = async (clip: Clip) => {
    try {
      await invoke("copy_clip", { id: clip.id });
      setCopiedId(clip.id);
      window.setTimeout(() => setCopiedId(null), 1200);
    } catch (error) {
      window.alert("Sao chép lỗi: " + String(error));
    }
  };

  const togglePin = async (id: number) => {
    setClips(await invoke<Clip[]>("toggle_pin", { id }));
  };

  const deleteClip = async (id: number) => {
    setClips(await invoke<Clip[]>("delete_clip", { id }));
  };

  useEffect(() => {
    setSelectedIndex((index) =>
      Math.min(index, Math.max(0, filteredClips.length - 1)),
    );
  }, [filteredClips.length]);

  useEffect(() => {
    document
      .querySelector(".clip-row.selected")
      ?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  useEffect(() => { reviewingClipRef.current = reviewingClip; }, [reviewingClip]);
  useEffect(() => { settingsOpenRef.current = settingsOpen; }, [settingsOpen]);

  useEffect(() => {
    const onKeyDown = async (event: KeyboardEvent) => {
      const isReviewing = reviewingClipRef.current;
      const isSettings = settingsOpenRef.current;

      if (event.key === "Escape") {
        if (isReviewing) {
          event.preventDefault();
          event.stopImmediatePropagation();
          setReviewingClip(null);
          return;
        }
        if (isSettings) {
          await closeSettings();
          return;
        }
        await invoke("hide_window");
        return;
      }
      if (isSettings) return;
      if (event.target instanceof HTMLInputElement) return;

      if (isReviewing) {
        const shouldCopy =
          event.ctrlKey &&
          !event.altKey &&
          !event.metaKey &&
          !event.shiftKey &&
          event.key.toLocaleLowerCase() === "c";
        if (shouldCopy) {
          event.preventDefault();
          await copyClip(isReviewing);
          await invoke("hide_window");
        }
        return;
      }

      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const direction = event.key === "ArrowDown" ? 1 : -1;
        setSelectedIndex((index) =>
          Math.min(
            Math.max(index + direction, 0),
            Math.max(filteredClips.length - 1, 0),
          ),
        );
        return;
      }

      const shouldCopySelected =
        event.ctrlKey &&
        !event.altKey &&
        !event.metaKey &&
        !event.shiftKey &&
        event.key.toLocaleLowerCase() === "c";
      const clip = shouldCopySelected ? filteredClips[selectedIndex] : undefined;
      if (clip) {
        event.preventDefault();
        await copyClip(clip);
        await invoke("hide_window");
      }
    };

    window.addEventListener("keydown", onKeyDown, { capture: true });
    return () => window.removeEventListener("keydown", onKeyDown, { capture: true });
  }, [filteredClips, selectedIndex]);

  const openSettings = () => {
    setSettingsOpen(true);
    setSettingsTab("general");
    setShortcutError("");
    setSettingsError("");
    resetUpdateStatus();
    refreshSettings().catch(console.error);
  };

  const closeSettings = async () => {
    await refreshSettings();
    setSettingsOpen(false);
    setShortcutError("");
    setSettingsError("");
    resetUpdateStatus();
  };

  const resetUpdateStatus = () => {
    setUpdateState("idle");
    setUpdateMessage("");
    setUpdateVersion("");
    setUpdateProgress(0);
  };

  const captureShortcut = (event: React.KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return;
    const parts = [
      event.ctrlKey && "Ctrl",
      event.altKey && "Alt",
      event.shiftKey && "Shift",
      event.metaKey && "Super",
      event.key.length === 1 ? event.key.toUpperCase() : event.key,
    ].filter(Boolean);
    setShortcut(parts.join("+"));
    setShortcutError("");
  };

  const saveShortcut = async () => {
    try {
      await invoke("set_shortcut", { shortcut });
      setShortcutError("");
      setSettingsOpen(false);
    } catch (error) {
      setShortcutError(String(error));
    }
  };

  const updateAutostart = async (enabled: boolean) => {
    const previous = autostartEnabled;
    setAutostartEnabled(enabled);
    setSettingsError("");
    try {
      setAutostartEnabled(
        await invoke<boolean>("set_autostart_enabled", { enabled }),
      );
    } catch (error) {
      setAutostartEnabled(previous);
      setSettingsError(String(error));
    }
  };

  const changeTheme = async (preference: ThemePreference) => {
    const previous = themePreference;
    setThemePreference(preference);
    setSettingsError("");
    try {
      setThemePreference(
        await invoke<ThemePreference>("set_theme", { theme: preference }),
      );
    } catch (error) {
      setThemePreference(previous);
      setSettingsError(String(error));
    }
  };

  const installUpdate = async () => {
    setUpdateState("checking");
    setUpdateMessage("Đang kiểm tra cập nhật...");
    setUpdateVersion("");
    setUpdateProgress(0);

    try {
      const update = await check();
      if (!update) {
        setUpdateState("idle");
        setUpdateMessage("Bạn đang dùng phiên bản mới nhất.");
        return;
      }

      setUpdateState("available");
      setUpdateVersion(update.version);
      setUpdateMessage(`Có phiên bản ${update.version}. Đang tải...`);

      let downloaded = 0;
      let contentLength = 0;
      setUpdateState("downloading");
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          contentLength = event.data.contentLength ?? 0;
          setUpdateProgress(0);
        }
        if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (contentLength > 0) {
            setUpdateProgress(Math.round((downloaded / contentLength) * 100));
          }
        }
        if (event.event === "Finished") {
          setUpdateProgress(100);
        }
      });

      setUpdateState("installed");
      setUpdateMessage("Đã cài cập nhật. Đang khởi động lại ứng dụng...");
      // Tự khởi động lại để chạy ngay phiên bản mới.
      await relaunch();
    } catch (error) {
      setUpdateState("idle");
      setUpdateMessage(String(error));
    }
  };

  return (
    <FluentProvider theme={darkMode ? webDarkTheme : webLightTheme}>
      <main className="app">
        <header className="header">
          <div className="brand">
            <img className="brand-logo" src="/logo.svg" alt="Clipboard Manager" />
            <div>
              <h1>Clipboard</h1>
              <span>{shortcut} · ↑↓ chọn · Ctrl+C copy</span>
            </div>
          </div>
          <div className="header-actions">
            <Tooltip content="Xóa mục chưa ghim" relationship="label">
              <Button
                appearance="subtle"
                size="small"
                icon={<DeleteRegular />}
                onClick={async () =>
                  setClips(await invoke<Clip[]>("clear_unpinned"))
                }
              />
            </Tooltip>
            <Tooltip content="Cài đặt" relationship="label">
              <Button
                appearance="subtle"
                size="small"
                icon={<SettingsRegular />}
                onClick={openSettings}
              />
            </Tooltip>
          </div>
        </header>

        {newVersionAvailable && (
          <button
            type="button"
            className="update-banner"
            onClick={() => {
              setSettingsOpen(true);
              setSettingsTab("updates");
              setShortcutError("");
              setSettingsError("");
              resetUpdateStatus();
              refreshSettings().catch(console.error);
            }}
          >
            Có phiên bản mới v{newVersionAvailable} — bấm để cập nhật
          </button>
        )}

        <section className="controls">
          <Input
            className="search"
            size="medium"
            contentBefore={<SearchRegular />}
            contentAfter={
              query ? (
                <Button
                  appearance="transparent"
                  size="small"
                  icon={<DismissRegular />}
                  aria-label="Xóa tìm kiếm"
                  onClick={() => setQuery("")}
                />
              ) : null
            }
            placeholder="Tìm nội dung đã sao chép"
            value={query}
            onChange={(_, data) => setQuery(data.value)}
          />
          <div className="filters">
            {filters.map((item) => (
              <button
                className={filter === item.value ? "active" : ""}
                key={item.value}
                onClick={() => setFilter(item.value)}
              >
                {item.label}
              </button>
            ))}
          </div>
        </section>

        <section className="history">
          {filteredClips.map((clip, index) => (
            <article
              className={`clip-row ${selectedIndex === index ? "selected" : ""} ${clip.clipType === "image" ? "image-row" : ""}`}
              key={clip.id}
              onClick={() => setSelectedIndex(index)}
              onDoubleClick={() => setReviewingClip(clip)}
            >
              {clip.clipType === "image" && clip.imagePath ? (
                <img
                  className="preview"
                  src={convertFileSrc(clip.imagePath)}
                  alt=""
                  title="Bấm để sao chép · Bấm đúp để phóng to"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedIndex(index);
                    void copyClip(clip);
                  }}
                  onDoubleClick={(e) => {
                    e.stopPropagation();
                    setReviewingClip(clip);
                  }}
                  style={{ cursor: "copy" }}
                />
              ) : (
                <div className={`type-icon ${clip.clipType}`}>
                  <TypeIcon type={clip.clipType} />
                </div>
              )}

              <div className="clip-content">
                <p>{clip.content || "Hình ảnh từ clipboard"}</p>
              </div>

              <div className="row-actions">
                <Button
                  appearance={copiedId === clip.id ? "primary" : "subtle"}
                  size="small"
                  icon={<CopyRegular />}
                  aria-label="Sao chép lại"
                  onClick={() => copyClip(clip)}
                />
                <Button
                  appearance="subtle"
                  size="small"
                  icon={clip.pinned ? <PinFilled /> : <PinRegular />}
                  aria-label={clip.pinned ? "Bỏ ghim" : "Ghim"}
                  onClick={() => togglePin(clip.id)}
                />
                <Button
                  appearance="subtle"
                  size="small"
                  icon={<DeleteRegular />}
                  aria-label="Xóa"
                  onClick={() => deleteClip(clip.id)}
                />
              </div>
            </article>
          ))}

          {filteredClips.length === 0 && (
            <div className="empty">
              <ClipboardRegular />
              <strong>Chưa có nội dung</strong>
              <span>Hãy sao chép văn bản hoặc hình ảnh.</span>
            </div>
          )}
        </section>

        <Dialog
          open={Boolean(reviewingClip)}
          onOpenChange={(_, data) => {
            if (!data.open) setReviewingClip(null);
          }}
        >
          <DialogSurface className="review-dialog">
            <DialogBody>
              <DialogContent className="review-content">
                {reviewingClip?.imagePath ? (
                  <img
                    className="review-image"
                    src={convertFileSrc(reviewingClip.imagePath)}
                    alt={reviewingClip.content}
                  />
                ) : (
                  <pre className="review-text">{reviewingClip?.content}</pre>
                )}
              </DialogContent>
            </DialogBody>
          </DialogSurface>
        </Dialog>

        <Dialog
          open={settingsOpen}
          onOpenChange={(_, data) => {
            if (!data.open) void closeSettings();
          }}
        >
          <DialogSurface className="settings-dialog">
            <DialogBody>
              <DialogTitle>Cài đặt</DialogTitle>
              <DialogContent className="settings-content">
                <div className="settings-tabs" role="tablist" aria-label="Settings sections">
                  <button
                    className={settingsTab === "general" ? "active" : ""}
                    role="tab"
                    aria-selected={settingsTab === "general"}
                    onClick={() => setSettingsTab("general")}
                  >
                    Chung
                  </button>
                  <button
                    className={settingsTab === "updates" ? "active" : ""}
                    role="tab"
                    aria-selected={settingsTab === "updates"}
                    onClick={() => setSettingsTab("updates")}
                  >
                    Cập nhật
                  </button>
                </div>
                {settingsTab === "general" && (
                  <div className="settings-panel" role="tabpanel">
                <div className="settings-field">
                  <label>Phím tắt mở Clipboard</label>
                  <p>Nhấn tổ hợp phím mới vào ô bên dưới.</p>
                  <Input
                    value={shortcut}
                    readOnly
                    autoFocus
                    onKeyDown={captureShortcut}
                    onFocus={(event) => event.currentTarget.select()}
                  />
                  {shortcutError && (
                    <span className="settings-error">{shortcutError}</span>
                  )}
                </div>

                <div className="settings-field">
                  <label>Giao diện</label>
                  <p>Chọn chế độ sáng, tối hoặc theo hệ thống.</p>
                  <div className="theme-options" role="radiogroup" aria-label="Giao diện">
                    {themeOptions.map((option) => (
                      <button
                        className={themePreference === option.value ? "active" : ""}
                        key={option.value}
                        role="radio"
                        aria-checked={themePreference === option.value}
                        onClick={() => changeTheme(option.value)}
                      >
                        {option.label}
                      </button>
                    ))}
                  </div>
                </div>

                <Switch
                  checked={autostartEnabled}
                  label="Chạy ứng dụng khi khởi động Windows"
                  onChange={(_, data) => updateAutostart(Boolean(data.checked))}
                />
                {settingsError && (
                  <span className="settings-error">{settingsError}</span>
                )}
                  </div>
                )}
                {settingsTab === "updates" && (
                  <div className="settings-panel" role="tabpanel">
                <div className="settings-field update-field">
                  <label>Cập nhật ứng dụng</label>
                  <p>Kiểm tra, tải và cài đặt bản phát hành mới nhất.</p>
                  <span className="app-version">Phiên bản hiện tại: v{appVersion}</span>
                  <div className="update-actions">
                    <Button
                      className="update-check-button"
                      appearance="secondary"
                      disabled={updateState === "checking" || updateState === "downloading"}
                      onClick={installUpdate}
                    >
                      {updateState === "checking"
                        ? "Đang kiểm tra..."
                        : updateState === "downloading"
                          ? "Đang tải..."
                          : "Kiểm tra cập nhật"}
                    </Button>
                    {updateVersion && <span>v{updateVersion}</span>}
                  </div>
                  {updateState === "downloading" && (
                    <div className="update-progress">
                      <span style={{ width: `${updateProgress}%` }} />
                    </div>
                  )}
                  {updateMessage && (
                    <span
                      className={
                        updateState === "installed" ||
                        updateMessage.includes("phiên bản mới nhất")
                          ? "settings-success"
                          : "settings-error"
                      }
                    >
                      {updateMessage}
                    </span>
                  )}
                </div>
                  </div>
                )}
                <div className="settings-dialog-actions">
                  {settingsTab === "general" ? (
                    <>
                      <Button appearance="subtle" onClick={closeSettings}>
                        Hủy
                      </Button>
                      <Button appearance="primary" onClick={saveShortcut}>
                        Lưu
                      </Button>
                    </>
                  ) : (
                    <Button appearance="primary" onClick={closeSettings}>
                      Đóng
                    </Button>
                  )}
                </div>
              </DialogContent>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      </main>
    </FluentProvider>
  );
}

export default App;
