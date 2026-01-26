use std::fs;
use anyhow::{Result, Context};
use crate::core::{CleanupItem, ItemStatus};

pub fn clean_item(item: &mut CleanupItem, dry_run: bool) -> Result<()> {
    if !item.path.exists() {
        return Ok(());
    }

    if dry_run {
        // Just simulate
        item.status = ItemStatus::Deleted; // Visually indicate it *would* be deleted or add a "DryRun" status
        // For MVP, lets just print to helpful logs or stdout if we weren't in TUI mode,
        // but in TUI we update status.
        // Let's rely on the TUI updating the color.
        // To be safe, maybe we don't mark as "Deleted" but "Scanned" with a log?
        // Actually, the user wants "Dry-Run Mode: Default behavior to show what would be deleted."
        // Usually this means printing a plan.
        // If we are in the TUI, pressing "Enter" matches "Clean".
        // If dry_run is true, we simply don't delete.
        return Ok(());
    }

    if item.path.is_file() {
        fs::remove_file(&item.path).context("Failed to delete file")?;
    } else if item.path.is_dir() {
        fs::remove_dir_all(&item.path).context("Failed to delete directory")?;
    }

    item.status = ItemStatus::Deleted;
    Ok(())
}
