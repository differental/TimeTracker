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

function formatRounded(ms) {
    const d = new Date(Number(ms));
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

// Format a ms timestamp as a local `YYYY-MM-DDTHH:MM` string for a
// `datetime-local` input value. Rounds to the minute to match formatRounded's
// display granularity. Read back with `new Date(value).getTime()` (local).
function msToDatetimeLocal(ms) {
    const d = new Date(Number(ms));
    d.setSeconds(0, 0);
    const y = d.getFullYear();
    const mo = pad(d.getMonth() + 1);
    const day = pad(d.getDate());
    const h = pad(d.getHours());
    const mi = pad(d.getMinutes());
    return `${y}-${mo}-${day}T${h}:${mi}`;
}