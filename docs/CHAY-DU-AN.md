# Hướng dẫn chạy dự án Clipboard Manager

Tài liệu này hướng dẫn cách cài đặt môi trường, chạy ở chế độ phát triển (dev), build bản phát hành và xử lý các lỗi thường gặp.

Dự án dùng **Tauri 2** (backend Rust) + **React 19** + **TypeScript** + **Vite** + **Fluent UI 2**. Nền tảng mục tiêu chính là **Windows**.

---

## 1. Yêu cầu môi trường (cài 1 lần)

| Thành phần | Phiên bản đề nghị | Ghi chú |
|-----------|-------------------|---------|
| **Node.js** | 18 LTS trở lên (khuyến nghị 20+) | Tải tại https://nodejs.org. Đi kèm `npm`. |
| **Rust** | Bản ổn định mới nhất | Cài qua https://rustup.rs |
| **Microsoft C++ Build Tools** | VS 2022 Build Tools | Cần "Desktop development with C++". |
| **WebView2 Runtime** | Mới nhất | Windows 10/11 thường đã có sẵn. |

Kiểm tra nhanh sau khi cài:

```bash
node -v      # ví dụ v20.x
npm -v
rustc --version
cargo --version
```

> Lần đầu build Rust sẽ tải và biên dịch khá nhiều crate nên hơi lâu (vài phút). Các lần sau sẽ nhanh hơn nhờ cache.

---

## 2. Cài đặt dependencies

Tại thư mục gốc dự án:

```bash
npm install
```

Lệnh này cài các package frontend (React, Vite, Fluent UI, Tauri CLI...). Dependencies Rust sẽ được `cargo` tự tải khi chạy/build lần đầu.

---

## 3. Chạy ở chế độ phát triển (Dev)

Đây là lệnh dùng hằng ngày khi code:

```bash
npm run tauri dev
```

Lệnh này sẽ:

1. Khởi động Vite dev server tại `http://localhost:1420` (cấu hình trong `tauri.conf.json`).
2. Biên dịch backend Rust.
3. Mở cửa sổ ứng dụng desktop (kích thước cố định **400x650**).

Đặc điểm khi chạy dev:

- Hot-reload: sửa code trong `src/` (React) sẽ cập nhật ngay.
- Sửa code Rust trong `src-tauri/src/` sẽ build lại backend (chậm hơn).
- Phím tắt toàn cục mặc định để hiện/ẩn cửa sổ: **Ctrl + Shift + V**.
- Ứng dụng vẫn chạy nền và có icon ở khay hệ thống (system tray).

> Chỉ chạy frontend (không có cửa sổ Tauri) nếu cần kiểm thử UI nhanh: `npm run dev` rồi mở `http://localhost:1420`. Tuy nhiên các tính năng clipboard/tray/shortcut chỉ hoạt động khi chạy qua `npm run tauri dev`.

---

## 4. Kiểm tra backend Rust (tùy chọn)

```bash
cd src-tauri
cargo check     # kiểm tra biên dịch, không tạo file chạy
cargo test      # chạy unit test (nếu có)
```

---

## 5. Build bản phát hành (Production)

Tạo bản cài đặt desktop:

```bash
npm run tauri build
```

Kết quả nằm trong `src-tauri/target/release/` và các bộ cài (NSIS/MSI) trong `src-tauri/target/release/bundle/`.

### Build bản NSIS có ký (signed) cho updater

Dự án có sẵn script đóng gói NSIS kèm artifact cho cơ chế tự cập nhật:

```bash
npm run release:nsis        # build + ký bản NSIS
npm run release:latest-json # tạo file latest.json cho updater
```

