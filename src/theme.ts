/** Switches between Catppuccin Mocha (dark) and Latte (light); see styles/tokens.css. */
export function applyTheme(dark: boolean): void {
  document.documentElement.dataset.theme = dark ? "mocha" : "latte";
}
