const NAV_PATHS = new Set(['/', '/recents', '/summary', '/explanations']);
const NAV_CACHE_TTL_MS = 30000;
const NAV_PENDING_DELAY_MS = 120;

const navCache = new Map();
let navPage = null;
let navToken = 0;
let navPendingTimer = null;

function navLifecycle(name) {
    return (window.PAGES && window.PAGES[name]) || null;
}

function navTarget(node) {
    const anchor = node && node.closest ? node.closest('a[href]') : null;
    if (!anchor || anchor.target || anchor.hasAttribute('download')) return null;

    let url;
    try {
        url = new URL(anchor.href, location.href);
    } catch (err) {
        return null;
    }

    if (url.origin !== location.origin || !NAV_PATHS.has(url.pathname)) return null;
    return url;
}

function navInvalidate() {
    navCache.clear();
}

async function navFetch(href) {
    const hit = navCache.get(href);
    if (hit && Date.now() - hit.at < NAV_CACHE_TTL_MS) return hit.html;

    const resp = await fetch(href, { credentials: 'same-origin' });
    if (!resp.ok) throw new Error(`Request failed (${resp.status}).`);

    const html = await resp.text();
    navCache.set(href, { html, at: Date.now() });
    return html;
}

function navPrefetch(href) {
    const hit = navCache.get(href);
    if (hit && Date.now() - hit.at < NAV_CACHE_TTL_MS) return;
    navFetch(href).catch(() => {});
}

function navSetPending(on) {
    if (on) {
        document.documentElement.setAttribute('data-nav-pending', 'true');
    } else {
        document.documentElement.removeAttribute('data-nav-pending');
    }
}

function navSyncLinks() {
    document.querySelectorAll('[data-page]').forEach(link => {
        if (link.dataset.page === navPage) {
            link.setAttribute('aria-current', 'page');
        } else {
            link.removeAttribute('aria-current');
        }
    });
}

function navApply(doc) {
    const incoming = doc.getElementById('app');
    const current = document.getElementById('app');
    if (!incoming || !current) return;

    incoming.querySelectorAll('noscript').forEach(el => el.remove());

    const outgoing = navLifecycle(navPage);
    if (outgoing && outgoing.destroy) outgoing.destroy();

    document.title = doc.title;

    const root = document.documentElement;
    const style = doc.documentElement.getAttribute('style');
    if (style) {
        root.setAttribute('style', style);
    } else {
        root.removeAttribute('style');
    }
    if (doc.documentElement.hasAttribute('data-emergency')) {
        root.setAttribute('data-emergency', 'true');
    } else {
        root.removeAttribute('data-emergency');
    }

    current.replaceWith(document.adoptNode(incoming));

    applyAppData();
    navPage = APP.page;
    navSyncLinks();
    window.scrollTo(0, 0);

    const incomingPage = navLifecycle(navPage);
    if (incomingPage && incomingPage.init) incomingPage.init();
}

async function navigate(href, options = {}) {
    const token = ++navToken;

    clearTimeout(navPendingTimer);
    navPendingTimer = setTimeout(() => navSetPending(true), NAV_PENDING_DELAY_MS);

    let html;
    try {
        html = await navFetch(href);
    } catch (err) {
        clearTimeout(navPendingTimer);
        navSetPending(false);
        location.href = href;
        return;
    }

    clearTimeout(navPendingTimer);
    navSetPending(false);
    if (token !== navToken) return;

    const doc = new DOMParser().parseFromString(html, 'text/html');
    if (!doc.getElementById('app')) {
        location.href = href;
        return;
    }

    if (options.push !== false) history.pushState({ nav: true }, '', href);

    const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (document.startViewTransition && !reduced) {
        try {
            await document.startViewTransition(() => navApply(doc)).finished;
        } catch (err) {
            navSetPending(false);
        }
    } else {
        navApply(doc);
    }
}

function onNavClick(ev) {
    if (ev.defaultPrevented || ev.button !== 0) return;
    if (ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) return;

    const url = navTarget(ev.target);
    if (!url) return;

    ev.preventDefault();
    if (url.href === location.href) return;
    navigate(url.href);
}

function onNavHover(ev) {
    const url = navTarget(ev.target);
    if (url && url.href !== location.href) navPrefetch(url.href);
}

function navInit() {
    applyAppData();
    navPage = APP.page;
    navSyncLinks();

    document.addEventListener('click', onNavClick);
    document.addEventListener('pointerover', onNavHover);
    document.addEventListener('touchstart', onNavHover, { passive: true });
    window.addEventListener('popstate', () => navigate(location.href, { push: false }));

    const page = navLifecycle(navPage);
    if (page && page.init) page.init();
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', navInit);
} else {
    navInit();
}
