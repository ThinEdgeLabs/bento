CREATE TABLE marmalade_v2_collections (
  id character varying PRIMARY KEY,
  name character varying NOT NULL,
  size bigint NOT NULL,
  operator_guard jsonb NOT NULL,
  chain_id bigint NOT NULL,
  block character varying NOT NULL REFERENCES blocks(hash),
  request_key character varying NOT NULL,
  creation_time timestamp with time zone NOT NULL
);

CREATE INDEX marmalade_v2_collections_id_chain_id_idx
  ON marmalade_v2_collections
  USING btree (id, chain_id);

CREATE TABLE marmalade_v2_tokens (
  id character varying PRIMARY KEY,
  collection_id character varying REFERENCES marmalade_v2_collections(id),
  chain_id bigint NOT NULL,
  precision integer NOT NULL,
  uri character varying NOT NULL,
  supply numeric NOT NULL,
  policies jsonb NOT NULL,
  block character varying NOT NULL REFERENCES blocks(hash),
  request_key character varying NOT NULL,
  creation_time timestamp with time zone NOT NULL
);

CREATE TABLE marmalade_v2_balances (
  account character varying NOT NULL,
  guard character varying NOT NULL,
  token_id character varying NOT NULL REFERENCES marmalade_v2_tokens(id),
  amount numeric NOT NULL,
  chain_id bigint NOT NULL,
  PRIMARY KEY (account, token_id)
);

CREATE TABLE marmalade_v2_activity (
  id bigserial PRIMARY KEY,
  token_id character varying NOT NULL REFERENCES marmalade_v2_tokens(id),
  creation_time timestamp with time zone NOT NULL,
  event_type character varying NOT NULL,
  event_data jsonb NOT NULL
);

