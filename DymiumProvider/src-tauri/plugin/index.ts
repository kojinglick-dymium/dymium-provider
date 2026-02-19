import fs from "fs"
import path from "path"
import os from "os"
import http from "http"
import https from "https"

// Path to the auth.json file
const AUTH_JSON_PATH = path.join(os.homedir(), ".local/share/opencode/auth.json")

// Log file for debugging (no console.log to avoid polluting OpenCode UI)
const LOG_FILE = path.join(os.homedir(), ".local/share/dymium-opencode-plugin/debug.log")

function log(message: string) {
  const timestamp = new Date().toISOString()
  const line = `${timestamp} ${message}\n`
  // Only write to file, not console (console output appears in OpenCode UI)
  try {
    fs.appendFileSync(LOG_FILE, line)
  } catch {}
}

interface DymiumAuth {
  key: string
  app?: string
}

/**
 * Read the current Dymium auth from auth.json
 * This is called on EVERY request to ensure we always have fresh credentials
 */
function getDymiumAuth(): DymiumAuth | null {
  try {
    if (!fs.existsSync(AUTH_JSON_PATH)) {
      log(`auth.json not found at ${AUTH_JSON_PATH}`)
      return null
    }
    
    const content = fs.readFileSync(AUTH_JSON_PATH, "utf-8")
    const auth = JSON.parse(content)
    
    if (auth.dymium?.key) {
      return {
        key: auth.dymium.key,
        app: auth.dymium.app || undefined
      }
    }
    
    log("No dymium.key found in auth.json")
    return null
  } catch (error) {
    log(`Failed to read auth.json: ${error}`)
    return null
  }
}

/**
 * Inject the app name into the URL path
 * Transforms: /v1/models -> /{app}/v1/models
 */
function injectAppIntoPath(pathname: string, app: string): string {
  // If path already starts with the app, don't double-inject
  if (pathname.startsWith(`/${app}/`)) {
    return pathname
  }
  // Insert app before /v1/ if present
  if (pathname.startsWith("/v1/") || pathname === "/v1") {
    return `/${app}${pathname}`
  }
  // For other paths, prepend the app
  return `/${app}${pathname}`
}

/**
 * Make an HTTP/1.1 request using Node's http module
 * This avoids HTTP/2 issues with kubectl port-forward
 * 
 * IMPORTANT: For Istio Gateway routing through port-forward:
 * - Use hostname without port in Host header (Istio VirtualService matches on hostname only)
 * - Use Connection: close to avoid keep-alive issues
 * - Ensure proper Content-Length for POST requests
 */
function http11Request(
  url: URL,
  options: {
    method: string
    headers: Record<string, string>
    body?: string
  }
): Promise<Response> {
  return new Promise((resolve, reject) => {
    const isHttps = url.protocol === "https:"
    const lib = isHttps ? https : http
    
    // For Istio, the Host header should be just the hostname (without port)
    // because VirtualService typically matches on hostname only
    const hostHeader = url.hostname
    
    const reqOptions: http.RequestOptions | https.RequestOptions = {
      hostname: url.hostname,
      port: url.port || (isHttps ? 443 : 80),
      path: url.pathname + url.search,
      method: options.method,
      headers: {
        ...options.headers,
        // Use hostname only for Host header (Istio best practice)
        "Host": hostHeader,
        // Prevent keep-alive issues with port-forward
        "Connection": "close",
      },
      // Accept self-signed certificates for development/port-forwarding scenarios
      rejectUnauthorized: false,
    }
    
    // Add Content-Length for requests with body
    if (options.body) {
      reqOptions.headers!["Content-Length"] = Buffer.byteLength(options.body).toString()
    }
    
    log(`HTTP/1.1 ${options.method} ${url.toString()} Host: ${hostHeader} (TLS: ${isHttps})`)
    
    const req = lib.request(reqOptions, (res) => {
      const chunks: Buffer[] = []
      
      res.on("data", (chunk) => chunks.push(chunk))
      res.on("end", () => {
        const body = Buffer.concat(chunks)
        const responseHeaders = new Headers()
        
        for (const [key, value] of Object.entries(res.headers)) {
          if (value) {
            if (Array.isArray(value)) {
              value.forEach(v => responseHeaders.append(key, v))
            } else {
              responseHeaders.set(key, value)
            }
          }
        }
        
        log(`Response: ${res.statusCode} ${res.statusMessage}`)
        
        // Create a Response object that supports streaming
        resolve(new Response(body, {
          status: res.statusCode || 200,
          statusText: res.statusMessage || "",
          headers: responseHeaders,
        }))
      })
    })
    
    req.on("error", (err) => {
      log(`Request error: ${err.message}`)
      reject(err)
    })
    
    // Set a reasonable timeout
    req.setTimeout(120000, () => {
      log("Request timeout")
      req.destroy(new Error("Request timeout"))
    })
    
    if (options.body) {
      req.write(options.body)
    }
    
    req.end()
  })
}

