CREATE TABLE transfers (
  amount numeric NOT NULL,
  block character varying NOT NULL,
  chain_id bigint NOT NULL,
  from_account character varying NOT NULL,
  height bigint NOT NULL,
  idx bigint NOT NULL,
  module_hash character varying NOT NULL,
  module_name character varying NOT NULL,
  request_key character varying NOT NULL,
  to_account character varying NOT NULL
);

ALTER TABLE ONLY transfers
    ADD CONSTRAINT transfers_pkey PRIMARY KEY (block, chain_id, idx, module_hash, request_key);

CREATE INDEX transfers_from_acct_height_idx
  ON transfers
  USING btree (from_account, height DESC, idx);


CREATE INDEX transfers_to_acct_height_idx_idx
  ON transfers
  USING btree (to_account, height DESC, idx);

ALTER TABLE ONLY transfers
    ADD CONSTRAINT transfers_block_fkey FOREIGN KEY (block) REFERENCES blocks(hash);
