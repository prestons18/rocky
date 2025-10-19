pub const CHECK_ELEMENT_STATE: &str = r#"
(selector) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { exists: false };
        
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        const isVisible = rect.width > 0 && rect.height > 0 && 
                         style.visibility !== 'hidden' && 
                         style.display !== 'none' &&
                         style.opacity !== '0';
        
        if (!isVisible) return { exists: true, visible: false };
        
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        const topEl = document.elementFromPoint(centerX, centerY);
        const isObscured = topEl && !el.contains(topEl) && topEl !== el;
        
        return {
            exists: true,
            visible: isVisible,
            obscured: isObscured,
            obscuredBy: isObscured ? (topEl.tagName + (topEl.className ? '.' + topEl.className.split(' ').join('.') : '')) : null,
            inViewport: rect.top >= 0 && rect.left >= 0 && 
                       rect.bottom <= window.innerHeight && 
                       rect.right <= window.innerWidth,
            disabled: el.disabled || el.getAttribute('aria-disabled') === 'true',
            rect: { 
                top: rect.top, 
                left: rect.left, 
                width: rect.width, 
                height: rect.height 
            },
            matchedSelector: selector,
            actualTag: el.tagName.toLowerCase()
        };
    } catch (error) {
        return { exists: false, error: error.message };
    }
}
"#;

pub const SCROLL_INTO_VIEW: &str = r#"
(selector, block = 'center') => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        el.scrollIntoView({ 
            behavior: 'smooth', 
            block: block,
            inline: 'nearest'
        });
        
        return { success: true };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const SAFE_CLICK: &str = r#"
(selector) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        // Check if element is interactable
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        const isInteractable = rect.width > 0 && rect.height > 0 && 
                               style.visibility !== 'hidden' && 
                               style.display !== 'none' &&
                               style.pointerEvents !== 'none';
        
        if (!isInteractable) {
            return { success: false, error: 'Element not interactable' };
        }
        
        el.click();
        return { success: true };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const EXTRACT_TEXT: &str = r#"
(selector) => {
    try {
        return Array.from(document.querySelectorAll(selector))
            .map(e => e.textContent?.trim() || '');
    } catch (error) {
        return [];
    }
}
"#;

pub const EXTRACT_ATTR: &str = r#"
(selector, attr) => {
    try {
        return Array.from(document.querySelectorAll(selector))
            .map(e => e.getAttribute(attr) || '');
    } catch (error) {
        return [];
    }
}
"#;

pub const EXTRACT_MULTIPLE: &str = r#"
(selector, attrs) => {
    try {
        return Array.from(document.querySelectorAll(selector)).map(e => {
            const result = {};
            attrs.forEach(attr => {
                if (attr === 'text') {
                    result[attr] = e.textContent?.trim() || '';
                } else if (attr === 'html') {
                    result[attr] = e.innerHTML || '';
                } else {
                    result[attr] = e.getAttribute(attr) || '';
                }
            });
            return result;
        });
    } catch (error) {
        return [];
    }
}
"#;

pub const TYPE_TEXT: &str = r#"
(selector, text, clear = false) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        el.focus();
        
        if (clear) {
            el.value = '';
        }
        
        // Support both input and contenteditable elements
        if (el.isContentEditable) {
            if (clear) {
                el.textContent = text;
            } else {
                el.textContent += text;
            }
        } else {
            el.value = clear ? text : (el.value || '') + text;
        }
        
        // Dispatch events in proper order
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        el.dispatchEvent(new Event('blur', { bubbles: true }));
        
        return { success: true };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const SUBMIT_FORM: &str = r#"
(selector) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        // Find the form
        const form = el.closest('form');
        if (form) {
            // Trigger submit event first (allows preventDefault)
            const submitEvent = new Event('submit', { 
                bubbles: true, 
                cancelable: true 
            });
            form.dispatchEvent(submitEvent);
            
            // If not prevented, submit the form
            if (!submitEvent.defaultPrevented) {
                form.submit();
            }
            return { success: true, method: 'form.submit()' };
        }
        
        // Try clicking if it's a button
        if (el.tagName === 'BUTTON' || el.type === 'submit') {
            el.click();
            return { success: true, method: 'button.click()' };
        }
        
        // Fallback: trigger Enter key event
        ['keydown', 'keypress', 'keyup'].forEach(eventType => {
            const event = new KeyboardEvent(eventType, {
                key: 'Enter',
                code: 'Enter',
                keyCode: 13,
                which: 13,
                bubbles: true,
                cancelable: true
            });
            el.dispatchEvent(event);
        });
        
        return { success: true, method: 'keypress' };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const HOVER_ELEMENT: &str = r#"
(selector) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        // Dispatch all relevant mouse events
        ['mouseenter', 'mouseover', 'mousemove'].forEach(eventType => {
            const event = new MouseEvent(eventType, { 
                bubbles: true, 
                cancelable: true,
                view: window
            });
            el.dispatchEvent(event);
        });
        
        return { success: true };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const SELECT_OPTION: &str = r#"
