const PIE_NS = 'http://www.w3.org/2000/svg';
const PIE_C = 21;
const PIE_R = 15.91549431;
const RANGE_MAX_DAYS = 3650;

function summaryError(message) {
    const el = document.getElementById('summary-error');
    if (!el) return;
    if (!message) {
        el.textContent = '';
        el.classList.add('hidden');
        return;
    }
    el.textContent = message;
    el.classList.remove('hidden');
}

let summaryTotals = [];
let summaryTotal = 0;
let summaryDays = 7;
let pinnedSlice = null;
let hoverSlice = null;

function rangeCaption(days) {
    if (days === 1) return 'in the last 24 hours';
    return `in the last ${days} days`;
}

function setDonutFace(idx) {
    const labelEl = document.getElementById('range-label');
    const totalEl = document.getElementById('range-total');
    const pctEl = document.getElementById('range-pct');
    if (!labelEl || !totalEl || !pctEl) return;

    if (summaryTotal <= 0) {
        labelEl.textContent = 'Nothing tracked';
        totalEl.textContent = '—';
        pctEl.textContent = '';
        return;
    }

    if (idx === null) {
        labelEl.textContent = 'Tracked';
        totalEl.textContent = msToReadable(summaryTotal);
        pctEl.textContent = rangeCaption(summaryDays);
        return;
    }

    const ms = summaryTotals[idx] || 0;
    labelEl.textContent = stateLabel(idx);
    totalEl.textContent = msToReadable(ms);
    pctEl.textContent = `${((ms / summaryTotal) * 100).toFixed(1)}% of ${msToReadable(summaryTotal)}`;
}

function activeSlice() {
    return pinnedSlice !== null ? pinnedSlice : hoverSlice;
}

function applyHighlight() {
    const idx = activeSlice();

    document.querySelectorAll('#pie circle').forEach(c => {
        c.style.opacity = (idx === null || Number(c.dataset.state) === idx) ? '1' : '0.28';
    });

    document.querySelectorAll('#legend .legend-row[data-state]').forEach(row => {
        const on = Number(row.dataset.state) === idx;
        row.classList.toggle('is-active', on);
        row.setAttribute('aria-pressed', String(on));
    });

    setDonutFace(idx);
}

function sliceFromEvent(ev) {
    const el = ev.target?.closest?.('#pie circle[data-state], #legend .legend-row[data-state]');
    return el ? Number(el.dataset.state) : null;
}

function renderSegments(msArray) {
    const total = msArray.reduce((a, b) => a + b, 0);
    const svg = document.getElementById('pie');
    svg.innerHTML = '';
    const legend = document.getElementById('legend');
    legend.innerHTML = '';

    summaryTotals = msArray;
    summaryTotal = total;
    pinnedSlice = null;
    hoverSlice = null;
    setDonutFace(null);

    const denom = total || 1;
    let offset = 25;

    msArray.forEach((ms, idx) => {
        const percent = (ms / denom) * 100;
        const color = stateColour(idx);

        if (ms > 0) {
            const circle = document.createElementNS(PIE_NS, 'circle');
            circle.setAttribute('r', String(PIE_R));
            circle.setAttribute('cx', String(PIE_C));
            circle.setAttribute('cy', String(PIE_C));
            circle.setAttribute('stroke', color);
            circle.setAttribute('stroke-width', '5.5');
            circle.setAttribute('fill', 'transparent');
            circle.setAttribute('stroke-dasharray', `${percent} ${100 - percent}`);
            circle.setAttribute('stroke-dashoffset', offset.toString());
            circle.dataset.state = String(idx);
            svg.appendChild(circle);
        }

        const li = document.createElement('li');
        const row = document.createElement(ms > 0 ? 'button' : 'div');
        row.className = ms > 0 ? 'legend-row' : 'legend-row legend-empty';

        if (ms > 0) {
            row.setAttribute('type', 'button');
            row.dataset.state = String(idx);
            row.setAttribute('aria-pressed', 'false');
        }

        const colorDot = document.createElement('span');
        colorDot.className = 'legend-color';
        colorDot.style.background = color;

        const labelText = document.createElement('span');
        labelText.className = 'legend-name';
        labelText.textContent = stateLabel(idx);

        const bar = document.createElement('span');
        bar.className = 'legend-bar';
        const fill = document.createElement('i');
        fill.style.background = color;
        fill.style.width = `${percent}%`;
        bar.appendChild(fill);

        const amount = document.createElement('span');
        amount.className = 'legend-amount';
        amount.textContent = msToReadable(ms);

        row.append(colorDot, labelText, bar, amount);
        li.appendChild(row);
        legend.appendChild(li);

        offset = offset - percent;
    });
}

