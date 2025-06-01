-- ALTER SYSTEM SET client_min_messages = ...;
-- ALTER SYSTEM SET log_min_messages = ...;
-- SELECT pg_reload_conf();

CREATE SCHEMA IF NOT EXISTS guardrail;

-- CREATE ROLE authenticator LOGIN PASSWORD '<password>' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
-- CREATE ROLE guardrail_webuser LOGIN PASSWORD '<password>' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
-- CREATE ROLE guardrail_anonymous NOLOGIN;
-- CREATE ROLE guardrail_apiuser NOLOGIN;
-- GRANT guardrail_anonymous TO authenticator;
-- GRANT guardrail_apiuser TO authenticator;

GRANT USAGE ON SCHEMA guardrail TO guardrail_anonymous;
GRANT USAGE ON SCHEMA guardrail TO guardrail_apiuser;
GRANT USAGE ON SCHEMA guardrail TO guardrail_webuser;
GRANT USAGE ON SCHEMA guardrail TO guardrail;

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

--
-- Products
--
CREATE TABLE guardrail.products (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    accepting_crashes BOOLEAN NOT NULL DEFAULT TRUE
);
ALTER TABLE guardrail.products OWNER TO guardrail;

CREATE OR REPLACE FUNCTION guardrail.get_product_id(product_name TEXT) RETURNS UUID AS $$
BEGIN
    RETURN (
        SELECT id
		FROM guardrail.products
		WHERE name = product_name
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION guardrail.get_product_id OWNER TO guardrail;

--
-- Users
--
CREATE TABLE guardrail.users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    username TEXT NOT NULL UNIQUE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    last_login_at TIMESTAMP
);
ALTER TABLE guardrail.users OWNER TO guardrail;

CREATE INDEX idx_users_is_admin ON guardrail.users (id, is_admin);

--
-- Access Control
--

CREATE TABLE guardrail.user_access (
    user_id UUID NOT NULL REFERENCES guardrail.users (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES guardrail.products (id),
    role TEXT CHECK (
        role IN ('read', 'write', 'admin')
    ),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, product_id)
);
ALTER TABLE guardrail.user_access OWNER TO guardrail;

