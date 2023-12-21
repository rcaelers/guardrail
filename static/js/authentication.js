window.onload = function () {
  const { startAuthentication } = SimpleWebAuthnBrowser;

  const button = document.getElementById("login-button");
  const passkey = document.getElementById("passkey");
  const error = document.getElementById("error-label");
  const info = document.getElementById("info-label");

  async function start_loading() {
    error.innerText = "";
    button.classList.add("loading");
    info.classList.add("hidden");
    passkey.classList.add("hidden");
  }

  async function stop_loading() {
    button.classList.remove("loading");
    passkey.classList.remove("hidden");
  }

  async function display_error(error_message) {
    error.innerText = error_message;
    stop_loading();
  }

  async function display_info(info_message) {
    button.classList.add("hidden");
    info.classList.remove("hidden");
    info.innerText = info_message;
  }

  async function perform_authentication() {
    start_loading();

    const username = document.getElementById("username");
    if (username.value === "") {
      display_error("Please enter username.");
      return;
    }

    const response = await fetch("/auth/authenticate_start/" + username.value, {
      method: "POST",
    });
    if (response.status !== 200) {
      display_error(await response.text());
      return;
    }

    let authentication_response;
    try {
      let json = await response.json();
      authentication_response = await startAuthentication(json["publicKey"]);
    } catch (error) {
      display_error(error);
      return;
    }

    const url = "/auth/authenticate_finish";
    const finish_response = await fetch("/auth/authenticate_finish", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(authentication_response),
    });

    if (finish_response.status !== 200) {
      display_error(await finish_response.text());
      return;
    }

    display_info("Authentication successful");
    await new Promise((r) => setTimeout(r, 500));
    document.location.href = "/";
  }

  button.addEventListener("click", () => {
    perform_authentication();
  });
  username.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      perform_authentication();
    }
  });
};
