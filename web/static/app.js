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
  navModules: document.querySelector("#nav-modules"),
  navModeration: document.querySelector("#nav-moderation"),
  navAudit: document.querySelector("#nav-audit"),
  navAnalytics: document.querySelector("#nav-analytics"),
  viewModules: document.querySelector("#view-modules"),
  viewModeration: document.querySelector("#view-moderation"),
  viewAudit: document.querySelector("#view-audit"),
  viewAnalytics: document.querySelector("#view-analytics"),
  casesList: document.querySelector("#cases-list"),
  auditList: document.querySelector("#audit-list"),
  toastContainer: document.querySelector("#toast-container"),
  kpiActiveModules: document.querySelector("#kpi-active-modules"),
  kpiModulesHint: document.querySelector("#kpi-modules-hint"),
  kpiTotalCases: document.querySelector("#kpi-total-cases"),
  kpiAuditEvents: document.querySelector("#kpi-audit-events"),
  kpiBotStatus: document.querySelector("#kpi-bot-status"),
  kpiUptime: document.querySelector("#kpi-uptime"),
  voidDialog: document.querySelector("#void-dialog"),
  voidForm: document.querySelector("#void-form"),
  voidReasonInput: document.querySelector("#void-reason-input"),
  voidCancelBtn: document.querySelector("#void-cancel-btn"),
  analyticsHealthDonut: document.querySelector("#analytics-health-donut"),
  analyticsRequestsSparkline: document.querySelector("#analytics-requests-sparkline"),
  analyticsDetails: document.querySelector("#analytics-details"),
};

const state = {
  session: null,
  guildId: null,
  generationId: 0,
  activeTab: "modules",
  modulesData: [],
  analyticsTimer: null,
  eventSource: null,
  requestHistory: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  lastRequestsTotal: null,
  pendingVoidCase: null,
};

const dateFmt = new Intl.DateTimeFormat("pl-PL", {
  dateStyle: "short",
  timeStyle: "short",
});

function formatDate(value) {
  if (!value) return "teraz";
  try {
    return dateFmt.format(new Date(value));
  } catch {
    return String(value);
  }
}

