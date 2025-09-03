// Swagger UI HTML serving

use axum::response::{Html, IntoResponse};

/// Serve Swagger UI HTML at /v1/docs
pub async fn serve_swagger_ui() -> impl IntoResponse {
    Html(SWAGGER_UI_HTML)
}

// Embedded Swagger UI HTML
const SWAGGER_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>QCK API Documentation</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui.css" />
    <style>
        body {
            margin: 0;
            padding: 0;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
        }
        #swagger-ui {
            max-width: 1460px;
            margin: 0 auto;
            padding: 20px;
        }
        .topbar {
            display: none;
        }
        .swagger-ui .info {
            margin: 50px 0;
        }
        .swagger-ui .info .title {
            color: #3b4151;
        }
        .swagger-ui .btn.execute {
            background-color: #4990e2;
            border-color: #4990e2;
        }
        .swagger-ui .btn.execute:hover {
            background-color: #1268c3;
            border-color: #1268c3;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 2rem;
            text-align: center;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            position: relative;
        }
        .header h1 {
            margin: 0;
            font-size: 2.5rem;
            font-weight: 600;
        }
        .header p {
            margin: 0.5rem 0 0;
            opacity: 0.9;
            font-size: 1.1rem;
        }
        .version-selector {
            position: absolute;
            top: 2rem;
            right: 2rem;
            background: rgba(255, 255, 255, 0.2);
            border: 2px solid white;
            border-radius: 4px;
            padding: 0.5rem 1rem;
            color: white;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: background-color 0.3s;
        }
        .version-selector:hover {
            background: rgba(255, 255, 255, 0.3);
        }
        .version-selector option {
            background: #667eea;
            color: white;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>🚀 QCK Backend API</h1>
        <p>REST API Documentation</p>
        <select class="version-selector" id="versionSelector" onchange="handleVersionChange(this.value)">
            <option value="v1" selected>Version 1.0</option>
            <option value="v2" disabled>Version 2.0 (Coming Soon)</option>
        </select>
    </div>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-standalone-preset.js"></script>
    <script>
        // Handle version selector changes
        function handleVersionChange(version) {
            if (version === 'v2') {
                alert('Version 2 API documentation is coming soon!');
                document.getElementById('versionSelector').value = 'v1';
                return;
            }
            // In the future, load different OpenAPI specs based on version
            window.location.reload();
        }
        
        window.onload = function() {
            // Detect if we're running behind /api prefix
            const currentPath = window.location.pathname;
            const needsApiPrefix = currentPath.includes('/api/');
            const specUrl = needsApiPrefix ? '/api/v1/docs/openapi.json' : '/v1/docs/openapi.json';
            
            const ui = SwaggerUIBundle({
                url: specUrl,
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout",
                defaultModelsExpandDepth: 1,
                defaultModelExpandDepth: 1,
                docExpansion: "list",
                filter: true,
                showExtensions: true,
                showCommonExtensions: true,
                tryItOutEnabled: true,
                supportedSubmitMethods: ['get', 'post', 'put', 'delete', 'patch'],
                validatorUrl: null
            });
            window.ui = ui;
        }
    </script>
</body>
</html>"#;
