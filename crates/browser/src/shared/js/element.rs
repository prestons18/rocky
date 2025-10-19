pub const CHECK_ELEMENT_STATE: &str = r#"
(selector) => {
    const el = document.querySelector(selector);
    if (!el) return { exists: false };
    
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const isVisible = rect.width > 0 && rect.height > 0 && 
                     style.visibility !== 'hidden' && style.display !== 'none';
    
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
        inViewport: rect.top >= 0 && rect.left >= 0 && rect.bottom <= window.innerHeight && rect.right <= window.innerWidth,
        disabled: el.disabled || el.getAttribute('aria-disabled') === 'true',
        rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height },
        matchedSelector: selector,
        actualTag: el.tagName
    };
}
"#;

pub const SCROLL_INTO_VIEW: &str = r#"
(selector, block) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    el.scrollIntoView({ behavior: 'smooth', block: block || 'center' });
    return { success: true };
}
"#;

pub const SAFE_CLICK: &str = r#"
(selector) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    el.click();
    return { success: true };
}
"#;

pub const EXTRACT_TEXT: &str = r#"
(selector) => Array.from(document.querySelectorAll(selector)).map(e => e.textContent.trim())
"#;

pub const EXTRACT_ATTR: &str = r#"
(selector, attr) => Array.from(document.querySelectorAll(selector)).map(e => e.getAttribute(attr))
"#;

pub const EXTRACT_MULTIPLE: &str = r#"
(selector, attrs) => Array.from(document.querySelectorAll(selector)).map(e => {
    const result = {};
    attrs.forEach(attr => {
        result[attr] = attr === 'text' ? e.textContent.trim() : (e.getAttribute(attr) || '');
    });
    return result;
})
"#;

pub const TYPE_TEXT: &str = r#"
(selector, text, clear) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    if (clear) el.value = '';
    el.focus();
    el.value = clear ? text : el.value + text;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return { success: true };
}
"#;

pub const SUBMIT_FORM: &str = r#"
(selector) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    
    // Find the form
    const form = el.closest('form');
    if (form) {
        form.submit();
        return { success: true, method: 'form.submit()' };
    }
    
    // Try triggering Enter key event on the element
    const enterEvent = new KeyboardEvent('keydown', {
        key: 'Enter',
        code: 'Enter',
        keyCode: 13,
        which: 13,
        bubbles: true,
        cancelable: true
    });
    el.dispatchEvent(enterEvent);
    
    const enterUpEvent = new KeyboardEvent('keyup', {
        key: 'Enter',
        code: 'Enter',
        keyCode: 13,
        which: 13,
        bubbles: true,
        cancelable: true
    });
    el.dispatchEvent(enterUpEvent);
    
    return { success: true, method: 'keypress' };
}
"#;

pub const HOVER_ELEMENT: &str = r#"
(selector) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    const event = new MouseEvent('mouseover', { bubbles: true, cancelable: true });
    el.dispatchEvent(event);
    return { success: true };
}
"#;

pub const SELECT_OPTION: &str = r#"
(selector, value) => {
    const el = document.querySelector(selector);
    if (!el) return { success: false, error: 'Element not found' };
    el.value = value;
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return { success: true };
}
"#;

pub const SET_COOKIE: &str = r#"
(name, value, domain) => {
    document.cookie = name + '=' + value + (domain ? '; domain=' + domain : '') + '; path=/';
    return { success: true };
}
"#;