// Lane-callout filter — the site's only script. With JavaScript off, no
// control is rendered and every lane callout stays visible.
(function () {
  var header = document.querySelector('.site-inner');
  if (!header || !document.querySelector('aside.lane')) return;
  var lanes = ['all', 'degen', 'builder', 'scholar'];
  var box = document.createElement('div');
  box.className = 'lane-toggle';
  box.setAttribute('role', 'group');
  box.setAttribute('aria-label', 'Filter lane callouts');
  var label = document.createElement('span');
  label.textContent = 'lanes:';
  box.appendChild(label);
  lanes.forEach(function (lane) {
    var b = document.createElement('button');
    b.type = 'button';
    b.textContent = lane;
    b.setAttribute('aria-pressed', lane === 'all' ? 'true' : 'false');
    b.addEventListener('click', function () {
      if (lane === 'all') {
        delete document.body.dataset.lane;
      } else {
        document.body.dataset.lane = lane;
      }
      try { sessionStorage.setItem('lane', lane); } catch (e) {}
      box.querySelectorAll('button').forEach(function (btn) {
        btn.setAttribute('aria-pressed', btn === b ? 'true' : 'false');
      });
    });
    box.appendChild(b);
  });
  header.appendChild(box);
  var saved = null;
  try { saved = sessionStorage.getItem('lane'); } catch (e) {}
  if (saved && saved !== 'all' && lanes.indexOf(saved) > 0) {
    document.body.dataset.lane = saved;
    box.querySelectorAll('button').forEach(function (btn) {
      btn.setAttribute('aria-pressed', btn.textContent === saved ? 'true' : 'false');
    });
  }
})();