function escapeHtml(str) {
  if (str == null) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function showToast(message, type = "info", duration = 4000) {
  if (!ui.toastContainer) return;
  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;
  toast.textContent = message;
  ui.toastContainer.append(toast);

  setTimeout(() => {
    toast.style.opacity = "0";
    toast.style.transform = "translateY(10px) scale(0.95)";
    setTimeout(() => toast.remove(), 250);
  }, duration);
}

function setStatus(message, kind) {
  if (!ui.saveStatus) return;
  ui.saveStatus.textContent = message;
  ui.saveStatus.className = kind;
  if (kind === "error") {
    showToast(message, "error");
  } else if (kind === "success") {
    showToast(message, "success");
  }
}

function clearStatus() {
  if (!ui.saveStatus) return;
  ui.saveStatus.textContent = "";
  ui.saveStatus.className = "";
}

async function request(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (options.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  if (state.session && options.method && options.method !== "GET") {
    headers.set("x-csrf-token", state.session.csrf_token);
  }

  const signal = options.signal || AbortSignal.timeout(10000);

  const response = await fetch(path, {
    credentials: "same-origin",
    ...options,
    signal,
    headers,
  });

  if (response.status === 204) {
    return null;
  }

  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    let msg = body.error || `Błąd HTTP ${response.status}`;
    if (Array.isArray(body.details) && body.details.length > 0) {
      msg += `: ${body.details.join("; ")}`;
    }
    throw new Error(msg);
  }
  return body;
}

async function boot() {
  setupTabs();
  setupVoidDialog();
  try {
    state.session = await request("/api/session");
    showDashboard();
  } catch {
    showLogin();
  }
}

function setupTabs() {
  const tabs = [
    { el: ui.navModules, tab: "modules" },
    { el: ui.navModeration, tab: "moderation" },
    { el: ui.navAudit, tab: "audit" },
    { el: ui.navAnalytics, tab: "analytics" },
  ];

  for (const { el, tab } of tabs) {
    el?.addEventListener("click", (e) => {
      e.preventDefault();
      switchTab(tab);
    });
  }
}

function switchTab(tab) {
  state.activeTab = tab;

  const tabDefs = [
    { nav: ui.navModules, view: ui.viewModules, name: "modules" },
    { nav: ui.navModeration, view: ui.viewModeration, name: "moderation" },
    { nav: ui.navAudit, view: ui.viewAudit, name: "audit" },
    { nav: ui.navAnalytics, view: ui.viewAnalytics, name: "analytics" },
  ];

  for (const t of tabDefs) {
    const isCurrent = t.name === tab;
    t.nav?.classList.toggle("active", isCurrent);
    t.nav?.setAttribute("aria-selected", isCurrent ? "true" : "false");
    t.view?.classList.toggle("hidden", !isCurrent);
  }

  if (state.analyticsTimer) {
    clearInterval(state.analyticsTimer);
    state.analyticsTimer = null;
  }

  if (state.guildId) {
    if (tab === "modules") loadModules();
    else if (tab === "moderation") loadCases();
    else if (tab === "audit") loadAudit();
    else if (tab === "analytics") {
      loadAnalytics();
      state.analyticsTimer = setInterval(loadAnalytics, 5000);
    }
  }

  updateKpis();
}

function showLogin() {
  if (state.eventSource) {
    state.eventSource.close();
    state.eventSource = null;
  }
  state.session = null;
  ui.loginView?.classList.remove("hidden");
  ui.dashboardView?.classList.add("hidden");
  ui.account?.classList.add("hidden");
  ui.logoutButton?.classList.add("hidden");
}

function initRealtimeStream() {
  if (state.eventSource) {
    state.eventSource.close();
    state.eventSource = null;
  }
  if (!state.guildId) return;

  try {
    const sse = new EventSource(`/api/guilds/${encodeURIComponent(state.guildId)}/events`);
    state.eventSource = sse;

    sse.addEventListener("connected", () => {
      console.log("Realtime SSE połączony dla serwera:", state.guildId);
    });

    sse.addEventListener("case_updated", () => {
      showToast("Zaktualizowano sprawę moderacyjną", "info", 3000);
      if (state.activeTab === "moderation") {
        loadCases();
      }
      updateKpis();
    });

    sse.addEventListener("case_created", () => {
      showToast("Utworzono nową sprawę moderacyjną", "info", 3000);
      if (state.activeTab === "moderation") {
        loadCases();
      }
      updateKpis();
    });

    sse.addEventListener("module_updated", () => {
      if (state.activeTab === "modules") {
        loadModules();
      }
      updateKpis();
    });

    sse.addEventListener("audit_entry", () => {
      if (state.activeTab === "audit") {
        loadAudit();
      }
      updateKpis();
    });

    sse.onerror = () => {
      // EventSource automatycznie ponawia połączenie
    };
  } catch (err) {
    console.warn("Błąd inicjalizacji SSE, aktywne odpytywanie awaryjne", err);
  }
}

async function showDashboard() {
  ui.loginView?.classList.add("hidden");
  ui.dashboardView?.classList.remove("hidden");
  ui.account?.classList.remove("hidden");
  ui.logoutButton?.classList.remove("hidden");

  if (ui.accountName) {
    ui.accountName.textContent =
      state.session.user.global_name || state.session.user.username;
  }
  if (ui.accountId) {
    ui.accountId.textContent = `Discord ID: ${state.session.user.id}`;
  }

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
  initRealtimeStream();
  switchTab(state.activeTab);
  loadLiveUptime();
}

async function loadLiveUptime() {
  try {
    const live = await request("/health/live");
    if (ui.kpiUptime) {
      const upSec = live.uptime_seconds || 0;
      const hours = Math.floor(upSec / 3600);
      const minutes = Math.floor((upSec % 3600) / 60);
      ui.kpiUptime.textContent = `Uptime: ${hours}h ${minutes}m (v${live.version})`;
    }
  } catch {
    if (ui.kpiUptime) {
      ui.kpiUptime.textContent = "Uptime: niedostępny";
    }
  }
}

async function updateKpis() {
  if (!state.guildId) return;

  if (state.modulesData && state.modulesData.length > 0) {
    const enabled = state.modulesData.filter((m) => m.enabled).length;
    const total = state.modulesData.length;
    if (ui.kpiActiveModules) ui.kpiActiveModules.textContent = `${enabled} / ${total}`;
    if (ui.kpiModulesHint) {
      ui.kpiModulesHint.textContent = `${total - enabled} wyłączonych`;
    }
  }

  try {
    const cases = await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases`);
    if (ui.kpiTotalCases) ui.kpiTotalCases.textContent = String(cases.length || 0);
  } catch {}

  try {
    const audit = await request(`/api/guilds/${encodeURIComponent(state.guildId)}/audit`);
    if (ui.kpiAuditEvents) ui.kpiAuditEvents.textContent = String(audit.length || 0);
  } catch {}
}

async function loadModules() {
  clearStatus();
  ui.modules.replaceChildren(
    messageCard("Ładowanie konfiguracji", "Pobieranie modułów Lua z serwera...")
  );

  const gen = state.generationId;
  const targetGuild = state.guildId;

  try {
    const modules = await request(
      `/api/guilds/${encodeURIComponent(targetGuild)}/modules`
    );
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    state.modulesData = modules;
    renderModules(modules);
    updateKpis();
  } catch (error) {
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    ui.modules.replaceChildren(
      messageCard("Nie udało się pobrać modułów", error.message)
    );
  }
}

function renderModules(modules) {
  ui.modules.replaceChildren();
  if (ui.moduleCount) {
    ui.moduleCount.textContent = `${modules.length} ${
      modules.length === 1 ? "moduł" : "modułów"
    }`;
  }

  for (const module of modules) {
    const fragment = ui.template.content.cloneNode(true);
    const card = fragment.querySelector(".module-card");
    const enabled = fragment.querySelector(".module-enabled");
    const configTextarea = fragment.querySelector(".module-config");
    const saveBtn = fragment.querySelector(".save-button");
    const formContainer = fragment.querySelector(".module-form-container");
    const jsonDetails = fragment.querySelector(".json-details");
    const modeBtns = fragment.querySelectorAll(".mode-btn");

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

    let currentConfig = JSON.parse(JSON.stringify(module.config || {}));
    configTextarea.value = JSON.stringify(currentConfig, null, 2);

    const updateFromForm = (newConfig) => {
      currentConfig = newConfig;
      configTextarea.value = JSON.stringify(currentConfig, null, 2);
    };

    buildSchemaForm(formContainer, module.manifest, currentConfig, updateFromForm);

    for (const btn of modeBtns) {
      btn.addEventListener("click", () => {
        for (const b of modeBtns) b.classList.remove("active");
        btn.classList.add("active");
        const mode = btn.dataset.mode;
        if (mode === "form") {
          try {
            currentConfig = JSON.parse(configTextarea.value || "{}");
          } catch {}
          buildSchemaForm(formContainer, module.manifest, currentConfig, updateFromForm);
          formContainer.classList.remove("hidden");
          jsonDetails?.classList.add("hidden");
        } else {
          formContainer.classList.add("hidden");
          jsonDetails?.classList.remove("hidden");
        }
      });
    }

    saveBtn.addEventListener("click", async () => {
      await saveModule(card, module.manifest.id, enabled, configTextarea, saveBtn);
    });

    ui.modules.append(fragment);
  }
}

function buildSchemaForm(container, manifest, config, onUpdate) {
  container.replaceChildren();
  const schema = manifest.config_schema;
  if (!schema || !schema.properties || Object.keys(schema.properties).length === 0) {
    const emptyP = document.createElement("p");
    emptyP.style.fontSize = "12px";
    emptyP.style.color = "var(--muted)";
    emptyP.textContent = "Ten moduł nie posiada dodatkowych parametrów konfiguracyjnych.";
    container.append(emptyP);
    return;
  }

  const form = document.createElement("div");
  form.className = "schema-form";

  for (const [key, prop] of Object.entries(schema.properties)) {
    const field = document.createElement("div");
    field.className = "form-field";

    const label = document.createElement("label");
    const titleText = prop.title || key;

    if (key.includes("channel") || key.includes("role") || key.includes("user")) {
      const badge = document.createElement("span");
      badge.className = "discord-badge";
      badge.textContent = key.includes("channel") ? "#" : key.includes("role") ? "@" : "👤";
      label.append(badge);
    }
    label.append(document.createTextNode(titleText));
    field.append(label);

    if (prop.description) {
      const desc = document.createElement("span");
      desc.className = "field-desc";
      desc.textContent = prop.description;
      field.append(desc);
    }

    const currentValue = config[key] !== undefined ? config[key] : prop.default;

    if (prop.type === "boolean") {
      const switchLabel = document.createElement("label");
      switchLabel.className = "switch";
      switchLabel.style.marginTop = "4px";
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = Boolean(currentValue);
      checkbox.addEventListener("change", () => {
        config[key] = checkbox.checked;
        onUpdate(config);
      });
      const track = document.createElement("span");
      track.className = "switch-track";
      switchLabel.append(checkbox, track);
      field.append(switchLabel);
    } else if (prop.type === "array" && prop.items && prop.items.type === "string") {
      const chipsContainer = document.createElement("div");
      chipsContainer.className = "chips-container";

      let list = Array.isArray(currentValue) ? [...currentValue] : [];

      const renderChips = () => {
        chipsContainer.replaceChildren();
        for (let idx = 0; idx < list.length; idx++) {
          const chip = document.createElement("span");
          chip.className = "tag-chip";
          chip.textContent = list[idx];

          const delBtn = document.createElement("button");
          delBtn.type = "button";
          delBtn.className = "chip-del-btn";
          delBtn.textContent = "×";
          delBtn.setAttribute("aria-label", `Usuń ${list[idx]}`);
          delBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            list.splice(idx, 1);
            config[key] = list;
            onUpdate(config);
            renderChips();
          });

          chip.append(delBtn);
          chipsContainer.append(chip);
        }

        const input = document.createElement("input");
        input.type = "text";
        input.className = "chip-input";
        input.placeholder = list.length === 0 ? "Wpisz i naciśnij Enter..." : "+ Dodaj...";
        input.addEventListener("keydown", (e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            const val = input.value.trim().replace(/^,+|,+$/g, "");
            if (val && !list.includes(val)) {
              list.push(val);
              config[key] = list;
              onUpdate(config);
              renderChips();
            }
          }
        });
        chipsContainer.append(input);
      };

      renderChips();
      field.append(chipsContainer);
    } else if (prop.enum && Array.isArray(prop.enum)) {
      const select = document.createElement("select");
      select.className = "text-input";
      for (const optVal of prop.enum) {
        const option = document.createElement("option");
        option.value = optVal;
        option.textContent = optVal;
        if (optVal === currentValue) option.selected = true;
        select.append(option);
      }
      select.addEventListener("change", () => {
        config[key] = select.value;
        onUpdate(config);
      });
      field.append(select);
    } else if (prop.type === "integer" || prop.type === "number") {
      const input = document.createElement("input");
      input.type = "number";
      input.className = "text-input";
      if (prop.minimum !== undefined) input.min = prop.minimum;
      if (prop.maximum !== undefined) input.max = prop.maximum;
      input.step = prop.type === "number" ? "0.05" : "1";
      input.value = currentValue !== undefined ? currentValue : "";
      input.addEventListener("input", () => {
        const val = prop.type === "number" ? parseFloat(input.value) : parseInt(input.value, 10);
        if (!isNaN(val)) {
          config[key] = val;
          onUpdate(config);
        }
      });
      field.append(input);
    } else {
      const input = document.createElement("input");
      input.type = "text";
      input.className = "text-input";
      input.value = currentValue !== undefined ? currentValue : "";
      input.addEventListener("input", () => {
        config[key] = input.value;
        onUpdate(config);
      });
      field.append(input);
    }

    form.append(field);
  }

  container.append(form);
}

async function saveModule(card, moduleId, enabled, config, button) {
  clearStatus();
  button.disabled = true;
  button.textContent = "Zapisywanie...";

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
    setStatus("Zapisano pomyślnie konfigurację modułu.", "success");
    updateKpis();
  } catch (error) {
    setStatus(error.message, "error");
  } finally {
    button.disabled = false;
    button.textContent = "Zapisz zmiany";
  }
}

async function loadCases() {
  clearStatus();
  ui.casesList.replaceChildren(
    messageCard("Ładowanie spraw", "Pobieranie rejestru moderacji...")
  );

  const gen = state.generationId;
  const targetGuild = state.guildId;

  try {
    const cases = await request(
      `/api/guilds/${encodeURIComponent(targetGuild)}/cases`
    );
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    renderCases(cases);
  } catch (error) {
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    ui.casesList.replaceChildren(
      messageCard("Błąd pobierania spraw moderacyjnych", error.message)
    );
  }
}

function renderCases(cases) {
  ui.casesList.replaceChildren();
  if (!cases || cases.length === 0) {
    ui.casesList.append(
      messageCard("Brak spraw", "Ten serwer nie ma jeszcze żadnych zarejestrowanych spraw moderacyjnych.")
    );
    return;
  }

  for (const c of cases) {
    const card = document.createElement("article");
    card.className = "case-card";
    card.id = `case-card-${c.case_number}`;

    const badgeClass =
      c.status === "active"
        ? "badge-active"
        : c.status === "voided"
        ? "badge-voided"
        : "badge-expired";

    card.innerHTML = `
      <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
        <strong>#${c.case_number} [${escapeHtml(c.kind)}]</strong>
        <div>
          <span class="case-badge ${badgeClass}">${escapeHtml(c.status)}</span>
          <button type="button" class="case-action-btn toggle-details-btn" data-case="${c.case_number}" style="margin-left:8px;">Szczegóły</button>
        </div>
      </div>
      <p style="margin:4px 0 10px 0;"><strong>Powód:</strong> <span class="case-reason-text">${escapeHtml(c.reason)}</span></p>
      <div style="display:flex; gap:16px; font-size:12px; color:var(--muted); flex-wrap:wrap; margin-bottom:8px;">
        <span>Użytkownik: <code>${escapeHtml(String(c.subject_user_id))}</code></span>
        <span>Moderator: <code>${escapeHtml(String(c.actor_user_id))}</code></span>
        <span>Punkty: <strong class="case-points-text">${c.points}</strong></span>
        <span>Poziom: ${escapeHtml(c.severity)}</span>
        <span>Utworzono: ${formatDate(c.created_at)}</span>
      </div>
      <div class="case-details-container" style="display:none; margin-top:12px; border-top:1px solid var(--border-soft); padding-top:12px;"></div>
    `;

    const toggleBtn = card.querySelector(".toggle-details-btn");
    const container = card.querySelector(".case-details-container");

    toggleBtn.addEventListener("click", async () => {
      if (container.style.display !== "none") {
        container.style.display = "none";
        toggleBtn.textContent = "Szczegóły";
        return;
      }
      toggleBtn.textContent = "Ukryj";
      container.style.display = "block";
      await loadCaseDetails(c.case_number, container, c);
    });

    ui.casesList.append(card);
  }
}

async function loadCaseDetails(caseNumber, container, caseItem) {
  container.innerHTML = `<p style="font-size:13px; color:var(--muted);">Ładowanie szczegółów sprawy...</p>`;
  try {
    const details = await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases/${caseNumber}`);
    renderCaseDetailsContent(container, details, caseItem);
  } catch (error) {
    container.innerHTML = `<p style="color:var(--danger); font-size:13px;">Błąd pobierania szczegółów: ${escapeHtml(error.message)}</p>`;
  }
}

