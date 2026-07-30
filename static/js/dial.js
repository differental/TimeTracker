const DIAL_NS = 'http://www.w3.org/2000/svg';
const DIAL_CX = 100;
const DIAL_CY = 100;
const DIAL_R = 70;
const DIAL_WINDOW_MS = 86400000;
const DIAL_MIN_SPAN = 0.35;
const DIAL_SWEEP_MS = 1000;
const DIAL_PATH_LEN = 440;

let dialSegsGroup = null;
let dialMarkerGroup = null;
let dialFirstSeg = null;
let dialLastSeg = null;
let dialSegCount = -1;
let dialSweepTimer = null;

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
    if (span >= 360) {
        const [xm, ym] = dialPolar(r, a1 + 180);
        return `M ${x1.toFixed(3)} ${y1.toFixed(3)} A ${r} ${r} 0 0 1 ${xm.toFixed(3)} ${ym.toFixed(3)}`
            + ` A ${r} ${r} 0 0 1 ${x1.toFixed(3)} ${y1.toFixed(3)}`;
    }
    const [x2, y2] = dialPolar(r, a1 + span);
    return `M ${x1.toFixed(3)} ${y1.toFixed(3)} A ${r} ${r} 0 ${span > 180 ? 1 : 0} 1 ${x2.toFixed(3)} ${y2.toFixed(3)}`;
}

function dialScaffold(svg) {
    const g = dialEl('g', { class: 'dial-scaffold' });
    g.appendChild(dialEl('circle', { class: 'dial-track', cx: DIAL_CX, cy: DIAL_CY, r: DIAL_R }));

    for (let h = 0; h < 24; h++) {
        const major = h % 6 === 0;
        const deg = (h / 24) * 360;
        const [x1, y1] = dialPolar(81, deg);
        const [x2, y2] = dialPolar(major ? 87 : 84.5, deg);
        g.appendChild(dialEl('line', {
            class: major ? 'dial-tick-major' : 'dial-tick',
            x1: x1.toFixed(2), y1: y1.toFixed(2), x2: x2.toFixed(2), y2: y2.toFixed(2)
        }));

        if (major) {
            const [lx, ly] = dialPolar(93, deg);
            const label = dialEl('text', { class: 'dial-num', x: lx.toFixed(2), y: ly.toFixed(2) });
            label.textContent = pad(h);
            g.appendChild(label);
        }
    }

    svg.appendChild(g);
}

function dialMarker(svg, nowMs) {
    const deg = dialAngle(nowMs);
    const g = dialEl('g', { class: 'dial-marker' });

    const [lix, liy] = dialPolar(60.5, deg);
    const [lox, loy] = dialPolar(79.5, deg);
    g.appendChild(dialEl('line', {
        class: 'dial-now-line',
        x1: lix.toFixed(2), y1: liy.toFixed(2), x2: lox.toFixed(2), y2: loy.toFixed(2)
    }));

    const [tx, ty] = dialPolar(79.5, deg);
    const [rx, ry] = dialPolar(86, deg - 2.6);
    const [sx, sy] = dialPolar(86, deg + 2.6);
    const marker = dialEl('polygon', {
        class: 'dial-now',
        points: `${tx.toFixed(2)},${ty.toFixed(2)} ${rx.toFixed(2)},${ry.toFixed(2)} ${sx.toFixed(2)},${sy.toFixed(2)}`
    });
    marker.appendChild(dialEl('title', {})).textContent = `Now, ${clockHM(nowMs)}`;
    g.appendChild(marker);

    if (dialMarkerGroup && dialMarkerGroup.isConnected) {
        dialMarkerGroup.replaceWith(g);
    } else {
        svg.appendChild(g);
    }
    dialMarkerGroup = g;
}

function dialSegmentTitle(seg) {
    return `${stateLabel(seg.state)} · ${clockHM(seg.start)}–${clockHM(seg.end)} · ${msToReadable(seg.end - seg.start)}`;
}

function dialSegmentGeometry(seg, total) {
    const a1 = dialAngle(seg.start);
    const dur = seg.end - seg.start;
    const full = dur >= total - 1000;
    const raw = full ? 360 : dialSpan(a1, dialAngle(seg.end));
    return { a1, raw, span: Math.max(raw, DIAL_MIN_SPAN) };
}

