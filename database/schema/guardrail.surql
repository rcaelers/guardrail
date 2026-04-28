-- Guardrail SurrealDB desired-state schema managed by SurrealKit.
--
-- Keep this file as the schema source of truth for local sync and rollout planning.

-- ============================================================
-- Products
-- ============================================================
DEFINE TABLE products SCHEMAFULL;

DEFINE FIELD name ON TABLE products TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD slug ON TABLE products TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD description ON TABLE products TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD color ON TABLE products TYPE string ASSERT $value != NONE DEFAULT "#6b7280";
DEFINE FIELD public ON TABLE products TYPE bool ASSERT $value != NONE DEFAULT false;
DEFINE FIELD accepting_crashes ON TABLE products TYPE bool ASSERT $value != NONE DEFAULT true;
DEFINE FIELD metadata ON TABLE products TYPE object FLEXIBLE ASSERT $value != NONE DEFAULT {};
DEFINE FIELD created_at ON TABLE products TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE products TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_products_name ON TABLE products COLUMNS name UNIQUE;
DEFINE INDEX idx_products_slug ON TABLE products COLUMNS slug UNIQUE;

-- ============================================================
-- Users
-- ============================================================
-- `username` is the JWT subject and the auth identity used by fn::auth::*.
-- `email` is the human-friendly sign-in identifier surfaced by the website.
DEFINE TABLE users SCHEMAFULL;

DEFINE FIELD username ON TABLE users TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD email ON TABLE users TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD name ON TABLE users TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD avatar ON TABLE users TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD is_admin ON TABLE users TYPE bool ASSERT $value != NONE DEFAULT false;
DEFINE FIELD last_login_at ON TABLE users TYPE option<datetime>;
DEFINE FIELD created_at ON TABLE users TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE users TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_users_username ON TABLE users COLUMNS username UNIQUE;
DEFINE INDEX idx_users_email ON TABLE users COLUMNS email UNIQUE;

-- ============================================================
-- User Access (per-product role membership)
-- ============================================================
-- Roles align with the website vocabulary:
--   readonly   - view crashes & symbols
--   readwrite  - + change crash status, add notes, upload symbols
--   maintainer - + merge groups, manage members, delete the product
DEFINE TABLE user_access SCHEMAFULL;

DEFINE FIELD user_id ON TABLE user_access TYPE record<users> ASSERT $value != NONE;
DEFINE FIELD product_id ON TABLE user_access TYPE record<products> ASSERT $value != NONE;
DEFINE FIELD role ON TABLE user_access TYPE string
    ASSERT $value != NONE AND $value INSIDE ["readonly", "readwrite", "maintainer"];
DEFINE FIELD created_at ON TABLE user_access TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_access TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_user_access_pk ON TABLE user_access COLUMNS user_id, product_id UNIQUE;

-- ============================================================
-- Crashes (raw ingest)
-- ============================================================
-- Each row is one crash event. `report` holds the full minidump-stackwalk
-- JSON (modules, threads, frames, system info, etc.) and is the single
-- source of truth for everything the detail pane renders.
--
-- `fingerprint` is computed by the processor and is what binds crashes into
-- a group: all crashes with the same fingerprint (within a product) belong
-- to the same `crash_groups` row. `group_id` denormalizes that link for
-- fast lookup; the processor writes both.
DEFINE TABLE crashes SCHEMAFULL;

DEFINE FIELD product_id ON TABLE crashes TYPE record<products> ASSERT $value != NONE;
DEFINE FIELD group_id ON TABLE crashes TYPE option<record<crash_groups>>;
DEFINE FIELD fingerprint ON TABLE crashes TYPE option<string>;
DEFINE FIELD report ON TABLE crashes TYPE option<object> FLEXIBLE;
DEFINE FIELD minidump ON TABLE crashes TYPE option<uuid>;
DEFINE FIELD created_at ON TABLE crashes TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE crashes TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_crashes_group ON TABLE crashes COLUMNS group_id;
DEFINE INDEX idx_crashes_product ON TABLE crashes COLUMNS product_id;
DEFINE INDEX idx_crashes_fingerprint ON TABLE crashes COLUMNS product_id, fingerprint;

-- ============================================================
-- Crash groups (workflow buckets keyed by processor fingerprint)
-- ============================================================
-- Crashes that share a `fingerprint` (per product) belong to the same group.
-- The group carries no per-crash data: signal + count + status + workflow
-- only. The detail pane lists the crashes in the group (via crashes.group_id)
-- and renders whichever one the user selects.
--
--   crashes in group = SELECT FROM crashes WHERE group_id = $g ORDER BY created_at DESC
--   notes on group   = SELECT FROM annotations WHERE group_id = $g AND source = "user"
--
-- `signal` is duplicated from the crashes for fast list filtering; it's
-- expected to be identical across all crashes in the group (same fingerprint
-- = same crash kind).
DEFINE TABLE crash_groups SCHEMAFULL;

