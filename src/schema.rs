// @generated automatically by Diesel CLI.

diesel::table! {
    blocks (hash) {
        chain_id -> Int8,
        creation_time -> Timestamptz,
        epoch -> Timestamptz,
        flags -> Numeric,
        hash -> Varchar,
        height -> Int8,
        miner -> Varchar,
        nonce -> Numeric,
        parent -> Varchar,
        payload -> Varchar,
        pow_hash -> Varchar,
        predicate -> Varchar,
        target -> Numeric,
        weight -> Numeric,
    }
}

diesel::table! {
    events (block, idx, request_key) {
        block -> Varchar,
        chain_id -> Int8,
        height -> Int8,
        idx -> Int8,
        module -> Varchar,
        module_hash -> Varchar,
        name -> Varchar,
        params -> Jsonb,
        param_text -> Varchar,
        qual_name -> Varchar,
        request_key -> Varchar,
        pact_id -> Nullable<Varchar>,
    }
}

diesel::table! {
    marmalade_v2_activity (id) {
        id -> Int8,
        token_id -> Varchar,
        creation_time -> Timestamptz,
        event_type -> Varchar,
        event_data -> Jsonb,
    }
}

diesel::table! {
    marmalade_v2_balances (account, token_id) {
        account -> Varchar,
        guard -> Varchar,
        token_id -> Varchar,
        amount -> Numeric,
        chain_id -> Int8,
    }
}

diesel::table! {
    marmalade_v2_collections (id) {
        id -> Varchar,
        name -> Varchar,
        size -> Int8,
        operator_guard -> Jsonb,
        chain_id -> Int8,
        block -> Varchar,
        request_key -> Varchar,
        creation_time -> Timestamptz,
    }
}

diesel::table! {
    marmalade_v2_tokens (id) {
        id -> Varchar,
        collection_id -> Nullable<Varchar>,
        chain_id -> Int8,
        precision -> Int4,
        uri -> Varchar,
        supply -> Numeric,
        policies -> Jsonb,
        block -> Varchar,
        request_key -> Varchar,
        creation_time -> Timestamptz,
    }
}

diesel::table! {
    transactions (block, request_key) {
        bad_result -> Nullable<Jsonb>,
        block -> Varchar,
        chain_id -> Int8,
        code -> Nullable<Varchar>,
        continuation -> Nullable<Jsonb>,
        creation_time -> Timestamptz,
        data -> Nullable<Jsonb>,
        gas -> Int8,
        gas_limit -> Int8,
        gas_price -> Float8,
        good_result -> Nullable<Jsonb>,
        height -> Int8,
        logs -> Nullable<Varchar>,
        metadata -> Nullable<Jsonb>,
        nonce -> Varchar,
        num_events -> Nullable<Int8>,
        pact_id -> Nullable<Varchar>,
        proof -> Nullable<Varchar>,
        request_key -> Varchar,
        rollback -> Nullable<Bool>,
        sender -> Varchar,
        step -> Nullable<Int8>,
        ttl -> Int8,
        tx_id -> Nullable<Int8>,
    }
}

diesel::table! {
    transfers (block, chain_id, idx, module_hash, request_key) {
        amount -> Numeric,
        block -> Varchar,
        chain_id -> Int8,
        from_account -> Varchar,
        height -> Int8,
        idx -> Int8,
        module_hash -> Varchar,
        module_name -> Varchar,
        request_key -> Varchar,
        to_account -> Varchar,
        pact_id -> Nullable<Varchar>,
        creation_time -> Timestamptz,
    }
}

diesel::joinable!(events -> blocks (block));
diesel::joinable!(marmalade_v2_activity -> marmalade_v2_tokens (token_id));
diesel::joinable!(marmalade_v2_balances -> marmalade_v2_tokens (token_id));
diesel::joinable!(marmalade_v2_collections -> blocks (block));
diesel::joinable!(marmalade_v2_tokens -> blocks (block));
diesel::joinable!(marmalade_v2_tokens -> marmalade_v2_collections (collection_id));
diesel::joinable!(transactions -> blocks (block));
diesel::joinable!(transfers -> blocks (block));

diesel::allow_tables_to_appear_in_same_query!(
    blocks,
    events,
    marmalade_v2_activity,
    marmalade_v2_balances,
    marmalade_v2_collections,
    marmalade_v2_tokens,
    transactions,
    transfers,
);
