-- This file should undo anything in `up.sql`
ALTER TABLE ONLY transfers
DROP COLUMN creation_time;