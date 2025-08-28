function renderSegments(msArray, rangeLabel) {
    const total = msArray.reduce((a,b) => a + b, 0) || 1;
    const svg = document.getElementById('pie');
    svg.innerHTML = '';
    const legend = document.getElementById('legend');
    legend.innerHTML = '';

    let offset = 25;
    msArray.forEach((ms, idx) => {
        const percent = (ms / total) * 100;
        const color = STATES_DATA[idx][1];

        const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
        circle.setAttribute("r", "16");
        circle.setAttribute("cx", "16");
        circle.setAttribute("cy", "16");
        circle.setAttribute("stroke", color);
        circle.setAttribute("stroke-width", "32");
        circle.setAttribute("fill", "transparent");
        circle.setAttribute("stroke-dasharray", `${percent} ${100 - percent}`);
        circle.setAttribute("stroke-dashoffset", offset.toString());
        circle.style.transition = "stroke-dasharray 500ms, stroke-dashoffset 500ms";
        svg.appendChild(circle);

        const li = document.createElement('li');
        li.className = 'px-4 py-3 flex justify-between items-center';
        const leftSpan = document.createElement('div');
        leftSpan.className = 'flex items-center gap-3';
        const colorDot = document.createElement('span');
        colorDot.className = 'legend-color';
        colorDot.style.background = color;
        leftSpan.appendChild(colorDot);
        const labelText = document.createElement('span');
        labelText.className = 'text-lg md:text-sm';
        labelText.textContent = STATES_DATA[idx][0];
        leftSpan.appendChild(labelText);

        const rightSpan = document.createElement('span');
        rightSpan.className = 'text-base md:text-sm text-gray-600';
        rightSpan.textContent = msToReadable(ms);

        li.appendChild(leftSpan);
        li.appendChild(rightSpan);
        legend.appendChild(li);

        offset = offset - percent;
    });
    const labelEl = document.getElementById('range-label');
    if (labelEl) labelEl.textContent = rangeLabel;
}

async function loadRange(days) {
    try {
        const resp = await fetch(`/api/data?key=${window.ENTRY_KEY}&range=${encodeURIComponent(days)}`);
        if (!resp.ok) {
            const txt = await resp.text();
            alert("Error fetching data: " + txt);
            return;
        }
        const data = await resp.json();
        const arr = data;
        const rangeLabel = (days === 1) ? "Last 24 hours" : `Last ${days} days`;
        renderSegments(arr, rangeLabel);
    } catch (err) {
        alert("Network or unexpected error: " + (err && err.message ? err.message : err));
    }
}

document.querySelectorAll('.range-btn').forEach(btn => {
    btn.addEventListener('click', (ev) => {
        const days = parseInt(ev.currentTarget.getAttribute('data-range'), 10);
        loadRange(days);
    });
});