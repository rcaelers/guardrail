CREATE USER guardrail WITH PASSWORD 'wezei4Joozooz8To';
CREATE DATABASE guardrail;
GRANT ALL PRIVILEGES ON DATABASE guardrail TO guardrail;
\c guardrail;
GRANT ALL ON schema public TO guardrail;
