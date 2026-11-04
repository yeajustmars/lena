// lena-core/src/docgen/assets/theme.js

function applyTheme() {
  const savedTheme = localStorage.getItem('lena-theme') || 'grayscale';
  let savedMode = localStorage.getItem('lena-mode') || 'auto';

  // Explicitly evaluate OS Dark Mode and lock it in
  if (savedMode === 'auto') {
    savedMode = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  document.documentElement.setAttribute('data-theme', savedTheme);
  document.documentElement.setAttribute('data-mode', savedMode);
}

// 1. Apply immediately to prevent FOUC
applyTheme();

// 2. Attach UI handlers
document.addEventListener("DOMContentLoaded", () => {
  const themeSelect = document.getElementById("theme-selector");
  const modeToggle = document.getElementById("mode-toggle");

  if (themeSelect) {
    themeSelect.value = localStorage.getItem('lena-theme') || 'grayscale';
    themeSelect.addEventListener("change", (e) => {
      document.documentElement.setAttribute('data-theme', e.target.value);
      localStorage.setItem('lena-theme', e.target.value);
    });
  }

  if (modeToggle) {
    modeToggle.addEventListener("click", () => {
      const currentMode = document.documentElement.getAttribute('data-mode');
      const newMode = currentMode === 'dark' ? 'light' : 'dark';
      document.documentElement.setAttribute('data-mode', newMode);
      localStorage.setItem('lena-mode', newMode);
    });
  }

  if (typeof marked !== 'undefined') {
    document.querySelectorAll('.symbol-desc').forEach(el => {
      el.innerHTML = marked.parse(el.textContent);
    });
  }

  // Restore sidebar scroll position for multi-page navigation
  const sidebar = document.querySelector('.sidebar');
  if (sidebar) {
    const scrollPos = sessionStorage.getItem('lena-sidebar-scroll');
    if (scrollPos !== null) {
      sidebar.scrollTop = parseInt(scrollPos, 10);
    }
    window.addEventListener('beforeunload', () => {
      sessionStorage.setItem('lena-sidebar-scroll', sidebar.scrollTop);
    });
  }

  // Navigation Tree State Management
  const navDetails = document.querySelectorAll('details[data-nav-id]');
  const toggleNavBtn = document.getElementById('toggle-nav-tree');

  const savedTreeState = JSON.parse(localStorage.getItem('lena-nav-state') || '{}');
  let isRestoring = true; // Lock state to prevent startup events from wiping localStorage

  // 1. Apply saved state from localStorage
  navDetails.forEach(detail => {
    const id = detail.getAttribute('data-nav-id');

    if (savedTreeState[id] === true) {
      detail.setAttribute('open', '');
    } else if (savedTreeState[id] === false) {
      detail.removeAttribute('open');
    }

    detail.addEventListener('toggle', () => {
      if (isRestoring) return;
      const currentState = JSON.parse(localStorage.getItem('lena-nav-state') || '{}');
      currentState[id] = detail.hasAttribute('open');
      localStorage.setItem('lena-nav-state', JSON.stringify(currentState));
    });
  });

  // 2. Active Link Highlight & Auto-Expand
  function highlightActiveNav() {
    const currentUrl = new URL(window.location.href);
    let activeLink = null;

    // Reset previous active links (useful for single-page hash changes)
    document.querySelectorAll('.nav-modules a.active-nav').forEach(a => a.classList.remove('active-nav'));

    // Find the link that exactly matches the current URL path and hash
    document.querySelectorAll('.nav-modules a').forEach(link => {
      const linkUrl = new URL(link.href, window.location.href);
      if (currentUrl.pathname === linkUrl.pathname && currentUrl.hash === linkUrl.hash) {
        activeLink = link;
      }
    });

    if (activeLink) {
      activeLink.classList.add('active-nav');

      // Walk up the DOM tree and force all parent directories open
      let parent = activeLink.parentElement;
      while (parent && !parent.classList.contains('sidebar')) {
        if (parent.tagName === 'DETAILS') {
          parent.setAttribute('open', '');
        }
        parent = parent.parentElement;
      }

      // Calculate and scroll ONLY the sidebar, leaving the main window at the top
      const sidebar = document.querySelector('.sidebar');
      if (sidebar) {
        const sidebarRect = sidebar.getBoundingClientRect();
        const linkRect = activeLink.getBoundingClientRect();
        sidebar.scrollTop += (linkRect.top - sidebarRect.top) - (sidebar.clientHeight / 2) + (linkRect.height / 2);
      }
    } else {
      // Fallback: restore generic scroll position if no active link is found (e.g. index.html)
      const sidebar = document.querySelector('.sidebar');
      if (sidebar) {
        const scrollPos = sessionStorage.getItem('lena-sidebar-scroll');
        if (scrollPos !== null) sidebar.scrollTop = parseInt(scrollPos, 10);
      }
    }
  }

  // Run on page load, and also on hash change (for the single-page format)
  highlightActiveNav();
  window.addEventListener('hashchange', highlightActiveNav);

  // Save fallback scroll position on unload
  window.addEventListener('beforeunload', () => {
    const sidebar = document.querySelector('.sidebar');
    if (sidebar) sessionStorage.setItem('lena-sidebar-scroll', sidebar.scrollTop);
  });

  // Release the toggle listener lock after initial DOM painting
  setTimeout(() => { isRestoring = false; }, 50);

  // 3. Expand/Collapse All Button
  if (toggleNavBtn) {
    toggleNavBtn.addEventListener('click', () => {
      const anyClosed = Array.from(navDetails).some(d => !d.hasAttribute('open'));
      const newState = JSON.parse(localStorage.getItem('lena-nav-state') || '{}');

      navDetails.forEach(detail => {
        if (anyClosed) {
          detail.setAttribute('open', '');
        } else {
          detail.removeAttribute('open');
        }
        newState[detail.getAttribute('data-nav-id')] = detail.hasAttribute('open');
      });

      localStorage.setItem('lena-nav-state', JSON.stringify(newState));

      // If we collapsed everything, re-expand the active link so the user doesn't lose their place
      if (!anyClosed) highlightActiveNav();
    });
  }
});