DEFINE FIELD product_id ON TABLE crash_groups TYPE record<products> ASSERT $value != NONE;
DEFINE FIELD fingerprint ON TABLE crash_groups TYPE string
    ASSERT $value != NONE AND $value != "";
DEFINE FIELD signal ON TABLE crash_groups TYPE string ASSERT $value != NONE DEFAULT "";

-- Aggregations over the underlying crashes
DEFINE FIELD count ON TABLE crash_groups TYPE int ASSERT $value != NONE DEFAULT 0;
DEFINE FIELD first_seen ON TABLE crash_groups TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD last_seen ON TABLE crash_groups TYPE datetime ASSERT $value != NONE DEFAULT time::now();

-- Workflow state
DEFINE FIELD status ON TABLE crash_groups TYPE string
    ASSERT $value != NONE AND $value INSIDE ["new", "triaged", "resolved"]
    DEFAULT "new";
DEFINE FIELD assignee ON TABLE crash_groups TYPE option<record<users>>;

DEFINE FIELD created_at ON TABLE crash_groups TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE crash_groups TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_crash_groups_fingerprint ON TABLE crash_groups
    COLUMNS product_id, fingerprint UNIQUE;
DEFINE INDEX idx_crash_groups_status ON TABLE crash_groups COLUMNS product_id, status;
DEFINE INDEX idx_crash_groups_last_seen ON TABLE crash_groups COLUMNS product_id, last_seen;

-- ============================================================
-- Annotations (system- or user-generated context on crashes / groups)
-- ============================================================
-- A single table for everything that reads like "a string attached to a
-- crash": submission metadata, post-processing script output, and free-form
-- user comments.
--
--   source = "submission"  system    key required, attached to a raw crash
--   source = "script"      system    key required, attached to a raw crash
--   source = "user"        user      append-only note, optional key, `author` set
--
-- Either `crash_id` (raw crash) or `group_id` (aggregated group) must be set.
-- `product_id` is always set so permission checks stay cheap.
DEFINE TABLE annotations SCHEMAFULL;

DEFINE FIELD source ON TABLE annotations TYPE string
    ASSERT $value != NONE AND $value INSIDE ["submission", "script", "user"];
DEFINE FIELD key ON TABLE annotations TYPE option<string>
    ASSERT source = "user" OR $value != NONE;
DEFINE FIELD value ON TABLE annotations TYPE string ASSERT $value != NONE;
DEFINE FIELD author ON TABLE annotations TYPE option<string>
    ASSERT source != "user" OR $value != NONE;

DEFINE FIELD crash_id ON TABLE annotations TYPE option<record<crashes>>
    ASSERT $value != NONE OR group_id != NONE;
DEFINE FIELD group_id ON TABLE annotations TYPE option<record<crash_groups>>
    ASSERT $value != NONE OR crash_id != NONE;
DEFINE FIELD product_id ON TABLE annotations TYPE record<products> ASSERT $value != NONE;

DEFINE FIELD created_at ON TABLE annotations TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE annotations TYPE datetime ASSERT $value != NONE DEFAULT time::now();

-- Lookup indexes. Uniqueness for keyed annotations (submission / script) is
-- the app layer's job: SurrealDB doesn't support WHERE-filtered unique
-- indexes, and a blanket unique on (key, crash_id, source) would forbid
-- multiple user notes on the same crash.
DEFINE INDEX idx_annotations_crash ON TABLE annotations COLUMNS crash_id;
DEFINE INDEX idx_annotations_group ON TABLE annotations COLUMNS group_id;
DEFINE INDEX idx_annotations_source ON TABLE annotations COLUMNS source;

-- ============================================================
-- Attachments
-- ============================================================
DEFINE TABLE attachments SCHEMAFULL;

DEFINE FIELD name ON TABLE attachments TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD mime_type ON TABLE attachments TYPE string ASSERT $value != NONE;
DEFINE FIELD size ON TABLE attachments TYPE int ASSERT $value != NONE;
DEFINE FIELD filename ON TABLE attachments TYPE string ASSERT $value != NONE;
DEFINE FIELD storage_path ON TABLE attachments TYPE string ASSERT $value != NONE;
DEFINE FIELD crash_id ON TABLE attachments TYPE record<crashes> ASSERT $value != NONE;
DEFINE FIELD product_id ON TABLE attachments TYPE record<products> ASSERT $value != NONE;
DEFINE FIELD created_at ON TABLE attachments TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE attachments TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_attachments_unique ON TABLE attachments COLUMNS name, crash_id UNIQUE;

