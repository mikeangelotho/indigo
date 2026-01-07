import { createEffect, onMount } from 'solid-js';

/**
 * CSP utility functions for frontend security
 */

/**
 * Inject Content Security Policy meta tags
 * This provides defense-in-depth against XSS attacks
 */
export function injectCSP() {
  if (typeof document !== 'undefined') {
    // Check if CSP meta tag already exists
    const existingCSP = document.querySelector('meta[http-equiv="Content-Security-Policy"]');
    if (existingCSP) return;

    // Get current origin for flexible CSP
    const currentOrigin = window.location.origin;
    const isDevelopment = process.env.NODE_ENV === 'development';
    
    const cspMeta = document.createElement('meta');
    cspMeta.httpEquiv = 'Content-Security-Policy';
    
    // Build CSP directives
    const directives = [
      "script-src 'self' 'unsafe-inline'", // Required for SolidJS
      "style-src 'self' 'unsafe-inline'", // Required for TailwindCSS
      "img-src 'self' data: https:",
      "font-src 'self'",
      "object-src 'none'",
      "media-src 'self'",
      "frame-src 'none'",
      "form-action 'self'",
      "base-uri 'self'",
      "manifest-src 'self'",
      "worker-src 'none'",
    ];

    // Connect-src directive - allow current origin and backend
    const connectSources = [
      "'self'",
      "ws:",
      "wss:",
      currentOrigin,
    ];
    
    // Add backend API URLs
    const backendUrl = getBackendUrl();
    connectSources.push(backendUrl);
    
    // Also add localhost variants for development
    if (isDevelopment) {
      connectSources.push(
        "http://localhost:3001", 
        "http://127.0.0.1:3001",
        "http://localhost:3002", // gRPC port
        "http://127.0.0.1:3002"
      );
    }
    
    directives.push(`connect-src ${connectSources.join(' ')}`);

    cspMeta.content = directives.join('; ');

    document.head.appendChild(cspMeta);
  }
}

/**
 * Add security headers via meta tags (for client-side defense)
 */
export function addSecurityHeaders() {
  if (typeof document !== 'undefined') {
    const securityHeaders = [
      { 'http-equiv': 'X-Content-Type-Options', content: 'nosniff' },
      { 'http-equiv': 'X-Frame-Options', content: 'DENY' },
      { 'http-equiv': 'X-XSS-Protection', content: '1; mode=block' },
      { 'http-equiv': 'Referrer-Policy', content: 'strict-origin-when-cross-origin' },
    ];

    securityHeaders.forEach(header => {
      // Check if header already exists
      const existing = document.querySelector(`meta[http-equiv="${header['http-equiv']}"]`);
      if (existing) return;

      const meta = document.createElement('meta');
      meta.httpEquiv = header.httpEquiv;
      meta.content = header.content;
      document.head.appendChild(meta);
    });
  }
}

/**
 * CSP Hook for SolidJS components
 * Usage: <CSPGuard />
 */
export function CSPGuard() {
  onMount(() => {
    injectCSP();
    addSecurityHeaders();
  });

  return null;
}

/**
 * Validate URLs to prevent XSS via links
 */
export function validateURL(url: string): boolean {
  if (!url || typeof url !== 'string') return false;

  try {
    // Allow relative URLs
    if (url.startsWith('/') || url.startsWith('./') || url.startsWith('../')) {
      return true;
    }

    const parsed = new URL(url, window.location.origin);
    
    // Allow only http and https protocols
    return ['http:', 'https:'].includes(parsed.protocol);
  } catch {
    return false;
  }
}

/**
 * Sanitize file names to prevent XSS
 */
export function sanitizeFileName(fileName: string): string {
  if (!fileName || typeof fileName !== 'string') return '';

  return fileName
    .replace(/[<>:"/\\|?*]/g, '') // Remove dangerous characters
    .replace(/\.\./g, '') // Remove directory traversal
    .replace(/[\x00-\x1f\x7f]/g, '') // Remove control characters
    .substring(0, 255); // Limit length
}

/**
 * CSP violation reporter
 */
export function setupCSPReporting() {
  if (typeof window !== 'undefined' && process.env.NODE_ENV === 'development') {
    window.addEventListener('securitypolicyviolation', (event) => {
      console.error('CSP Violation:', {
        violatedDirective: event.violatedDirective,
        blockedURI: event.blockedURI,
        sourceFile: event.sourceFile,
        lineNumber: event.lineNumber,
        columnNumber: event.columnNumber,
      });
    });
  }
}

/**
 * Get backend URL for CSP configuration
 */
export function getBackendUrl(): string {
  // Try to detect backend URL from environment or default to localhost
  if (typeof window !== 'undefined') {
    const hostname = window.location.hostname;
    const protocol = window.location.protocol;
    
    // Hub now runs on port 3001
    if (hostname === 'localhost' || hostname === '127.0.0.1') {
      return `${protocol}//${hostname}:3001`;
    }
    
    // For other origins, try to construct backend URL
    return `${protocol}//${hostname}:3001`;
  }
  
  return 'http://localhost:3001';
}