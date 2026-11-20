const SIDEBAR_KEY = "brachio.sidebarOpen";
const INSPECTOR_KEY = "brachio.inspectorOpen";

function load(key: string): boolean {
  try {
    return localStorage.getItem(key) !== "false";
  } catch {
    return true;
  }
}

function save(key: string, open: boolean): void {
  try {
    localStorage.setItem(key, String(open));
  } catch {
    // The panel state still applies for this run.
  }
}

export const loadSidebarOpen = () => load(SIDEBAR_KEY);
export const loadInspectorOpen = () => load(INSPECTOR_KEY);
export const saveSidebarOpen = (open: boolean) => save(SIDEBAR_KEY, open);
export const saveInspectorOpen = (open: boolean) => save(INSPECTOR_KEY, open);
