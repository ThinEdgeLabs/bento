CREATE TABLE events (
    block character varying NOT NULL,
    chain_id bigint NOT NULL,
    height bigint NOT NULL,
    idx bigint NOT NULL,
    module character varying NOT NULL,
    module_hash character varying NOT NULL,
    name character varying NOT NULL,
    params jsonb NOT NULL,
    param_text character varying NOT NULL,
    qual_name character varying NOT NULL,
    request_key character varying NOT NULL
);

ALTER TABLE ONLY events
    ADD CONSTRAINT events_pkey PRIMARY KEY (block, idx, request_key);

ALTER TABLE ONLY events
    ADD CONSTRAINT events_block_fkey FOREIGN KEY (block) REFERENCES blocks(hash);

CREATE INDEX events_height_chainid_idx
  ON events
  USING btree (height DESC, chain_id, idx);

CREATE INDEX events_height_name_expr_expr1_idx
  ON events
  USING btree (height DESC, name, ((params ->> 0)), ((params ->> 1))) WHERE ((name)::text = 'TRANSFER'::text);

CREATE INDEX events_requestkey_idx
  ON events
  USING btree (request_key);