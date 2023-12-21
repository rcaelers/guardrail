window.onload = function () {
  const { startRegistration } = SimpleWebAuthnBrowser;

  const button = document.getElementById("register-button");
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

  async function perform_registration() {
    start_loading();

    const username = document.getElementById("username");
    if (username.value === "") {
      display_error("Please enter username.");
      return;
    }

    const response = await fetch("/auth/register_start/" + username.value, {
      method: "POST",
    });
    if (response.status !== 200) {
      display_error(await response.text());
      return;
    }

    let registration_response;
    try {
      let json = await response.json();
      registration_response = await startRegistration(json["publicKey"]);
    } catch (error) {
      if (error.name === "InvalidStateError") {
        display_error("Authenticator already registered by user");
      } else if (error.name === "NotAllowedError") {
        display_error("User denied consent");
      } else {
        display_error(error);
      }
      return;
    }

    const finish_response = await fetch("/auth/register_finish", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(registration_response),
    });
    if (finish_response.status !== 200) {
      display_error(await finish_response.text());
      return;
    }

    display_info("Registration successful");
    await new Promise((r) => setTimeout(r, 500));
    document.location.href = "/";
  }

  button.addEventListener("click", () => {
    perform_registration();
  });
  username.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      perform_registration();
    }
  });
};
