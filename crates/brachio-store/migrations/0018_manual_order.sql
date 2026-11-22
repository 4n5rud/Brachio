-- Manual order set by drag and drop. NULL means never dragged: those rows keep their old order,
-- and sort after rows that were placed by hand (newest tasks sort before them).
ALTER TABLE repos ADD COLUMN sort_order INTEGER;
ALTER TABLE tasks ADD COLUMN sort_order INTEGER;
ALTER TABLE task_tabs ADD COLUMN sort_order INTEGER;
