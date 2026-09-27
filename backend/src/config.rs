use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: String,
    pub jwt_ttl_hours: i64,
    pub cors_origins: Vec<String>,
    /// Anthropic API key. When empty, AI endpoints fall back to built-in templates.
    pub anthropic_api_key: Option<String>,
    pub ai_model: String,
    pub anthropic_base_url: String,
    // Media storage
    pub media_driver: String,
    pub media_dir: String,
    pub media_public_url: String,
    pub media_max_image_mb: usize,
    pub media_max_video_mb: usize,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    // Moderation
    /// Products need admin approval before going live (PRODUCT_REVIEW=required|off).
    pub review_required: bool,
    /// Editing an approved product's content (name, description, category, media) re-queues it.
    pub review_on_edit: bool,
    /// Users with these emails are promoted to admin at startup/registration.
    pub admin_emails: Vec<String>,
    // Social commerce
    /// Public URL of the Nuxt site (used in checkout links sent to customers).
    pub public_web_url: String,
    pub meta_verify_token: String,
    pub meta_app_secret: String,
    pub meta_graph_url: String,
    pub tiktok_client_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            database_url: get(
                "DATABASE_URL",
                "postgres://zaokaiy:zaokaiy@localhost:5432/zaokaiy",
            ),
            bind_addr: get("BIND_ADDR", "0.0.0.0:8080"),
            jwt_secret: get("JWT_SECRET", "dev-secret-change-me"),
            jwt_ttl_hours: get("JWT_TTL_HOURS", "168").parse().unwrap_or(168),
            cors_origins: get("CORS_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty()),
            ai_model: get("AI_MODEL", "claude-sonnet-5"),
            anthropic_base_url: get("ANTHROPIC_BASE_URL", "https://api.anthropic.com"),
            media_driver: get("MEDIA_DRIVER", "local").to_lowercase(),
            media_dir: get("MEDIA_DIR", "./media"),
            media_public_url: get("MEDIA_PUBLIC_URL", "http://localhost:8080/media"),
            media_max_image_mb: get("MEDIA_MAX_IMAGE_MB", "15").parse().unwrap_or(15),
            media_max_video_mb: get("MEDIA_MAX_VIDEO_MB", "200").parse().unwrap_or(200),
            s3_endpoint: get("S3_ENDPOINT", ""),
            s3_bucket: get("S3_BUCKET", "zaokaiy-media"),
            s3_region: get("S3_REGION", "us-east-1"),
            s3_access_key: get("S3_ACCESS_KEY", ""),
            s3_secret_key: get("S3_SECRET_KEY", ""),
            review_required: get("PRODUCT_REVIEW", "required").to_lowercase() != "off",
            review_on_edit: get("REVIEW_ON_EDIT", "true").to_lowercase() != "false",
            admin_emails: get("ADMIN_EMAILS", "")
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            public_web_url: get("PUBLIC_WEB_URL", "http://localhost:3000"),
            meta_verify_token: get("META_VERIFY_TOKEN", ""),
            meta_app_secret: get("META_APP_SECRET", ""),
            meta_graph_url: get("META_GRAPH_URL", "https://graph.facebook.com/v21.0"),
            tiktok_client_secret: get("TIKTOK_CLIENT_SECRET", ""),
        }
    }
}
