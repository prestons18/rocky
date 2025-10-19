pub const COOKIE_PATTERNS: &[&str] = &[
    "Accept all", "Accept All", "Accept cookies", "Accept Cookies",
    "I agree", "I Agree", "Agree", "Accept", "Got it", "OK",
    "Allow all", "Allow All", "Consent", "Continue", "I accept"
];

pub const FIND_AND_CLICK_COOKIE: &str = r#"
(patterns) => {
    const selectors = [
        'button', 'a[role="button"]', 'div[role="button"]',
        '[class*="cookie"]', '[id*="cookie"]',
        '[class*="consent"]', '[id*="consent"]'
    ];
    const btns = Array.from(document.querySelectorAll(selectors.join(', ')));
    
    for (const btn of btns) {
        const text = btn.textContent.trim();
        for (const pattern of patterns) {
            if (text.toLowerCase().includes(pattern.toLowerCase())) {
                btn.click();
                return { clicked: true, text };
            }
        }
    }
    return { clicked: false };
}
"#;