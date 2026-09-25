use crate::function::export_checked;
use crate::structure::ExplorerState;
use leptos::prelude::*;

fn start_download(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_href(url);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
    }
}

#[component]
pub(crate) fn ExportBar() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let dest = RwSignal::new("download".to_string());
    let dest_dir = RwSignal::new(String::new());
    let format = RwSignal::new("jpeg".to_string());
    let exporting = RwSignal::new(false);

    view! {
        <div class="export-bar" class:panel-off=move || !state.show_export.get()>
            <span class="mark-label">"导出勾选"</span>
            <span class="mark-label">"导出到"</span>
            <select
                class="filter-select"
                prop:value=move || dest.get()
                on:change=move |ev| dest.set(event_target_value(&ev))
            >
                <option value="download" selected>"下载到本机"</option>
                <option value="current">"当前目录"</option>
                <option value="dir">"特定目录"</option>
            </select>
            <Show when=move || dest.get() == "dir">
                <input
                    class="filter-tag-input export-dir-input"
                    type="text"
                    placeholder="相对托管目录，如 export/out"
                    prop:value=move || dest_dir.get()
                    on:input=move |ev| dest_dir.set(event_target_value(&ev))
                />
            </Show>
            <span class="mark-label">"导出格式"</span>
            <select
                class="filter-select"
                prop:value=move || format.get()
                on:change=move |ev| format.set(event_target_value(&ev))
            >
                <option value="jpeg" selected>"JPEG"</option>
                <option value="png">"PNG"</option>
                <option value="webp">"WebP"</option>
                <option value="gif">"GIF"</option>
                <option value="bmp">"BMP"</option>
                <option value="tiff">"TIFF"</option>
                <option value="original">"原图"</option>
            </select>
            <button
                class="btn filter-apply"
                type="button"
                title="导出已勾选图片"
                disabled=move || {
                    !state.checked.with(|list| list.iter().any(|i| i.is_image))
                        || exporting.get()
                        || state.busy.get()
                        || (dest.get() == "dir" && dest_dir.get().trim().is_empty())
                }
                on:click=move |_| {
                    if exporting.get() || state.busy.get() {
                        return;
                    }
                    let items: Vec<_> = state
                        .checked
                        .with(|list| list.iter().filter(|i| i.is_image).cloned().collect());
                    if items.is_empty() {
                        state.status.set("请先勾选图片".into());
                        return;
                    }
                    let dest_mode = dest.get();
                    let dest_dir_val = if dest_mode == "current" {
                        state.current_dir_rel()
                    } else {
                        dest_dir.get()
                    };
                    if dest_mode == "dir" && dest_dir_val.trim().is_empty() {
                        state.status.set("请填写导出目录".into());
                        return;
                    }
                    let dest_api = if dest_mode == "download" {
                        "download".to_string()
                    } else {
                        "dir".to_string()
                    };
                    let format = format.get();
                    let n = items.len();
                    let paths: Vec<String> = items.into_iter().map(|i| i.path).collect();
                    exporting.set(true);
                    state.busy.set(true);
                    state.status.set(format!("正在导出 {n} 张…"));
                    leptos::task::spawn_local(async move {
                        match export_checked(paths, dest_api.clone(), dest_dir_val, format).await
                        {
                            Ok(report) => {
                                if report.total == 0 {
                                    state.status.set("请先勾选图片".into());
                                } else if report.failures.is_empty() {
                                    state.status.set(format!("已导出 {} 张", report.ok));
                                } else {
                                    let failed = report.failures.len();
                                    state.status.set(format!(
                                        "已导出 {}/{} 张，失败 {failed}",
                                        report.ok, report.total
                                    ));
                                    state.report_failures("导出失败", report.failures);
                                }
                                if let Some(url) = report.download_url {
                                    start_download(&url);
                                }
                                if dest_api == "dir" && report.ok > 0 {
                                    state.refresh.update(|v| *v += 1);
                                }
                            }
                            Err(e) => state.status.set(format!("导出失败：{e}")),
                        }
                        exporting.set(false);
                        state.busy.set(false);
                    });
                }
            >
                "导出"
            </button>
        </div>
    }
}
