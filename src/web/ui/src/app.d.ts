// See https://kit.svelte.dev/docs/types#app
import type { User } from '$lib/adapters/types';

declare global {
  namespace App {
    interface Locals {
      user: User | null;
      realUser: User | null; // set when admin is impersonating another user
    }
    // interface PageData {}
    // interface PageState {}
    // interface Platform {}
  }
}

export {};
