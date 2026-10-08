use crate::function::fs::paste_entry;
use crate::function::path::{kinds_from_name, rel_name};
use crate::structure::{Clipboard, ClipboardMode, ExplorerState, SelectedItem};
use leptos::prelude::*;

impl ExplorerState {
    pub fn uncheck_all(self) {
        if self.checked.with(|list| list.is_empty()) {
            return;
        }
        self.checked.update(|list| list.clear());
        self.status.set("已取消全选".into());
    }

    fn set_clipboard(self, items: Vec<SelectedItem>, mode: ClipboardMode) {
        if items.is_empty() {
            return;
        }
        let label = match (mode, items.len()) {
            (ClipboardMode::Copy, 1) => format!("已复制：{}", items[0].name),
            (ClipboardMode::Cut, 1) => format!("已剪切：{}", items[0].name),
            (ClipboardMode::Copy, n) => format!("已复制 {n} 项"),
            (ClipboardMode::Cut, n) => format!("已剪切 {n} 项"),
        };
        self.clipboard.set(Some(Clipboard { items, mode }));
        self.status.set(label);
    }

    pub fn copy_selected(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能复制根目录".into());
                return;
            }
            self.set_clipboard(vec![item], ClipboardMode::Copy);
        }
    }

    pub fn cut_selected(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能剪切根目录".into());
                return;
            }
            self.set_clipboard(vec![item], ClipboardMode::Cut);
        }
    }

    pub fn copy_checked(self) {
        let items = self.checked_clipboard_items();
        if items.is_empty() {
            self.status.set("未勾选项目".into());
            return;
        }
        self.set_clipboard(items, ClipboardMode::Copy);
    }

    pub fn cut_checked(self) {
        let items = self.checked_clipboard_items();
        if items.is_empty() {
            self.status.set("未勾选项目".into());
            return;
        }
        self.set_clipboard(items, ClipboardMode::Cut);
    }

    fn checked_clipboard_items(self) -> Vec<SelectedItem> {
        self.checked.with(|list| list.to_vec())
    }

    pub fn paste_clipboard(self) {
        let Some(clip) = self.clipboard.get() else {
            self.status.set("剪贴板为空".into());
            return;
        };
        if self.busy.get() {
            return;
        }
        let dest_dir = self.selected_dir_rel();
        let cut = clip.mode == ClipboardMode::Cut;
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            let mut ok = 0usize;
            let mut last_path = None;
            let mut last_item = None;
            let mut first_err = None;
            for item in &clip.items {
                match paste_entry(item.path.clone(), dest_dir.clone(), cut).await {
                    Ok(new_path) => {
                        ok += 1;
                        last_path = Some(new_path.clone());
                        last_item = Some(item.clone());
                        if cut && new_path != item.path {
                            self.forget_path(&item.path);
                        }
                    }
                    Err(e) => {
                        first_err = Some(e.to_string());
                        break;
                    }
                }
            }
            if let Some(err) = first_err {
                self.try_status(format!("粘贴失败（已完成 {ok} 项）：{err}"));
            } else if let (Some(new_path), Some(item)) = (last_path, last_item) {
                let name = rel_name(&new_path).to_string();
                let (is_image, is_text) = if item.is_dir {
                    (false, false)
                } else {
                    kinds_from_name(&name)
                };
                let _ = self.selected.try_update(|v| {
                    *v = Some(SelectedItem {
                        is_image,
                        is_text,
                        path: new_path,
                        name,
                        is_dir: item.is_dir,
                    });
                });
                let verb = if cut { "已移动" } else { "已粘贴" };
                self.try_status(if clip.items.len() == 1 {
                    format!("{verb}：{}", item.name)
                } else {
                    format!("{verb} {ok} 项")
                });
            }
            if cut && ok == clip.items.len() {
                let _ = self.clipboard.try_update(|v| *v = None);
            }
            self.bump_listings();
            self.try_busy(false);
        });
    }
}
