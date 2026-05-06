-- Rollback for 20260506120000__remove_webauthn_fix_grants
-- Removes grants[*] subfields, restores old grants field types,
-- and re-adds the credentials table that was removed.

-- Remove grants[*] subfields added in this rollout
REMOVE FIELD grants[*] ON TABLE invitations;
REMOVE FIELD grants[*] ON TABLE pending_access;

-- Restore old grants field definitions
DEFINE FIELD grants ON TABLE invitations TYPE array<object> FLEXIBLE ASSERT $value != NONE DEFAULT [];
DEFINE FIELD grants ON TABLE pending_access TYPE array<object> FLEXIBLE ASSERT $value != NONE DEFAULT [];

-- Re-add credentials table and fields
DEFINE TABLE credentials SCHEMAFULL;

DEFINE FIELD user_id ON TABLE credentials TYPE record<users> ASSERT $value != NONE;
DEFINE FIELD name ON TABLE credentials TYPE string ASSERT $value != NONE;
DEFINE FIELD data ON TABLE credentials TYPE object FLEXIBLE ASSERT $value != NONE;
DEFINE FIELD last_used ON TABLE credentials TYPE datetime ASSERT $value != NONE;
DEFINE FIELD created_at ON TABLE credentials TYPE datetime ASSERT $value != NONE DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE credentials TYPE datetime ASSERT $value != NONE DEFAULT time::now();

DEFINE TABLE OVERWRITE credentials SCHEMAFULL
	PERMISSIONS
		FOR select, create, update, delete WHERE fn::auth::is_admin()
			OR user_id = fn::auth::user_id();
