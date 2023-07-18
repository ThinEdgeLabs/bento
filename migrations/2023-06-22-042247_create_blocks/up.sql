CREATE TABLE blocks (
    chain_id bigint NOT NULL,
    creation_time timestamp with time zone NOT NULL,
    epoch timestamp with time zone NOT NULL,
    flags numeric(20,0) NOT NULL,
    hash character varying PRIMARY KEY,
    height bigint NOT NULL,
    miner character varying NOT NULL,
    nonce numeric(20,0) NOT NULL,
    parent character varying NOT NULL,
    payload character varying NOT NULL,
    pow_hash character varying NOT NULL,
    predicate character varying NOT NULL,
    target numeric(80,0) NOT NULL,
    weight numeric(80,0) NOT NULL
);

CREATE UNIQUE INDEX blocks_height_chainid_idx ON blocks (height DESC, chain_id);