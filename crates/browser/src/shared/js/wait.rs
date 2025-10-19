pub const CHECK_LOADING: &str = r#"
() => ({
    readyState: document.readyState,
    loading: document.readyState !== 'complete',
    activeRequests: performance.getEntriesByType('resource').filter(r => !r.responseEnd).length
})
"#;

pub const WAIT_FOR_NETWORK_IDLE: &str = r#"
() => {
    return new Promise((resolve) => {
        let timeout;
        let count = 0;
        
        const check = () => {
            const active = performance.getEntriesByType('resource').filter(r => !r.responseEnd).length;
            if (active === 0) {
                count++;
                if (count >= 3) { // 3 consecutive checks with no activity
                    resolve(true);
                    return;
                }
            } else {
                count = 0;
            }
            timeout = setTimeout(check, 200);
        };
        
        check();
        
        // Failsafe timeout
        setTimeout(() => {
            clearTimeout(timeout);
            resolve(false);
        }, 5000);
    });
}
"#;