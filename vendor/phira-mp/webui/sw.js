// Service Worker for Phira Management Interface PWA

const CACHE_NAME = 'phira-mgmt-v1.1';
const urlsToCache = [
  '/',
  '/index.html',
  '/manifest.json',
  '/room/',
  '/room/index.html',
  '/sw.js'
];

// 安装阶段 - 缓存资源
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => {
        console.log('缓存已打开');
        return cache.addAll(urlsToCache);
      })
      .then(() => {
        console.log('所有必需资源已缓存');
        return self.skipWaiting(); // 立即激活新的service worker
      })
  );
});

// 拦截请求并提供缓存的资源
self.addEventListener('fetch', event => {
  event.respondWith(
    caches.match(event.request)
      .then(response => {
        // 如果找到缓存的响应则返回它，否则发起网络请求
        if (response) {
          return response;
        }
        return fetch(event.request);
      }
    )
  );
});

// 更新阶段 - 清理旧缓存
self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys().then(cacheNames => {
      return Promise.all(
        cacheNames.map(cacheName => {
          if (cacheName !== CACHE_NAME) {
            console.log('删除旧缓存', cacheName);
            return caches.delete(cacheName);
          }
        })
      ).then(() => {
        console.log('Service Worker 已激活');
        return self.clients.claim(); // 控制所有页面
      });
    })
  );
});