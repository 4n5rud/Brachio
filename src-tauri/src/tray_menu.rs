//! What the tray menu lists and the Windows taskbar badge image. Pure, so it can be tested
//! without a tray; `lib.rs` turns the plan into native menu items.

use serde::Deserialize;

/// Tasks listed per section; the section header still shows the full count.
pub const SECTION_LIMIT: usize = 8;
/// Longest task label, in characters, before it is cut with an ellipsis.
pub const LABEL_LIMIT: usize = 60;

#[derive(Debug, Deserialize)]
pub struct TrayTask {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct TrayLabels {
    pub needs: String,
    pub working: String,
    pub new_task: String,
    pub show: String,
    pub quit: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Header(String),
    Task { id: String, label: String },
    Separator,
}

/// Cuts a label to `LABEL_LIMIT` characters and strips line breaks.
pub fn truncate_label(label: &str) -> String {
    let flat: String = label
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.chars().count() <= LABEL_LIMIT {
        return flat.to_owned();
    }
    let kept: String = flat.chars().take(LABEL_LIMIT - 1).collect();
    format!("{}…", kept.trim_end())
}

fn section(entries: &mut Vec<Entry>, title: &str, tasks: &[TrayTask], always_count: bool) {
    if tasks.is_empty() {
        return;
    }
    let header = if always_count || tasks.len() > SECTION_LIMIT {
        format!("{title} · {}", tasks.len())
    } else {
        title.to_owned()
    };
    if !entries.is_empty() {
        entries.push(Entry::Separator);
    }
    entries.push(Entry::Header(header));
    for task in tasks.iter().take(SECTION_LIMIT) {
        entries.push(Entry::Task {
            id: task.id.clone(),
            label: truncate_label(&task.label),
        });
    }
}

/// The task part of the menu: a "Needs you" section, then a "Working" section. Empty sections
/// are left out.
pub fn task_entries(labels: &TrayLabels, needs: &[TrayTask], working: &[TrayTask]) -> Vec<Entry> {
    let mut entries = Vec::new();
    section(&mut entries, &labels.needs, needs, false);
    section(&mut entries, &labels.working, working, true);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> TrayLabels {
        TrayLabels {
            needs: "Needs you".into(),
            working: "Working".into(),
            new_task: "New task".into(),
            show: "Show Brachio".into(),
            quit: "Quit".into(),
        }
    }

    fn task(id: usize) -> TrayTask {
        TrayTask {
            id: format!("t{id}"),
            label: format!("Task {id}"),
        }
    }

    #[test]
    fn empty_sections_are_left_out() {
        assert!(task_entries(&labels(), &[], &[]).is_empty());
        let entries = task_entries(&labels(), &[], &[task(1)]);
        assert_eq!(entries[0], Entry::Header("Working · 1".into()));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn needs_section_comes_first_and_sections_are_separated() {
        let entries = task_entries(&labels(), &[task(1)], &[task(2)]);
        assert_eq!(
            entries,
            vec![
                Entry::Header("Needs you".into()),
                Entry::Task {
                    id: "t1".into(),
                    label: "Task 1".into()
                },
                Entry::Separator,
                Entry::Header("Working · 1".into()),
                Entry::Task {
                    id: "t2".into(),
                    label: "Task 2".into()
                },
            ]
        );
    }

    #[test]
    fn long_sections_list_eight_and_keep_the_full_count() {
        let needs: Vec<_> = (0..12).map(task).collect();
        let entries = task_entries(&labels(), &needs, &[]);
        assert_eq!(entries[0], Entry::Header("Needs you · 12".into()));
        assert_eq!(entries.len(), 1 + 8);
    }

    #[test]
    fn labels_are_cut_with_an_ellipsis_and_flattened() {
        assert_eq!(truncate_label("short"), "short");
        assert_eq!(truncate_label("a\nb\tc"), "a b c");
        let long = "x".repeat(100);
        let cut = truncate_label(&long);
        assert_eq!(cut.chars().count(), 60);
        assert!(cut.ends_with('…'));
        assert_eq!(truncate_label(&"한".repeat(60)).chars().count(), 60);
    }
}
