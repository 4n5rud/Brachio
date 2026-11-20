ALTER TABLE tasks ADD COLUMN worker_report_generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN worker_report_generation INTEGER NOT NULL DEFAULT 0;
