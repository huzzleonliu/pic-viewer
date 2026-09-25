# Pic Viewer

托管一个目录，在浏览器里浏览、勾选、标记和导出图片，也可以打开文本文件编辑。技术栈是 [Leptos](https://leptos.dev/) 0.8（SSR + WASM hydrate）和 Axum。

路径全部锁在 `PIC_ROOT` 内，拒绝 `..` 越界。胶片条和预览默认经 imgproxy 缩放；简单调整栏勾选「原图」则加载本地原文件。

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [readme/部署.md](readme/部署.md) | `.env`、环境变量、端口、Compose / 开发栈 / 本机构建 / Kubernetes、imgproxy 参数 |
| [readme/流程结构.md](readme/流程结构.md) | 请求链路、图片与文本模式、列表扫描、标记 / 筛选 / 导出 |
| [readme/模块功能.md](readme/模块功能.md) | 界面各栏、文件操作、预览、编辑器、元数据与导出 |
| [readme/代码结构.md](readme/代码结构.md) | `src/` 目录、`page` / `function` / `structure`、Cargo feature |

仓库内其它入口：

| 路径 | 用途 |
| --- | --- |
| [`example.env`](example.env) | Compose 环境变量模板，复制为 `.env` 后使用 |
| [`docker-compose.yml`](docker-compose.yml) | 生产：拉镜像，不编译 |
| [`dev.docker-compose.yml`](dev.docker-compose.yml) | 开发：容器内 `cargo leptos watch` |
| [`Dockerfile`](Dockerfile) | 生产镜像（`cargo leptos build --release`） |
| [`Dockerfile.dev`](Dockerfile.dev) | 开发镜像（Rust + cargo-leptos） |
| [`k8s/deployment.yaml`](k8s/deployment.yaml) | Kubernetes 示例 |
| [`Cargo.toml`](Cargo.toml) | 依赖与 Leptos 站点元数据 |
| [`style/main.css`](style/main.css) | 样式 |
| [`src/`](src/) | 应用源码，见 [代码结构](readme/代码结构.md) |

## 功能概览

左侧文件管理器，右侧按当前项切换图片预览或文本编辑。顶栏可开关各面板。

- **文件管理**：树形浏览（跳过点文件）、勾选、复制 / 剪切 / 粘贴、重命名、删除、新建目录 / 文件。
- **图片浏览**：缩放、旋转并写回、左右切图、胶片条分页（每页 100 张）、可选含子目录（上限约 10000 张）。
- **标记与筛选**：EXIF 星标与 tag；内存 SQLite 索引后按星标 / tag 过滤胶片条。
- **导出**：勾选图下载到本机（多张打 zip）或写回托管目录；可转 JPEG/PNG/WebP 等，或保持原图。
- **文本**：按文件头嗅探为文本后打开编辑器（读上限 2 MB，写上限 8 MB）。

界面与限制的细节见 [模块功能](readme/模块功能.md)，数据怎么走见 [流程结构](readme/流程结构.md)。

## 部署（生产 Compose）

先把模板变成 `.env`（该文件已 gitignore，不要提交真实路径）：

```bash
cp example.env .env
```

编辑 `.env` 里的 `PIC_DIR`（相册）和可选的 `PIC_VIEWER_IMAGE`、`PIC_HOST_PORT`，然后：

```bash
podman compose up -d
```

不必再写 `PIC_DIR=/photos podman compose up -d` 这种前缀。浏览器打开 http://127.0.0.1:3020 （端口以 `.env` 的 `PIC_HOST_PORT` 为准）。停止：`podman compose down`。

没有 `.env` 时 compose 仍可用 yaml 里的缺省（相册 `./test/pic`、镜像 `ghcr.io/huzzleonliu/pic-viewer:latest`、端口 3020）。

浏览器只访问应用端口。imgproxy 在内部网络，缩略图和预览由应用转发。变量含义见 [部署.md](readme/部署.md)。

自己构建镜像时，把 `.env` 里的 `PIC_VIEWER_IMAGE` 改成本地标签即可：

```bash
podman build -t localhost/pic-viewer:local .
# 在 .env 中：PIC_VIEWER_IMAGE=localhost/pic-viewer:local
podman compose up -d
```

Kubernetes 示例在 `k8s/`，用法见 [部署.md · Kubernetes](readme/部署.md#kubernetes)。

## 开发（热更新）

同样用根目录 `.env`（生产、开发两份 compose 都读 `PIC_DIR`）。`dev.docker-compose.yml` 起 imgproxy，并在容器里跑 `cargo leptos watch`。源码挂进去，改文件会重编。本机不必装 Rust。不要和生产栈同时占用 `PIC_HOST_PORT`。

```bash
cp example.env .env          # 若还没有
podman compose down
podman compose -f dev.docker-compose.yml up --build
```

打开 http://127.0.0.1:3020。第一次会编开发镜像并完整编译，之后 crates / `target` 缓存在 `.dev-cache/`。建议前台 `up` 看编译输出；后台则用 `podman compose -f dev.docker-compose.yml logs -f pic-viewer`。

`PIC_DIR` 会同时挂给应用（容器内 `PIC_ROOT=/data`）和 imgproxy，相对路径才能对上。

只在本机跑 watch、容器只起 imgproxy 时，在 `.env` 里取消注释 `PIC_ROOT` 与 `IMGPROXY_URL`，再：

```bash
podman compose -f dev.docker-compose.yml up imgproxy
set -a && source .env && set +a
cargo leptos watch
```

不设 `IMGPROXY_URL` 时 `/thumb`、`/preview` 回退为原图。

## 健康检查

`GET /health` 返回 `ok`。