function dialSegmentElement(seg, total, sweep, lead) {
    const { a1, raw, span } = dialSegmentGeometry(seg, total);
    const colour = stateColour(seg.state);

    const path = dialEl('path', {
        class: 'dial-seg', d: dialArc(DIAL_R, a1, span), stroke: colour, pathLength: DIAL_PATH_LEN
    });

    if (sweep) {
        path.classList.add('is-sweeping');
        path.style.animationDelay = `${Math.round((lead / 360) * DIAL_SWEEP_MS)}ms`;
        path.style.animationDuration = `${Math.max(1, Math.round((span / 360) * DIAL_SWEEP_MS))}ms`;
    }

    path.appendChild(dialEl('title', {})).textContent = dialSegmentTitle(seg);
    return { path, tiny: raw < DIAL_MIN_SPAN, span };
}

function dialClearSweep() {
    clearTimeout(dialSweepTimer);
    dialSweepTimer = null;
    if (!dialSegsGroup) return;
    dialSegsGroup.querySelectorAll('.is-sweeping').forEach(path => {
        path.classList.remove('is-sweeping');
        path.style.removeProperty('animation-delay');
        path.style.removeProperty('animation-duration');
    });
}

function dialDrawSegments(svg, segs, fromMs, toMs, sweep) {
    const total = Math.max(1, toMs - fromMs);
    const g = dialEl('g', { class: 'dial-segments' });
    const tiny = [];
    let lead = 0;

    dialFirstSeg = null;
    dialLastSeg = null;

    segs.forEach((seg, i) => {
        const { path, tiny: isTiny, span } = dialSegmentElement(seg, total, sweep, lead);
        g.appendChild(path);
        if (isTiny) tiny.push(path);
        if (i === 0) dialFirstSeg = path;
        if (i === segs.length - 1) dialLastSeg = path;
        lead += span;
    });

    tiny.forEach(p => g.appendChild(p));

    if (dialSegsGroup && dialSegsGroup.isConnected) {
        dialSegsGroup.replaceWith(g);
    } else {
        svg.appendChild(g);
    }
    dialSegsGroup = g;
    dialSegCount = segs.length;

    clearTimeout(dialSweepTimer);
    dialSweepTimer = sweep ? setTimeout(dialClearSweep, DIAL_SWEEP_MS * 2 + 400) : null;
}

function dialResizeSegment(path, seg, total) {
    if (!path || path.tagName !== 'path') return;
    const { span, a1 } = dialSegmentGeometry(seg, total);
    path.setAttribute('d', dialArc(DIAL_R, a1, span));
    const title = path.querySelector('title');
    if (title) title.textContent = dialSegmentTitle(seg);
}

function dialGrow(segs, fromMs, toMs) {
    if (segs.length === 0) return;
    const total = Math.max(1, toMs - fromMs);
    dialResizeSegment(dialFirstSeg, segs[0], total);
    if (segs.length > 1) dialResizeSegment(dialLastSeg, segs[segs.length - 1], total);
}

function dialReset() {
    clearTimeout(dialSweepTimer);
    dialSweepTimer = null;
    dialSegsGroup = null;
    dialMarkerGroup = null;
    dialFirstSeg = null;
    dialLastSeg = null;
    dialSegCount = -1;
}

function dialUpdate(pairs, options = {}) {
    const svg = document.getElementById('dial');
    if (!svg) return;

    if (!svg.querySelector('.dial-scaffold')) {
        svg.innerHTML = '';
        dialReset();
        dialScaffold(svg);
    }

    const now = Date.now();
    const from = now - DIAL_WINDOW_MS;
    const segs = buildSegments(pairs, from, now);
    const stale = !dialSegsGroup || !dialSegsGroup.isConnected || segs.length !== dialSegCount;

    if (options.rebuild || stale) {
        dialDrawSegments(svg, segs, from, now, options.sweep === true && !document.hidden);
    } else {
        dialGrow(segs, from, now);
    }

    dialMarker(svg, now);
}