-- ============================================================
-- Symbols
-- ============================================================
-- Symbols are populated by the ingest pipeline from the parsed Breakpad header
-- plus the object storage location. Do not add UI-only fields here unless the
-- upload pipeline can actually provide them.
DEFINE TABLE symbols SCHEMAFULL;

DEFINE FIELD product_id ON TABLE symbols TYPE record<products> ASSERT $value != NONE;
DEFINE FIELD os ON TABLE symbols TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD arch ON TABLE symbols TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD build_id ON TABLE symbols TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD module_id ON TABLE symbols TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD storage_path ON TABLE symbols TYPE string ASSERT $value != NONE DEFAULT "";
DEFINE FIELD created_at ON TABLE symbols TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE symbols TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_symbols_product ON TABLE symbols COLUMNS product_id;
DEFINE INDEX idx_symbols_module_build ON TABLE symbols COLUMNS module_id, build_id;

-- ============================================================
-- API Tokens
-- ============================================================
DEFINE TABLE api_tokens SCHEMAFULL;

DEFINE FIELD description ON TABLE api_tokens TYPE string ASSERT $value != NONE;
DEFINE FIELD token_id ON TABLE api_tokens TYPE uuid ASSERT $value != NONE;
DEFINE FIELD token_hash ON TABLE api_tokens TYPE string ASSERT $value != NONE;
DEFINE FIELD product_id ON TABLE api_tokens TYPE option<record<products>>;
DEFINE FIELD user_id ON TABLE api_tokens TYPE option<record<users>>;
DEFINE FIELD entitlements ON TABLE api_tokens TYPE array<string> ASSERT $value != NONE;
DEFINE FIELD last_used_at ON TABLE api_tokens TYPE option<datetime>;
DEFINE FIELD expires_at ON TABLE api_tokens TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE api_tokens TYPE bool ASSERT $value != NONE DEFAULT true;
DEFINE FIELD created_at ON TABLE api_tokens TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE api_tokens TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_api_tokens_token_id ON TABLE api_tokens COLUMNS token_id UNIQUE;
DEFINE INDEX idx_api_tokens_token_hash ON TABLE api_tokens COLUMNS token_hash UNIQUE;
DEFINE INDEX idx_api_tokens_product ON TABLE api_tokens COLUMNS product_id;
DEFINE INDEX idx_api_tokens_user ON TABLE api_tokens COLUMNS user_id;

-- ============================================================
-- Invitations
-- ============================================================
DEFINE TABLE invitations SCHEMAFULL;

DEFINE FIELD code ON TABLE invitations TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD created_by ON TABLE invitations TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD expires_at ON TABLE invitations TYPE option<datetime>;
DEFINE FIELD max_uses ON TABLE invitations TYPE option<int>;
DEFINE FIELD use_count ON TABLE invitations TYPE int ASSERT $value != NONE DEFAULT 0;
DEFINE FIELD is_admin ON TABLE invitations TYPE bool ASSERT $value != NONE DEFAULT false;
DEFINE FIELD grants ON TABLE invitations TYPE array<object> FLEXIBLE ASSERT $value != NONE DEFAULT [];
DEFINE FIELD status ON TABLE invitations TYPE string
    ASSERT $value != NONE AND $value INSIDE ["Active", "Exhausted", "Expired", "Revoked"]
    DEFAULT "Active";
DEFINE FIELD created_at ON TABLE invitations TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE invitations TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_invitations_code ON TABLE invitations COLUMNS code UNIQUE;
DEFINE INDEX idx_invitations_created_by ON TABLE invitations COLUMNS created_by;

-- ============================================================
-- Pending access grants
-- ============================================================
DEFINE TABLE pending_access SCHEMAFULL;

DEFINE FIELD sub ON TABLE pending_access TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD invitation_id ON TABLE pending_access TYPE string ASSERT $value != NONE AND $value != "";
DEFINE FIELD is_admin ON TABLE pending_access TYPE bool ASSERT $value != NONE DEFAULT false;
DEFINE FIELD grants ON TABLE pending_access TYPE array<object> FLEXIBLE ASSERT $value != NONE DEFAULT [];
DEFINE FIELD created_at ON TABLE pending_access TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE INDEX idx_pending_access_sub ON TABLE pending_access COLUMNS sub UNIQUE;
DEFINE INDEX idx_pending_access_invitation ON TABLE pending_access COLUMNS invitation_id;

-- ============================================================
-- Credentials (WebAuthn passkeys)
-- ============================================================
DEFINE TABLE credentials SCHEMAFULL;

