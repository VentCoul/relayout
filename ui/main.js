const { invoke } = window.__TAURI__.core;

const hotkeyInput = document.getElementById("hotkey");
const errorBox = document.getElementById("error");
const autostart = document.getElementById("autostart");
const permission = document.getElementById("permission");

const isMac = navigator.platform.toLowerCase().includes("mac");
const LABELS = isMac
  ? { Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" }
  : { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Super: "Win" };

let saved = "";

function pretty(hotkey) {
  return hotkey
    .split("+")
    .map((part) => LABELS[part] ?? part.replace(/^Key|^Digit/, ""))
    .join(isMac ? " " : " + ");
}

function showError(message) {
  errorBox.textContent = message ?? "";
  errorBox.hidden = !message;
}

async function refresh() {
  const settings = await invoke("get_settings");
  saved = settings.hotkey;
  hotkeyInput.value = pretty(saved);
  autostart.checked = settings.autostart;
  permission.hidden = settings.permission;
}

hotkeyInput.addEventListener("focus", () => {
  hotkeyInput.value = "Натисни комбінацію…";
});
hotkeyInput.addEventListener("blur", () => {
  hotkeyInput.value = pretty(saved);
});

hotkeyInput.addEventListener("keydown", async (event) => {
  event.preventDefault();
  if (event.key === "Escape") return hotkeyInput.blur();
  if (["Control", "Alt", "Shift", "Meta"].includes(event.key)) return;

  const modifiers = [];
  if (event.ctrlKey) modifiers.push("Control");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.metaKey) modifiers.push("Super");

  const isFunctionKey = /^F\d{1,2}$/.test(event.code);
  if (modifiers.length === 0 && !isFunctionKey) {
    return showError("Додай Ctrl, Alt, Shift або ⌘, інакше клавіша перестане друкувати.");
  }

  const hotkey = [...modifiers, event.code].join("+");
  try {
    await invoke("set_hotkey", { hotkey });
    saved = hotkey;
    showError(null);
  } catch (error) {
    showError(String(error));
  }
  hotkeyInput.blur();
});

autostart.addEventListener("change", async () => {
  try {
    await invoke("set_autostart", { enabled: autostart.checked });
    showError(null);
  } catch (error) {
    autostart.checked = !autostart.checked;
    showError(String(error));
  }
});

document.getElementById("grant").addEventListener("click", async () => {
  await invoke("request_permission");
});

window.addEventListener("focus", refresh);
refresh();