function renderCaseDetailsContent(container, details, caseItem) {
  const c = details.case;
  container.replaceChildren();

  const notesHeader = document.createElement("h4");
  notesHeader.style.margin = "8px 0";
  notesHeader.textContent = `Notatki (${details.notes.length}):`;
  container.append(notesHeader);

  const notesList = document.createElement("div");
  notesList.className = "case-notes-list";
  notesList.style.display = "grid";
  notesList.style.gap = "8px";
  notesList.style.marginBottom = "12px";

  if (details.notes.length === 0) {
    const emptyNote = document.createElement("div");
    emptyNote.style.fontSize = "13px";
    emptyNote.style.color = "var(--muted)";
    emptyNote.textContent = "Brak notatek do tej sprawy.";
    notesList.append(emptyNote);
  } else {
    for (const note of details.notes) {
      const noteEl = document.createElement("div");
      noteEl.className = "case-note-item";
      noteEl.innerHTML = `
        <div style="font-size:11px; color:var(--muted); margin-bottom:2px;">
          <span>${formatDate(note.created_at)} przez <code>${escapeHtml(String(note.author_user_id))}</code></span>
          <span style="margin-left:6px; font-style:italic;">${note.visible_to_subject ? "[Widoczna dla ukaranego]" : "[Wewnętrzna]"}</span>
        </div>
        <p style="margin:0; font-size:13px;">${escapeHtml(note.body)}</p>
      `;
      notesList.append(noteEl);
    }
  }
  container.append(notesList);

  const addNoteForm = document.createElement("form");
  addNoteForm.className = "case-add-note-form";
  addNoteForm.style.display = "flex";
  addNoteForm.style.gap = "8px";
  addNoteForm.style.alignItems = "center";
  addNoteForm.style.marginBottom = "16px";
  addNoteForm.style.flexWrap = "wrap";
  addNoteForm.innerHTML = `
    <input type="text" placeholder="Dodaj notatkę moderatorską..." class="case-note-input text-input" style="flex:1; min-width:200px;" required />
    <label style="font-size:12px; display:inline-flex; align-items:center; gap:4px; cursor:pointer;">
      <input type="checkbox" class="case-note-visible" /> Widoczna dla ukaranego
    </label>
    <button type="submit" class="case-action-btn btn-primary">Zapisz notatkę</button>
  `;

  addNoteForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const input = addNoteForm.querySelector(".case-note-input");
    const visibleBox = addNoteForm.querySelector(".case-note-visible");
    const bodyText = input.value.trim();
    if (!bodyText) return;

    try {
      await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases/${c.case_number}/notes`, {
        method: "POST",
        body: JSON.stringify({
          body: bodyText,
          visible_to_subject: visibleBox.checked,
        }),
      });
      setStatus(`Dodano notatkę do sprawy #${c.case_number}.`, "success");
      await loadCaseDetails(c.case_number, container, c);
    } catch (err) {
      setStatus(`Błąd dodawania notatki: ${err.message}`, "error");
    }
  });
  container.append(addNoteForm);

  if (c.status === "active") {
    const actionRow = document.createElement("div");
    actionRow.style.display = "flex";
    actionRow.style.gap = "10px";
    actionRow.style.alignItems = "center";
    actionRow.style.flexWrap = "wrap";
    actionRow.style.paddingTop = "8px";
    actionRow.style.borderTop = "1px solid var(--border-soft)";

    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.className = "case-action-btn";
    editBtn.textContent = "Edytuj powód i punkty";

    const voidBtn = document.createElement("button");
    voidBtn.type = "button";
    voidBtn.className = "case-action-btn btn-danger";
    voidBtn.textContent = "Unieważnij sprawę";

    actionRow.append(editBtn, voidBtn);
    container.append(actionRow);

    const editContainer = document.createElement("div");
    editContainer.style.display = "none";
    editContainer.style.marginTop = "10px";
    editContainer.innerHTML = `
      <form class="case-edit-form" style="display:flex; gap:8px; align-items:center; flex-wrap:wrap;">
        <input type="text" class="edit-reason-input text-input" value="${escapeHtml(c.reason)}" placeholder="Nowy powód..." style="flex:1; min-width:200px;" required />
        <input type="number" class="edit-points-input text-input" value="${c.points}" placeholder="Punkty" style="width:80px;" min="0" max="10000" />
        <button type="submit" class="case-action-btn btn-primary">Zapisz zmiany</button>
      </form>
    `;
    editBtn.addEventListener("click", () => {
      editContainer.style.display = editContainer.style.display === "none" ? "block" : "none";
    });

    editContainer.querySelector(".case-edit-form").addEventListener("submit", async (e) => {
      e.preventDefault();
      const newReason = editContainer.querySelector(".edit-reason-input").value.trim();
      const newPoints = parseInt(editContainer.querySelector(".edit-points-input").value, 10);
      try {
        await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases/${c.case_number}`, {
          method: "PATCH",
          body: JSON.stringify({
            expected_version: c.version,
            reason: newReason,
            points: isNaN(newPoints) ? c.points : newPoints,
          }),
        });
        setStatus(`Zaktualizowano sprawę #${c.case_number}.`, "success");
        await loadCases();
      } catch (err) {
        setStatus(`Błąd aktualizacji sprawy: ${err.message}`, "error");
      }
    });
    container.append(editContainer);

    voidBtn.addEventListener("click", () => {
      openVoidModal(c);
    });
  }
}

