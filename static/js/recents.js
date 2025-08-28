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

async function loadEvents(count) {
    const tbody = document.getElementById('events-tbody');
    tbody.innerHTML = '';
    try {
        const resp = await fetch(`/api/recents?key=${window.ENTRY_KEY}&count=${encodeURIComponent(String(count))}`);
        if (!resp.ok) {
            const errText = await response.text();
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

function wireButtons() {
    const btn10 = document.getElementById('btn-10');
    const btn30 = document.getElementById('btn-30');
    const btnCustom = document.getElementById('btn-custom');
    const customInput = document.getElementById('custom-count');
    function setActiveButton(selected) {
        [btn10, btn30].forEach(b => b.classList.remove('ring-2', `ring-${window.BASE_COLOR}-300`));
        if (selected) selected.classList.add('ring-2', `ring-${window.BASE_COLOR}-300`);
    }
    btn10.addEventListener('click', () => {
        customInput.value = 10;
        setActiveButton(btn10);
        loadEvents(10);
    });
    btn30.addEventListener('click', () => {
        customInput.value = 30;
        setActiveButton(btn30);
        loadEvents(30);
    });
    btnCustom.addEventListener('click', () => {
        let n = parseInt(customInput.value, 10);
        if (Number.isNaN(n) || n < 1) {
            Swal.fire({ icon: 'error', title: 'Error', text: "Please enter a valid positive integer." });
            return;
        }
        setActiveButton(null);
        loadEvents(n);
    });
    setActiveButton(btn10);
    loadEvents(10);
}