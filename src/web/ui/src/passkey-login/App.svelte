<script>
  export let next = "/";
  export let authenticated = false;

  let username = "";
  let pending = false;
  let error = "";
  let mode = "login";

  function bufferFromBase64url(value) {
    const padded = value.replace(/-/g, "+").replace(/_/g, "/");
    const pad = "=".repeat((4 - (padded.length % 4 || 4)) % 4);
    const binary = atob(`${padded}${pad}`);
    const bytes = new Uint8Array(binary.length);

    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }

    return bytes.buffer;
  }

  function base64urlFromBuffer(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = "";

    for (const byte of bytes) {
      binary += String.fromCharCode(byte);
    }

    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
  }

  function normalizeCreationOptions(options) {
    return {
      ...options,
      publicKey: {
        ...options.publicKey,
        challenge: bufferFromBase64url(options.publicKey.challenge),
        user: {
          ...options.publicKey.user,
          id: bufferFromBase64url(options.publicKey.user.id),
        },
        excludeCredentials: (options.publicKey.excludeCredentials ?? []).map((credential) => ({
          ...credential,
          id: bufferFromBase64url(credential.id),
        })),
      },
    };
  }

  function normalizeRequestOptions(options) {
    return {
      ...options,
      publicKey: {
        ...options.publicKey,
        challenge: bufferFromBase64url(options.publicKey.challenge),
        allowCredentials: (options.publicKey.allowCredentials ?? []).map((credential) => ({
          ...credential,
          id: bufferFromBase64url(credential.id),
        })),
      },
    };
  }

  function credentialToJson(credential) {
    const response = {};

    if (credential.response.clientDataJSON) {
      response.clientDataJSON = base64urlFromBuffer(credential.response.clientDataJSON);
    }
    if (credential.response.attestationObject) {
      response.attestationObject = base64urlFromBuffer(credential.response.attestationObject);
    }
    if (credential.response.authenticatorData) {
      response.authenticatorData = base64urlFromBuffer(credential.response.authenticatorData);
    }
    if (credential.response.signature) {
      response.signature = base64urlFromBuffer(credential.response.signature);
    }
    if (credential.response.userHandle) {
      response.userHandle = base64urlFromBuffer(credential.response.userHandle);
    }

    return {
      id: credential.id,
      rawId: base64urlFromBuffer(credential.rawId),
      type: credential.type,
      response,
      clientExtensionResults: credential.getClientExtensionResults?.() ?? {},
      authenticatorAttachment: credential.authenticatorAttachment ?? null,
    };
  }

  async function begin(path) {
    const response = await fetch(path, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
    });

    if (!response.ok) {
      throw new Error(await response.text());
    }

    return response.json();
  }

  async function complete(path, payload) {
    const response = await fetch(path, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      throw new Error(await response.text());
    }
  }

  async function authenticate() {
    const challenge = await begin(`/auth/authenticate_start/${encodeURIComponent(username)}`);
    const requestOptions = normalizeRequestOptions(challenge);
    const credential = await navigator.credentials.get(requestOptions);

    if (!credential) {
      throw new Error("No credential returned by authenticator");
    }

    await complete("/auth/authenticate_finish", credentialToJson(credential));
  }

  async function register() {
    const challenge = await begin(`/auth/register_start/${encodeURIComponent(username)}`);
    const creationOptions = normalizeCreationOptions(challenge);
    const credential = await navigator.credentials.create(creationOptions);

    if (!credential) {
      throw new Error("No credential returned by authenticator");
    }

    await complete("/auth/register_finish", credentialToJson(credential));
  }

  async function submit() {
    error = "";

    if (!username.trim()) {
      error = "Enter a username first.";
      return;
    }

    pending = true;
    try {
      if (mode === "login") {
        await authenticate();
      } else {
        await register();
      }

      window.location.assign(next);
    } catch (err) {
      error = err instanceof Error ? err.message : "Authentication failed.";
    } finally {
      pending = false;
    }
  }
</script>

{#if authenticated}
  <div class="notice">You are already signed in.</div>
{:else}
  <div class="stack">
    <label class="stack">
      <span>Username</span>
      <input bind:value={username} autocomplete="username webauthn" placeholder="alice" />
    </label>

    <div class="actions">
      <button on:click={submit} disabled={pending}>
        {#if pending && mode === "login"}Signing in…{:else}Sign in with passkey{/if}
      </button>
      <button class="secondary" type="button" on:click={() => { mode = "register"; submit(); }} disabled={pending}>
        {#if pending && mode === "register"}Registering…{:else}Register passkey{/if}
      </button>
    </div>

    {#if error}
      <div class="notice error">{error}</div>
    {/if}
  </div>
{/if}
