document.addEventListener("DOMContentLoaded", async () => {
  const input = document.getElementById("search-box");
  const popover = document.getElementById("search-results");
  if (!input || !popover) return;

  let index = [];
  try {
    const res = await fetch("../../assets/search_index.json");
    index = await res.json();
  } catch (err) {
    console.warn("Doc search index unavailable:", err);
  }

  // Keyboard shortcut '/' to focus search
  window.addEventListener("keydown", (e) => {
    if (e.key === "/" && document.activeElement !== input) {
      e.preventDefault();
      input.focus();
    }
  });

  input.addEventListener("input", (e) => {
    const query = e.target.value.trim().toLowerCase();
    if (!query) {
      popover.classList.add("hidden");
      popover.innerHTML = "";
      return;
    }

    const matches = index
      .filter((s) => s.name.toLowerCase().includes(query) || s.fqsn.toLowerCase().includes(query))
      .slice(0, 10);

    if (matches.length === 0) {
      popover.innerHTML = '<div class="search-empty">No results found</div>';
    } else {
      popover.innerHTML = matches
        .map(
          (m) => `
          <a class="search-result-item" href="../../${m.href}">
            <div class="result-header">
              <span class="result-name">${m.name}</span>
              <span class="result-kind">${m.kind}</span>
            </div>
            <div class="result-summary">${m.summary || m.module}</div>
          </a>`
        )
        .join("");
    }
    popover.classList.remove("hidden");
  });
});
