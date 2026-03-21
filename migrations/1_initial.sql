-- ALTER SYSTEM SET client_min_messages = ...;
-- ALTER SYSTEM SET log_min_messages = ...;
-- SELECT pg_reload_conf();

CREATE SCHEMA IF NOT EXISTS core;
ALTER SCHEMA core OWNER TO guardrail;

-- CREATE ROLE authenticator LOGIN PASSWORD '<password>' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
-- CREATE ROLE guardrail_webuser LOGIN PASSWORD '<password>' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
-- CREATE ROLE guardrail_anonymous NOLOGIN;
-- CREATE ROLE guardrail_apiuser NOLOGIN;
-- GRANT guardrail_anonymous TO authenticator;
-- GRANT guardrail_apiuser TO authenticator;

GRANT USAGE ON SCHEMA core TO guardrail_anonymous;
GRANT USAGE ON SCHEMA core TO guardrail_apiuser;
GRANT USAGE ON SCHEMA core TO guardrail_webuser;
GRANT USAGE ON SCHEMA core TO guardrail;

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

--
-- Products
--
CREATE TABLE core.products (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    accepting_crashes BOOLEAN NOT NULL DEFAULT TRUE
);
ALTER TABLE core.products OWNER TO guardrail;

CREATE OR REPLACE FUNCTION core.get_product_id(product_name TEXT) RETURNS UUID AS $$
BEGIN
    RETURN (
        SELECT id
		FROM core.products
		WHERE name = product_name
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION core.get_product_id OWNER TO guardrail;

--
-- Users
--
CREATE TABLE core.users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    username TEXT NOT NULL UNIQUE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    last_login_at TIMESTAMP
);
ALTER TABLE core.users OWNER TO guardrail;

CREATE INDEX idx_users_is_admin ON core.users (id, is_admin);

--
-- Access Control
--

CREATE TABLE core.user_access (
    user_id UUID NOT NULL REFERENCES core.users (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES core.products (id),
    role TEXT CHECK (
        role IN ('read', 'write', 'admin')
    ),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, product_id)
);
ALTER TABLE core.user_access OWNER TO guardrail;

