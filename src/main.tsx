import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

document.addEventListener("contextmenu", (e) => e.preventDefault());

// Tauri v2 bơm __TAURI_INTERNALS__ vào webview. Nếu không có nghĩa là app
// đang bị mở bằng trình duyệt thường (vd: http://localhost:1420) — khi đó
// mọi lời gọi invoke/listen sẽ lỗi, nên ta chặn lại và hiển thị thông báo.
const isTauri = "__TAURI_INTERNALS__" in window;

function NotInApp() {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 12,
        height: "100vh",
        padding: 24,
        textAlign: "center",
        fontFamily: "Segoe UI, system-ui, sans-serif",
        color: "#e0e0e0",
        background: "#1f1f1f",
        boxSizing: "border-box",
      }}
    >
      <div style={{ fontSize: 40 }}>🚫</div>
      <h2 style={{ margin: 0, fontSize: 18 }}>Không mở được trên trình duyệt</h2>
      <p style={{ margin: 0, fontSize: 14, lineHeight: 1.5, maxWidth: 320, opacity: 0.85 }}>
        Clipboard Manager chỉ chạy được bên trong ứng dụng desktop. Vui lòng mở
        bằng cửa sổ app (chạy <code>npm run tauri dev</code> hoặc mở app đã cài),
        đừng dùng trình duyệt.
      </p>
    </div>
  );
}

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

root.render(
  <React.StrictMode>{isTauri ? <App /> : <NotInApp />}</React.StrictMode>,
);