(selector, value) => {
    try {
        const el = document.querySelector(selector);
        if (!el) return { success: false, error: 'Element not found' };
        
        if (el.tagName !== 'SELECT') {
            return { success: false, error: 'Element is not a select element' };
        }
        
        // Try to find and select the option
        const option = Array.from(el.options).find(opt => 
            opt.value === value || opt.text === value
        );
        
        if (!option) {
            return { 
                success: false, 
                error: 'Option not found',
                availableOptions: Array.from(el.options).map(opt => ({
                    value: opt.value,
                    text: opt.text
                }))
            };
        }
        
        el.value = option.value;
        el.dispatchEvent(new Event('change', { bubbles: true }));
        el.dispatchEvent(new Event('input', { bubbles: true }));
        
        return { success: true, selectedValue: option.value, selectedText: option.text };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const SET_COOKIE: &str = r#"
(name, value, options = {}) => {
    try {
        let cookieString = `${encodeURIComponent(name)}=${encodeURIComponent(value)}`;
        
        if (options.domain) cookieString += `; domain=${options.domain}`;
        if (options.path) cookieString += `; path=${options.path}`;
        else cookieString += '; path=/';
        
        if (options.maxAge) cookieString += `; max-age=${options.maxAge}`;
        if (options.expires) cookieString += `; expires=${options.expires}`;
        if (options.secure) cookieString += '; secure';
        if (options.sameSite) cookieString += `; samesite=${options.sameSite}`;
        
        document.cookie = cookieString;
        
        return { success: true, cookie: cookieString };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
"#;

pub const DETECT_CAPTCHA: &str = r#"
() => {
    try {
        const indicators = {
            recaptcha: {
                found: false,
                selectors: [
                    'iframe[src*="recaptcha"]', 
                    '.g-recaptcha', 
                    '#recaptcha', 
                    '[class*="recaptcha"]',
                    '[id*="recaptcha"]'
                ]
            },
            hcaptcha: {
                found: false,
                selectors: [
                    'iframe[src*="hcaptcha"]', 
                    '.h-captcha', 
                    '#hcaptcha', 
                    '[class*="hcaptcha"]',
                    '[id*="hcaptcha"]'
                ]
            },
            cloudflare: {
                found: false,
                selectors: [
                    '#challenge-form', 
                    '.cf-challenge', 
                    '[class*="cf-turnstile"]', 
                    'iframe[src*="challenges.cloudflare"]',
                    '[id*="cf-challenge"]'
                ]
            },
            generic: {
                found: false,
                selectors: [
                    '[id*="captcha"]:not([id*="recaptcha"]):not([id*="hcaptcha"])', 
                    '[class*="captcha"]:not([class*="recaptcha"]):not([class*="hcaptcha"])', 
                    '[name*="captcha"]'
                ]
            }
        };
        
        const detected = [];
        
        // Check each CAPTCHA type
        for (const [type, config] of Object.entries(indicators)) {
            for (const selector of config.selectors) {
                const elements = document.querySelectorAll(selector);
                for (const el of elements) {
                    const rect = el.getBoundingClientRect();
                    const style = window.getComputedStyle(el);
                    const isVisible = rect.width > 0 && rect.height > 0 && 
                                     style.visibility !== 'hidden' && 
                                     style.display !== 'none' &&
                                     style.opacity !== '0';
                    
                    if (isVisible) {
                        detected.push({
                            type: type,
                            selector: selector,
                            element: el.tagName.toLowerCase(),
                            id: el.id,
                            className: el.className
                        });
                        config.found = true;
                        break;
                    }
                }
                if (config.found) break;
            }
        }
        
        // Text analysis
        const getPageText = () => {
            const body = document.body?.innerText?.toLowerCase() || '';
            const html = document.documentElement?.innerText?.toLowerCase() || '';
            return body + ' ' + html;
        };
        
        const fullText = getPageText();
        
        const captchaKeywords = [
            'verify you are human',
            'complete the captcha',
            'prove you are not a robot',
            "i'm not a robot",
            'im not a robot',
            'unusual traffic',
            'automated requests',
            'our systems have detected unusual traffic',
            'please verify you are a human',
            'suspicious activity',
            'verify that you are not a robot',
            'security check',
            'are you a robot'
        ];
        
        const cookieConsentPhrases = [
            'accept cookies',
            'cookie policy',
            'privacy policy',
            'we use cookies',
            'manage cookies'
        ];
        
        const foundKeywords = captchaKeywords.filter(keyword => fullText.includes(keyword));
        const hasCookieConsent = cookieConsentPhrases.some(phrase => fullText.includes(phrase));
        const isCaptchaText = foundKeywords.length > 0;
        
        // Title and URL checks
        const title = document.title.toLowerCase();
        const titleIndicators = ['captcha', 'security check', 'verify', 'unusual traffic', 'attention required'];
        const titleMatch = titleIndicators.some(indicator => title.includes(indicator));
        
        const url = window.location.href.toLowerCase();
        const pathname = window.location.pathname.toLowerCase();
        const urlIndicators = ['captcha', '/sorry', 'ipv6_or_unusual_traffic', 'challenge', '/cdn-cgi/challenge'];
        const urlMatch = urlIndicators.some(indicator => url.includes(indicator) || pathname.includes(indicator));
        
        const isDetected = detected.length > 0 || isCaptchaText || titleMatch || urlMatch;
        
        return {
            detected: isDetected,
            confidence: isDetected ? (detected.length > 0 ? 'high' : 'medium') : 'none',
            types: [...new Set(detected.map(d => d.type))],
            details: detected,
            keywords: foundKeywords,
            titleMatch: titleMatch,
            urlMatch: urlMatch,
            hasCookieConsent: hasCookieConsent,
            pageTitle: document.title,
            url: window.location.href,
            bodyTextSample: fullText.substring(0, 300)
        };
    } catch (error) {
        return { 
            detected: false, 
            error: error.message 
        };
    }
}
"#;