/**
 * Custom fetch function that injects the fresh Dymium token on every request
 * Uses HTTP/1.1 to avoid issues with kubectl port-forward
 * Also injects the app name into the URL path if configured
 */
async function dymiumFetch(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  const auth = getDymiumAuth()
  
  if (!auth) {
    throw new Error("[dymium-auth] No valid Dymium token available. Please ensure the Dymium Provider app is running.")
  }
  
  // Parse the URL
  const url = typeof input === "string" ? new URL(input) : input instanceof URL ? input : new URL(input.url)
  
  // Inject app name into URL path if configured
  if (auth.app) {
    const newPath = injectAppIntoPath(url.pathname, auth.app)
    if (newPath !== url.pathname) {
      log(`Rewriting path: ${url.pathname} -> ${newPath}`)
      url.pathname = newPath
    }
  }
  
  // Build headers object - start with defaults for OpenAI-compatible API
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "Accept": "application/json, text/event-stream",
  }
  
  // Copy any headers from init
  if (init?.headers) {
    const initHeaders = new Headers(init.headers)
    initHeaders.forEach((value, key) => {
      headers[key] = value
    })
  }
  
  // Set authorization (overwrite any existing)
  headers["Authorization"] = `Bearer ${auth.key}`
  
  // Get body as string if present
  let body: string | undefined
  if (init?.body) {
    if (typeof init.body === "string") {
      body = init.body
    } else if (init.body instanceof ArrayBuffer) {
      body = new TextDecoder().decode(init.body)
    } else if (ArrayBuffer.isView(init.body)) {
      body = new TextDecoder().decode(init.body)
    } else {
      // For other types, try to convert
      body = String(init.body)
    }
  }
  
  // Use HTTP/1.1 request to avoid HTTP/2 issues with port-forward
  return http11Request(url, {
    method: init?.method || "GET",
    headers,
    body,
  })
}

/**
 * OpenCode Plugin Export
 * 
 * This plugin provides authentication for the "dymium" provider.
 * It reads the token fresh from auth.json on every API call,
 * ensuring that token refreshes by the Dymium Provider app are
 * immediately picked up without needing to restart OpenCode.
 * 
 * Uses HTTP/1.1 explicitly to work with kubectl port-forward.
 * Sets Host header to hostname only (without port) for Istio compatibility.
 * Injects the app name into URL paths (e.g., /v1/models -> /{app}/v1/models)
 */
export default async function plugin({ client, project, directory }: any) {
  log(`Plugin initialized for project: ${project?.name || directory}`)
  
  return {
    auth: {
      // Match the provider name exactly
      provider: "dymium",
      
      // Empty methods array - we only use the loader
      methods: [],
      
      /**
       * Loader for the "dymium" provider
       * Called by OpenCode to get auth credentials and custom fetch
       */
      async loader(getAuth: () => Promise<any>, provider: any) {
        log(`Loader called for provider: ${provider?.id || provider}`)
        
        // Return auth info with empty apiKey and custom fetch
        // The custom fetch handles authentication via the token
        return {
          // Empty string - auth is handled in our custom fetch
          apiKey: "",
          
          // Custom fetch that reads token fresh and uses HTTP/1.1
          fetch: dymiumFetch,
        }
      },
    },
  }
}
