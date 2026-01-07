import type { Middleware } from 'solid-start';

/**
 * Content Security Policy middleware for XSS protection
 * Adds CSP headers to all responses
 */
export const cspMiddleware: Middleware = ({ forward }) => {
  return async (event) => {
    const response = await forward(event);

    // Define CSP policy for security
    const cspDirectives = [
      // Allow scripts only from same origin and inline (for SolidJS)
      "script-src 'self' 'unsafe-inline'",
      
      // Allow styles from same origin and inline (for TailwindCSS and dynamic styles)
      "style-src 'self' 'unsafe-inline'",
      
      // Allow images from same origin and data URIs (for avatars, etc.)
      "img-src 'self' data: https:",
      
      // Allow connections only to same origin and required APIs
      "connect-src 'self' ws: wss:",
      
      // Allow fonts from same origin
      "font-src 'self'",
      
      // Allow objects and media only from same origin
      "object-src 'none'",
      "media-src 'self'",
      
      // Prevent frame clickjacking
      "frame-src 'none'",
      "frame-ancestors 'none'",
      
      // Prevent mixed content
      "upgrade-insecure-requests",
      
      // Restrict form submissions
      "form-action 'self'",
      
      // Prevent base tag attacks
      "base-uri 'self'",
      
      // Prevent manifest attacks
      "manifest-src 'self'",
      
      // Worker restrictions
      "worker-src 'none'",
      
      // Report violations (in development)
      ...(process.env.NODE_ENV === 'development' 
        ? ["report-uri /csp-violation-report"] 
        : []
      )
    ];

    const cspHeaderValue = cspDirectives.join('; ');

    // Add CSP header
    response.headers.set('Content-Security-Policy', cspHeaderValue);

    // Add other security headers
    response.headers.set('X-Content-Type-Options', 'nosniff');
    response.headers.set('X-Frame-Options', 'DENY');
    response.headers.set('X-XSS-Protection', '1; mode=block');
    response.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
    response.headers.set('Permissions-Policy', 'geolocation=(), microphone=(), camera=()');

    return response;
  };
};