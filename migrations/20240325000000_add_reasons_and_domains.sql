ALTER TABLE records ADD COLUMN reason_type TEXT;
ALTER TABLE records ADD COLUMN reason_comment TEXT;
ALTER TABLE records ADD COLUMN dkim_domain TEXT;
ALTER TABLE records ADD COLUMN spf_domain TEXT;
