-- What a tab points at: the shell of a shell tab, the address of a browser tab, the file of a file tab.
ALTER TABLE task_tabs ADD COLUMN target TEXT;
UPDATE task_tabs SET target = shell_id WHERE shell_id IS NOT NULL;