function setupVoidDialog() {
  if (!ui.voidDialog || !ui.voidForm) return;

  ui.voidCancelBtn?.addEventListener("click", () => {
    ui.voidDialog.close();
    state.pendingVoidCase = null;
  });

  ui.voidForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const c = state.pendingVoidCase;
    if (!c) return;

    const reason = ui.voidReasonInput.value.trim();
    if (!reason) return;

    try {
      await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases/${c.case_number}/void`, {
        method: "POST",
        body: JSON.stringify({
          expected_version: c.version,
          reason,
        }),
      });
      ui.voidDialog.close();
      state.pendingVoidCase = null;
      setStatus(`Sprawa #${c.case_number} została pomyślnie unieważniona.`, "success");
      await loadCases();
    } catch (err) {
      setStatus(`Błąd unieważnienia: ${err.message}`, "error");
    }
  });
}

function openVoidModal(caseItem) {
  state.pendingVoidCase = caseItem;
  if (ui.voidReasonInput) ui.voidReasonInput.value = "";
  if (ui.voidDialog && typeof ui.voidDialog.showModal === "function") {
    ui.voidDialog.showModal();
  }
}

async function loadAudit() {
  clearStatus();
  ui.auditList.replaceChildren(
    messageCard("Ładowanie dziennika", "Pobieranie zdarzeń audytowych...")
  );

  const gen = state.generationId;
  const targetGuild = state.guildId;

  try {
    const events = await request(
      `/api/guilds/${encodeURIComponent(targetGuild)}/audit`
    );
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    renderAudit(events);
  } catch (error) {
    if (gen !== state.generationId || state.guildId !== targetGuild) return;
    ui.auditList.replaceChildren(
      messageCard("Błąd pobierania audytu", error.message)
    );
  }
}

