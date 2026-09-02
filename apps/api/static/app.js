const healthState = document.querySelector("#health-state");
const uptime = document.querySelector("#uptime");
const sidebarStatus = document.querySelector("#sidebar-status");
const sidebarVersion = document.querySelector("#sidebar-version");
const moduleGrid = document.querySelector("#module-grid");
const moduleCount = document.querySelector("#module-count");

const statusLabels = {
  available: "Dostępny",
  foundation: "Fundament",
  planned: "Planowany",
};

function formatUptime(totalSeconds) {
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);

  if (days > 0) return `${days} d ${hours} h`;
  if (hours > 0) return `${hours} h ${minutes} min`;
  return `${minutes} min`;
}

function moduleCard(module) {
  const status = statusLabels[module.status] ?? module.status;
  return `
    <article class="module-card">
      <div class="module-card-top">
        <span class="module-id">${module.id.toUpperCase()}</span>
        <span class="module-status ${module.status}">${status}</span>
      </div>
      <h3>${module.name}</h3>
      <p>${module.description}</p>
      <div class="module-line" aria-hidden="true"></div>
    </article>
  `;
}

async function loadHealth() {
  try {
    const response = await fetch("/health", { headers: { accept: "application/json" } });
    if (!response.ok) throw new Error(`health returned ${response.status}`);
    const data = await response.json();

    healthState.textContent = "Operacyjny";
    healthState.classList.add("healthy");
    uptime.textContent = `Czas działania: ${formatUptime(data.uptime_seconds)}`;
    sidebarStatus.textContent = "Usługa dostępna";
    sidebarVersion.textContent = `API v${data.version}`;
  } catch (error) {
    healthState.textContent = "Niedostępny";
    healthState.classList.add("unhealthy");
    uptime.textContent = "Brak odpowiedzi z API";
    sidebarStatus.textContent = "Usługa niedostępna";
    console.error(error);
  }
}

async function loadMeta() {
  try {
    const response = await fetch("/api/v1/meta", { headers: { accept: "application/json" } });
    if (!response.ok) throw new Error(`metadata returned ${response.status}`);
    const data = await response.json();

    moduleGrid.innerHTML = data.modules.map(moduleCard).join("");
    moduleCount.textContent = `${data.modules.length} moduły`;
  } catch (error) {
    moduleGrid.innerHTML = `
      <article class="module-card error-card">
        <h3>Nie udało się pobrać modułów</h3>
        <p>API metadanych nie odpowiedziało. Sprawdź logi usługi.</p>
      </article>
    `;
    moduleCount.textContent = "Błąd";
    console.error(error);
  }
}

loadHealth();
loadMeta();
setInterval(loadHealth, 30000);
