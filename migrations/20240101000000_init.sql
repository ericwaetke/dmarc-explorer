CREATE TABLE IF NOT EXISTS reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    org_name TEXT NOT NULL,
    email TEXT NOT NULL,
    report_id TEXT NOT NULL UNIQUE,
    begin_date INTEGER NOT NULL,
    end_date INTEGER NOT NULL,
    domain TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    report_id TEXT NOT NULL,
    source_ip TEXT NOT NULL,
    count INTEGER NOT NULL,
    disposition TEXT NOT NULL,
    dkim_result TEXT,
    spf_result TEXT,
    FOREIGN KEY (report_id) REFERENCES reports (report_id) ON DELETE CASCADE
);