DEFINE FIELD user_id ON TABLE credentials TYPE record<users> ASSERT $value != NONE;
DEFINE FIELD name ON TABLE credentials TYPE string ASSERT $value != NONE;
DEFINE FIELD data ON TABLE credentials TYPE object FLEXIBLE ASSERT $value != NONE;
DEFINE FIELD last_used ON TABLE credentials TYPE datetime ASSERT $value != NONE;
DEFINE FIELD created_at ON TABLE credentials TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE credentials TYPE datetime ASSERT $value != NONE DEFAULT time::now();

-- ============================================================
-- Sessions (tower-sessions)
-- ============================================================
DEFINE TABLE sessions SCHEMAFULL;

DEFINE FIELD expires_at ON TABLE sessions TYPE option<datetime>;
DEFINE FIELD data ON TABLE sessions TYPE bytes ASSERT $value != NONE;

-- ============================================================
-- Authorization helpers
-- ============================================================

DEFINE FUNCTION OVERWRITE fn::auth::username() -> option<string> {
	RETURN IF $auth.username != NONE THEN $auth.username ELSE $token.username END;
};

DEFINE FUNCTION OVERWRITE fn::auth::user_id() -> option<record<users>> {
	RETURN (SELECT VALUE id FROM users WHERE username = fn::auth::username() LIMIT 1)[0];
};

DEFINE FUNCTION OVERWRITE fn::auth::is_admin() -> bool {
	RETURN $auth.is_admin = true
		OR $token.is_admin = true
		OR fn::auth::username() = "admin"
		OR array::len((
			SELECT VALUE id
			FROM users
			WHERE username = fn::auth::username()
			  AND is_admin = true
			LIMIT 1
		)) > 0;
};

-- A "product admin" is anyone with the `maintainer` role on that product
-- (or a global admin).
DEFINE FUNCTION OVERWRITE fn::auth::is_product_admin($product_id: record<products>) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = fn::auth::user_id()
			  AND product_id = $product_id
			  AND role = "maintainer"
			LIMIT 1
		)) > 0;
};

-- Required role hierarchy: maintainer > readwrite > readonly. Asking for
-- "readonly" is satisfied by any role; "readwrite" needs readwrite or
-- maintainer; "maintainer" needs maintainer.
DEFINE FUNCTION OVERWRITE fn::auth::has_product_role($product_id: record<products>, $required_role: string) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = fn::auth::user_id()
			  AND product_id = $product_id
			  AND (
					role = "maintainer"
				 OR (role = "readwrite" AND $required_role INSIDE ["readonly", "readwrite"])
				 OR (role = "readonly" AND $required_role = "readonly")
			  )
			LIMIT 1
		)) > 0;
};

DEFINE FUNCTION OVERWRITE fn::auth::shares_admin_product_with_user($user_id: record<users>) -> bool {
	RETURN fn::auth::is_admin()
		OR array::len((
			SELECT VALUE product_id
			FROM user_access
			WHERE user_id = $user_id
			  AND product_id INSIDE (
					SELECT VALUE product_id
					FROM user_access
					WHERE user_id = fn::auth::user_id()
					  AND role = "maintainer"
			  )
			LIMIT 1
		)) > 0;
};

-- ============================================================
-- Table permissions
-- ============================================================

DEFINE TABLE OVERWRITE products SCHEMAFULL
	PERMISSIONS
		FOR select WHERE public = true OR fn::auth::has_product_role(id, "readonly")
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
		FOR select WHERE product_id.public = true OR fn::auth::has_product_role(product_id, "readonly")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "readwrite");

DEFINE TABLE OVERWRITE crash_groups SCHEMAFULL
	PERMISSIONS
		FOR select WHERE product_id.public = true OR fn::auth::has_product_role(product_id, "readonly")
		FOR create, update WHERE fn::auth::has_product_role(product_id, "readwrite")
		FOR delete WHERE fn::auth::is_product_admin(product_id);

DEFINE TABLE OVERWRITE annotations SCHEMAFULL
	PERMISSIONS
		FOR select WHERE product_id.public = true OR fn::auth::has_product_role(product_id, "readonly")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "readwrite");

DEFINE TABLE OVERWRITE attachments SCHEMAFULL
	PERMISSIONS
		FOR select WHERE product_id.public = true OR fn::auth::has_product_role(product_id, "readonly")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "readwrite");

DEFINE TABLE OVERWRITE symbols SCHEMAFULL
	PERMISSIONS
		FOR select WHERE product_id.public = true OR fn::auth::has_product_role(product_id, "readonly")
		FOR create, update, delete WHERE fn::auth::has_product_role(product_id, "readwrite");

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
