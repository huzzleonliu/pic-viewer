mod clipboard;
mod tree;

pub(crate) use tree::{checkable_range, gallery_dir_from, visible_tree_items};

use crate::function::compress::compress_checked_images;
use crate::function::fs::{
    create_dir, create_file, delete_entry, list_dir, rename_entries_numbered, rename_entry,
};
use crate::function::nav::{apply_nav, Nav, NavSnapshot};
use crate::function::path::{
    is_image_name, kinds_from_name, number_run, parent_path, rel_name, rewrite_prefix,
    validate_file_name, validate_rename_prefix,
};
use crate::structure::{Clipboard, ExplorerState, SelectedItem};
use leptos::prelude::*;

impl ExplorerState {
    pub fn is_checked(self, path: &str) -> bool {
        self.checked
            .try_with(|list| list.contains(path))
            .unwrap_or(false)
    }

    pub fn set_checked(self, item: SelectedItem, on: bool) {
        if item.path.is_empty() {
            return;
        }
        self.checked.update(|list| {
            list.set_item(item, on);
        });
    }

    pub fn ensure_dir_listing(self, path: String, epoch: u64) {
        let fresh = self
            .dir_listings
            .try_with_untracked(|store| {
                store.get(&path).is_some_and(|listed| listed.epoch == epoch)
            })
            .unwrap_or(false);
        if fresh {
            return;
        }
        let start = self
            .dir_listings
            .try_maybe_update(|store| {
                let start = store.begin_fetch(&path, epoch);
                (start, start)
            })
            .unwrap_or(false);
        if !start {
            return;
        }
        leptos::task::spawn_local(async move {
            let result = list_dir(path.clone(), epoch)
                .await
                .map_err(|e| e.to_string());
            let keep = self.expanded_dirs.try_get().unwrap_or_default();
            let _ = self.dir_listings.try_update(|store| {
                store.finish(path, epoch, result, &keep);
            });
        });
    }

    pub fn navigate(self, nav: Nav) {
        let prev = NavSnapshot {
            selected: self.selected.try_get().flatten(),
            viewed: self.viewed.try_get().flatten(),
            browse_dir: self.browse_dir.try_get().flatten(),
        };
        let next = apply_nav(prev.clone(), nav);
        if prev == next {
            return;
        }
        if prev.selected != next.selected {
            let _ = self.selected.try_update(|v| *v = next.selected);
        }
        if prev.viewed != next.viewed {
            let _ = self.viewed.try_update(|v| *v = next.viewed);
        }
        if prev.browse_dir != next.browse_dir {
            let _ = self.browse_dir.try_update(|v| *v = next.browse_dir);
        }
    }

    pub fn gallery_dir(self) -> String {
        gallery_dir_from(
            self.browse_dir.try_get().flatten().as_deref(),
            self.selected.try_get().flatten().as_ref(),
            self.viewed.try_get().flatten().as_deref(),
        )
    }

    pub fn on_tree_click(self, item: SelectedItem, shift: bool, additive: bool) {
        if shift {
            self.apply_check_range(&item, additive);
            self.navigate(Nav::Focus(item));
            return;
        }
        if additive {
            if !item.path.is_empty() {
                let on = !self.is_checked(&item.path);
                self.set_checked(item.clone(), on);
            }
            let _ = self
                .check_anchor
                .try_update(|v| *v = Some(item.path.clone()));
            self.navigate(Nav::Focus(item));
            return;
        }
        let _ = self
            .check_anchor
            .try_update(|v| *v = Some(item.path.clone()));
        self.navigate(Nav::TreePlain(item));
    }

    fn apply_check_range(self, item: &SelectedItem, additive: bool) {
        let expanded = self.expanded_dirs.try_get().unwrap_or_default();
        let listings = self.dir_listings.try_get().unwrap_or_default().ok_entries();
        let visible = visible_tree_items(&expanded, &listings);
        let from = self
            .check_anchor
            .try_get()
            .flatten()
            .unwrap_or_else(|| item.path.clone());
        let mut range = checkable_range(&visible, &from, &item.path);
        if range.is_empty() && !item.path.is_empty() {
            range.push(item.clone());
        }
        self.checked.update(|list| {
            if !additive {
                list.clear();
            }
            for it in range {
                list.insert_missing(it);
            }
        });
    }