CREATE OR REPLACE FUNCTION core.get_current_username() RETURNS TEXT AS $$
BEGIN
    RETURN (
        current_setting('request.jwt.claims', TRUE)::json->>'username'::text
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION core.get_current_username OWNER TO guardrail;

CREATE OR REPLACE FUNCTION core.has_access (product_id_param UUID, required_role TEXT) RETURNS BOOLEAN AS $$
BEGIN
    RETURN (
        -- Grant full access if the user is an admin
        core.is_admin()
        OR -- Otherwise, check if the user has the required role for the application
        EXISTS (
            SELECT 1
            FROM core.user_access
            JOIN core.users ON core.user_access.user_id = core.users.id
            WHERE core.users.username = core.get_current_username()
                AND core.user_access.product_id = product_id_param
                AND (
                    -- Admins have all permissions
                    core.user_access.role = 'admin' -- Write access includes both write and read operations
                    OR (
                        core.user_access.role = 'write'
                        AND required_role IN ('read', 'write')
                    ) -- Read access allows only reading
                    OR (
                        core.user_access.role = 'read'
                        AND required_role = 'read'
                    )
                )
        )
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION core.has_access OWNER TO guardrail;

CREATE OR REPLACE FUNCTION core.is_admin () RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin %', core.get_current_username();
    RETURN
    (
        core.get_current_username() = 'admin'
    )
    OR
    (
        SELECT core.users.is_admin
        FROM core.users
        WHERE core.users.username = core.get_current_username()
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
ALTER FUNCTION core.is_admin OWNER TO guardrail;

--
-- Functions
--
CREATE OR REPLACE FUNCTION core.update_updated_timestamp () RETURNS TRIGGER AS $$
BEGIN
    NEW .updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE 'plpgsql';
ALTER FUNCTION core.update_updated_timestamp OWNER TO guardrail;

--
-- Symbols
--
CREATE TABLE core.symbols (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    build_id TEXT NOT NULL,
    module_id TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    product_id UUID NOT NULL REFERENCES core.products (id)
);
ALTER TABLE core.symbols OWNER TO guardrail;

--
-- Crashes
--
CREATE TABLE core.crashes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    report JSONB,
    signature TEXT,
    minidump UUID,
    product_id UUID NOT NULL REFERENCES core.products (id)
);
ALTER TABLE core.crashes OWNER TO guardrail;

--
-- Annotations
--
CREATE TABLE core.annotations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    key TEXT NOT NULL,
    source TEXT NOT NULL CHECK (
        source IN ('submission', 'script', 'user')
    ),
    value TEXT NOT NULL,
    crash_id UUID NOT NULL REFERENCES core.crashes (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES core.products (id),
    UNIQUE (key, crash_id, source)
);
ALTER TABLE core.annotations OWNER TO guardrail;

--
-- Attachments
--
CREATE TABLE core.attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size bigint NOT NULL,
    filename TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    crash_id UUID NOT NULL REFERENCES core.crashes (id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES core.products (id),
    UNIQUE (name, crash_id)
);
ALTER TABLE core.attachments OWNER TO guardrail;

--
-- API Tokens
--
CREATE TABLE core.api_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    description TEXT NOT NULL,
    token_id UUID NOT NULL UNIQUE,
    token_hash TEXT NOT NULL UNIQUE,
    product_id UUID REFERENCES core.products (id),
    user_id UUID REFERENCES core.users (id) ON DELETE CASCADE,
    entitlements TEXT[] NOT NULL CHECK (
        array_length(entitlements, 1) > 0 AND
        entitlements <@ ARRAY['symbol-upload', 'minidump-upload', 'token']::TEXT[]
    ),
    last_used_at TIMESTAMP,
    expires_at TIMESTAMP,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
ALTER TABLE core.api_tokens OWNER TO guardrail;
CREATE INDEX idx_api_tokens_token ON core.api_tokens (token_hash);
CREATE INDEX idx_api_tokens_product ON core.api_tokens (product_id);
CREATE INDEX idx_api_tokens_user ON core.api_tokens (user_id) WHERE user_id IS NOT NULL;


--
-- Credentials
--
CREATE TABLE core.credentials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES core.users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    last_used TIMESTAMP NOT NULL,
    data json NOT NULL
);
ALTER TABLE core.credentials OWNER TO guardrail;

--
-- Sessions
--
CREATE TABLE core.sessions (
    id text PRIMARY KEY NOT NULL,
    expires_at TIMESTAMP with TIME ZONE,
    data bytea NOT NULL
);
ALTER TABLE core.sessions OWNER TO guardrail;

--
-- User Access + Triggers
--

DO $$
DECLARE tbl TEXT;
schema_name TEXT := 'guardrail';
tables_to_apply TEXT [ ] := ARRAY [ 'symbols', 'crashes', 'annotations', 'attachments' ];
BEGIN FOREACH tbl IN ARRAY tables_to_apply
LOOP EXECUTE format(
        'ALTER TABLE core.%I ENABLE ROW LEVEL SECURITY;
         CREATE POLICY policy_can_read ON core.%I FOR SELECT USING (core.has_access(product_id, ''read''));
         CREATE POLICY policy_can_insert ON core.%I FOR INSERT WITH CHECK (core.has_access(product_id, ''write''));
         CREATE POLICY policy_can_update ON core.%I FOR UPDATE USING (core.has_access(product_id, ''write''));
         CREATE POLICY policy_can_delete ON core.%I FOR DELETE USING (core.has_access(product_id, ''write''));',
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
        'GRANT SELECT, INSERT, UPDATE, DELETE ON core.%I TO guardrail_apiuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON core.%I TO guardrail_webuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON core.%I TO guardrail;
         CREATE TRIGGER trigger_updated_at BEFORE UPDATE ON core.%I FOR EACH ROW EXECUTE PROCEDURE core.update_updated_timestamp ();
         REVOKE UPDATE (updated_at, created_at) ON core.%I FROM guardrail_apiuser;
         REVOKE UPDATE (updated_at, created_at) ON core.%I FROM guardrail_webuser;',
        tbl, tbl, tbl, tbl, tbl, tbl
    );
END
LOOP;
END $$;

REVOKE UPDATE (last_login_at) ON core.users FROM guardrail_apiuser;

DO $$
DECLARE tbl TEXT;
schema_name TEXT := 'guardrail';
tables_to_apply TEXT [ ] := ARRAY [ 'credentials', 'sessions' ];
BEGIN FOREACH tbl IN ARRAY tables_to_apply
LOOP EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE, DELETE ON core.%I TO guardrail_webuser;
         GRANT SELECT, INSERT, UPDATE, DELETE ON core.%I TO guardrail;
',
        tbl, tbl
    );
END
LOOP;
END $$;

CREATE TRIGGER trigger_updated_at BEFORE UPDATE ON core.credentials FOR EACH ROW EXECUTE PROCEDURE core.update_updated_timestamp ();
REVOKE UPDATE (updated_at, created_at) ON core.credentials FROM guardrail_webuser;

-- Products

ALTER TABLE core.products ENABLE ROW LEVEL SECURITY;

CREATE POLICY policy_can_read ON core.products FOR SELECT
USING (
        core.has_access(id, 'read')
    );

