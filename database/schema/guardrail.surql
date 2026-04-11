-- Guardrail SurrealDB desired-state schema managed by SurrealKit.
--
-- Keep this file as the schema source of truth for local sync and rollout planning.
-- The application still defines the JWT access method at runtime because it depends on
-- the configured Ed25519 public key.

-- ============================================================
-- Products
-- ============================================================
DEFINE TABLE products SCHEMAFULL;

DEFINE FIELD name ON TABLE products TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD description ON TABLE products TYPE string;
DEFINE FIELD accepting_crashes ON TABLE products TYPE bool DEFAULT true;
DEFINE FIELD metadata ON TABLE products TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD created_at ON TABLE products TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE products TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_products_name ON TABLE products COLUMNS name UNIQUE;

-- ============================================================
-- Users
-- ============================================================
DEFINE TABLE users SCHEMAFULL;

DEFINE FIELD username ON TABLE users TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD is_admin ON TABLE users TYPE bool DEFAULT false;
DEFINE FIELD last_login_at ON TABLE users TYPE option<datetime>;
DEFINE FIELD created_at ON TABLE users TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE users TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_users_username ON TABLE users COLUMNS username UNIQUE;

-- ============================================================
-- User Access (access control per product)
-- ============================================================
DEFINE TABLE user_access SCHEMAFULL;

DEFINE FIELD user_id ON TABLE user_access TYPE uuid;
DEFINE FIELD product_id ON TABLE user_access TYPE uuid;
DEFINE FIELD role ON TABLE user_access TYPE string ASSERT $value INSIDE ["read", "write", "admin"];
DEFINE FIELD created_at ON TABLE user_access TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_access TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_user_access_pk ON TABLE user_access COLUMNS user_id, product_id UNIQUE;

-- ============================================================
-- Crashes
-- ============================================================
DEFINE TABLE crashes SCHEMAFULL;

DEFINE FIELD product_id ON TABLE crashes TYPE uuid;
DEFINE FIELD report ON TABLE crashes TYPE option<object> FLEXIBLE;
DEFINE FIELD signature ON TABLE crashes TYPE option<string>;
DEFINE FIELD minidump ON TABLE crashes TYPE option<uuid>;
DEFINE FIELD created_at ON TABLE crashes TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE crashes TYPE datetime DEFAULT time::now();

-- ============================================================
-- Annotations
-- ============================================================
DEFINE TABLE annotations SCHEMAFULL;

DEFINE FIELD key ON TABLE annotations TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD source ON TABLE annotations TYPE string ASSERT $value INSIDE ["submission", "script", "user"];
DEFINE FIELD value ON TABLE annotations TYPE string;
DEFINE FIELD crash_id ON TABLE annotations TYPE uuid;
DEFINE FIELD product_id ON TABLE annotations TYPE uuid;
DEFINE FIELD created_at ON TABLE annotations TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE annotations TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_annotations_unique ON TABLE annotations COLUMNS key, crash_id, source UNIQUE;

-- ============================================================
-- Attachments
-- ============================================================
DEFINE TABLE attachments SCHEMAFULL;

DEFINE FIELD name ON TABLE attachments TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD mime_type ON TABLE attachments TYPE string;
DEFINE FIELD size ON TABLE attachments TYPE int;
DEFINE FIELD filename ON TABLE attachments TYPE string;
DEFINE FIELD storage_path ON TABLE attachments TYPE string;
DEFINE FIELD crash_id ON TABLE attachments TYPE uuid;
DEFINE FIELD product_id ON TABLE attachments TYPE uuid;
DEFINE FIELD created_at ON TABLE attachments TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE attachments TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_attachments_unique ON TABLE attachments COLUMNS name, crash_id UNIQUE;

-- ============================================================
-- Symbols
-- ============================================================
DEFINE TABLE symbols SCHEMAFULL;

DEFINE FIELD os ON TABLE symbols TYPE string;
DEFINE FIELD arch ON TABLE symbols TYPE string;
DEFINE FIELD build_id ON TABLE symbols TYPE string;
DEFINE FIELD module_id ON TABLE symbols TYPE string;
DEFINE FIELD storage_path ON TABLE symbols TYPE string;
DEFINE FIELD product_id ON TABLE symbols TYPE uuid;
DEFINE FIELD created_at ON TABLE symbols TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE symbols TYPE datetime DEFAULT time::now();

-- ============================================================
-- API Tokens
-- ============================================================
DEFINE TABLE api_tokens SCHEMAFULL;

DEFINE FIELD description ON TABLE api_tokens TYPE string;
DEFINE FIELD token_id ON TABLE api_tokens TYPE uuid;
DEFINE FIELD token_hash ON TABLE api_tokens TYPE string;
DEFINE FIELD product_id ON TABLE api_tokens TYPE option<uuid>;
DEFINE FIELD user_id ON TABLE api_tokens TYPE option<uuid>;
DEFINE FIELD entitlements ON TABLE api_tokens TYPE array<string>;
DEFINE FIELD last_used_at ON TABLE api_tokens TYPE option<datetime>;
DEFINE FIELD expires_at ON TABLE api_tokens TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE api_tokens TYPE bool DEFAULT true;
DEFINE FIELD created_at ON TABLE api_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE api_tokens TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_api_tokens_token_id ON TABLE api_tokens COLUMNS token_id UNIQUE;
DEFINE INDEX idx_api_tokens_token_hash ON TABLE api_tokens COLUMNS token_hash UNIQUE;
DEFINE INDEX idx_api_tokens_product ON TABLE api_tokens COLUMNS product_id;
DEFINE INDEX idx_api_tokens_user ON TABLE api_tokens COLUMNS user_id;

