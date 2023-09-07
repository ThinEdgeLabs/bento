CREATE TABLE IF NOT EXISTS "public"."__diesel_schema_migrations" (
    "version" character varying(50) NOT NULL,
    "run_on" timestamp DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT "__diesel_schema_migrations_pkey" PRIMARY KEY ("version")
) WITH (oids = false);
