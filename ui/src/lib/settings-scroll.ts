/** A section of the settings page that should be scrolled into view once its page has opened. */
export type SettingsSection = "analytics";

let wanted: SettingsSection | undefined;

/** Asks for `section` to be shown the next time the settings page that holds it draws. */
export function wantSettingsSection(section: SettingsSection): void {
  wanted = section;
}

/** True once, when `section` was asked for and has not been shown yet. */
export function takeSettingsSection(section: SettingsSection): boolean {
  const asked = wanted === section;
  if (asked) wanted = undefined;
  return asked;
}
