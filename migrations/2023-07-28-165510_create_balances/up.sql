CREATE TABLE balances (
    account character varying NOT NULL,
    chain_id bigint NOT NULL,
    height bigint NOT NULL,
    qual_name character varying NOT NULL,
    module character varying NOT NULL,
    amount numeric NOT NULL
);

ALTER TABLE ONLY balances
    ADD CONSTRAINT balances_pkey PRIMARY KEY (account, chain_id, qual_name);

CREATE INDEX balances_account_idx
  ON balances
  USING btree (account);

-- CREATE INDEX events_height_name_expr_expr1_idx
--   ON events
--   USING btree (height DESC, name, ((params ->> 0)), ((params ->> 1))) WHERE ((name)::text = 'TRANSFER'::text);

-- CREATE INDEX events_requestkey_idx
--   ON events
--   USING btree (request_key);
