function buildRow(stateIdx, startMs, endMs) {
    const tr = document.createElement('tr');
    const stateTd = document.createElement('td');
    stateTd.className = 'px-6 py-4 whitespace-nowrap text-sm';
    const stateName = STATES_DATA[Number(stateIdx)][0];
    const stateDiv = document.createElement('div');
    stateDiv.className = 'flex items-center gap-3';
    const dot = document.createElement('span');
    dot.className = 'inline-block w-3 h-3 rounded-full';
    dot.style.background = (Number(stateIdx) >= 0 && Number(stateIdx) < STATES_DATA.length) ? STATES_DATA[Number(stateIdx)][1] : '#ccc';
    stateDiv.appendChild(dot);
    const label = document.createElement('span');
    label.textContent = stateName;
    stateDiv.appendChild(label);
    stateTd.appendChild(stateDiv);

    const startTd = document.createElement('td');
    startTd.className = 'px-6 py-4 whitespace-nowrap text-sm text-gray-700';
    startTd.textContent = formatRounded(startMs);

    const endTd = document.createElement('td');
    endTd.className = 'px-6 py-4 whitespace-nowrap text-sm text-gray-700';
    endTd.textContent = formatRounded(endMs);

    const durTd = document.createElement('td');
    durTd.className = 'px-6 py-4 whitespace-nowrap text-sm text-gray-700';
    const durationMs = Number(endMs) - Number(startMs);
    durTd.textContent = msToReadable(durationMs);

    tr.appendChild(stateTd);
    tr.appendChild(startTd);
    tr.appendChild(endTd);
    tr.appendChild(durTd);
    return tr;
}

async function loadEvents(count = 0, days = 1) {
    const tbody = document.getElementById('events-tbody');
    tbody.innerHTML = '';
    try {
        const params = new URLSearchParams();
        if (typeof count !== 'undefined' && count !== null && count !== 0) params.set('count', String(count));
        if (typeof days !== 'undefined' && days !== null && days !== 0) params.set('days', String(days));
        params.set('key', window.ENTRY_KEY);

        const resp = await fetch(`/api/recents?${params.toString()}`);
        if (!resp.ok) {
            const errText = await resp.text();
            await Swal.fire({ icon: 'error', title: 'Error', text: errText });
            return;
        }
        const data = await resp.json();
        if (!Array.isArray(data)) {
            await Swal.fire({ icon: 'error', title: 'Error', text: "API returned unexpected data." });
            return;
        }
        for (let i = 0; i < data.length; i++) {
            const entry = data[i];
            if (!Array.isArray(entry) || entry.length < 2) continue;
            const stateIdx = entry[0];
            const startMs = Number(entry[1]);
            const endMs = (i === 0) ? Date.now() : Number(data[i - 1][1]);
            const row = buildRow(stateIdx, startMs, endMs);
            tbody.appendChild(row);
        }
    } catch (err) {
        Swal.close();
        const msg = (err && err.message) ? err.message : String(err);
        await Swal.fire({ icon: 'error', title: 'Error', text: msg });
    }
}

function wireFilters() {
    const countSelect = document.getElementById('count-select');
    const countCustom = document.getElementById('count-custom-input');
    const daysSelect = document.getElementById('days-select');
    const daysCustom = document.getElementById('days-custom-input');
    const applyBtn = document.getElementById('apply-filters');

    function toggleCustom(selectEl, customInputEl) {
        if (selectEl.value === 'custom') {
            customInputEl.classList.remove('hidden');
            customInputEl.focus();
        } else {
            customInputEl.classList.add('hidden');
        }
    }

    countSelect.addEventListener('change', () => {
        toggleCustom(countSelect, countCustom);
        if (countSelect.value !== 'custom') {
            const r = parseInt(countSelect.value, 10);
            if (!Number.isNaN(r) && r > 0) loadEvents(r, getDaysValue());
        }
    });

    daysSelect.addEventListener('change', () => {
        toggleCustom(daysSelect, daysCustom);
        if (daysSelect.value !== 'custom') {
            const d = parseInt(daysSelect.value, 10);
            if (!Number.isNaN(d) && d > 0) loadEvents(getCountValue(), d);
        }
    });

    // Allow pressing Enter on custom inputs to apply
    [countCustom, daysCustom].forEach(inp => {
        inp.addEventListener('keydown', (ev) => {
            if (ev.key === 'Enter') applyBtn.click();
        });
    });

    applyBtn.addEventListener('click', () => {
        const r = getCountValue();
        const d = getDaysValue();
        if (!Number.isInteger(r) || r < 0) {
            Swal.fire({ icon: 'error', title: 'Error', text: 'Please enter a valid positive integer for items.' });
            return;
        }
        if (!Number.isInteger(d) || d < 0) {
            Swal.fire({ icon: 'error', title: 'Error', text: 'Please enter a valid positive integer for days.' });
            return;
        }
        loadEvents(r, d);
    });

    function getCountValue() {
        if (countSelect.value === 'custom') {
            const n = parseInt(countCustom.value, 10);
            return Number.isNaN(n) ? null : n;
        }
        const n = parseInt(countSelect.value, 10);
        return Number.isNaN(n) ? null : n;
    }

    function getDaysValue() {
        if (daysSelect.value === 'custom') {
            const n = parseInt(daysCustom.value, 10);
            return Number.isNaN(n) ? null : n;
        }
        const n = parseInt(daysSelect.value, 10);
        return Number.isNaN(n) ? null : n;
    }

    countSelect.value = '0';
    daysSelect.value = '1';
    countCustom.value = '10';
    daysCustom.value = '1';
    toggleCustom(countSelect, countCustom);
    toggleCustom(daysSelect, daysCustom);

    loadEvents(0, 1);
}