-- ============================================================
-- Credentials (WebAuthn passkeys)
-- ============================================================
DEFINE TABLE credentials SCHEMAFULL;

DEFINE FIELD user_id ON TABLE credentials TYPE uuid;
DEFINE FIELD name ON TABLE credentials TYPE string;
DEFINE FIELD data ON TABLE credentials TYPE object FLEXIBLE;
DEFINE FIELD last_used ON TABLE credentials TYPE datetime;
DEFINE FIELD created_at ON TABLE credentials TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE credentials TYPE datetime DEFAULT time::now();

-- ============================================================
-- Sessions (tower-sessions)
-- ============================================================
DEFINE TABLE sessions SCHEMAFULL;

DEFINE FIELD expires_at ON TABLE sessions TYPE option<datetime>;
DEFINE FIELD data ON TABLE sessions TYPE bytes;

-- ============================================================
-- Authorization helpers
-- ============================================================

DEFINE FUNCTION fn::auth::username() -> option<string> {
	RETURN $auth.username;
};

DEFINE FUNCTION fn::auth::user_id() -> option<uuid> {
	RETURN (SELECT VALUE id FROM users WHERE username = fn::auth::username() LIMIT 1)[0];
};

DEFINE FUNCTION fn::auth::is_admin() -> bool {
	RETURN $auth.is_admin = true
		OR fn::auth::username() = "admin"
		OR array::len((
			SELECT VALUE id
			FROM users
			WHERE username = fn::auth::username()
			  AND is_admin = true
			LIMIT 1
		)) > 0;
};

DEFINE FUNCTION fn::auth::is_product_admin($product_id: uuid) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = fn::auth::user_id()
			  AND product_id = $product_id
			  AND role = "admin"
			LIMIT 1
		)) > 0;
};

DEFINE FUNCTION fn::auth::has_product_role($product_id: uuid, $required_role: string) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = fn::auth::user_id()
			  AND product_id = $product_id
			  AND (
					role = "admin"
				 OR (role = "write" AND $required_role INSIDE ["read", "write"])
				 OR (role = "read" AND $required_role = "read")
			  )
			LIMIT 1
		)) > 0;
};

DEFINE FUNCTION fn::auth::shares_admin_product_with_user($user_id: uuid) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = $user_id
			  AND product_id INSIDE (
					SELECT VALUE product_id
					FROM user_access
					WHERE user_id = fn::auth::user_id()
					  AND role = "admin"
			  )
			LIMIT 1
		)) > 0;
};

-- ============================================================
-- Table permissions
-- ============================================================

DEFINE TABLE OVERWRITE products SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::has_product_role(id, "read")
		FOR create WHERE fn::auth::is_admin()
		FOR update WHERE fn::auth::is_admin()
		FOR delete WHERE fn::auth::is_admin();

DEFINE TABLE OVERWRITE users SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::is_admin()
			OR username = fn::auth::username()
			OR fn::auth::shares_admin_product_with_user(id)
		FOR create WHERE fn::auth::is_admin() OR is_admin = false
		FOR update WHERE fn::auth::is_admin()
			OR (id = fn::auth::user_id() AND is_admin = false)
		FOR delete WHERE fn::auth::is_admin();

DEFINE TABLE OVERWRITE user_access SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::is_admin()
			OR user_id = fn::auth::user_id()
			OR fn::auth::is_product_admin(product_id)
		FOR create, update, delete WHERE fn::auth::is_product_admin(product_id);

DEFINE TABLE OVERWRITE crashes SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::has_product_role(product_id, "read")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "write");

DEFINE TABLE OVERWRITE annotations SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::has_product_role(product_id, "read")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "write");

DEFINE TABLE OVERWRITE attachments SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::has_product_role(product_id, "read")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "write");

DEFINE TABLE OVERWRITE symbols SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::has_product_role(product_id, "read")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "write");

DEFINE TABLE OVERWRITE api_tokens SCHEMAFULL
	PERMISSIONS
		FOR select WHERE fn::auth::is_admin()
			OR user_id = fn::auth::user_id()
			OR (product_id != NONE AND fn::auth::is_product_admin(product_id))
		FOR create, update, delete WHERE fn::auth::is_admin()
			OR (product_id != NONE AND fn::auth::is_product_admin(product_id));

DEFINE TABLE OVERWRITE credentials SCHEMAFULL
	PERMISSIONS
		FOR select, create, update, delete WHERE fn::auth::is_admin()
			OR user_id = fn::auth::user_id();

DEFINE TABLE OVERWRITE sessions SCHEMAFULL PERMISSIONS NONE;
