/// Generate HTML for link processing page
pub fn processing_page(short_code: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Link Processing - QCK</title>
    <style>
        body {{
            margin: 0;
            padding: 0;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
        }}
        .spinner {{
            width: 50px;
            height: 50px;
            border: 4px solid rgba(255, 255, 255, 0.3);
            border-top-color: white;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin: 2rem auto;
        }}
        @keyframes spin {{
            to {{ transform: rotate(360deg); }}
        }}
        h1 {{
            margin: 1rem 0;
        }}
        p {{
            opacity: 0.9;
            max-width: 400px;
            margin: 1rem auto;
            line-height: 1.6;
        }}
        .retry-info {{
            margin-top: 2rem;
            font-size: 0.9rem;
            opacity: 0.8;
        }}
    </style>
    <script>
        // Auto-refresh every 2 seconds
        setTimeout(() => {{
            window.location.reload();
        }}, 2000);
    </script>
</head>
<body>
    <div class="container">
        <div class="spinner"></div>
        <h1>Processing Your Link</h1>
        <p>We're extracting metadata for <strong>/{}</strong></p>
        <p>This usually takes just a moment. The page will refresh automatically.</p>
        <div class="retry-info">
            If this takes longer than expected, the link will still work without metadata.
        </div>
    </div>
</body>
</html>"#,
        short_code
    )
}
