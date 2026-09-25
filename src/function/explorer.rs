use crate::function::fs::{
    create_dir, create_file, delete_entry, list_dir, paste_entry, rename_entry,
};
use crate::function::path::{
    is_image_name, parent_path, rel_name, rewrite_prefix, validate_file_name,
};
use crate::structure::{Clipboard, ClipboardItem, ClipboardMode, ExplorerState, SelectedItem};
use leptos::prelude::*;

impl ExplorerState {
    pub fn is_checked(self, path: &str) -> bool {
        self.checked.with(|list| list.contains(path))
    }

    pub fn set_checked(self, item: SelectedItem, on: bool) {
        if item.path.is_empty() {
            return;
        }
        self.checked.update(|list| {
            list.set_item(item, on);
        });
    }

    pub fn check_all_current(self) {
        if self.busy.get() {
            return;
        }
        let dir = self.current_dir_rel();
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match list_dir(dir).await {
                Ok(entries) => {
                    let items: Vec<SelectedItem> = entries
                        .into_iter()
                        .filter(|e| !e.path.is_empty())
                        .map(SelectedItem::from)
                        .collect();
                    let n = items.len();
                    self.checked.update(|list| {
                        for item in items {
                            list.insert_missing(item);
                        }
                    });
                    self.status.set(if n == 0 {
                        "当前目录没有可勾选项目".into()
                    } else {
                        format!("已全选当前目录 {n} 项")
                    });
                }
                Err(e) => self.status.set(format!("全选失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    pub fn current_dir_rel(self) -> String {
        match self.viewed.get() {
            Some(path) => parent_path(&path),
            None => match self.selected.get() {
                Some(s) if s.is_dir => s.path,
                Some(s) => parent_path(&s.path),
                None => String::new(),
            },
        }
    }

    pub fn selected_dir_rel(self) -> String {
        match self.selected.get() {
            Some(s) if s.is_dir => s.path,
            Some(s) => parent_path(&s.path),
            None => String::new(),
        }
    }
    pub fn uncheck_all(self) {
        if self.checked.with(|list| list.is_empty()) {
            return;
        }
        self.checked.update(|list| list.clear());
        self.status.set("已取消全选".into());
    }

    fn set_clipboard(self, items: Vec<ClipboardItem>, mode: ClipboardMode) {
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
            self.set_clipboard(vec![ClipboardItem::from(&item)], ClipboardMode::Copy);
        }
    }

    pub fn cut_selected(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能剪切根目录".into());
                return;
            }
            self.set_clipboard(vec![ClipboardItem::from(&item)], ClipboardMode::Cut);
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

    fn checked_clipboard_items(self) -> Vec<ClipboardItem> {
        self.checked
            .with(|list| list.iter().map(ClipboardItem::from).collect())
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
                self.status
                    .set(format!("粘贴失败（已完成 {ok} 项）：{err}"));
            } else if let (Some(new_path), Some(item)) = (last_path, last_item) {
                let name = rel_name(&new_path).to_string();
                self.selected.set(Some(SelectedItem {
                    is_image: !item.is_dir && is_image_name(&name),
                    is_text: false,
                    path: new_path,
                    name,
                    is_dir: item.is_dir,
                }));
                let verb = if cut { "已移动" } else { "已粘贴" };
                self.status.set(if clip.items.len() == 1 {
                    format!("{verb}：{}", item.name)
                } else {
                    format!("{verb} {ok} 项")
                });
            }
            if cut && ok == clip.items.len() {
                self.clipboard.set(None);
            }
            self.refresh.update(|n| *n += 1);
            self.busy.set(false);
        });
    }

    pub fn request_mkdir(self) {
        self.mkdir_draft.set("新建文件夹".into());
        self.mkdir_parent.set(Some(self.selected_dir_rel()));
    }

    pub fn request_mkfile(self) {
        self.mkfile_draft.set("新建文件.txt".into());
        self.mkfile_parent.set(Some(self.selected_dir_rel()));
    }

    pub fn confirm_mkfile(self) {
        let Some(parent) = self.mkfile_parent.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        let name = self.mkfile_draft.get();
        if let Err(err) = validate_file_name(&name) {
            self.status.set(err);
            return;
        }
        let name = name.trim().to_string();
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match create_file(parent.clone(), name).await {
                Ok(new_path) => {
                    let name = rel_name(&new_path).to_string();
                    self.expanded_dirs.update(|set| {
                        set.insert(parent);
                    });
                    self.selected.set(Some(SelectedItem {
                        path: new_path,
                        name: name.clone(),
                        is_dir: false,
                        is_image: false,
                        is_text: true,
                    }));
                    self.mkfile_parent.set(None);
                    self.refresh.update(|n| *n += 1);
                    self.status.set(format!("已新建文件：{name}"));
                }
                Err(e) => self.status.set(format!("新建文件失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    pub fn confirm_mkdir(self) {
        let Some(parent) = self.mkdir_parent.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        let name = self.mkdir_draft.get();
        if let Err(err) = validate_file_name(&name) {
            self.status.set(err);
            return;
        }
        let name = name.trim().to_string();
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match create_dir(parent, name).await {
                Ok(new_path) => {
                    let name = rel_name(&new_path).to_string();
                    self.selected.set(Some(SelectedItem {
                        path: new_path,
                        name: name.clone(),
                        is_dir: true,
                        is_image: false,
                        is_text: false,
                    }));
                    self.mkdir_parent.set(None);
                    self.refresh.update(|n| *n += 1);
                    self.status.set(format!("已新建目录：{name}"));
                }
                Err(e) => self.status.set(format!("新建目录失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    pub fn request_rename(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能重命名根目录".into());
                return;
            }
            self.rename_draft.set(item.name.clone());
            self.rename_target.set(Some(item));
        }
    }

    pub fn confirm_rename(self) {
        let Some(target) = self.rename_target.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        let new_name = self.rename_draft.get();
        if let Err(err) = validate_file_name(&new_name) {
            self.status.set(err);
            return;
        }
        let new_name = new_name.trim().to_string();
        if new_name == target.name {
            self.rename_target.set(None);
            return;
        }
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match rename_entry(target.path.clone(), new_name).await {
                Ok(new_path) => {
                    self.retarget_path(&target.path, &new_path);
                    self.rename_target.set(None);
                    self.refresh.update(|n| *n += 1);
                    let name = rel_name(&new_path).to_string();
                    self.status.set(format!("已重命名为：{name}"));
                }
                Err(e) => self.status.set(format!("重命名失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    fn retarget_path(self, old: &str, new: &str) {
        let map = |path: &str| rewrite_prefix(path, old, new);
        let file_name = |path: &str| rel_name(path).to_string();

        if let Some(v) = self.viewed.get() {
            let nv = map(&v);
            if nv != v {
                if is_image_name(&file_name(&nv)) {
                    self.viewed.set(Some(nv));
                } else {
                    self.viewed.set(None);
                }
            }
        }
        if let Some(mut selected) = self.selected.get() {
            let np = map(&selected.path);
            if np != selected.path {
                selected.name = file_name(&np);
                selected.is_image = !selected.is_dir && is_image_name(&selected.name);
                selected.path = np;
                self.selected.set(Some(selected));
            }
        }
        self.checked.update(|list| {
            list.for_each_mut(|item| {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    item.is_image = !item.is_dir && is_image_name(&item.name);
                    item.path = np;
                }
            });
        });
        if let Some(mut clip) = self.clipboard.get() {
            for item in &mut clip.items {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    item.path = np;
                }
            }
            self.clipboard.set(Some(clip));
        }
        self.expanded_dirs.update(|set| {
            let keys: Vec<String> = set.iter().cloned().collect();
            for k in keys {
                let nk = map(&k);
                if nk != k {
                    set.remove(&k);
                    set.insert(nk);
                }
            }
        });
    }

    pub fn request_delete(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能删除根目录".into());
                return;
            }
            self.confirm_delete.set(Some(vec![item]));
        }
    }

    pub fn request_delete_checked(self) {
        let items = self.checked.with(|list| list.to_vec());
        if items.is_empty() {
            self.status.set("未勾选项目".into());
            return;
        }
        self.confirm_delete.set(Some(items));
    }

    pub fn confirm_delete_selected(self) {
        let Some(items) = self.confirm_delete.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            let mut ok = 0usize;
            let mut first_err = None;
            let total = items.len();
            for item in items {
                match delete_entry(item.path.clone()).await {
                    Ok(()) => {
                        ok += 1;
                        self.forget_path(&item.path);
                    }
                    Err(e) => {
                        first_err = Some((item.name, e.to_string()));
                        break;
                    }
                }
            }
            if let Some((name, err)) = first_err {
                self.status
                    .set(format!("删除失败（已完成 {ok} 项，{name}）：{err}"));
            } else {
                self.status.set(if total == 1 {
                    "已删除".into()
                } else {
                    format!("已删除 {ok} 项")
                });
            }
            self.confirm_delete.set(None);
            self.refresh.update(|n| *n += 1);
            self.busy.set(false);
        });
    }

    fn forget_path(self, path: &str) {
        if self.viewed.get().as_deref() == Some(path) {
            self.viewed.set(None);
        }
        if self.selected.get().as_ref().map(|s| s.path.as_str()) == Some(path) {
            self.selected.set(None);
        }
        self.checked.update(|list| list.retain(|i| i.path != path));
        if let Some(clip) = self.clipboard.get() {
            let items: Vec<_> = clip.items.into_iter().filter(|i| i.path != path).collect();
            if items.is_empty() {
                self.clipboard.set(None);
            } else {
                self.clipboard.set(Some(Clipboard {
                    mode: clip.mode,
                    items,
                }));
            }
        }
        if !path.is_empty() {
            let prefix = format!("{path}/");
            self.expanded_dirs.update(|set| {
                set.retain(|p| p != path && !p.starts_with(&prefix));
            });
        }
    }
}
