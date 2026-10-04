//! Bound directory enumeration as well as the number of completed residual records.
use std::{
    io,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const ENTRY_LIMIT: usize = 100_000;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Stop {
    Cancelled,
    Limit,
}

impl Stop {
    pub(super) fn warning(&self) -> &'static str {
        match self {
            Self::Cancelled => "检查已停止，结果不完整",
            Self::Limit => "残留检查达到十万条遍历上限，结果不完整",
        }
    }
}

pub(super) fn queue_children(
    mut children: impl Iterator<Item = io::Result<PathBuf>>,
    pending: &mut Vec<PathBuf>,
    remaining: &mut usize,
    cancel: &AtomicBool,
    warnings: &mut Vec<String>,
) -> Option<Stop> {
    loop {
        if cancel.load(Ordering::Acquire) {
            return Some(Stop::Cancelled);
        }
        // Check before advancing the OS iterator. Failed entries consume budget too;
        // otherwise an unreadable or changing directory could bypass the work limit.
        if *remaining == 0 {
            return Some(Stop::Limit);
        }
        let child = children.next()?;
        *remaining -= 1;
        // The current read may finish after Stop was pressed. Do not queue its result.
        if cancel.load(Ordering::Acquire) {
            return Some(Stop::Cancelled);
        }
        match child {
            Ok(path) => pending.push(path),
            Err(_) => warnings.push("部分目录条目读取失败".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_during_a_large_directory_stops_before_the_next_entry() {
        let cancel = AtomicBool::new(false);
        let mut reads = 0;
        let children = (0..20).map(|index| {
            reads += 1;
            if index == 2 {
                cancel.store(true, Ordering::Release);
            }
            Ok(PathBuf::from(format!("entry-{index}")))
        });
        let mut pending = vec![];
        let mut warnings = vec![];
        let mut remaining = ENTRY_LIMIT;
        assert_eq!(
            queue_children(
                children,
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            Some(Stop::Cancelled)
        );
        assert_eq!(reads, 3);
        assert_eq!(pending.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn traversal_budget_includes_unreadable_entries_and_other_directories() {
        let cancel = AtomicBool::new(false);
        let mut remaining = 3;
        let mut pending = vec![];
        let mut warnings = vec![];
        let first = [Ok(PathBuf::from("first"))].into_iter();
        assert_eq!(
            queue_children(first, &mut pending, &mut remaining, &cancel, &mut warnings),
            None
        );
        assert_eq!(remaining, 2);
        let mut reads = 0;
        let children = (0..20).map(|index| {
            reads += 1;
            if index == 0 {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                Ok(PathBuf::from(format!("next-{index}")))
            }
        });
        assert_eq!(
            queue_children(
                children,
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            Some(Stop::Limit)
        );
        assert_eq!(reads, 2);
        assert_eq!(remaining, 0);
        assert_eq!(pending, [PathBuf::from("first"), PathBuf::from("next-1")]);
        assert_eq!(warnings, ["部分目录条目读取失败"]);
        // Even an endless stream of read errors must not allocate unbounded warnings.
        let untouched = std::iter::from_fn(|| -> Option<io::Result<PathBuf>> {
            panic!("an exhausted traversal must not read another directory entry")
        });
        assert_eq!(
            queue_children(
                untouched,
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            Some(Stop::Limit)
        );
    }

    #[test]
    fn completed_empty_and_partly_unreadable_directories_remain_distinct() {
        let cancel = AtomicBool::new(false);
        let mut remaining = 10;
        let mut pending = vec![];
        let mut warnings = vec![];
        assert_eq!(
            queue_children(
                std::iter::empty(),
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            None
        );
        assert!(warnings.is_empty());
        let children = [
            Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            Ok(PathBuf::from("readable")),
        ]
        .into_iter();
        assert_eq!(
            queue_children(
                children,
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            None
        );
        assert_eq!(pending, [PathBuf::from("readable")]);
        assert_eq!(warnings, ["部分目录条目读取失败"]);
        cancel.store(true, Ordering::Release);
        let untouched = std::iter::from_fn(|| -> Option<io::Result<PathBuf>> {
            panic!("a cancelled scan must not read another directory entry")
        });
        assert_eq!(
            queue_children(
                untouched,
                &mut pending,
                &mut remaining,
                &cancel,
                &mut warnings
            ),
            Some(Stop::Cancelled)
        );
    }
}
