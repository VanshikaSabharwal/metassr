//! MetaSSR API Handler - Polyglot API routes support.
//!
//! This crate provides API route handling for MetaSSR, allowing developers to
//! define backend API endpoints in `./src/api/` using JavaScript files.
//!
//! # Example
//!
//! Create a file at `./src/api/hello.js`:
//!
//! ```javascript
//! function GET(req) {
//!     return JSON.stringify({
//!         status: 200,
//!         body: { message: "Hello from API!" }
//!     });
//! }
//!
//! function POST(req) {
//!     const data = JSON.parse(req.body || "{}");
//!     return JSON.stringify({
//!         status: 201,
//!         body: { received: data }
//!     });
//! }
//!
//! module.exports = { GET, POST };
//! ```

pub mod scanner;
pub mod types;

use anyhow::{anyhow, Result};
use axum::{
    extract::Query,
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::{get, MethodRouter},
    Router,
};
use metacall::{load, metacall};
use scanner::{scan_api_dir, ApiRouteFile};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{debug, error, info, warn};
use types::{ApiRequest, ApiResponse};

/// Stores loaded API route scripts.
///
/// NOTE: Currently uses the shared global MetaCall context.
/// This is a testing behavior that might change in the future to use
/// a dedicated MetaCall runtime thread for better isolation.
pub struct ApiRoutes {
    /// Set of loaded script paths (to avoid reloading).
    loaded_scripts: HashMap<String, PathBuf>,
    /// List of discovered route files.
    routes: Vec<ApiRouteFile>,
    /// Route handler mapping: "hello.GET" -> "/path/to/hello.js"
    route_handlers: HashMap<String, String>,
}

impl ApiRoutes {
    /// Create a new empty ApiRoutes instance.
    pub fn new() -> Self {
        Self {
            loaded_scripts: HashMap::new(),
            routes: Vec::new(),
            route_handlers: HashMap::new(),
        }
    }

    /// Scan the given API directory and load all JavaScript files.
    ///
    /// # Arguments
    /// * `api_dir` - Path to the API directory (typically `./src/api/`)
    pub fn load_from_dir(&mut self, api_dir: &Path) -> Result<()> {
        let route_files = scan_api_dir(api_dir);

        if route_files.is_empty() {
            debug!("No API routes found in {:?}", api_dir);
            return Ok(());
        }

        info!("Found {} API route(s)", route_files.len());

        for route_file in &route_files {
            if let Err(e) = self.load_script(&route_file.file_path) {
                warn!("Failed to load API route {:?}: {}", route_file.file_path, e);
            } else {
                info!(
                    "  Loaded: {} -> {:?}",
                    route_file.route_path, route_file.file_path
                );
            }
        }

        self.routes = route_files;
        Ok(())
    }

    fn detect_runtime(path: &Path) -> Option<load::Tag> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("js") => Some(load::Tag::NodeJS),
            Some("ts") => Some(load::Tag::NodeJS),
            Some("py") => Some(load::Tag::Python),
            Some("rb") => Some(load::Tag::Ruby),
            _ => None,
        }
    }

    /// Load a single script file into MetaCall.
    fn load_script(&mut self, file_path: &Path) -> Result<()> {
        let path_str = file_path.to_string_lossy().to_string();
        let runtime = Self::detect_runtime(file_path)
            .ok_or_else(|| anyhow!("Unsupported API language {:?}", file_path))?;

        // from_file expects an iterator of paths — wrap in a slice
        load::from_file(runtime, &[file_path], None)
            .map_err(|e| anyhow!("Failed to load script {:?}: {:?}", file_path, e))?;

        let file_stem = file_path.file_stem().unwrap().to_string_lossy().to_string();

        // Register GET and POST optimistically — metacall() will return an error at
        // call time if the function isn't actually exported from the script.
        for method in ["GET", "POST"] {
            let qualified = format!("{}.{}", file_stem, method);
            self.route_handlers.insert(qualified.clone(), path_str.clone());
            info!("Registered handler: {}", qualified);
        }

        self.loaded_scripts.insert(path_str, file_path.to_path_buf());
        Ok(())
    }

    /// Call a handler function (GET, POST) on a loaded script.
    ///
    /// MetaCall's NodeJS runtime exposes `module.exports` functions by their
    /// bare name ("GET"), not the qualified name ("hello.GET"). So we look up
    /// the handler in our map using the qualified key for namespacing, but
    /// invoke MetaCall using only the bare method name.
    pub fn call_handler(
        &self,
        file_path: &str,
        method: &str,
        request: ApiRequest,
    ) -> Result<ApiResponse> {
        let request_json = serde_json::to_string(&request)?;

        let file_name = Path::new(file_path).file_stem().unwrap().to_string_lossy();
        let function_name = format!("{}.{}", file_name, method);

        if !self.route_handlers.contains_key(&function_name) {
            return Err(anyhow!("Handler {} not registered", function_name));
        }

        debug!("Calling {} (as bare '{}') with request: {}", function_name, method, request_json);

        // Call using bare method name — that's what MetaCall NodeJS runtime exposes
        let result: String = metacall(method, [request_json])
            .map_err(|e| anyhow!("Failed to call {}: {:?}", function_name, e))?;

        let response: ApiResponse = serde_json::from_str(&result)
            .map_err(|e| anyhow!("Failed to parse response: {} (raw: {})", e, result))?;

        Ok(response)
    }

    /// Get the list of discovered routes.
    pub fn routes(&self) -> &[ApiRouteFile] {
        &self.routes
    }
}

