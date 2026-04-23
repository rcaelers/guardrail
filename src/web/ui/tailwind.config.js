/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'monospace']
      },
      colors: {
        // Mirror of the prototype's aTheme — tweak here and it's global.
        accent: {
          DEFAULT: '#3b6fd4',
          soft: '#e8efff',
          softDark: 'rgba(59,111,212,0.18)'
        },
        ink: {
          DEFAULT: '#18181b',
          dark: '#e8e8ea',
          muted: '#71717a',
          mutedDark: '#8c8c95'
        },
        surface: {
          DEFAULT: '#ffffff',
          panel: '#fafafa',
          dark: '#151517',
          panelDark: '#1b1b1e'
        },
        line: {
          DEFAULT: '#ececec',
          dark: '#232326'
        },
        signal: {
          danger: '#c0392b',
          dangerDark: '#e06666',
          warn: '#a86a00',
          warnDark: '#e0b050',
          ok: '#2f7d3b',
          okDark: '#5cb370'
        }
      }
    }
  },
  plugins: []
};
