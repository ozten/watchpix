pub fn gallery_html() -> &'static str {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>watchpix</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #0f0f0f;
    color: #e0e0e0;
    min-height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    background: #1a1a1a;
    border-bottom: 1px solid #2a2a2a;
    position: sticky;
    top: 0;
    z-index: 10;
  }
  header h1 {
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }
  .header-right {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: #888;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: inline-block;
    transition: background 0.3s;
  }
  .status-dot.connected { background: #4caf50; }
  .status-dot.reconnecting { background: #ff9800; }
  .status-dot.disconnected { background: #f44336; }
  #gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
    padding: 20px 24px;
  }
  .card {
    background: #1e1e1e;
    border-radius: 8px;
    overflow: hidden;
    cursor: pointer;
    transition: transform 0.15s, opacity 0.3s;
    animation: fadeIn 0.3s ease;
  }
  .card:hover { transform: scale(1.02); }
  .card.removing {
    opacity: 0;
    transform: scale(0.95);
    transition: opacity 0.3s, transform 0.3s;
  }
  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .card img {
    width: 100%;
    height: 180px;
    object-fit: cover;
    display: block;
    background: #2a2a2a;
  }
  .card-info {
    padding: 8px 10px;
  }
  .card-path {
    font-size: 12px;
    color: #ccc;
    word-break: break-all;
    line-height: 1.3;
  }
  .card-meta {
    font-size: 11px;
    color: #777;
    margin-top: 4px;
    display: flex;
    justify-content: space-between;
  }
  .empty-state {
    text-align: center;
    padding: 80px 20px;
    color: #666;
  }
  .empty-state h2 { font-size: 20px; margin-bottom: 8px; color: #888; }
  .empty-state p { font-size: 14px; }
  #load-more-wrap {
    text-align: center;
    padding: 20px 24px 32px;
  }
  #load-more {
    background: #2a2a2a;
    color: #ccc;
    border: 1px solid #444;
    border-radius: 6px;
    padding: 10px 28px;
    font-size: 14px;
    cursor: pointer;
    transition: background 0.15s;
  }
  #load-more:hover { background: #3a3a3a; }

  /* Lightbox */
  #lightbox {
    display: none;
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.92);
    z-index: 100;
    justify-content: center;
    align-items: center;
    cursor: pointer;
  }
  #lightbox.active { display: flex; }
  #lightbox img {
    max-width: 95vw;
    max-height: 95vh;
    object-fit: contain;
    border-radius: 4px;
  }
</style>
</head>
<body>
<header>
  <h1>watchpix</h1>
  <div class="header-right">
    <span id="count"></span>
    <span class="status-dot connected" id="status-dot" title="Connected"></span>
  </div>
</header>
<div id="gallery"></div>
<div id="load-more-wrap" style="display:none"><button id="load-more">Load more</button></div>
<div id="lightbox"><img id="lightbox-img" src="" alt=""></div>

