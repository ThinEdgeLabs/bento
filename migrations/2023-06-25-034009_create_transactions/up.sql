CREATE TABLE transactions (
    bad_result jsonb,
    block character varying NOT NULL,
    chain_id bigint NOT NULL,
    code character varying,
    continuation jsonb,
    creation_time timestamp with time zone NOT NULL,
    data jsonb,
    gas bigint NOT NULL,
    gas_limit bigint NOT NULL,
    gas_price double precision NOT NULL,
    good_result jsonb,
    height bigint NOT NULL,
    logs character varying,
    metadata jsonb,
    nonce character varying NOT NULL,
    num_events bigint,
    pact_id character varying,
    proof character varying,
    request_key character varying NOT NULL,
    rollback boolean,
    sender character varying NOT NULL,
    step bigint,
    ttl bigint NOT NULL,
    tx_id bigint
);

ALTER TABLE ONLY transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (block, request_key);

ALTER TABLE ONLY transactions
    ADD CONSTRAINT transactions_block_fkey FOREIGN KEY (block) REFERENCES blocks(hash);

CREATE INDEX transactions_height_idx
  ON transactions
  USING btree (height);

CREATE INDEX transactions_requestkey_idx
  ON transactions
  USING btree (request_key);

CREATE INDEX transactions_pactid_index
  ON transactions (pact_id, (good_result IS NOT NULL) DESC, height DESC)
  WHERE pact_id IS NOT NULL;