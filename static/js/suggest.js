const SUGGEST_LIMIT = 3;

function suggestPanel() {
    return document.getElementById('all-activities-panel');
}

function suggestToggle() {
    return document.getElementById('all-activities-btn');
}

function setAllActivitiesOpen(open) {
    const panel = suggestPanel();
    const btn = suggestToggle();
    if (panel) panel.classList.toggle('hidden', !open);
    if (btn) btn.setAttribute('aria-expanded', String(open));
}

function stateButton(stateIdx) {
    return document.querySelector(`.change-state-btn[value="${stateIdx}"]`);
}

function showAllActivities() {
    const list = document.getElementById('suggest-list');
    const btn = suggestToggle();
    if (list) list.remove();
    if (btn) btn.remove();
    setAllActivitiesOpen(true);
}

function returnPromotedButtons() {
    const list = document.getElementById('suggest-list');
    const panel = suggestPanel();
    if (!list || !panel) return;

    list.querySelectorAll('.change-state-btn').forEach(btn => {
        btn.style.removeProperty('--delay');
        panel.appendChild(btn);
    });

    Array.from(panel.children)
        .sort((a, b) => Number(a.value) - Number(b.value))
        .forEach(btn => panel.appendChild(btn));
}

async function loadSuggestions() {
    const list = document.getElementById('suggest-list');
    if (!list || !suggestPanel()) return;

    const tzOffset = -new Date().getTimezoneOffset();

    let suggestions = [];
    try {
        const resp = await fetch(
            `/api/suggest?key=${window.ENTRY_KEY}&limit=${SUGGEST_LIMIT}&tz_offset=${tzOffset}`
        );
        if (!resp.ok) {
            showAllActivities();
            return;
        }
        const data = await resp.json();
        suggestions = Array.isArray(data.suggestions) ? data.suggestions : [];
    } catch (err) {
        showAllActivities();
        return;
    }

    returnPromotedButtons();

    const promoted = suggestions
        .map(s => stateButton(s.state))
        .filter(btn => btn !== null);

    if (promoted.length === 0) {
        showAllActivities();
        return;
    }

    list.innerHTML = '';
    list.removeAttribute('aria-busy');

    promoted.forEach((btn, i) => {
        btn.style.setProperty('--delay', `${i * 45}ms`);
        list.appendChild(btn);
    });
}

function initSuggest() {
    const allActivitiesBtn = suggestToggle();
    if (allActivitiesBtn) {
        allActivitiesBtn.addEventListener('click', () => {
            setAllActivitiesOpen(allActivitiesBtn.getAttribute('aria-expanded') !== 'true');
        });
    }

    loadSuggestions();
}