<script>
(function() {
  const gallery = document.getElementById('gallery');
  const lightbox = document.getElementById('lightbox');
  const lightboxImg = document.getElementById('lightbox-img');
  const statusDot = document.getElementById('status-dot');
  const countEl = document.getElementById('count');
  const loadMoreWrap = document.getElementById('load-more-wrap');
  const loadMoreBtn = document.getElementById('load-more');

  const PAGE_SIZE = 15;
  let loaded = 0;
  let totalImages = 0;

  function formatSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / 1048576).toFixed(1) + ' MB';
  }

  function formatTime(epoch) {
    const d = new Date(epoch * 1000);
    const pad = n => String(n).padStart(2, '0');
    return d.getFullYear() + '-' + pad(d.getMonth()+1) + '-' + pad(d.getDate())
      + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
  }

  function updateCount() {
    const shown = gallery.querySelectorAll('.card').length;
    countEl.textContent = shown + ' of ' + totalImages + ' image' + (totalImages !== 1 ? 's' : '');
  }

  function updateLoadMore() {
    loadMoreWrap.style.display = loaded < totalImages ? '' : 'none';
  }

  function createCard(img) {
    const card = document.createElement('div');
    card.className = 'card';
    card.dataset.path = img.path;
    card.innerHTML =
      '<img src="' + img.url + '?t=' + img.mtime + '" alt="' + img.path + '" loading="lazy">' +
      '<div class="card-info">' +
        '<div class="card-path">' + img.path + '</div>' +
        '<div class="card-meta"><span>' + formatTime(img.mtime) + '</span><span>' + formatSize(img.size) + '</span></div>' +
      '</div>';
    card.addEventListener('click', function() {
      lightboxImg.src = img.url + '?t=' + img.mtime;
      lightbox.classList.add('active');
    });
    return card;
  }

  function findCard(path) {
    return gallery.querySelector('.card[data-path="' + CSS.escape(path) + '"]');
  }

  function showEmpty() {
    if (gallery.querySelectorAll('.card').length === 0) {
      gallery.innerHTML = '<div class="empty-state"><h2>No images found</h2><p>Add image files to the watched directory to see them here.</p></div>';
    }
  }

  function clearEmpty() {
    const empty = gallery.querySelector('.empty-state');
    if (empty) empty.remove();
  }

  function fetchImages(offset) {
    return fetch('/api/images?offset=' + offset + '&limit=' + PAGE_SIZE)
      .then(function(r) { return r.json(); });
  }

  // Initial load
  fetchImages(0).then(function(data) {
    totalImages = data.total;
    data.images.forEach(function(img) { gallery.appendChild(createCard(img)); });
    loaded = data.count;
    updateCount();
    updateLoadMore();
    showEmpty();
  });

  // Load more
  loadMoreBtn.addEventListener('click', function() {
    fetchImages(loaded).then(function(data) {
      totalImages = data.total;
      data.images.forEach(function(img) { gallery.appendChild(createCard(img)); });
      loaded += data.count;
      updateCount();
      updateLoadMore();
    });
  });

  // Lightbox close
  lightbox.addEventListener('click', function() {
    lightbox.classList.remove('active');
    lightboxImg.src = '';
  });
  document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape' && lightbox.classList.contains('active')) {
      lightbox.classList.remove('active');
      lightboxImg.src = '';
    }
  });

  // WebSocket with exponential backoff
  let ws;
  let retryDelay = 1000;
  const maxDelay = 30000;

  function setStatus(state) {
    statusDot.className = 'status-dot ' + state;
    statusDot.title = state.charAt(0).toUpperCase() + state.slice(1);
  }

  function connectWs() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(proto + '//' + location.host + '/ws');

    ws.onopen = function() {
      retryDelay = 1000;
      setStatus('connected');
    };

    ws.onclose = function() {
      setStatus('reconnecting');
      setTimeout(function() {
        retryDelay = Math.min(retryDelay * 2, maxDelay);
        connectWs();
      }, retryDelay);
    };

    ws.onerror = function() {
      setStatus('disconnected');
    };

    ws.onmessage = function(e) {
      let msg;
      try { msg = JSON.parse(e.data); } catch(_) { return; }

      if (msg.type === 'add') {
        clearEmpty();
        const existing = findCard(msg.image.path);
        if (!existing) totalImages++;
        if (existing) existing.remove(); else loaded++;
        gallery.prepend(createCard(msg.image));
        updateCount();
        updateLoadMore();
      } else if (msg.type === 'update') {
        const card = findCard(msg.image.path);
        if (card) card.remove();
        clearEmpty();
        gallery.prepend(createCard(msg.image));
        updateCount();
      } else if (msg.type === 'remove') {
        const card = findCard(msg.path);
        totalImages--;
        if (card) {
          loaded--;
          card.classList.add('removing');
          setTimeout(function() {
            card.remove();
            updateCount();
            updateLoadMore();
            showEmpty();
          }, 300);
        }
      }
    };
  }

  connectWs();
})();
</script>
</body>
</html>"##
}
