import DOMPurify from 'dompurify';

/**
 * Secure HTML rendering utilities for Indigo Frontend
 * Prevents XSS attacks by sanitizing all HTML content
 */

// Configure DOMPurify for security
const domPurifyConfig = {
  ALLOWED_TAGS: [
    // Text formatting
    'p', 'br', 'strong', 'b', 'em', 'i', 'u', 's', 'sub', 'sup', 'mark', 'small', 'del', 'ins',
    // Headings
    'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
    // Lists
    'ul', 'ol', 'li', 'dl', 'dt', 'dd',
    // Tables
    'table', 'thead', 'tbody', 'tfoot', 'tr', 'th', 'td', 'caption', 'col', 'colgroup',
    // Code blocks
    'pre', 'code', 'blockquote',
    // Links and media
    'a', 'img',
    // Structure
    'div', 'span', 'hr',
  ],
  ALLOWED_ATTR: [
    // Standard attributes
    'class', 'id', 'title', 'alt', 'lang', 'dir',
    // Links
    'href', 'target', 'rel',
    // Images
    'src', 'width', 'height',
    // Code
    'data-language',
  ],
  ALLOW_DATA_ATTR: false, // Disallow data attributes for security
  FORBID_ATTR: [
    'onclick', 'onload', 'onerror', 'onmouseover', 'onmouseout', 'onfocus', 'onblur',
    'onkeydown', 'onkeyup', 'onsubmit', 'onchange', 'oninput', 'onselect'
  ],
  FORBID_TAGS: [
    'script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'textarea', 'button',
    'select', 'option', 'link', 'meta', 'base', 'head', 'html', 'body'
  ],
  ADD_ATTR: ['target'], // Allow target="_blank" for links
  SANITIZE_DOM: true,
  SANITIZE_NAMED_PROPS: true,
  WHOLE_DOCUMENT: false,
  RETURN_DOM: false,
  RETURN_DOM_FRAGMENT: false,
  RETURN_DOM_IMPORT: false,
  SAFE_FOR_TEMPLATES: true,
  KEEP_CONTENT: true,
};

/**
 * Securely sanitize HTML content
 * @param html - Raw HTML string to sanitize
 * @returns Sanitized HTML string safe to render
 */
export function sanitizeHtml(html: string): string {
  if (!html || typeof html !== 'string') {
    return '';
  }

  try {
    return DOMPurify.sanitize(html, domPurifyConfig);
  } catch (error) {
    console.error('Error sanitizing HTML:', error);
    // Fallback to empty string if sanitization fails
    return '';
  }
}

/**
 * Securely sanitize and render markdown content
 * @param markdown - Raw markdown string
 * @returns Sanitized HTML string safe to render
 */
export function sanitizeMarkdown(markdown: string): string {
  if (!markdown || typeof markdown !== 'string') {
    return '';
  }

  try {
    // Import marked dynamically to avoid SSR issues
    const { marked } = require('marked');
    const rawHtml = marked(markdown);
    return sanitizeHtml(rawHtml);
  } catch (error) {
    console.error('Error processing markdown:', error);
    return sanitizeHtml(markdown); // Fallback to basic HTML sanitization
  }
}

/**
 * Validate URL to prevent malicious links
 * @param url - URL to validate
 * @returns True if URL is safe
 */
export function isValidUrl(url: string): boolean {
  if (!url || typeof url !== 'string') {
    return false;
  }

  try {
    const parsed = new URL(url, window.location.origin);
    
    // Allow only http, https, and relative protocols
    const allowedProtocols = ['http:', 'https:'];
    if (!allowedProtocols.includes(parsed.protocol)) {
      return false;
    }

    // Prevent javascript: and data: URLs
    if (url.toLowerCase().startsWith('javascript:') || 
        url.toLowerCase().startsWith('data:') ||
        url.toLowerCase().startsWith('vbscript:')) {
      return false;
    }

    return true;
  } catch {
    return false;
  }
}

/**
 * Securely sanitize SVG content
 * @param svg - SVG string content
 * @returns Sanitized SVG string safe to render
 */
export function sanitizeSvg(svg: string): string {
  if (!svg || typeof svg !== 'string') {
    return '';
  }

  try {
    // SVG-specific sanitization
    return DOMPurify.sanitize(svg, {
      ...domPurifyConfig,
      ALLOWED_TAGS: [
        ...domPurifyConfig.ALLOWED_TAGS,
        // SVG specific elements
        'svg', 'g', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon',
        'text', 'tspan', 'use', 'defs', 'clipPath', 'mask', 'pattern', 'gradient',
        'stop', 'linearGradient', 'radialGradient', 'filter', 'feGaussianBlur'
      ],
      ALLOWED_ATTR: [
        ...domPurifyConfig.ALLOWED_ATTR,
        // SVG specific attributes
        'viewBox', 'width', 'height', 'fill', 'stroke', 'stroke-width', 'd', 'x', 'y',
        'cx', 'cy', 'r', 'rx', 'ry', 'x1', 'y1', 'x2', 'y2', 'points', 'transform'
      ],
      FORBID_TAGS: [
        'script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'textarea', 'button',
        'select', 'option', 'link', 'meta', 'base', 'head', 'html', 'body',
        // SVG dangerous elements
        'animate', 'animateMotion', 'animateTransform', 'set', 'discard'
      ]
    });
  } catch (error) {
    console.error('Error sanitizing SVG:', error);
    return '';
  }
}

/**
 * Create a secure copy icon SVG string
 * @returns Sanitized SVG string
 */
export const SECURE_COPY_ICON = sanitizeSvg(
  `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`
);

/**
 * Create a secure check icon SVG string
 * @returns Sanitized SVG string
 */
export const SECURE_CHECK_ICON = sanitizeSvg(
  `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>`
);

/**
 * Secure HTML component props for SolidJS
 */
export interface SecureHtmlProps {
  content: string;
  class?: string;
  onClick?: (event: MouseEvent) => void;
}

/**
 * Create a secure HTML rendering function for SolidJS
 * @param content - Content to render
 * @param options - Rendering options
 * @returns Sanitized content safe for innerHTML
 */
export function createSecureHtml(content: string, options: { 
  allowMarkdown?: boolean;
  maxContentLength?: number;
} = {}): string {
  const { allowMarkdown = false, maxContentLength = 100000 } = options;

  // Content length validation
  if (content.length > maxContentLength) {
    console.warn('Content length exceeds maximum, truncating');
    content = content.substring(0, maxContentLength) + '...';
  }

  // Sanitize based on content type
  if (allowMarkdown) {
    return sanitizeMarkdown(content);
  } else {
    return sanitizeHtml(content);
  }
}