function setActiveRange(key) {
    document.querySelectorAll('.range-btn').forEach(btn => {
        btn.setAttribute('aria-pressed', String(btn.dataset.range === String(key)));
    });
}

function setCustomOpen(open) {
    const panel = document.getElementById('range-custom');
    const btn = document.querySelector('.range-btn[data-range="custom"]');
    if (panel) panel.classList.toggle('hidden', !open);
    if (btn) btn.setAttribute('aria-expanded', String(open));
}

async function loadRange(days, key = String(days)) {
    setActiveRange(key);
    summaryDays = days;

    try {
        const resp = await fetch(`/api/data?key=${window.ENTRY_KEY}&days=${encodeURIComponent(days)}`);
        if (!resp.ok) {
            const txt = await resp.text();
            summaryError(`Could not load data: ${txt || resp.status}`);
            return;
        }
        const data = await resp.json();
        summaryError('');
        renderSegments(data);
    } catch (err) {
        summaryError(`Network or unexpected error: ${(err && err.message) ? err.message : err}`);
    }
}

function applyCustomRange() {
    const input = document.getElementById('range-custom-input');
    if (!input) return;

    const raw = input.value.trim();
    const days = Number(raw);

    if (raw === '' || !Number.isInteger(days) || days < 1 || days > RANGE_MAX_DAYS) {
        summaryError(`Please enter a whole number of days between 1 and ${RANGE_MAX_DAYS}.`);
        return;
    }

    summaryError('');
    loadRange(days, 'custom');
}

function onSummaryClick(ev) {
    const idx = sliceFromEvent(ev);
    pinnedSlice = (idx !== null && idx !== pinnedSlice) ? idx : null;
    applyHighlight();
}

function onSummaryKeydown(ev) {
    if (ev.key === 'Escape' && pinnedSlice !== null) {
        pinnedSlice = null;
        applyHighlight();
    }
}

function initSummary() {
    const pie = document.getElementById('pie');
    if (!pie) return;

    summaryTotals = [];
    summaryTotal = 0;
    summaryDays = 7;
    pinnedSlice = null;
    hoverSlice = null;

    document.querySelectorAll('.range-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            if (btn.dataset.range === 'custom') {
                setCustomOpen(true);
                setActiveRange('custom');
                const input = document.getElementById('range-custom-input');
                if (input) input.focus();
                return;
            }

            setCustomOpen(false);
            const days = parseInt(btn.dataset.range, 10);
            if (!Number.isNaN(days)) loadRange(days);
        });
    });

    const rangeCustomApply = document.getElementById('range-custom-apply');
    if (rangeCustomApply) {
        rangeCustomApply.addEventListener('click', () => applyCustomRange());
    }

    const rangeCustomInput = document.getElementById('range-custom-input');
    if (rangeCustomInput) {
        rangeCustomInput.addEventListener('keydown', (ev) => {
            if (ev.key === 'Enter') {
                ev.preventDefault();
                applyCustomRange();
            }
        });
    }

    document.addEventListener('click', onSummaryClick);
    document.addEventListener('keydown', onSummaryKeydown);

    ['pie', 'legend'].forEach(id => {
        const root = document.getElementById(id);
        if (!root) return;

        root.addEventListener('pointerover', (ev) => {
            if (ev.pointerType !== 'mouse') return;
            hoverSlice = sliceFromEvent(ev);
            applyHighlight();
        });

        root.addEventListener('pointerleave', (ev) => {
            if (ev.pointerType !== 'mouse') return;
            hoverSlice = null;
            applyHighlight();
        });
    });

    loadRange(7);
}

function destroySummary() {
    document.removeEventListener('click', onSummaryClick);
    document.removeEventListener('keydown', onSummaryKeydown);
}

window.PAGES = window.PAGES || {};
window.PAGES.summary = { init: initSummary, destroy: destroySummary };
