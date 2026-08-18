/* 深色／淺色主題：記在 localStorage，首次載入跟隨系統設定 */
(function () {
  var KEY = 'worklog.theme';

  function stored() {
    try { return localStorage.getItem(KEY); } catch (e) { return null; }
  }

  function apply(mode) {
    var dark = mode === 'dark';
    document.documentElement.classList.toggle('dark', dark);
    document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
  }

  var initial = stored();
  if (!initial) {
    initial = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  apply(initial);

  window.Theme = {
    current: function () {
      return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
    },
    toggle: function () {
      var next = window.Theme.current() === 'dark' ? 'light' : 'dark';
      apply(next);
      try { localStorage.setItem(KEY, next); } catch (e) {}
      window.Theme.sync();
      return next;
    },
    /* 讓頁面上所有 [data-theme-toggle] 按鈕更新圖示與無障礙標籤 */
    sync: function () {
      var dark = window.Theme.current() === 'dark';
      document.querySelectorAll('[data-theme-toggle]').forEach(function (btn) {
        var sun = btn.querySelector('[data-icon="sun"]');
        var moon = btn.querySelector('[data-icon="moon"]');
        if (sun) sun.classList.toggle('hidden', !dark);
        if (moon) moon.classList.toggle('hidden', dark);
        btn.setAttribute('aria-label', dark ? '切換為淺色模式' : '切換為深色模式');
        btn.setAttribute('title', dark ? '切換為淺色模式' : '切換為深色模式');
      });
    },
    mount: function () {
      document.querySelectorAll('[data-theme-toggle]').forEach(function (btn) {
        btn.addEventListener('click', window.Theme.toggle);
      });
      window.Theme.sync();
    },
  };

  document.addEventListener('DOMContentLoaded', function () { window.Theme.mount(); });
})();
