/** @type {import('tailwindcss').Config} */
let colors = require("tailwindcss/colors");

module.exports = {
  content: ["./**/*.{html,js}", "./static/**/*.{html,js}", "./app/**/*.rs"],
  theme: {
    extend: {
      colors: {
        neutral: colors.slate,
        positive: colors.green,
        urge: colors.violet,
        warning: colors.yellow,
        info: colors.blue,
        critical: colors.red,
      },
    },
  },
  plugins: [require("daisyui"), require("@tailwindcss/typography")],

  daisyui: {
    styled: true,
    themes: false,
    base: true,
    utils: true,
    logs: true,
    darkTheme: "dark",
    prefix: "",
    themeRoot: ":root",
    themes: [
      {
        dark: {
          ...require("daisyui/src/theming/themes")["dark"],

          "primary": "#2a2aca",
          "primary-focus": "#9945FF",
          "primary-content": "#ffffff",

          "base-content": "#f9fafb",
          "base-100": "#181818",
          "base-200": "#35363a",
          "base-300": "#222222",

          "info": "#2094f3",
          "info-content": "#ffffff",
          //success: "#009485",
          "success-content": "#ffffff",
          "warning": "#ff9900",
          "warning-content": "#ffffff",
          "error": "#ff5724",
          "error-content": "#ffffff",
        },
      },
    ],
  },
};
