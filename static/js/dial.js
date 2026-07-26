const DIAL_NS = 'http://www.w3.org/2000/svg';
const DIAL_CX = 100;
const DIAL_CY = 100;
const DIAL_R = 70;
const DIAL_WINDOW_MS = 86400000;
const DIAL_MIN_SPAN = 0.35;
const DIAL_SWEEP_MS = 1000;

function dialEl(name, attrs) {
    const el = document.createElementNS(DIAL_NS, name);
    for (const k in attrs) el.setAttribute(k, attrs[k]);
    return el;
}

function dialPolar(r, deg) {
    const a = (deg - 90) * Math.PI / 180;
    return [DIAL_CX + r * Math.cos(a), DIAL_CY + r * Math.sin(a)];
}

function dialAngle(ms) {
    const d = new Date(Number(ms));
    const mins = d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60;
    return (mins / 1440) * 360;
}

function dialSpan(a1, a2) {
    let span = (a2 - a1) % 360;
    if (span < 0) span += 360;
    return span;
}

function dialArc(r, a1, span) {
    const [x1, y1] = dialPolar(r, a1);
    const [x2, y2] = dialPolar(r, a1 + span);
    return `M ${x1.toFixed(3)} ${y1.toFixed(3)} A ${r} ${r} 0 ${span > 180 ? 1 : 0} 1 ${x2.toFixed(3)} ${y2.toFixed(3)}`;
}

function dialScaffold(svg) {
    svg.appendChild(dialEl('circle', { class: 'dial-track', cx: DIAL_CX, cy: DIAL_CY, r: DIAL_R }));

    for (let h = 0; h < 24; h++) {
        const major = h % 6 === 0;
        const deg = (h / 24) * 360;
        const [x1, y1] = dialPolar(81, deg);
        const [x2, y2] = dialPolar(major ? 87 : 84.5, deg);
        svg.appendChild(dialEl('line', {
            class: major ? 'dial-tick-major' : 'dial-tick',
            x1: x1.toFixed(2), y1: y1.toFixed(2), x2: x2.toFixed(2), y2: y2.toFixed(2)
        }));

        if (major) {
            const [lx, ly] = dialPolar(93, deg);
            const label = dialEl('text', { class: 'dial-num', x: lx.toFixed(2), y: ly.toFixed(2) });
            label.textContent = pad(h);
            svg.appendChild(label);
        }
    }
}

function dialMarker(svg, nowMs) {
    const deg = dialAngle(nowMs);
    const [tx, ty] = dialPolar(79.5, deg);
    const [rx, ry] = dialPolar(86, deg - 2.6);
    const [sx, sy] = dialPolar(86, deg + 2.6);
    const marker = dialEl('polygon', {
        class: 'dial-now',
        points: `${tx.toFixed(2)},${ty.toFixed(2)} ${rx.toFixed(2)},${ry.toFixed(2)} ${sx.toFixed(2)},${sy.toFixed(2)}`
    });
    marker.appendChild(dialEl('title', {})).textContent = `Now, ${clockHM(nowMs)}`;
    svg.appendChild(marker);
}

function dialSegmentTitle(seg) {
    const meta = STATE_META[seg.state];
    const name = meta ? `${meta[0]} ${meta[1]}` : `State ${seg.state}`;
    return `${name} · ${clockHM(seg.start)}–${clockHM(seg.end)} · ${msToReadable(seg.end - seg.start)}`;
}

function dialRenderSegments(svg, segs, fromMs, toMs) {
    const total = Math.max(1, toMs - fromMs);
    const tiny = [];
    let elapsed = 0;

    segs.forEach(seg => {
        const meta = STATE_META[seg.state];
        const dur = seg.end - seg.start;
        const a1 = dialAngle(seg.start);
        const full = dur >= total - 1000;
        const raw = full ? 360 : dialSpan(a1, dialAngle(seg.end));
        const span = Math.max(raw, DIAL_MIN_SPAN);
        const frac = dur / total;

        const path = full
            ? dialEl('circle', {
                class: 'dial-seg', cx: DIAL_CX, cy: DIAL_CY, r: DIAL_R,
                stroke: meta ? meta[2] : 'currentColor'
            })
            : dialEl('path', {
                class: 'dial-seg',
                d: dialArc(DIAL_R, a1, span),
                stroke: meta ? meta[2] : 'currentColor'
            });
        path.style.setProperty('--len', (DIAL_R * span * Math.PI / 180).toFixed(2));
        path.style.setProperty('--delay', `${Math.round((elapsed / total) * DIAL_SWEEP_MS)}ms`);
        path.style.animationDuration = `${Math.max(90, Math.round(frac * DIAL_SWEEP_MS))}ms`;
        path.appendChild(dialEl('title', {})).textContent = dialSegmentTitle(seg);

        svg.appendChild(path);
        if (raw < DIAL_MIN_SPAN) tiny.push(path);
        elapsed += seg.end - seg.start;
    });

    tiny.forEach(p => svg.appendChild(p));
}

function dialRenderLegend(segs) {
    const list = document.getElementById('dial-legend');
    if (!list) return;
    list.innerHTML = '';

    if (segs.length === 0) {
        const li = document.createElement('li');
        li.textContent = 'Nothing recorded in the last 24 hours.';
        list.appendChild(li);
        return;
    }

    const totals = new Map();
    segs.forEach(s => totals.set(s.state, (totals.get(s.state) || 0) + (s.end - s.start)));

    [...totals.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 6)
        .forEach(([stateIdx, ms]) => {
            const meta = STATE_META[stateIdx];
            const li = document.createElement('li');

            const swatch = document.createElement('span');
            swatch.className = 'swatch';
            swatch.style.background = meta ? meta[2] : '#ccc';

            const name = document.createElement('span');
            name.textContent = meta ? meta[1] : `State ${stateIdx}`;

            const amount = document.createElement('span');
            amount.className = 'amount';
            amount.textContent = msToReadable(ms);

            li.append(swatch, name, amount);
            list.appendChild(li);
        });
}

async function loadDial() {
    const svg = document.getElementById('dial');
    if (!svg) return;

    const now = Date.now();
    const from = now - DIAL_WINDOW_MS;

    svg.innerHTML = '';
    dialScaffold(svg);

    const since = document.getElementById('since');
    if (since) {
        since.textContent = (typeof HAS_ENTRY !== 'undefined' && HAS_ENTRY) ? `since ${clockHM(start)}` : '';
    }

    let pairs = [];
    try {
        const resp = await fetch(`/api/recents?days=2&key=${window.ENTRY_KEY}`);
        if (resp.ok) pairs = await resp.json();
    } catch (err) {
        pairs = [];
    }

    const segs = buildSegments(pairs, from, now);
    dialRenderSegments(svg, segs, from, now);
    dialMarker(svg, now);
    dialRenderLegend(segs);
}

document.addEventListener('DOMContentLoaded', () => loadDial());