function renderAudit(events) {
  ui.auditList.replaceChildren();
  if (!events || events.length === 0) {
    ui.auditList.append(
      messageCard("Brak wpisów", "Brak zarejestrowanych zdarzeń audytowych dla tego serwera.")
    );
    return;
  }

  const table = document.createElement("table");
  table.className = "audit-table";
  table.innerHTML = `
    <thead>
      <tr>
        <th>Data</th>
        <th>Moduł</th>
        <th>Zdarzenie</th>
        <th>Aktor</th>
        <th>Dane zdarzenia</th>
      </tr>
    </thead>
    <tbody>
      ${events
        .map(
          (e) => `
        <tr>
          <td style="white-space:nowrap;">${formatDate(e.created_at)}</td>
          <td><code>${escapeHtml(e.module_id)}</code></td>
          <td><strong>${escapeHtml(e.event)}</strong></td>
          <td>${e.actor_id ? `<code>${escapeHtml(e.actor_id)}</code>` : "<span style='color:var(--muted);'>System</span>"}</td>
          <td><pre style="margin:0; font-size:11px; max-width:380px; overflow-x:auto;">${escapeHtml(JSON.stringify(e.data))}</pre></td>
        </tr>
      `
        )
        .join("")}
    </tbody>
  `;
  ui.auditList.append(table);
}