    pub(crate) fn try_status(self, msg: impl Into<String>) {
        let msg = msg.into();
        let _ = self.status.try_update(|s| *s = msg);
    }

    pub(crate) fn try_busy(self, on: bool) {
        let _ = self.busy.try_update(|b| *b = on);
    }

    pub fn check_all_current(self) {
        if self.busy.try_get().unwrap_or(true) {
            return;
        }
        let dir = self.gallery_dir();
        let epoch = self.tree_epoch.try_get().unwrap_or(0);
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match list_dir(dir, epoch).await {
                Ok(entries) => {
                    let items: Vec<SelectedItem> =
                        entries.into_iter().filter(|e| !e.path.is_empty()).collect();
                    let n = items.len();
                    let _ = self.checked.try_update(|list| {
                        for item in items {
                            list.insert_missing(item);
                        }
                    });
                    self.try_status(if n == 0 {
                        "当前目录没有可勾选项目".into()
                    } else {
                        format!("已全选当前目录 {n} 项")
                    });
                }
                Err(e) => self.try_status(format!("全选失败：{e}")),
            }
            self.try_busy(false);
        });
    }

    pub fn current_dir_rel(self) -> String {
        self.gallery_dir()
    }

    pub fn selected_dir_rel(self) -> String {
        match self.selected.get() {
            Some(s) if s.is_dir => s.path,
            Some(s) => parent_path(&s.path),
            None => String::new(),
        }
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
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match create_file(parent.clone(), name).await {
                Ok(new_path) => {
                    let name = rel_name(&new_path).to_string();
                    let (is_image, is_text) = kinds_from_name(&name);
                    let _ = self.expanded_dirs.try_update(|set| {
                        set.insert(parent);
                    });
                    let _ = self.selected.try_update(|v| {
                        *v = Some(SelectedItem {
                            path: new_path,
                            name: name.clone(),
                            is_dir: false,
                            is_image,
                            is_text,
                        });
                    });
                    let _ = self.mkfile_parent.try_update(|v| *v = None);
                    self.bump_tree();
                    self.try_status(format!("已新建文件：{name}"));
                }
                Err(e) => self.try_status(format!("新建文件失败：{e}")),
            }
            self.try_busy(false);
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
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match create_dir(parent, name).await {
                Ok(new_path) => {
                    let name = rel_name(&new_path).to_string();
                    let _ = self.selected.try_update(|v| {
                        *v = Some(SelectedItem {
                            path: new_path,
                            name: name.clone(),
                            is_dir: true,
                            is_image: false,
                            is_text: false,
                        });
                    });
                    let _ = self.mkdir_parent.try_update(|v| *v = None);
                    self.bump_tree();
                    self.try_status(format!("已新建目录：{name}"));
                }
                Err(e) => self.try_status(format!("新建目录失败：{e}")),
            }
            self.try_busy(false);
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
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match rename_entry(target.path.clone(), new_name).await {
                Ok(new_path) => {
                    self.retarget_path(&target.path, &new_path);
                    let _ = self.rename_target.try_update(|v| *v = None);
                    self.bump_listings();
                    let name = rel_name(&new_path).to_string();
                    self.try_status(format!("已重命名为：{name}"));
                }
                Err(e) => self.try_status(format!("重命名失败：{e}")),
            }
            self.try_busy(false);
        });
    }

    pub fn request_batch_rename(self) {
        let files: Vec<SelectedItem> = self.checked.with(|list| {
            list.iter()
                .filter(|item| !item.path.is_empty() && !item.is_dir)
                .cloned()
                .collect()
        });
        if files.is_empty() {
            self.status
                .set("没有可重命名的文件（目录不参与批量重命名）".into());
            return;
        }
        let n = files.len();
        self.batch_rename
            .set(Some(crate::structure::BatchRenameDraft {
                name: String::new(),
                start: "1".into(),
                end: n.to_string(),
                file_count: n,
            }));
    }

    pub fn confirm_batch_rename(self) {
        let Some(draft) = self.batch_rename.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        let prefix = match validate_rename_prefix(&draft.name) {
            Ok(name) => name,
            Err(err) => {
                self.status.set(err);
                return;
            }
        };
        let start = match draft.start.trim().parse::<i32>() {
            Ok(n) => n,
            Err(_) => {
                self.status.set("起始数不是整数".into());
                return;
            }
        };
        let end = match draft.end.trim().parse::<i32>() {
            Ok(n) => n,
            Err(_) => {
                self.status.set("结束数不是整数".into());
                return;
            }
        };
        let nums = number_run(start, end);
        let files: Vec<SelectedItem> = self.checked.with(|list| {
            list.iter()
                .filter(|item| !item.path.is_empty() && !item.is_dir)
                .cloned()
                .collect()
        });
        if files.is_empty() {
            self.status.set("没有可重命名的文件".into());
            return;
        }
        if nums.len() != files.len() {
            self.status.set(format!(
                "序号个数（{}）与已选文件数（{}）不一致",
                nums.len(),
                files.len()
            ));
            return;
        }
        let paths: Vec<String> = files.into_iter().map(|item| item.path).collect();
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match rename_entries_numbered(paths, prefix, start, end).await {
                Ok(report) => {
                    for (old, new) in &report.renamed {
                        self.retarget_path(old, new);
                    }
                    let _ = self.batch_rename.try_update(|v| *v = None);
                    self.bump_listings();
                    if report.failures.is_empty() {
                        self.try_status(format!("已重命名 {} 个文件", report.ok));
                    } else {
                        self.try_status(format!(
                            "已重命名 {}/{} 个文件",
                            report.ok,
                            report.ok as usize + report.failures.len()
                        ));
                        self.report_failures("批量重命名失败", report.failures);
                    }
                }
                Err(e) => self.try_status(format!("批量重命名失败：{e}")),
            }
            self.try_busy(false);
        });
    }

    pub fn retarget_path(self, old: &str, new: &str) {
        let map = |path: &str| rewrite_prefix(path, old, new);
        let file_name = |path: &str| rel_name(path).to_string();

        if let Some(v) = self.viewed.try_get().flatten() {
            let nv = map(&v);
            if nv != v {
                let next = is_image_name(&file_name(&nv)).then_some(nv);
                let _ = self.viewed.try_update(|v| *v = next);
            }
        }
        if let Some(mut selected) = self.selected.try_get().flatten() {
            let np = map(&selected.path);
            if np != selected.path {
                selected.name = file_name(&np);
                selected.is_image = !selected.is_dir && kinds_from_name(&selected.name).0;
                selected.is_text = !selected.is_dir && kinds_from_name(&selected.name).1;
                selected.path = np;
                let _ = self.selected.try_update(|v| *v = Some(selected));
            }
        }
        let _ = self.checked.try_update(|list| {
            list.for_each_mut(|item| {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    let (is_image, is_text) = kinds_from_name(&item.name);
                    item.is_image = !item.is_dir && is_image;
                    item.is_text = !item.is_dir && is_text;
                    item.path = np;
                }
            });
        });
        if let Some(mut clip) = self.clipboard.try_get().flatten() {
            for item in &mut clip.items {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    item.path = np;
                }
            }
            let _ = self.clipboard.try_update(|v| *v = Some(clip));
        }
        let _ = self.expanded_dirs.try_update(|set| {
            let keys: Vec<String> = set.iter().cloned().collect();
            for k in keys {
                let nk = map(&k);
                if nk != k {
                    set.remove(&k);
                    set.insert(nk);
                }
            }
        });
        let _ = self.filter_paths.try_update(|opt| {
            if let Some(set) = opt.as_mut() {
                let keys: Vec<String> = set.iter().cloned().collect();
                for k in keys {
                    let nk = map(&k);
                    if nk != k {
                        set.remove(&k);
                        set.insert(nk);
                    }
                }
            }
        });
        if let Some(anchor) = self.check_anchor.try_get().flatten() {
            let na = map(&anchor);
            if na != anchor {
                let _ = self.check_anchor.try_update(|v| *v = Some(na));
            }
        }
        if let Some(dir) = self.browse_dir.try_get().flatten() {
            let nd = map(&dir);
            if nd != dir {
                let _ = self.browse_dir.try_update(|v| *v = Some(nd));
            }
        }
        let _ = self
            .dir_listings
            .try_update(|store| store.retarget_with(|p| map(p)));
    }

    pub fn compress_checked(self) {
        if self.busy.get() {
            return;
        }
        let paths: Vec<String> = self.checked.with(|list| {
            list.iter()
                .filter(|item| !item.path.is_empty() && !item.is_dir)
                .map(|item| item.path.clone())
                .collect()
        });
        if paths.is_empty() {
            self.status.set("没有可压缩的文件（目录不参与）".into());
            return;
        }
        self.try_busy(true);
        leptos::task::spawn_local(async move {
            match compress_checked_images(paths).await {
                Ok(report) => {
                    for (old, new) in &report.converted {
                        self.retarget_path(old, new);
                    }
                    self.bump_listings();
                    let _ = self.media_rev.try_update(|n| *n += 1);
                    let fail_n = report.failures.len();
                    if report.ok == 0 && fail_n == 0 {
                        self.try_status("没有可压缩的图片（已跳过 WebP、GIF 和非图片）");
                    } else if fail_n == 0 {
                        self.try_status(format!(
                            "已压缩 {} 张为 WebP，跳过 {} 张",
                            report.ok, report.skipped
                        ));
                    } else {
                        self.try_status(format!(
                            "已压缩 {} 张为 WebP，跳过 {} 张，失败 {} 张",
                            report.ok, report.skipped, fail_n
                        ));
                        self.report_failures("压缩图片失败", report.failures);
                    }
                }
                Err(e) => self.try_status(format!("压缩失败：{e}")),
            }
            self.try_busy(false);
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
        self.try_busy(true);
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
                self.try_status(format!("删除失败（已完成 {ok} 项，{name}）：{err}"));
            } else {
                self.try_status(if total == 1 {
                    "已删除".to_string()
                } else {
                    format!("已删除 {ok} 项")
                });
            }
            let _ = self.confirm_delete.try_update(|v| *v = None);
            self.bump_listings();
            self.try_busy(false);
        });
    }

    pub(crate) fn forget_path(self, path: &str) {
        if self.viewed.try_get().flatten().as_deref() == Some(path) {
            let _ = self.viewed.try_update(|v| *v = None);
        }
        if self
            .selected
            .try_get()
            .flatten()
            .as_ref()
            .map(|s| s.path.as_str())
            == Some(path)
        {
            let _ = self.selected.try_update(|v| *v = None);
        }
        let anchor = self.check_anchor.try_get().flatten();
        if anchor.as_deref() == Some(path) {
            let _ = self.check_anchor.try_update(|v| *v = None);
        } else if !path.is_empty() {
            let prefix = format!("{path}/");
            if anchor.is_some_and(|a| a.starts_with(&prefix)) {
                let _ = self.check_anchor.try_update(|v| *v = None);
            }
        }
        let browse = self.browse_dir.try_get().flatten();
        if browse.as_deref() == Some(path) {
            let _ = self.browse_dir.try_update(|v| *v = None);
        } else if !path.is_empty() {
            let prefix = format!("{path}/");
            if browse.is_some_and(|d| d.starts_with(&prefix)) {
                let _ = self.browse_dir.try_update(|v| *v = None);
            }
        }
        let _ = self
            .checked
            .try_update(|list| list.retain(|i| i.path != path));
        if let Some(clip) = self.clipboard.try_get().flatten() {
            let items: Vec<_> = clip.items.into_iter().filter(|i| i.path != path).collect();
            let next = if items.is_empty() {
                None
            } else {
                Some(Clipboard {
                    mode: clip.mode,
                    items,
                })
            };
            let _ = self.clipboard.try_update(|v| *v = next);
        }
        if !path.is_empty() {
            let prefix = format!("{path}/");
            let _ = self.expanded_dirs.try_update(|set| {
                set.retain(|p| p != path && !p.starts_with(&prefix));
            });
            let _ = self
                .dir_listings
                .try_update(|store| store.forget_path(path));
        }
    }
}
