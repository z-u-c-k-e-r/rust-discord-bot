const ui = {
  loginView: document.querySelector("#login-view"),
  dashboardView: document.querySelector("#dashboard-view"),
  account: document.querySelector("#account"),
  accountName: document.querySelector("#account-name"),
  accountId: document.querySelector("#account-id"),
  guildSelect: document.querySelector("#guild-select"),
  modules: document.querySelector("#modules"),
  moduleCount: document.querySelector("#module-count"),
  saveStatus: document.querySelector("#save-status"),
  logoutButton: document.querySelector("#logout-button"),
  template: document.querySelector("#module-template"),
};

const state = {
  session: null,
  guildId: null,
};

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (options.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  if (state.session && options.method && options.method !== "GET") {
    headers.set("x-csrf-token", state.session.csrf_token);
  }

  const response = await fetch(path, {
    credentials: "same-origin",
    ...options,
    headers,
  });

  if (response.status === 204) {
    return null;
  }

  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(body.error || `Błąd HTTP ${response.status}`);
  }
  return body;
}

async function boot() {
  try {
    state.session = await request("/api/session");
    showDashboard();
  } catch {
    showLogin();
  }
}

function showLogin() {
  state.session = null;
  ui.loginView.classList.remove("hidden");
  ui.dashboardView.classList.add("hidden");
  ui.account.classList.add("hidden");
  ui.logoutButton.classList.add("hidden");
}

async function showDashboard() {
  ui.loginView.classList.add("hidden");
  ui.dashboardView.classList.remove("hidden");
  ui.account.classList.remove("hidden");
  ui.logoutButton.classList.remove("hidden");

  ui.accountName.textContent =
    state.session.user.global_name || state.session.user.username;
  ui.accountId.textContent = `Discord ID: ${state.session.user.id}`;

  ui.guildSelect.replaceChildren();
  for (const guild of state.session.guilds) {
    const option = document.createElement("option");
    option.value = guild.id;
    option.textContent = guild.name;
    ui.guildSelect.append(option);
  }

  if (state.session.guilds.length === 0) {
    ui.guildSelect.disabled = true;
    ui.modules.replaceChildren(
      messageCard(
        "Brak serwerów",
        "Discord nie zwrócił żadnego serwera, którym możesz zarządzać."
      )
    );
    return;
  }

  state.guildId = state.session.guilds[0].id;
  ui.guildSelect.value = state.guildId;
  await loadModules();
}

async function loadModules() {
  clearStatus();
  ui.modules.replaceChildren(
    messageCard("Ładowanie konfiguracji", "Pobieranie modułów Lua z serwera.")
  );

  try {
    const modules = await request(
      `/api/guilds/${encodeURIComponent(state.guildId)}/modules`
    );
    renderModules(modules);
  } catch (error) {
    ui.modules.replaceChildren(
      messageCard("Nie udało się pobrać modułów", error.message)
    );
  }
}

function renderModules(modules) {
  ui.modules.replaceChildren();
  ui.moduleCount.textContent = `${modules.length} ${
    modules.length === 1 ? "moduł" : "modułów"
  }`;

  for (const module of modules) {
    const fragment = ui.template.content.cloneNode(true);
    const card = fragment.querySelector(".module-card");
    const enabled = fragment.querySelector(".module-enabled");
    const config = fragment.querySelector(".module-config");
    const save = fragment.querySelector(".save-button");

    fragment.querySelector(".module-category").textContent =
      module.manifest.category;
    fragment.querySelector(".module-name").textContent = module.manifest.name;
    fragment.querySelector(".module-description").textContent =
      module.manifest.description;
    fragment.querySelector(".module-id").textContent = module.manifest.id;
    fragment.querySelector(
      ".module-version"
    ).textContent = `v${module.manifest.version}`;
    fragment.querySelector(".module-state").textContent = module.configured
      ? `Zmieniono: ${formatDate(module.updated_at)}`
      : "Ustawienia domyślne";

    enabled.checked = module.enabled;
    config.value = JSON.stringify(module.config || {}, null, 2);

    save.addEventListener("click", async () => {
      await saveModule(card, module.manifest.id, enabled, config, save);
    });

    ui.modules.append(fragment);
  }
}

async function saveModule(card, moduleId, enabled, config, button) {
  clearStatus();
  button.disabled = true;
  button.textContent = "Zapisywanie";

  try {
    const parsed = JSON.parse(config.value || "{}");
    const updated = await request(
      `/api/guilds/${encodeURIComponent(
        state.guildId
      )}/modules/${encodeURIComponent(moduleId)}`,
      {
        method: "PUT",
        body: JSON.stringify({
          enabled: enabled.checked,
          config: parsed,
        }),
      }
    );

    card.querySelector(".module-state").textContent = `Zmieniono: ${formatDate(
      updated.updated_at
    )}`;
    setStatus("Zapisano konfigurację.", "success");
  } catch (error) {
    setStatus(error.message, "error");
  } finally {
    button.disabled = false;
    button.textContent = "Zapisz zmiany";
  }
}

function messageCard(title, body) {
  const article = document.createElement("article");
  article.className = "module-card";
  const heading = document.createElement("h2");
  heading.className = "module-name";
  heading.textContent = title;
  const paragraph = document.createElement("p");
  paragraph.className = "module-description";
  paragraph.textContent = body;
  article.append(heading, paragraph);
  return article;
}

function formatDate(value) {
  if (!value) {
    return "teraz";
  }
  return new Intl.DateTimeFormat("pl-PL", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
}

function setStatus(message, kind) {
  ui.saveStatus.textContent = message;
  ui.saveStatus.className = kind;
}

function clearStatus() {
  ui.saveStatus.textContent = "";
  ui.saveStatus.className = "";
}

ui.guildSelect.addEventListener("change", async (event) => {
  state.guildId = event.target.value;
  await loadModules();
});

ui.logoutButton.addEventListener("click", async () => {
  try {
    await request("/api/logout", { method: "POST" });
  } finally {
    showLogin();
  }
});

boot();
