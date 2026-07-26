function msToReadable(ms) {
    if (!ms || ms <= 500) return "0s";
    const totalMinutes = Math.floor(ms / 60000);
    if (totalMinutes == 0) return "<1m";
    const days = Math.floor(totalMinutes / 1440);
    const hours = Math.floor((totalMinutes % 1440) / 60);
    const minutes = totalMinutes % 60;
    const parts = [];
    if (days) parts.push(days + 'd');
    if (hours) parts.push(hours + 'h');
    if (minutes || parts.length === 0) parts.push(minutes + 'm');
    return parts.join(' ');
}

function isValidMs(ms) {
    const n = Number(ms);
    return Number.isFinite(n) && !Number.isNaN(new Date(n).getTime());
}

function formatRounded(ms) {
    const d = new Date(Number(ms));
    if (Number.isNaN(d.getTime())) return 'Error';
    if (d.getSeconds() >= 30) {
        d.setMinutes(d.getMinutes() + 1);
    }
    d.setSeconds(0, 0);
    const day = d.getDate();
    const month = d.toLocaleString(undefined, { month: 'short' });
    const hours = String(d.getHours()).padStart(2, '0');
    const minutes = String(d.getMinutes()).padStart(2, '0');
    return `${day} ${month}, ${hours}:${minutes}`;
}

const pad = n => n.toString().padStart(2,'0');

function clockHM(ms) {
    const d = new Date(Number(ms));
    if (Number.isNaN(d.getTime())) return '--:--';
    return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function buildSegments(pairs, fromMs, toMs) {
    if (!Array.isArray(pairs) || pairs.length === 0) return [];
    const asc = pairs.slice().reverse();
    const out = [];
    for (let i = 0; i < asc.length; i++) {
        const state = asc[i][0];
        const rawStart = Number(asc[i][1]);
        const rawEnd = (i + 1 < asc.length) ? Number(asc[i + 1][1]) : toMs;
        if (!isValidMs(rawStart) || !isValidMs(rawEnd)) continue;
        const s = Math.max(rawStart, fromMs);
        const e = Math.min(rawEnd, toMs);
        if (e > s) out.push({ state, start: s, end: e });
    }
    return out;
}

// Format a ms timestamp as a local `YYYY-MM-DDTHH:MM` string for a
// `datetime-local` input value. Rounds to the minute to match formatRounded's
// display granularity. Read back with `new Date(value).getTime()` (local).
function msToDatetimeLocal(ms) {
    let d = new Date(Number(ms));
    if (Number.isNaN(d.getTime())) d = new Date();
    d.setSeconds(0, 0);
    const y = d.getFullYear();
    const mo = pad(d.getMonth() + 1);
    const day = pad(d.getDate());
    const h = pad(d.getHours());
    const mi = pad(d.getMinutes());
    return `${y}-${mo}-${day}T${h}:${mi}`;
}