> **Lưu ý về ký bản (signing):** khóa bí mật ký updater nằm trong file `.env` (`TAURI_SIGNING_PRIVATE_KEY`). File này đã được `.gitignore` và **không được commit**. Khóa công khai tương ứng nằm trong `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). Cơ chế tự cập nhật lấy bản mới từ GitHub Releases.

### Tạo cặp khóa ký updater

Dự án **đã có sẵn cặp khóa** (private trong `.env`, public trong `tauri.conf.json`). Bạn **chỉ cần tạo khóa mới khi**: khóa cũ bị lộ/mất, hoặc khởi tạo dự án mới từ đầu.

Lệnh interactive (hỏi password và đường dẫn lưu):

```bash
npm run tauri signer generate
```

Lệnh non-interactive (không password, ghi ra file — khớp cấu hình hiện tại):

```bash
npm run tauri signer generate -- -w src-tauri/clipboard-manager.key --ci
```

Lệnh tạo 2 file:

- `clipboard-manager.key` — **private key (giữ bí mật, KHÔNG commit)**
- `clipboard-manager.key.pub` — public key (dán vào `tauri.conf.json`)

> ⚠️ **Cảnh báo — thao tác khó hoàn tác:** nếu tạo khóa mới, bạn phải (1) thay `plugins.updater.pubkey` mới vào `tauri.conf.json` và (2) ký lại các bản build bằng private key mới. Hệ quả: **mọi bản đã cài trước đó sẽ không tự cập nhật được nữa**, vì bản update mới ký bằng khóa khác với pubkey đã đóng trong app cũ.

---

## 6. Cấu trúc thư mục chính

```text
src/                  Frontend React
  App.tsx             UI và logic tương tác
  App.css             Layout kiểu Fluent, cửa sổ compact
  main.tsx            Điểm vào React

src-tauri/            Backend Tauri/Rust
  src/lib.rs          Theo dõi clipboard, lệnh, tray, shortcut, autostart
  src/main.rs         Điểm vào ứng dụng Tauri
  tauri.conf.json     Cấu hình cửa sổ, bảo mật, updater, bundle
  Cargo.toml          Dependencies Rust
  icons/              Icon ứng dụng và tray

scripts/              Script PowerShell đóng gói release
docs/                 Tài liệu
```

---

## 7. Các lệnh npm có sẵn

| Lệnh | Tác dụng |
|------|----------|
| `npm run dev` | Chỉ chạy Vite dev server (frontend). |
| `npm run build` | Biên dịch TypeScript + build frontend ra `dist/`. |
| `npm run preview` | Xem trước bản frontend đã build. |
| `npm run tauri dev` | **Chạy app desktop ở chế độ dev** (dùng thường xuyên). |
| `npm run tauri build` | Build bản cài đặt production. |
| `npm run release:nsis` | Build bản NSIS có ký. |
| `npm run release:latest-json` | Tạo `latest.json` cho updater. |

---

## 8. Lỗi thường gặp

**`cargo` / `rustc` không nhận diện được**
→ Chưa cài Rust hoặc chưa mở lại terminal sau khi cài. Cài qua https://rustup.rs rồi mở terminal mới.

**Lỗi liên quan `link.exe` / MSVC khi build Rust**
→ Thiếu C++ Build Tools. Cài "Desktop development with C++" trong Visual Studio Build Tools 2022.

**Cửa sổ app không hiện khi chạy dev**
→ Đúng thiết kế: cửa sổ mặc định ẩn (`"visible": false`). Nhấn **Ctrl + Shift + V** hoặc click icon ở khay hệ thống để mở.

**Build lần đầu rất lâu**
→ Bình thường, do biên dịch toàn bộ crate Rust. Các lần sau sẽ nhanh hơn nhờ cache trong `src-tauri/target/`.

**Cổng 1420 đã bị chiếm**
→ Tắt tiến trình đang dùng cổng đó, hoặc đổi cổng trong `vite.config.ts` và `tauri.conf.json` (`build.devUrl`) cho khớp nhau.

---

## Tóm tắt nhanh

```bash
# Lần đầu
npm install

# Chạy phát triển
npm run tauri dev

# Build bản cài đặt
npm run tauri build
```
