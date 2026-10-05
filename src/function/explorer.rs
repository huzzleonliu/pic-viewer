use crate::function::compress::compress_checked_images;
use crate::function::fs::{
    create_dir, create_file, delete_entry, list_dir, paste_entry, rename_entries_numbered,
    rename_entry,
};
use crate::function::path::{
    is_image_name, number_run, parent_path, rel_name, rewrite_prefix, validate_file_name,
    validate_rename_prefix,
};
use crate::structure::{
    Clipboard, ClipboardItem, ClipboardMode, ExplorerState, FsEntry, SelectedItem,
};
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

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

    pub fn remember_dir_listing(self, path: String, entries: Vec<FsEntry>) {
        let _ = self.dir_listings.try_update(|map| {
            if map.get(&path) != Some(&entries) {
                map.insert(path, entries);
            }
        });
    }

    pub fn on_tree_click(self, item: SelectedItem, shift: bool, additive: bool) {
        if shift {
            self.apply_check_range(&item, additive);
            self.selected.set(Some(item));
            return;
        }
        if additive {
            if !item.path.is_empty() {
                let on = !self.is_checked(&item.path);
                self.set_checked(item.clone(), on);
            }
            self.check_anchor.set(Some(item.path.clone()));
            self.selected.set(Some(item));
            return;
        }
        self.check_anchor.set(Some(item.path.clone()));
        if item.is_dir {
            let keep = self
                .viewed
                .try_get()
                .flatten()
                .is_some_and(|path| parent_path(&path) == item.path);
            if self.browse_dir.try_get().flatten().as_deref() != Some(item.path.as_str()) && !keep {
                self.viewed.set(None);
            }
            self.browse_dir.set(Some(item.path.clone()));
        } else {
            self.browse_dir.set(None);
            if item.is_image {
                self.viewed.set(Some(item.path.clone()));
            }
        }
        self.selected.set(Some(item));
    }

    fn apply_check_range(self, item: &SelectedItem, additive: bool) {
        let expanded = self.expanded_dirs.try_get().unwrap_or_default();
        let listings = self.dir_listings.try_get().unwrap_or_default();
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
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match rename_entries_numbered(paths, prefix, start, end).await {
                Ok(report) => {
                    for (old, new) in &report.renamed {
                        self.retarget_path(old, new);
                    }
                    self.batch_rename.set(None);
                    self.refresh.update(|n| *n += 1);
                    if report.failures.is_empty() {
                        self.status.set(format!("已重命名 {} 个文件", report.ok));
                    } else {
                        self.status.set(format!(
                            "已重命名 {}/{} 个文件",
                            report.ok,
                            report.ok as usize + report.failures.len()
                        ));
                        self.report_failures("批量重命名失败", report.failures);
                    }
                }
                Err(e) => self.status.set(format!("批量重命名失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    pub fn retarget_path(self, old: &str, new: &str) {
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
        self.filter_paths.update(|opt| {
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
        if let Some(anchor) = self.check_anchor.get() {
            let na = map(&anchor);
            if na != anchor {
                self.check_anchor.set(Some(na));
            }
        }
        if let Some(dir) = self.browse_dir.get() {
            let nd = map(&dir);
            if nd != dir {
                self.browse_dir.set(Some(nd));
            }
        }
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
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match compress_checked_images(paths).await {
                Ok(report) => {
                    for (old, new) in &report.converted {
                        self.retarget_path(old, new);
                    }
                    self.refresh.update(|n| *n += 1);
                    self.media_rev.update(|n| *n += 1);
                    let fail_n = report.failures.len();
                    if report.ok == 0 && fail_n == 0 {
                        self.status
                            .set("没有可压缩的图片（已跳过 WebP、GIF 和非图片）".into());
                    } else if fail_n == 0 {
                        self.status.set(format!(
                            "已压缩 {} 张为 WebP，跳过 {} 张",
                            report.ok, report.skipped
                        ));
                    } else {
                        self.status.set(format!(
                            "已压缩 {} 张为 WebP，跳过 {} 张，失败 {} 张",
                            report.ok, report.skipped, fail_n
                        ));
                        self.report_failures("压缩图片失败", report.failures);
                    }
                }
                Err(e) => self.status.set(format!("压缩失败：{e}")),
            }
            self.busy.set(false);
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
        if self.check_anchor.get().as_deref() == Some(path) {
            self.check_anchor.set(None);
        } else if !path.is_empty() {
            let prefix = format!("{path}/");
            if self
                .check_anchor
                .get()
                .is_some_and(|a| a.starts_with(&prefix))
            {
                self.check_anchor.set(None);
            }
        }
        if self.browse_dir.get().as_deref() == Some(path) {
            self.browse_dir.set(None);
        } else if !path.is_empty() {
            let prefix = format!("{path}/");
            if self
                .browse_dir
                .get()
                .is_some_and(|d| d.starts_with(&prefix))
            {
                self.browse_dir.set(None);
            }
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

pub(crate) fn visible_tree_items(
    expanded: &HashSet<String>,
    listings: &HashMap<String, Vec<FsEntry>>,
) -> Vec<SelectedItem> {
    let mut out = vec![SelectedItem::root()];
    walk_visible("", expanded, listings, &mut out);
    out
}

fn walk_visible(
    path: &str,
    expanded: &HashSet<String>,
    listings: &HashMap<String, Vec<FsEntry>>,
    out: &mut Vec<SelectedItem>,
) {
    if !expanded.contains(path) {
        return;
    }
    let Some(children) = listings.get(path) else {
        return;
    };
    for child in children {
        out.push(SelectedItem::from(child));
        if child.is_dir {
            walk_visible(&child.path, expanded, listings, out);
        }
    }
}

pub(crate) fn checkable_range(visible: &[SelectedItem], from: &str, to: &str) -> Vec<SelectedItem> {
    let find = |p: &str| visible.iter().position(|item| item.path == p);
    match (find(from), find(to)) {
        (Some(i), Some(j)) => {
            let (a, b) = if i <= j { (i, j) } else { (j, i) };
            visible[a..=b]
                .iter()
                .filter(|item| !item.path.is_empty())
                .cloned()
                .collect()
        }
        (_, Some(_)) => visible
            .iter()
            .filter(|item| item.path == to && !item.path.is_empty())
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn gallery_dir_from(
    browse_dir: Option<&str>,
    selected: Option<&SelectedItem>,
    viewed: Option<&str>,
) -> String {
    if let Some(dir) = browse_dir {
        return dir.to_string();
    }
    if let Some(path) = viewed {
        return parent_path(path);
    }
    match selected {
        Some(s) if s.is_dir => s.path.clone(),
        Some(s) => parent_path(&s.path),
        None => String::new(),
    }
}

pub(crate) fn next_viewed_for_dir(shown: &[String], current: Option<&str>) -> Option<String> {
    if shown.is_empty() {
        None
    } else if current.is_some_and(|p| shown.iter().any(|x| x == p)) {
        current.map(str::to_string)
    } else {
        shown.first().cloned()
    }
}

#[cfg(test)]
mod tree_select_tests {
    use super::{checkable_range, gallery_dir_from, next_viewed_for_dir, visible_tree_items};
    use crate::structure::{FsEntry, SelectedItem};
    use std::collections::{HashMap, HashSet};

    fn entry(path: &str, is_dir: bool) -> FsEntry {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        FsEntry {
            name,
            path: path.to_string(),
            is_dir,
            is_image: !is_dir && path.ends_with(".jpg"),
            is_text: !is_dir && path.ends_with(".txt"),
        }
    }

    fn sample() -> (HashSet<String>, HashMap<String, Vec<FsEntry>>) {
        let mut listings = HashMap::new();
        listings.insert(
            String::new(),
            vec![entry("photos", true), entry("readme.txt", false)],
        );
        listings.insert(
            "photos".into(),
            vec![entry("photos/a.jpg", false), entry("photos/b.jpg", false)],
        );
        (HashSet::from([String::new()]), listings)
    }

    #[test]
    fn flatten_skips_collapsed_children() {
        let (expanded, listings) = sample();
        let paths: Vec<_> = visible_tree_items(&expanded, &listings)
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["", "photos", "readme.txt"]);
    }

    #[test]
    fn flatten_includes_expanded_children() {
        let (mut expanded, listings) = sample();
        expanded.insert("photos".into());
        let paths: Vec<_> = visible_tree_items(&expanded, &listings)
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(
            paths,
            ["", "photos", "photos/a.jpg", "photos/b.jpg", "readme.txt"]
        );
    }

    #[test]
    fn range_is_inclusive_and_skips_root() {
        let (mut expanded, listings) = sample();
        expanded.insert("photos".into());
        let visible = visible_tree_items(&expanded, &listings);
        let paths: Vec<_> = checkable_range(&visible, "", "photos/b.jpg")
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["photos", "photos/a.jpg", "photos/b.jpg"]);
    }

    #[test]
    fn range_works_backwards() {
        let (expanded, listings) = sample();
        let visible = visible_tree_items(&expanded, &listings);
        let paths: Vec<_> = checkable_range(&visible, "readme.txt", "photos")
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["photos", "readme.txt"]);
    }

    #[test]
    fn gallery_follows_browse_dir_over_viewed() {
        let image = SelectedItem::from_image_path("photos/a.jpg".into());
        assert_eq!(
            gallery_dir_from(Some("animals"), Some(&image), Some("photos/a.jpg")),
            "animals"
        );
        assert_eq!(
            gallery_dir_from(None, Some(&image), Some("photos/a.jpg")),
            "photos"
        );
        assert_eq!(gallery_dir_from(Some(""), None, None), "");
    }

    #[test]
    fn dir_without_images_clears_viewed() {
        assert_eq!(next_viewed_for_dir(&[], Some("a.jpg")), None);
        assert_eq!(next_viewed_for_dir(&[], None), None);
    }

    #[test]
    fn dir_with_images_keeps_or_opens_first() {
        let shown = vec!["photos/a.jpg".into(), "photos/b.jpg".into()];
        assert_eq!(
            next_viewed_for_dir(&shown, Some("photos/b.jpg")).as_deref(),
            Some("photos/b.jpg")
        );
        assert_eq!(
            next_viewed_for_dir(&shown, Some("other.jpg")).as_deref(),
            Some("photos/a.jpg")
        );
        assert_eq!(
            next_viewed_for_dir(&shown, None).as_deref(),
            Some("photos/a.jpg")
        );
    }
}
