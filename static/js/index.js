function updateElapsed() {
    const d = Date.now() - start;
    const h = Math.floor(d / 3600000);
    const m = Math.floor((d % 3600000) / 60000);
    const s = Math.floor((d % 60000) / 1000);
    document.getElementById('elapsed').textContent = `${pad(h)}:${pad(m)}:${pad(s)}`;
}

document.querySelectorAll('.change-state-btn').forEach(btn => {
    btn.addEventListener('click', async (ev) => {
        const newState = parseInt(ev.currentTarget.value, 10);
        const result = await Swal.fire({
            icon: 'warning',
            title: 'Confirmation',
            text: `Change state to ${STATES_NAMES[newState]}?`,
            showCancelButton: true,
            confirmButtonText: 'Yes',
            cancelButtonText: 'Cancel'
        });
        if (!result.isConfirmed) return;

        try {
            Swal.fire({
                allowOutsideClick: false,
                didOpen: () => {
                    Swal.showLoading();
                }
            });
            const payload = { new_state: newState, start_timestamp: Date.now() };
            const response = await fetch(`/api/entry?key=${window.ENTRY_KEY}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload)
            });
            Swal.close();

            if (!response.ok) {
                const errText = await response.text();
                await Swal.fire({ icon: 'error', title: 'Error', text: errText });
                return;
            }

            await response.json();

            Swal.fire({
                icon: 'success',
                title: 'Update Successful',
                text: 'State saved. Reloading...',
                showConfirmButton: false,
                timer: 2000,
                timerProgressBar: true,
                willClose: () => location.reload()
            });
        } catch (err) {
            Swal.close();
            const msg = (err && err.message) ? err.message : String(err);
            await Swal.fire({ icon: 'error', title: 'Error', text: msg });
        }
    });
});