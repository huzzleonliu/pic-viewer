# Pic Viewer

基于 [Leptos](https://leptos.dev/) + Axum 的图片浏览器。服务端接管一个目录（通过环境变量 `PIC_ROOT`），浏览器左侧浏览/管理文件，右侧查看图片。胶片条和预览默认经 imgproxy 缩放；简单调整栏勾选「原图」则拉本地原文件。

路径操作限制在 `PIC_ROOT` 内，会拒绝 `..` 越界。

## 部署（Podman Compose）

默认 `docker-compose.yml` 只拉镜像、不编译。仓库里的 `test/pic/` 会挂到 `/data`，可直接看效果。

```bash
podman compose up -d
```

打开 http://127.0.0.1:3020 。停止：`podman compose down`。

换成自己的相册：

```bash
PIC_DIR=/path/to/photos podman compose up -d
```

指定应用镜像（默认 `ghcr.io/huzzleonliu/pic-viewer:latest`）：

```bash
PIC_VIEWER_IMAGE=ghcr.io/huzzleonliu/pic-viewer:v0.0.1 podman compose up -d
```

浏览器只访问 pic-viewer（3020）。imgproxy 只在内部网络，缩略图/预览由应用转发。

## 开发（热更新）

`dev.docker-compose.yml` 起 imgproxy，并在容器里跑 `cargo leptos watch`：源码挂进去，改文件会自动重编。不需要本机安装 Rust。

```bash
podman compose down   # 避免和生产栈抢 3020
podman compose -f dev.docker-compose.yml up --build
```

打开 http://127.0.0.1:3020。第一次会构建开发镜像并完整编译，之后 crates / `target` 缓存在仓库的 `.dev-cache/` 里。建议前台 `up`，才能直接看到编译输出；后台跑的话用 `podman compose -f dev.docker-compose.yml logs -f pic-viewer`。

换相册：

```bash
PIC_DIR=/path/to/photos podman compose -f dev.docker-compose.yml up --build
```

`PIC_DIR` 会同时挂给应用（`PIC_ROOT=/data`）和 imgproxy，相对路径才能对上。

若只想在本机跑 `cargo leptos watch`：

```bash
podman compose -f dev.docker-compose.yml up imgproxy
export PIC_ROOT="$(pwd)/test/pic"
export IMGPROXY_URL="http://127.0.0.1:8080"
cargo leptos watch
```

不设 `IMGPROXY_URL` 时 `/thumb`、`/preview` 会回退为原图，胶片条会按原图加载。

## 环境变量

| 变量 | 说明 | 默认 |
| --- | --- | --- |
| `PIC_ROOT` | 托管的图片/文件根目录（应用进程） | `./pics` |
| `PIC_DIR` | Compose 挂到容器 `/data` 的宿主机目录 | `./test/pic` |
| `PIC_VIEWER_IMAGE` | 生产 Compose 使用的应用镜像 | `ghcr.io/huzzleonliu/pic-viewer:latest` |
| `IMGPROXY_URL` | imgproxy 根 URL；设置后缩略图和预览走缩放 | 空 |
| `LEPTOS_SITE_ADDR` | 监听地址 | `0.0.0.0:3000`（watch 用 metadata 的 3020） |
| `LEPTOS_SITE_ROOT` | 静态资源目录（生产环境） | `target/site` |
| `LEPTOS_OUTPUT_NAME` | 前端包名 | `pic-viewer` |
| `LEPTOS_SITE_PKG_DIR` | wasm/css 子目录 | `pkg` |

健康检查：`GET /health` 返回 `ok`。

## 自己构建镜像

```bash
podman build -t localhost/pic-viewer:local .
PIC_VIEWER_IMAGE=localhost/pic-viewer:local podman compose up -d
```

## Kubernetes

示例清单在 `k8s/`。把镜像推到你的仓库后修改 `k8s/deployment.yaml` 里的 `image`。

用 PVC 或 `hostPath` 把目标目录挂到容器的 `/data`（`PIC_ROOT=/data`）。
