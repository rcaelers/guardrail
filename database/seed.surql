--- SEED

-- Ensure the anonymous user record exists so anonymous JWT authentication
-- succeeds. This record has no admin rights and no user_access rows, so
-- SurrealDB RLS restricts it to public-only data.
UPSERT type::record('users', 'anonymous') CONTENT {
    username: 'anonymous',
    email:    'anonymous@localhost',
    name:     'Anonymous',
    avatar:   'A',
    is_admin: false,
    created_at: time::now(),
    updated_at: time::now()
};