CREATE POLICY policy_can_insert ON core.products FOR INSERT
WITH CHECK (
        core.is_admin()
    );

CREATE POLICY policy_can_update ON core.products FOR UPDATE
USING (
        core.is_admin ()
    );

CREATE POLICY policy_can_delete ON core.products FOR DELETE
USING (
        core.is_admin ()
    );

-- Users

CREATE OR REPLACE FUNCTION core.is_admin_of_product_of_user (user_id_param UUID) RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin_of_product_of_user % %', user_id_param, core.get_current_username();
	RETURN
	EXISTS ( SELECT 1
			 FROM core.users u1
			 JOIN core.user_access ua1 ON ua1.user_id = u1.id
			 JOIN core.user_access ua2 ON ua1.product_id = ua2.product_id
			 WHERE ua1.role = 'admin' AND u1.username = core.get_current_username() AND ua2.user_id = user_id_param
	       );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

ALTER TABLE core.users ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS policy_can_read ON core.users;
CREATE POLICY policy_can_read ON core.users FOR SELECT
USING (
        core.is_admin()
        OR
        username = core.get_current_username()
        OR
        core.is_admin_of_product_of_user(id)
    );

DROP POLICY IF EXISTS policy_can_update ON core.users;
CREATE POLICY policy_can_update ON core.users FOR UPDATE
USING (
        core.is_admin() OR
        username = core.get_current_username()
    )
WITH CHECK (
        core.is_admin() OR
        (is_admin = (SELECT ui.is_admin FROM core.users ui WHERE ui.id = core.users.id))
    );

DROP POLICY IF EXISTS policy_can_insert ON core.users;
CREATE POLICY policy_can_insert ON core.users FOR INSERT
WITH CHECK (
        core.is_admin() OR
        (is_admin = false)
	);

DROP POLICY IF EXISTS policy_can_delete ON core.users;
CREATE POLICY policy_can_delete ON core.users FOR DELETE
USING (
	    core.is_admin()
	);

-- User Access

ALTER TABLE core.user_access ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION core.get_current_user_id () RETURNS UUID AS $$
BEGIN
	RAISE NOTICE 'get_current_user %', core.get_current_username();
	RETURN
	( SELECT id
	  FROM core.users
      WHERE username = core.get_current_username()
	);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE OR REPLACE FUNCTION core.is_admin_of_product (product_id_param UUID) RETURNS BOOLEAN AS $$
BEGIN
	RAISE NOTICE 'is_admin_of_product % %', product_id_param, core.get_current_username();
	RETURN
	EXISTS ( SELECT 1
			FROM core.users u1
			JOIN core.user_access ua1 ON ua1.user_id = u1.id
			WHERE ua1.role = 'admin' AND u1.username = core.get_current_username() AND ua1.product_id = product_id_param
	    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP POLICY IF EXISTS user_can_read ON core.user_access;
CREATE POLICY user_can_read ON core.user_access FOR
SELECT USING (
        core.is_admin()
        OR
        user_id = core.get_current_user_id()
        OR
        core.is_admin_of_product(product_id)
    );

DROP POLICY IF EXISTS user_can_update ON core.user_access;
CREATE POLICY user_can_update ON core.user_access FOR UPDATE
USING (
        core.has_access(product_id, 'admin')
    )
WITH CHECK (
        core.has_access(product_id, 'admin')
    );

DROP POLICY IF EXISTS  user_can_insert ON core.user_access;
CREATE POLICY user_can_insert ON core.user_access FOR INSERT
WITH CHECK (
        core.has_access(product_id, 'admin')
    );

DROP POLICY IF EXISTS user_can_delete ON core.user_access;
CREATE POLICY user_can_delete ON core.user_access FOR DELETE
USING (
        core.has_access(product_id, 'admin')
    );


-- API Tokens

ALTER TABLE core.api_tokens ENABLE ROW LEVEL SECURITY;

CREATE POLICY policy_token_can_read ON core.api_tokens
FOR SELECT USING (
    core.is_admin() OR
    (
        (user_id = core.get_current_user_id()) AND
        (product_id IS NOT NULL AND core.is_admin_of_product(product_id))
    )
);

CREATE POLICY policy_token_can_insert ON core.api_tokens
FOR INSERT WITH CHECK (
    core.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        core.is_admin_of_product(product_id)
    )
);

CREATE POLICY policy_token_can_update ON core.api_tokens
FOR UPDATE USING (
    core.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        core.is_admin_of_product(product_id)
    )
);

CREATE POLICY policy_token_can_delete ON core.api_tokens
FOR DELETE USING (
    core.is_admin() OR
    (
        product_id IS NOT NULL AND
        user_id IS NOT NULL AND
        core.is_admin_of_product(product_id)
    )
);
