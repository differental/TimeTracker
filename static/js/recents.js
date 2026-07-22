// Remember the last-applied filter so we can reload the table after an edit.
let lastCount = 0;
let lastDays = 1;

function buildRow(stateIdx, startMs, endMs, entryIdx) {
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
    // A duration is only meaningful when both endpoints are valid timestamps.
    // Otherwise show 'Error' so the corrupted entry is obvious and can be fixed.
    if (isValidMs(startMs) && isValidMs(endMs)) {
        const durationMs = Number(endMs) - Number(startMs);
        durTd.textContent = msToReadable(durationMs);
    } else {
        durTd.textContent = 'Error';
    }

    const editTd = document.createElement('td');
    editTd.className = 'px-6 py-4 whitespace-nowrap text-right text-sm';
    // entryIdx is null when we couldn't determine the global index (e.g. the
    // length lookup failed) - omit the button rather than risk editing the wrong row.
    if (Number.isInteger(entryIdx) && entryIdx >= 0) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = `edit-btn text-${window.BASE_COLOR || 'blue'}-600 hover:underline font-medium`;
        btn.textContent = 'Edit';
        btn.setAttribute('aria-label', 'Edit start time');
        btn.addEventListener('click', () => openEditDialog(entryIdx, startMs));
        editTd.appendChild(btn);
    }

    tr.appendChild(stateTd);
    tr.appendChild(startTd);
    tr.appendChild(endTd);
    tr.appendChild(durTd);
    tr.appendChild(editTd);
    return tr;
}

// Opens a SweetAlert dialog to edit a single entry's start time and PUTs the
// change. The backend (PUT /api/entry/{idx}) validates that the new start stays
// between the neighbouring entries and returns the error text on violation.
async function openEditDialog(entryIdx, startMs) {
    const initial = msToDatetimeLocal(startMs);
    const result = await Swal.fire({
        title: 'Edit start time',
        html: `<input id="edit-start-input" type="datetime-local" class="swal2-input" value="${initial}" step="60">`,
        showCancelButton: true,
        confirmButtonText: 'Save',
        cancelButtonText: 'Cancel',
        preConfirm: async () => {
            const el = document.getElementById('edit-start-input');
            const val = el && el.value;
            if (!val) {
                Swal.showValidationMessage('Please choose a date and time.');
                return false;
            }
            const ms = new Date(val).getTime();
            if (Number.isNaN(ms)) {
                Swal.showValidationMessage('Invalid date/time.');
                return false;
            }
            try {
                const resp = await fetch(`/api/entry/${entryIdx}?key=${encodeURIComponent(window.ENTRY_KEY)}`, {
                    method: 'PUT',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ start_timestamp: ms })
                });
                if (!resp.ok) {
                    Swal.showValidationMessage(await resp.text());
                    return false;
                }
                return await resp.json();
            } catch (err) {
                Swal.showValidationMessage((err && err.message) ? err.message : String(err));
                return false;
            }
        }
    });

    if (result.isConfirmed) {
        await Swal.fire({ icon: 'success', title: 'Updated', timer: 1200, showConfirmButton: false });
        loadEvents(lastCount, lastDays);
    }
}

async function loadEvents(count = 0, days = 1) {
    lastCount = count;
    lastDays = days;
    const tbody = document.getElementById('events-tbody');
    tbody.innerHTML = '';
    try {
        // Recents entries come back newest-first without their global index, but
        // the feed always starts at the newest entry (len-1) and the days cutoff
        // only trims the old tail, so row i is entry index len-1-i. Fetch the total
        // length to map rows to indices for editing.
        let length = null;
        try {
            const lenResp = await fetch(`/api/length?key=${encodeURIComponent(window.ENTRY_KEY)}`);
            if (lenResp.ok) {
                const n = Number(await lenResp.json());
                if (Number.isFinite(n)) length = n;
            }
        } catch (_) {
            length = null;
        }

        const params = new URLSearchParams();
        // 0 means N/A - ignoring the limit in either count or days. Here we don't pass it in.
        // Note the backend logic actually replaces Nones with very large defaults (300 count, 30 days).
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
            const entryIdx = (length !== null) ? (length - 1 - i) : null;
            const row = buildRow(stateIdx, startMs, endMs, entryIdx);
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
            if (!Number.isNaN(r)) loadEvents(r, getDaysValue());
        }
    });

    daysSelect.addEventListener('change', () => {
        toggleCustom(daysSelect, daysCustom);
        if (daysSelect.value !== 'custom') {
            const d = parseInt(daysSelect.value, 10);
            if (!Number.isNaN(d)) loadEvents(getCountValue(), d);
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
