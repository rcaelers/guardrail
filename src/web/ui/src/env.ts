import { defineEnvVars } from '@sveltejs/kit/env';

// Missing variables become an empty string; the API adapter refuses to start
// without GUARDRAIL_API_URL, so a misconfiguration still fails loudly.
export const variables = defineEnvVars({
	GUARDRAIL_API_URL: { schema: (input) => input ?? '' },
	GUARDRAIL_INGESTION_URL: { schema: (input) => input ?? '' }
});