impl Default for ApiRoutes {
    fn default() -> Self {
        Self::new()
    }
}

/// Register API routes on an Axum router.
///
/// # Arguments
/// * `router` - The Axum router to add routes to
/// * `root_path` - Root path of the project (to find `./src/api/`)
pub fn register_api_routes(
    mut router: Router,
    root_path: &Path,
) -> Result<(Router, Option<Arc<std::sync::Mutex<ApiRoutes>>>)> {
    let api_dir = root_path.join("src").join("api");

    if !api_dir.exists() {
        debug!(
            "No API directory found at {:?}, skipping API routes",
            api_dir
        );
        return Ok((router, None));
    }

    let mut api_routes = ApiRoutes::new();
    api_routes.load_from_dir(&api_dir)?;

    if api_routes.routes().is_empty() {
        return Ok((router, None));
    }

    let api_routes = Arc::new(std::sync::Mutex::new(api_routes));

    // Clone routes info before moving api_routes
    let routes_info: Vec<_> = api_routes
        .lock()
        .unwrap()
        .routes()
        .iter()
        .map(|r| {
            (
                r.route_path.clone(),
                r.file_path.to_string_lossy().to_string(),
            )
        })
        .collect();

    for (route_path, file_path) in routes_info {
        let api_routes_clone = Arc::clone(&api_routes);
        let file_path_clone = file_path.clone();
        let route_path_clone = route_path.clone();

        let method_router: MethodRouter = get({
            let api_routes = Arc::clone(&api_routes_clone);
            let file_path = file_path_clone.clone();
            let route_path = route_path_clone.clone();
            move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>, body: String| {
                let api_routes = Arc::clone(&api_routes);
                let file_path = file_path.clone();
                let route_path = route_path.clone();
                async move {
                    handle_api_request(
                        api_routes,
                        headers,
                        Method::GET,
                        query,
                        body,
                        file_path,
                        route_path,
                    )
                }
            }
        })
        .post({
            let api_routes = Arc::clone(&api_routes_clone);
            let file_path = file_path_clone.clone();
            let route_path = route_path_clone.clone();
            move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>, body: String| {
                let api_routes = Arc::clone(&api_routes);
                let file_path = file_path.clone();
                let route_path = route_path.clone();
                async move {
                    handle_api_request(
                        api_routes,
                        headers,
                        Method::POST,
                        query,
                        body,
                        file_path,
                        route_path,
                    )
                }
            }
        });

        router = router.route(&route_path, method_router);
        info!("Registered API route: {}", route_path);
    }

    Ok((router, Some(api_routes)))
}

/// Handle an incoming API request.
fn handle_api_request(
    api_routes: Arc<std::sync::Mutex<ApiRoutes>>,
    headers: HeaderMap,
    method: Method,
    query: HashMap<String, String>,
    body: String,
    file_path: String,
    route_path: String,
) -> impl IntoResponse {
    let headers_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (key.to_string(), v.to_string()))
        })
        .collect();

    let request = ApiRequest {
        url: route_path,
        headers: headers_map,
        method: method.to_string(),
        query,
        body: if body.is_empty() { None } else { Some(body) },
        params: HashMap::new(),
    };

    let routes = api_routes.lock().unwrap();
    match routes.call_handler(&file_path, method.as_str(), request) {
        Ok(response) => (
            StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK),
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            serde_json::to_string(&response.body).unwrap_or_else(|_| "{}".to_string()),
        ),
        Err(error) => {
            error!("API handler error: {}", error);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                format!("{{\"error\": \"{}\"}}", error),
            )
        }
    }
}