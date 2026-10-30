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
  viewModules: document.querySelector("#view-modules"),
  viewModeration: document.querySelector("#view-moderation"),
  viewAudit: document.querySelector("#view-audit"),
  casesList: document.querySelector("#cases-list"),
  auditList: document.querySelector("#audit-list"),
};

const state = {
  session: null,
  guildId: null,
  activeTab: "modules",
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
  try {
    state.session = await request("/api/session");
    showDashboard();
  } catch {
    showLogin();
  }
}

function setupTabs() {
  ui.navModules?.addEventListener("click", (e) => {
    e.preventDefault();
    switchTab("modules");
  });
  ui.navModeration?.addEventListener("click", (e) => {
    e.preventDefault();
    switchTab("moderation");
  });
  ui.navAudit?.addEventListener("click", (e) => {
    e.preventDefault();
    switchTab("audit");
  });
}

function switchTab(tab) {
  state.activeTab = tab;
  ui.navModules?.classList.toggle("active", tab === "modules");
  ui.navModeration?.classList.toggle("active", tab === "moderation");
  ui.navAudit?.classList.toggle("active", tab === "audit");

  ui.viewModules?.classList.toggle("hidden", tab !== "modules");
  ui.viewModeration?.classList.toggle("hidden", tab !== "moderation");
  ui.viewAudit?.classList.toggle("hidden", tab !== "audit");

  if (state.guildId) {
    if (tab === "modules") loadModules();
    else if (tab === "moderation") loadCases();
    else if (tab === "audit") loadAudit();
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
  switchTab(state.activeTab);
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

async function loadCases() {
  clearStatus();
  ui.casesList.replaceChildren(
    messageCard("Ładowanie spraw", "Pobieranie rejestru moderacji...")
  );

  try {
    const cases = await request(
      `/api/guilds/${encodeURIComponent(state.guildId)}/cases`
    );
    renderCases(cases);
  } catch (error) {
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

    const voidContainer = document.createElement("div");
    voidContainer.style.display = "none";
    voidContainer.style.marginTop = "10px";
    voidContainer.innerHTML = `
      <form class="case-void-form" style="display:flex; gap:8px; align-items:center; flex-wrap:wrap;">
        <input type="text" class="void-reason-input text-input" placeholder="Powód unieważnienia (wymagany)..." style="flex:1; min-width:200px;" required />
        <button type="submit" class="case-action-btn btn-danger">Potwierdź unieważnienie</button>
      </form>
    `;
    voidBtn.addEventListener("click", () => {
      voidContainer.style.display = voidContainer.style.display === "none" ? "block" : "none";
    });

    voidContainer.querySelector(".case-void-form").addEventListener("submit", async (e) => {
      e.preventDefault();
      const voidReason = voidContainer.querySelector(".void-reason-input").value.trim();
      if (!voidReason) return;
      try {
        await request(`/api/guilds/${encodeURIComponent(state.guildId)}/cases/${c.case_number}/void`, {
          method: "POST",
          body: JSON.stringify({
            expected_version: c.version,
            reason: voidReason,
          }),
        });
        setStatus(`Unieważniono sprawę #${c.case_number}.`, "success");
        await loadCases();
      } catch (err) {
        setStatus(`Błąd unieważnienia sprawy: ${err.message}`, "error");
      }
    });
    container.append(voidContainer);
  }
}

async function loadAudit() {
  clearStatus();
  ui.auditList.replaceChildren(
    messageCard("Ładowanie dziennika", "Pobieranie zdarzeń audytowych...")
  );

  try {
    const events = await request(
      `/api/guilds/${encodeURIComponent(state.guildId)}/audit`
    );
    renderAudit(events);
  } catch (error) {
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
        <th>Dane</th>
      </tr>
    </thead>
    <tbody>
      ${events
        .map(
          (e) => `
        <tr>
          <td>${formatDate(e.created_at)}</td>
          <td><code>${escapeHtml(e.module_id)}</code></td>
          <td><strong>${escapeHtml(e.event)}</strong></td>
          <td>${e.actor_id ? `<code>${escapeHtml(e.actor_id)}</code>` : "System"}</td>
          <td><pre style="margin:0; font-size:11px;">${escapeHtml(JSON.stringify(e.data))}</pre></td>
        </tr>
      `
        )
        .join("")}
    </tbody>
  `;
  ui.auditList.append(table);
}

function escapeHtml(str) {
  if (!str) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
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
  switchTab(state.activeTab);
});

ui.logoutButton.addEventListener("click", async () => {
  try {
    await request("/api/logout", { method: "POST" });
  } finally {
    showLogin();
  }
});

boot();
