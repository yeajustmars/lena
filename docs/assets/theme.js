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
});