CREATE OR REPLACE FUNCTION guardrail.get_current_username() RETURNS TEXT AS $$
BEGIN
    RETURN (
        current_setting('request.jwt.claims', TRUE)::json->>'username'::text
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION guardrail.get_current_username OWNER TO guardrail;

CREATE OR REPLACE FUNCTION guardrail.has_access (product_id_param UUID, required_role TEXT) RETURNS BOOLEAN AS $$
BEGIN
    RETURN (
        -- Grant full access if the user is an admin
        guardrail.is_admin()
        OR -- Otherwise, check if the user has the required role for the application
        EXISTS (
            SELECT 1
            FROM guardrail.user_access
            JOIN guardrail.users ON guardrail.user_access.user_id = guardrail.users.id
            WHERE guardrail.users.username = guardrail.get_current_username()
                AND guardrail.user_access.product_id = product_id_param
                AND (
                    -- Admins have all permissions
                    guardrail.user_access.role = 'admin' -- Write access includes both write and read operations
                    OR (
                        guardrail.user_access.role = 'write'
                        AND required_role IN ('read', 'write')
                    ) -- Read access allows only reading
                    OR (
                        guardrail.user_access.role = 'read'
                        AND required_role = 'read'
                    )
                )
        )
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION guardrail.has_access OWNER TO guardrail;

CREATE OR REPLACE FUNCTION guardrail.is_admin () RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin %', guardrail.get_current_username();
    RETURN
    (
        guardrail.get_current_username() = 'admin'
    )
    OR
    (
        SELECT guardrail.users.is_admin
        FROM guardrail.users
        WHERE guardrail.users.username = guardrail.get_current_username()
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION guardrail.is_admin OWNER TO guardrail;

--
-- Functions
--
CREATE OR REPLACE FUNCTION guardrail.update_updated_timestamp () RETURNS TRIGGER AS $$
BEGIN
    NEW .updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE 'plpgsql';
ALTER FUNCTION guardrail.update_updated_timestamp OWNER TO guardrail;

--
-- Symbols
--
CREATE TABLE guardrail.symbols (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    build_id TEXT NOT NULL,
    module_id TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    product_id UUID NOT NULL REFERENCES guardrail.products (id)
);
ALTER TABLE guardrail.symbols OWNER TO guardrail;

--
-- Crashes
--
CREATE TABLE guardrail.crashes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    info TEXT,
    report JSONB,
    signature TEXT,
    minidump UUID,
    version TEXT,
    channel TEXT,
    commit TEXT,
    build_id TEXT,
    product_id UUID NOT NULL REFERENCES guardrail.products (id)
);
ALTER TABLE guardrail.crashes OWNER TO guardrail;

--
-- Annotations
--
CREATE TABLE guardrail.annotations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    key TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (
        kind IN ('system', 'user')
    ),
    value TEXT NOT NULL,
    crash_id UUID NOT NULL REFERENCES guardrail.crashes (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES guardrail.products (id),
    UNIQUE (key, crash_id)
);
ALTER TABLE guardrail.annotations OWNER TO guardrail;

--
-- Attachments
--
CREATE TABLE guardrail.attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size bigint NOT NULL,
    filename TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    crash_id UUID NOT NULL REFERENCES guardrail.crashes (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES guardrail.products (id),
    UNIQUE (name, crash_id)
);
ALTER TABLE guardrail.attachments OWNER TO guardrail;

--
-- API Tokens
--
CREATE TABLE guardrail.api_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    description TEXT NOT NULL,
    token_id UUID NOT NULL UNIQUE,
    token_hash TEXT NOT NULL UNIQUE,
    product_id UUID REFERENCES guardrail.products (id),
    user_id UUID REFERENCES guardrail.users (id) ON DELETE CASCADE,
    entitlements TEXT[] NOT NULL CHECK (
        array_length(entitlements, 1) > 0 AND
        entitlements <@ ARRAY['symbol-upload', 'minidump-upload', 'token']::TEXT[]
    ),
    last_used_at TIMESTAMP,
    expires_at TIMESTAMP,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
ALTER TABLE guardrail.api_tokens OWNER TO guardrail;
CREATE INDEX idx_api_tokens_token ON guardrail.api_tokens (token_hash);
CREATE INDEX idx_api_tokens_product ON guardrail.api_tokens (product_id);
CREATE INDEX idx_api_tokens_user ON guardrail.api_tokens (user_id) WHERE user_id IS NOT NULL;


--
-- Credentials
--
CREATE TABLE guardrail.credentials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES guardrail.users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    last_used TIMESTAMP NOT NULL,
    data json NOT NULL
);
ALTER TABLE guardrail.credentials OWNER TO guardrail;

--
-- Sessions
--
CREATE TABLE guardrail.sessions (
    id text PRIMARY KEY NOT NULL,
    expires_at TIMESTAMP with TIME ZONE,
    data bytea NOT NULL
);
ALTER TABLE guardrail.sessions OWNER TO guardrail;

--
-- User Access + Triggers
--

DO $$
DECLARE tbl TEXT;
schema_name TEXT := 'guardrail';
tables_to_apply TEXT [ ] := ARRAY [ 'symbols', 'crashes', 'annotations', 'attachments' ];
BEGIN FOREACH tbl IN ARRAY tables_to_apply
LOOP EXECUTE format(
        'ALTER TABLE guardrail.%I ENABLE ROW LEVEL SECURITY;
         CREATE POLICY policy_can_read ON guardrail.%I FOR SELECT USING (guardrail.has_access(product_id, ''read''));
         CREATE POLICY policy_can_insert ON guardrail.%I FOR INSERT WITH CHECK (guardrail.has_access(product_id, ''write''));
         CREATE POLICY policy_can_update ON guardrail.%I FOR UPDATE USING (guardrail.has_access(product_id, ''write''));
         CREATE POLICY policy_can_delete ON guardrail.%I FOR DELETE USING (guardrail.has_access(product_id, ''write''));',
        tbl, tbl, tbl, tbl, tbl
    );
END
LOOP;
END $$;

DO $$
DECLARE tbl TEXT;
schema_name TEXT := 'guardrail';
tables_to_apply TEXT [ ] := ARRAY [ 'users', 'user_access', 'products', 'symbols', 'crashes', 'annotations', 'attachments', 'api_tokens' ];
BEGIN FOREACH tbl IN ARRAY tables_to_apply
LOOP EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE, DELETE ON guardrail.%I TO guardrail_apiuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON guardrail.%I TO guardrail_webuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON guardrail.%I TO guardrail;
         CREATE TRIGGER trigger_updated_at BEFORE UPDATE ON guardrail.%I FOR EACH ROW EXECUTE PROCEDURE guardrail.update_updated_timestamp ();
         REVOKE UPDATE (updated_at, created_at) ON guardrail.%I FROM guardrail_apiuser;
         REVOKE UPDATE (updated_at, created_at) ON guardrail.%I FROM guardrail_webuser;',
        tbl, tbl, tbl, tbl, tbl, tbl
    );
END
LOOP;
END $$;

REVOKE UPDATE (last_login_at) ON guardrail.users FROM guardrail_apiuser;

DO $$
DECLARE tbl TEXT;
schema_name TEXT := 'guardrail';
tables_to_apply TEXT [ ] := ARRAY [ 'credentials', 'sessions' ];
BEGIN FOREACH tbl IN ARRAY tables_to_apply
LOOP EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE, DELETE ON guardrail.%I TO guardrail_webuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON guardrail.%I TO guardrail;
',
        tbl, tbl
    );
END
LOOP;
END $$;

CREATE TRIGGER trigger_updated_at BEFORE UPDATE ON guardrail.credentials FOR EACH ROW EXECUTE PROCEDURE guardrail.update_updated_timestamp ();
REVOKE UPDATE (updated_at, created_at) ON guardrail.credentials FROM guardrail_webuser;

-- Products

ALTER TABLE guardrail.products ENABLE ROW LEVEL SECURITY;

CREATE POLICY policy_can_read ON guardrail.products FOR SELECT
USING (
        guardrail.has_access(id, 'read')
    );

