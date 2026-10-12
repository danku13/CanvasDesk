/* Content OS landing — style switcher.
   Swaps data-style on <html>, persists the choice, announces it. */
(function () {
  'use strict';

  var KEY = 'content-os-style';
  var root = document.documentElement;
  var form = document.querySelector('.switcher');
  var status = document.querySelector('.switcher__status');
  if (!form) return;

  var NAMES = {
    'art-nouveau': 'Ар-нуво',
    'de-stijl': 'Де Стейл',
    'bauhaus': 'Баухаус',
    'constructivism': 'Конструктивизм',
    'art-deco': 'Ар-деко'
  };

  function current() {
    return root.getAttribute('data-style') || 'bauhaus';
  }

  function announce(name) {
    if (!status) return;
    status.textContent = 'Стиль оформления: ' + name;
  }

  // restore saved choice (validate against available radios)
  var saved = null;
  try { saved = localStorage.getItem(KEY); } catch (e) { /* private mode */ }
  if (saved && form.querySelector('input[value="' + saved + '"]')) {
    root.setAttribute('data-style', saved);
  }

  var active = form.querySelector('input[value="' + current() + '"]');
  if (active) active.checked = true;

  form.addEventListener('change', function (e) {
    var v = e.target && e.target.value;
    if (!v || !NAMES[v]) return;
    root.setAttribute('data-style', v);
    try { localStorage.setItem(KEY, v); } catch (err) { /* ignore */ }
    announce(NAMES[v]);
  });

  announce(NAMES[current()]);
})();