async function loadAnalytics() {
  try {
    const ready = await request("/health/ready");
    renderAnalytics(ready);
  } catch (err) {
    if (ui.analyticsDetails) {
      ui.analyticsDetails.innerHTML = `<p style="padding:16px; color:var(--danger);">Nie można pobrać stanu gotowości: ${escapeHtml(err.message)}</p>`;
    }
  }
}

function renderAnalytics(readyData) {
  const checks = readyData.checks || {};
  const entries = Object.entries(checks);
  const okCount = entries.filter(([_, v]) => String(v).startsWith("ok")).length;
  const totalCount = Math.max(1, entries.length);
  const ratio = okCount / totalCount;

  if (ui.analyticsHealthDonut) {
    const size = 160;
    const stroke = 18;
    const radius = (size - stroke) / 2;
    const circ = 2 * Math.PI * radius;
    const offset = circ * (1 - ratio);
    const strokeColor = ratio === 1 ? "var(--success)" : "var(--danger)";

    ui.analyticsHealthDonut.innerHTML = `
      <svg width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
        <circle cx="${size/2}" cy="${size/2}" r="${radius}" fill="none" stroke="var(--border-soft)" stroke-width="${stroke}" />
        <circle cx="${size/2}" cy="${size/2}" r="${radius}" fill="none" stroke="${strokeColor}" stroke-width="${stroke}"
          stroke-dasharray="${circ}" stroke-dashoffset="${offset}" stroke-linecap="round"
          transform="rotate(-90 ${size/2} ${size/2})" style="transition: stroke-dashoffset .4s ease;" />
        <text x="50%" y="48%" text-anchor="middle" dominant-baseline="central" fill="var(--text)" font-size="22" font-weight="800">
          ${Math.round(ratio * 100)}%
        </text>
        <text x="50%" y="64%" text-anchor="middle" dominant-baseline="central" fill="var(--muted)" font-size="11">
          ${okCount}/${totalCount} OK
        </text>
      </svg>
    `;
  }

  const currentTotal =
    typeof readyData.dashboard_requests_total === "number"
      ? readyData.dashboard_requests_total
      : null;

  if (currentTotal !== null) {
    if (state.lastRequestsTotal !== null && currentTotal >= state.lastRequestsTotal) {
      const delta = currentTotal - state.lastRequestsTotal;
      state.requestHistory.shift();
      state.requestHistory.push(delta);
    } else {
      state.requestHistory.shift();
      state.requestHistory.push(0);
    }
    state.lastRequestsTotal = currentTotal;
  }

  if (ui.analyticsRequestsSparkline) {
    const width = 280;
    const height = 90;
    const max = Math.max(...state.requestHistory, 10);
    const step = width / (state.requestHistory.length - 1);

    const points = state.requestHistory.map((val, idx) => {
      const x = idx * step;
      const y = height - (val / max) * (height - 20) - 10;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(" ");

    ui.analyticsRequestsSparkline.innerHTML = `
      <svg width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" style="overflow:visible;">
        <polyline fill="none" stroke="var(--accent)" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" points="${points}" />
        ${state.requestHistory.map((val, idx) => {
          const x = idx * step;
          const y = height - (val / max) * (height - 20) - 10;
          return `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="3" fill="var(--accent-strong)" />`;
        }).join("")}
      </svg>
    `;
  }

  if (ui.analyticsDetails) {
    const table = document.createElement("table");
    table.className = "audit-table";
    table.innerHTML = `
      <thead>
        <tr>
          <th>Podsystem</th>
          <th>Status</th>
          <th>Opis / Szczegóły</th>
        </tr>
      </thead>
      <tbody>
        ${entries.map(([sub, status]) => {
          const isOk = String(status).startsWith("ok");
          return `
            <tr>
              <td><strong>${escapeHtml(sub)}</strong></td>
              <td><span class="case-badge ${isOk ? 'badge-active' : 'badge-voided'}">${isOk ? 'SPRAWNY' : 'BŁĄD'}</span></td>
              <td><code>${escapeHtml(String(status))}</code></td>
            </tr>
          `;
        }).join("")}
      </tbody>
    `;
    ui.analyticsDetails.replaceChildren(table);
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

ui.guildSelect?.addEventListener("change", async (event) => {
  state.generationId++;
  if (ui.voidDialog && typeof ui.voidDialog.close === "function") {
    try { ui.voidDialog.close(); } catch {}
  }
  state.pendingVoidCase = null;
  ui.modules?.replaceChildren();
  ui.casesList?.replaceChildren();
  ui.auditList?.replaceChildren();
  state.guildId = event.target.value;
  initRealtimeStream();
  switchTab(state.activeTab);
});

ui.logoutButton?.addEventListener("click", async () => {
  try {
    await request("/api/logout", { method: "POST" });
  } finally {
    showLogin();
  }
});

boot();