CREATE POLICY policy_can_insert ON guardrail.products FOR INSERT
WITH CHECK (
        guardrail.is_admin()
    );

CREATE POLICY policy_can_update ON guardrail.products FOR UPDATE
USING (
        guardrail.is_admin ()
    );

CREATE POLICY policy_can_delete ON guardrail.products FOR DELETE
USING (
        guardrail.is_admin ()
    );

-- Users

CREATE OR REPLACE FUNCTION guardrail.is_admin_of_product_of_user (user_id_param UUID) RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin_of_product_of_user % %', user_id_param, guardrail.get_current_username();
	RETURN
	EXISTS ( SELECT 1
			 FROM guardrail.users u1
			 JOIN guardrail.user_access ua1 ON ua1.user_id = u1.id
			 JOIN guardrail.user_access ua2 ON ua1.product_id = ua2.product_id
			 WHERE ua1.role = 'admin' AND u1.username = guardrail.get_current_username() AND ua2.user_id = user_id_param
	       );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

ALTER TABLE guardrail.users ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS policy_can_read ON guardrail.users;
CREATE POLICY policy_can_read ON guardrail.users FOR SELECT
USING (
        guardrail.is_admin()
        OR
        username = guardrail.get_current_username()
        OR
        guardrail.is_admin_of_product_of_user(id)
    );

DROP POLICY IF EXISTS policy_can_update ON guardrail.users;
CREATE POLICY policy_can_update ON guardrail.users FOR UPDATE
USING (
        guardrail.is_admin() OR
        username = guardrail.get_current_username()
    )
WITH CHECK (
        guardrail.is_admin() OR
        (is_admin = (SELECT ui.is_admin FROM guardrail.users ui WHERE ui.id = guardrail.users.id))
    );

DROP POLICY IF EXISTS policy_can_insert ON guardrail.users;
CREATE POLICY policy_can_insert ON guardrail.users FOR INSERT
WITH CHECK (
        guardrail.is_admin() OR
        (is_admin = false)
	);

DROP POLICY IF EXISTS policy_can_delete ON guardrail.users;
CREATE POLICY policy_can_delete ON guardrail.users FOR DELETE
USING (
	    guardrail.is_admin()
	);

-- User Access

ALTER TABLE guardrail.user_access ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION guardrail.get_current_user_id () RETURNS UUID AS $$
BEGIN
	RAISE NOTICE 'get_current_user %', guardrail.get_current_username();
	RETURN
	( SELECT id
	  FROM guardrail.users
      WHERE username = guardrail.get_current_username()
	);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE OR REPLACE FUNCTION guardrail.is_admin_of_product (product_id_param UUID) RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin_of_product % %', product_id_param, guardrail.get_current_username();
	RETURN
	EXISTS ( SELECT 1
			FROM guardrail.users u1
			JOIN guardrail.user_access ua1 ON ua1.user_id = u1.id
			WHERE ua1.role = 'admin' AND u1.username = guardrail.get_current_username() AND ua1.product_id = product_id_param
	    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP POLICY IF EXISTS user_can_read ON guardrail.user_access;
CREATE POLICY user_can_read ON guardrail.user_access FOR
SELECT USING (
        guardrail.is_admin()
        OR
        user_id = guardrail.get_current_user_id()
        OR
        guardrail.is_admin_of_product(product_id)
    );

DROP POLICY IF EXISTS user_can_update ON guardrail.user_access;
CREATE POLICY user_can_update ON guardrail.user_access FOR UPDATE
USING (
        guardrail.has_access(product_id, 'admin')
    )
WITH CHECK (
        guardrail.has_access(product_id, 'admin')
    );

DROP POLICY IF EXISTS  user_can_insert ON guardrail.user_access;
CREATE POLICY user_can_insert ON guardrail.user_access FOR INSERT
WITH CHECK (
        guardrail.has_access(product_id, 'admin')
    );

DROP POLICY IF EXISTS user_can_delete ON guardrail.user_access;
CREATE POLICY user_can_delete ON guardrail.user_access FOR DELETE
USING (
        guardrail.has_access(product_id, 'admin')
    );


-- API Tokens

ALTER TABLE guardrail.api_tokens ENABLE ROW LEVEL SECURITY;

CREATE POLICY policy_token_can_read ON guardrail.api_tokens
FOR SELECT USING (
    guardrail.is_admin() OR
    (
        (user_id = guardrail.get_current_user_id()) AND
        (product_id IS NOT NULL AND guardrail.is_admin_of_product(product_id))
    )
);

CREATE POLICY policy_token_can_insert ON guardrail.api_tokens
FOR INSERT WITH CHECK (
    guardrail.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        guardrail.is_admin_of_product(product_id)
    )
);

CREATE POLICY policy_token_can_update ON guardrail.api_tokens
FOR UPDATE USING (
    guardrail.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        guardrail.is_admin_of_product(product_id)
    )
);

CREATE POLICY policy_token_can_delete ON guardrail.api_tokens
FOR DELETE USING (
    guardrail.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        guardrail.is_admin_of_product(product_id)
    )
);
