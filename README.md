# Pic Viewer

基于 [Leptos](https://leptos.dev/) + Axum 的图片浏览器。服务端接管一个目录（通过环境变量 `PIC_ROOT`），浏览器左侧浏览/管理文件，右侧查看图片。

## 功能

- 左侧文件树：展开目录，选择文件
- 复制 / 粘贴 / 删除（工具栏或 `Ctrl+C` / `Ctrl+V` / `Delete`）
- 右侧看图：放大、缩小、旋转、拖拽平移
- 滚轮缩放，`R` 旋转，`←` `→` 同目录切图，`0` 重置

路径操作限制在 `PIC_ROOT` 内，会拒绝 `..` 越界。

## 本地运行

```bash
rustup target add wasm32-unknown-unknown
# 如尚未安装
cargo install cargo-leptos --locked

export PIC_ROOT="$(pwd)/pics"
cargo leptos watch
```

打开 http://127.0.0.1:3000 （默认监听 `0.0.0.0:3000`）。若 3000 已被占用：

```bash
export LEPTOS_SITE_ADDR=127.0.0.1:3020
cargo leptos watch
```

把图片放进 `pics/`（或你设置的 `PIC_ROOT`）即可在左侧看到。

容器调试请用下面的 Podman Compose，测试图在 `test/pic/`。

## 环境变量

| 变量 | 说明 | 默认 |
| --- | --- | --- |
| `PIC_ROOT` | 托管的图片/文件根目录 | `./pics` |
| `LEPTOS_SITE_ADDR` | 监听地址 | `0.0.0.0:3000` |
| `LEPTOS_SITE_ROOT` | 静态资源目录（生产环境） | `target/site` |
| `LEPTOS_OUTPUT_NAME` | 前端包名 | `pic-viewer` |
| `LEPTOS_SITE_PKG_DIR` | wasm/css 子目录 | `pkg` |

健康检查：`GET /health` 返回 `ok`。

## Podman 本机调试

测试目录在 `test/pic/`（含子文件夹、示例 PNG，以及一个非图片 `misc/notes.txt`）。Compose 会把它挂到容器的 `/data`。

```bash
podman compose -f podman-compose.yml up --build
# 或
podman-compose -f podman-compose.yml up --build
```

打开 http://127.0.0.1:3020 。改测试图直接编辑 `test/pic/` 即可，刷新左侧文件树。

停止：`Ctrl+C`，或另开终端执行 `podman compose -f podman-compose.yml down`。

## Docker

```bash
docker build -t pic-viewer .
docker run --rm -p 3000:3000 -v /path/to/photos:/data -e PIC_ROOT=/data pic-viewer
```

## Kubernetes

示例清单在 `k8s/`。把镜像推到你的仓库后修改 `k8s/deployment.yaml` 里的 `image`。

用 PVC 或 `hostPath` 把目标目录挂到容器的 `/data`（`PIC_ROOT=/data`）。
