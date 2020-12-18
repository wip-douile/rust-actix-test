'use strict';

(async function() {
  let count = 0;
  const output = document.querySelector('.the-button-clicks');

  const url = new URL('/api/socket', location);
  url.protocol = 'ws:';
  console.log(url.toString());
  const ws = new WebSocket(url);

  window.addEventListener('click', function(e) {
    if (!e.isTrusted) return;
    if (!e.target.classList.contains('the-button')) return;
    ws.send(JSON.stringify({ t: 'pressed' }));
  });

  ws.addEventListener('message', function(e) {
    const d = JSON.parse(e.data);
    if (d.t === 'count') {
      output.innerText = d.c;
    }
  })

})();
