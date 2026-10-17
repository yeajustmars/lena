// lena-core/src/docgen/assets/search.js

document.addEventListener("DOMContentLoaded", async () => {
  const input = document.getElementById("search-box");
  const popover = document.getElementById("search-results");
  if (!input || !popover) return;

  const rootRel = window.ROOT_REL || "../../";
  let index = [];

  try {
    const res = await fetch(rootRel + "assets/search_index.json");
    if (res.ok) {
      index = await res.json();
    } else {
      console.warn("Doc search index returned status:", res.status);
    }
  } catch (err) {
    console.warn("Doc search index unavailable:", err);
  }

  // Keyboard shortcut '/' to focus search
  window.addEventListener("keydown", (e) => {
    if (e.key === "/" && document.activeElement !== input) {
      e.preventDefault();
      input.focus();
    }
    // Escape key closes the search popup
    if (e.key === "Escape" && !popover.classList.contains("hidden")) {
      popover.classList.add("hidden");
      input.blur();
    }
  });

  // Close popup when clicking anywhere outside of the search box
  document.addEventListener("click", (e) => {
    if (!input.contains(e.target) && !popover.contains(e.target)) {
      popover.classList.add("hidden");
    }
  });

  // Re-open popup if the user clicks back into a populated search box
  input.addEventListener("focus", (e) => {
    if (e.target.value.trim() !== "") {
      input.dispatchEvent(new Event("input"));
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
      .filter((s) => {
        const name = s.name ? s.name.toLowerCase() : "";
        const fqsn = s.fqsn ? s.fqsn.toLowerCase() : "";
        return name.includes(query) || fqsn.includes(query);
      })
      .slice(0, 15);

    if (matches.length === 0) {
      popover.innerHTML = `<div class="search-empty">No results found for "${e.target.value}"</div>`;
    } else {
      popover.innerHTML = matches
        .map(
          (m) => `
          <a class="search-result-item" href="${rootRel}${m.href}">
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
