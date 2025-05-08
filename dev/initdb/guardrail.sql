CREATE USER guardrail WITH PASSWORD 'wezei4Joozooz8To';
CREATE DATABASE guardrail;
GRANT ALL PRIVILEGES ON DATABASE guardrail TO guardrail;

CREATE DATABASE apalis;
GRANT ALL PRIVILEGES ON DATABASE apalis TO guardrail;

\c apalis;
GRANT ALL ON schema public TO guardrail;

\c guardrail;
GRANT ALL ON schema public TO guardrail;

CREATE ROLE apalis LOGIN PASSWORD 'wezei4Joozooz8To' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
CREATE ROLE authenticator LOGIN PASSWORD 'wezei4Joozooz8To' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
CREATE ROLE guardrail_webuser LOGIN PASSWORD 'wezei4Joozooz8To' NOINHERIT NOCREATEDB NOCREATEROLE NOSUPERUSER;
CREATE ROLE guardrail_anonymous NOLOGIN;
CREATE ROLE guardrail_apiuser NOLOGIN;

GRANT guardrail_anonymous TO authenticator;
GRANT guardrail_apiuser TO authenticator;
