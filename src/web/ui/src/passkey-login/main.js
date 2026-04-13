import { mount } from "svelte";
import App from "./App.svelte";

const target = document.getElementById("passkey-login-island");

if (target) {
  mount(App, {
    target,
    props: {
      next: target.dataset.next ?? "/",
      authenticated: target.dataset.authenticated === "true",
    },
  });
}
