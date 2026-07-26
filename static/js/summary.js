const PIE_NS = 'http://www.w3.org/2000/svg';
const PIE_C = 21;
const PIE_R = 15.91549431;

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

    let target = idx;
    if (target === null) {
        target = summaryTotals.reduce((best, ms, i) => (ms > summaryTotals[best] ? i : best), 0);
    }

    const ms = summaryTotals[target] || 0;
    labelEl.textContent = STATES_DATA[target][0];
    totalEl.textContent = msToReadable(ms);
    pctEl.textContent = `${((ms / summaryTotal) * 100).toFixed(1)}% of ${msToReadable(summaryTotal)}`;
}

function highlightSlice(idx) {
    document.querySelectorAll('#pie circle').forEach(c => {
        c.style.opacity = (idx === null || Number(c.dataset.state) === idx) ? '1' : '0.28';
    });
    setDonutFace(idx);
}

function renderSegments(msArray) {
    const total = msArray.reduce((a, b) => a + b, 0);
    const svg = document.getElementById('pie');
    svg.innerHTML = '';
    const legend = document.getElementById('legend');
    legend.innerHTML = '';

    summaryTotals = msArray;
    summaryTotal = total;
    setDonutFace(null);

    const denom = total || 1;
    let offset = 25;

    msArray.forEach((ms, idx) => {
        const percent = (ms / denom) * 100;
        const color = STATES_DATA[idx][1];

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
            circle.addEventListener('mouseenter', () => highlightSlice(idx));
            circle.addEventListener('mouseleave', () => highlightSlice(null));
            svg.appendChild(circle);
        }

        const li = document.createElement('li');
        if (ms <= 0) li.className = 'legend-empty';

        const colorDot = document.createElement('span');
        colorDot.className = 'legend-color';
        colorDot.style.background = color;

        const labelText = document.createElement('span');
        labelText.className = 'legend-name';
        labelText.textContent = STATES_DATA[idx][0];

        const bar = document.createElement('span');
        bar.className = 'legend-bar';
        const fill = document.createElement('i');
        fill.style.background = color;
        fill.style.width = `${percent}%`;
        bar.appendChild(fill);

        const amount = document.createElement('span');
        amount.className = 'legend-amount';
        amount.textContent = msToReadable(ms);

        li.append(colorDot, labelText, bar, amount);

        if (ms > 0) {
            li.addEventListener('mouseenter', () => highlightSlice(idx));
            li.addEventListener('mouseleave', () => highlightSlice(null));
        }

        legend.appendChild(li);

        offset = offset - percent;
    });
}

async function loadRange(days) {
    document.querySelectorAll('.range-btn').forEach(btn => {
        btn.setAttribute('aria-pressed', String(parseInt(btn.getAttribute('data-range'), 10) === days));
    });

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

document.querySelectorAll('.range-btn').forEach(btn => {
    btn.addEventListener('click', (ev) => {
        const days = parseInt(ev.currentTarget.getAttribute('data-range'), 10);
        loadRange(days);
    });
});
