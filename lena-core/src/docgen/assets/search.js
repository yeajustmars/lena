document.addEventListener("DOMContentLoaded", () => {
  const input = document.getElementById("search-box");
  const popover = document.getElementById("search-results");
  if (!input || !popover) return;

  const rootRel = window.ROOT_REL || "../../";

  // 1. Define the Web Worker logic inline
  const workerCode = `
    let index = [];
    let prefixTrie = {};

    function insertTrie(word, id) {
      if (!word) return;
      let node = prefixTrie;
      for (const char of word.toLowerCase()) {
        if (!node[char]) node[char] = { ids: new Set() };
        node = node[char];
        node.ids.add(id);
      }
    }

    self.onmessage = async function(e) {
      const { type, payload, rootRel, indexData } = e.data;

      if (type === 'INIT') {
        try {
          if (indexData) {
            index = indexData;
          } else {
            const res = await fetch(rootRel + "assets/search_index.json");
            if (res.ok || res.status === 0) index = await res.json();
          }

          // Build Prefix Trie
          index.forEach((item, id) => {
            insertTrie(item.name, id);
            insertTrie(item.fqsn, id);
          });
          self.postMessage({ type: 'READY' });
        } catch (err) {
          console.error("Worker init error:", err);
        }
      } else if (type === 'SEARCH') {
        const query = payload.toLowerCase();
        let node = prefixTrie;
        let matchFound = true;

        // Fast Prefix Traverse
        for (const char of query) {
          if (!node[char]) {
            matchFound = false;
            break;
          }
          node = node[char];
        }

        let results = [];
        if (matchFound && node.ids) {
          const ids = Array.from(node.ids).slice(0, 15);
          results = ids.map(id => index[id]);
        } else {
          // Fallback to substring scan if prefix fails
          results = index.filter(item =>
            (item.name && item.name.toLowerCase().includes(query)) ||
            (item.fqsn && item.fqsn.toLowerCase().includes(query))
          ).slice(0, 15);
        }

        self.postMessage({ type: 'RESULTS', payload: results, query: payload });
      }
    };
  `;

  // 2. Spawn the Worker via Blob URL
  const blob = new Blob([workerCode], { type: 'application/javascript' });
  const worker = new Worker(URL.createObjectURL(blob));

  // Init with either the global window object (Single-Page) or fetch via path (Multi-Page)
  worker.postMessage({ type: 'INIT', rootRel, indexData: window.SEARCH_INDEX || null });

  // 3. Handle Worker Results
  worker.onmessage = (e) => {
    const { type, payload, query } = e.data;
    if (type === 'RESULTS') {
      // Ensure we only render if the result matches the current input (handles out-of-order execution)
      if (input.value.trim().toLowerCase() !== query) return;

      if (payload.length === 0) {
        popover.innerHTML = `<div class="search-empty">No results found for "${query}"</div>`;
      } else {
        popover.innerHTML = payload.map(m => `
          <a class="search-result-item" href="${m.href.startsWith('#') ? '' : rootRel}${m.href}" onclick="document.getElementById('search-results').classList.add('hidden')">
            <div class="result-header">
              <span class="result-name">${m.name}</span>
              <span class="result-kind">${m.kind}</span>
            </div>
            <div class="result-summary">${m.summary || m.module}</div>
          </a>`).join("");
      }
      popover.classList.remove("hidden");
    }
  };

  // 4. UI Event Listeners with Debounce
  let debounceTimeout;

  window.addEventListener("keydown", (e) => {
    if (e.key === "/" && document.activeElement !== input) {
      e.preventDefault();
      input.focus();
    }
    if (e.key === "Escape" && !popover.classList.contains("hidden")) {
      popover.classList.add("hidden");
      input.blur();
    }
  });

  document.addEventListener("click", (e) => {
    if (!input.contains(e.target) && !popover.contains(e.target)) {
      popover.classList.add("hidden");
    }
  });

  input.addEventListener("focus", (e) => {
    if (e.target.value.trim() !== "") input.dispatchEvent(new Event("input"));
  });

  input.addEventListener("input", (e) => {
    const query = e.target.value.trim();
    if (!query) {
      popover.classList.add("hidden");
      popover.innerHTML = "";
      return;
    }

    clearTimeout(debounceTimeout);
    debounceTimeout = setTimeout(() => {
      worker.postMessage({ type: 'SEARCH', payload: query });
    }, 150); // 150ms debounce
  });
});
