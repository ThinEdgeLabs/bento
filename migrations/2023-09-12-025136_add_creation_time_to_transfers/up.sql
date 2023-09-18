ALTER TABLE ONLY transfers
ADD COLUMN creation_time timestamp with time zone NOT NULL DEFAULT current_timestamp;